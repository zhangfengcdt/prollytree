// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Integration tests for WorktreeManager and multi-branch concurrent operations.

#![cfg(feature = "git")]

mod common;

use prollytree::git::versioned_store::GitVersionedKvStore;
use prollytree::git::worktree::WorktreeManager;

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
