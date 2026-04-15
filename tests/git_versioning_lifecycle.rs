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

//! Integration tests for the full Git versioning lifecycle:
//! init → insert → commit → branch → checkout → merge → diff → history.

#![cfg(feature = "git")]

mod common;

use prollytree::diff::TakeSourceResolver;
use prollytree::git::versioned_store::{
    GitVersionedKvStore, HistoricalAccess, HistoricalCommitAccess,
};

// ---------------------------------------------------------------------------
// Full branching lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_full_branching_lifecycle() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    // Insert on main
    store
        .insert(b"main_k".to_vec(), b"main_v".to_vec())
        .unwrap();
    store.commit("main data").unwrap();

    // Create and switch to feature branch
    store.create_branch("feature").unwrap();
    store.checkout("feature").unwrap();

    // Insert on feature
    store
        .insert(b"feat_k".to_vec(), b"feat_v".to_vec())
        .unwrap();
    store.commit("feature data").unwrap();

    // Feature should have both keys
    assert!(store.get(b"main_k").is_some());
    assert!(store.get(b"feat_k").is_some());

    // Switch back to main
    store.checkout("main").unwrap();

    // Main should NOT have the feature key
    assert!(store.get(b"main_k").is_some());
    assert!(
        store.get(b"feat_k").is_none(),
        "main should not have feature-only key"
    );

    std::mem::forget(_temp);
}

// ---------------------------------------------------------------------------
// Multiple branches diverge and converge
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_branches_diverge_and_converge() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    // Base commit
    store.insert(b"base".to_vec(), b"val".to_vec()).unwrap();
    store.commit("base").unwrap();

    // Create 3 branches, each adding a unique key
    for i in 0..3 {
        let branch = format!("branch{i}");
        store.create_branch(&branch).unwrap();
        store.checkout(&branch).unwrap();
        store
            .insert(
                format!("b{i}_key").into_bytes(),
                format!("b{i}_val").into_bytes(),
            )
            .unwrap();
        store.commit(&format!("branch {i} commit")).unwrap();
        store.checkout("main").unwrap();
    }

    // Merge all branches into main
    for i in 0..3 {
        let branch = format!("branch{i}");
        store
            .merge(&branch, &TakeSourceResolver)
            .expect(&format!("merge {branch}"));
    }

    // Main should have all keys
    assert!(store.get(b"base").is_some());
    for i in 0..3 {
        assert!(
            store.get(format!("b{i}_key").as_bytes()).is_some(),
            "b{i}_key missing after merge"
        );
    }

    std::mem::forget(_temp);
}

// ---------------------------------------------------------------------------
// History tracks key across commits
// ---------------------------------------------------------------------------

#[test]
fn test_history_tracks_key_across_commits() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    // Update a key across multiple commits
    for i in 0..5 {
        store
            .insert(b"tracked".to_vec(), format!("v{i}").into_bytes())
            .unwrap();
        store.commit(&format!("update {i}")).unwrap();
    }

    let history = store.get_commits_for_key(b"tracked").unwrap();
    assert!(
        history.len() >= 5,
        "expected at least 5 commits for tracked key, got {}",
        history.len()
    );

    std::mem::forget(_temp);
}

// ---------------------------------------------------------------------------
// Get keys at historical ref
// ---------------------------------------------------------------------------

#[test]
fn test_get_keys_at_historical_ref() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    // Commit 1: key A
    store.insert(b"A".to_vec(), b"a1".to_vec()).unwrap();
    let c1 = store.commit("commit 1").unwrap();

    // Commit 2: key A + B
    store.insert(b"B".to_vec(), b"b1".to_vec()).unwrap();
    let c2 = store.commit("commit 2").unwrap();

    // Commit 3: key A + B + C
    store.insert(b"C".to_vec(), b"c1".to_vec()).unwrap();
    store.commit("commit 3").unwrap();

    // Check snapshot at commit 1: only A
    let keys_c1 = store.get_keys_at_ref(&c1.to_string()).unwrap();
    assert!(keys_c1.contains_key(b"A".as_ref()));
    assert!(
        !keys_c1.contains_key(b"B".as_ref()),
        "B should not exist at c1"
    );

    // Check snapshot at commit 2: A and B
    let keys_c2 = store.get_keys_at_ref(&c2.to_string()).unwrap();
    assert!(keys_c2.contains_key(b"A".as_ref()));
    assert!(keys_c2.contains_key(b"B".as_ref()));
    assert!(
        !keys_c2.contains_key(b"C".as_ref()),
        "C should not exist at c2"
    );

    std::mem::forget(_temp);
}

// ---------------------------------------------------------------------------
// Open after close preserves state
// ---------------------------------------------------------------------------

#[test]
fn test_open_after_close_preserves_state() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    // Create and populate
    {
        let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();
        for i in 0..20 {
            store
                .insert(
                    format!("pk{i:03}").into_bytes(),
                    format!("pv{i:03}").into_bytes(),
                )
                .unwrap();
        }
        store.commit("persist data").unwrap();
    }

    // Re-open and verify
    {
        let store = GitVersionedKvStore::<32>::open(&dataset).unwrap();
        for i in 0..20 {
            assert_eq!(
                store.get(format!("pk{i:03}").as_bytes()),
                Some(format!("pv{i:03}").into_bytes()),
                "pk{i:03} should persist"
            );
        }
        let log = store.log().unwrap();
        assert!(log.len() >= 2, "should have initial + persist commits");
    }
}
