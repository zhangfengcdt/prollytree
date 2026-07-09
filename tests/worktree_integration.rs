/*
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

//! Integration tests for WorktreeManager and multi-branch concurrent operations.

#![cfg(feature = "git")]

mod common;

use prollytree::git::versioned_store::GitVersionedKvStore;
use prollytree::git::worktree::WorktreeManager;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn run_git(repo_path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test User")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test User")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .current_dir(repo_path)
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn setup_plain_git_repo_with_main() -> TempDir {
    let temp = common::setup_git_repo();
    let repo_path = temp.path();

    std::fs::write(repo_path.join("README.md"), "# Test Repository")
        .expect("failed to write README");
    run_git(repo_path, &["add", "."]);
    let tree = run_git(repo_path, &["write-tree"]);
    let commit = run_git(repo_path, &["commit-tree", &tree, "-m", "initial"]);
    write_branch_ref(repo_path, "main", &commit);
    std::fs::write(
        repo_path.join(".git").join("HEAD"),
        "ref: refs/heads/main\n",
    )
    .expect("failed to write HEAD");

    temp
}

fn write_child_commit(repo_path: &Path, parent: &str, message: &str) -> String {
    let treeish = format!("{parent}^{{tree}}");
    let tree = run_git(repo_path, &["rev-parse", &treeish]);
    run_git(
        repo_path,
        &["commit-tree", &tree, "-p", parent, "-m", message],
    )
}

fn write_branch_ref(repo_path: &Path, branch: &str, commit: &str) {
    let branch_ref = repo_path
        .join(".git")
        .join("refs")
        .join("heads")
        .join(branch);
    if let Some(parent) = branch_ref.parent() {
        std::fs::create_dir_all(parent).expect("failed to create branch refs dir");
    }
    std::fs::write(branch_ref, commit).expect("failed to write branch ref");
}

// ---------------------------------------------------------------------------
// Create worktree and operate independently
// ---------------------------------------------------------------------------

#[test]
fn test_create_worktree_operate_independently() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    // Initialize main store and commit
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();
    store
        .insert(b"main_key".to_vec(), b"main_val".to_vec())
        .unwrap();
    store.commit("initial").unwrap();
    drop(store);

    // Create worktree manager from the dataset's git repo
    let git_root = _temp.path();
    let mut manager = WorktreeManager::new(git_root).unwrap();

    // Add a worktree
    let wt_path = _temp.path().join("worktree_agent1");
    let wt_info = manager
        .add_worktree(&wt_path, "agent1-branch", true)
        .unwrap();

    assert!(!wt_info.id.is_empty(), "worktree should have an id");
    assert!(wt_path.exists(), "worktree directory should exist");

    // List worktrees
    let worktrees = manager.list_worktrees();
    assert!(!worktrees.is_empty(), "should have at least one worktree");

    std::mem::forget(_temp);
}

// ---------------------------------------------------------------------------
// Worktree locking
// ---------------------------------------------------------------------------

#[test]
fn test_worktree_locking() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();
    store.insert(b"k".to_vec(), b"v".to_vec()).unwrap();
    store.commit("init").unwrap();
    drop(store);

    let git_root = _temp.path();
    let mut manager = WorktreeManager::new(git_root).unwrap();

    let wt_path = _temp.path().join("wt_lock_test");
    let wt_info = manager.add_worktree(&wt_path, "lock-branch", true).unwrap();
    let wt_id = wt_info.id.clone();

    // Lock the worktree
    manager.lock_worktree(&wt_id, "testing lock").unwrap();
    assert!(manager.is_locked(&wt_id), "worktree should be locked");

    // Unlock
    manager.unlock_worktree(&wt_id).unwrap();
    assert!(!manager.is_locked(&wt_id), "worktree should be unlocked");

    std::mem::forget(_temp);
}

// ---------------------------------------------------------------------------
// Remove worktree
// ---------------------------------------------------------------------------

#[test]
fn test_remove_worktree() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();
    store.insert(b"k".to_vec(), b"v".to_vec()).unwrap();
    store.commit("init").unwrap();
    drop(store);

    let git_root = _temp.path();
    let mut manager = WorktreeManager::new(git_root).unwrap();

    let wt_path = _temp.path().join("wt_remove");
    let wt_info = manager
        .add_worktree(&wt_path, "remove-branch", true)
        .unwrap();
    let wt_id = wt_info.id.clone();

    assert!(manager.get_worktree(&wt_id).is_some());

    manager.remove_worktree(&wt_id).unwrap();
    assert!(
        manager.get_worktree(&wt_id).is_none(),
        "removed worktree should not appear"
    );

    std::mem::forget(_temp);
}

#[test]
fn test_git_fallback_merge_rejects_divergent_worktree_history() {
    let temp = setup_plain_git_repo_with_main();
    let repo_path = temp.path();
    let mut manager = WorktreeManager::new(repo_path).unwrap();
    let base = manager.get_branch_commit("main").unwrap();
    let target_child = write_child_commit(repo_path, &base, "target child");

    let wt_path = repo_path.join("feature_worktree");
    let wt_info = manager.add_worktree(&wt_path, "feature", true).unwrap();
    let source_child = write_child_commit(repo_path, &base, "source child");

    write_branch_ref(repo_path, "main", &target_child);
    write_branch_ref(repo_path, "release", &target_child);
    write_branch_ref(repo_path, "feature", &source_child);

    let main_err = manager
        .merge_to_main(&wt_info.id, "merge feature to main")
        .unwrap_err();
    assert!(
        main_err
            .to_string()
            .contains("Refusing to merge divergent branch"),
        "unexpected merge_to_main error: {main_err}"
    );
    assert_eq!(
        manager.get_branch_commit("main").unwrap(),
        target_child,
        "merge_to_main fallback must not clobber divergent main history"
    );

    let release_err = manager
        .merge_branch(&wt_info.id, "release", "merge feature to release")
        .unwrap_err();
    assert!(
        release_err
            .to_string()
            .contains("Refusing to merge divergent branch"),
        "unexpected merge_branch error: {release_err}"
    );
    assert_eq!(
        manager.get_branch_commit("release").unwrap(),
        target_child,
        "merge_branch fallback must not clobber divergent target history"
    );
}
