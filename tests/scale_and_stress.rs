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

//! Scale and stress tests.
//!
//! These tests exercise ProllyTree with larger datasets and concurrent load.
//! They are marked `#[ignore]` so they don't run in normal CI — invoke with:
//!
//! ```sh
//! cargo test --features "git sql" --test scale_and_stress -- --ignored
//! ```

#![cfg(feature = "git")]

mod common;

use prollytree::config::TreeConfig;
use prollytree::git::versioned_store::{GitVersionedKvStore, HistoricalAccess, StoreFactory};
use prollytree::storage::InMemoryNodeStorage;
use prollytree::tree::{ProllyTree, Tree};

// ---------------------------------------------------------------------------
// 1000 keys insert/commit/retrieve
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn test_1000_keys_insert_commit_retrieve() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    for i in 0..1000 {
        store
            .insert(
                format!("k{i:04}").into_bytes(),
                format!("v{i:04}").into_bytes(),
            )
            .unwrap();
    }
    store.commit("1000 keys").unwrap();

    for i in 0..1000 {
        assert_eq!(
            store.get(format!("k{i:04}").as_bytes()),
            Some(format!("v{i:04}").into_bytes()),
            "key k{i:04} missing"
        );
    }

    std::mem::forget(_temp);
}

// ---------------------------------------------------------------------------
// 10000 keys with in-memory backend
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn test_10000_keys_inmemory() {
    let storage = InMemoryNodeStorage::<32>::new();
    let config = TreeConfig::<32>::default();
    let mut tree = ProllyTree::new(storage, config);

    for i in 0..10_000 {
        tree.insert(
            format!("k{i:05}").into_bytes(),
            format!("v{i:05}").into_bytes(),
        );
    }

    for i in 0..10_000 {
        assert!(
            tree.find(format!("k{i:05}").as_bytes()).is_some(),
            "k{i:05} not found"
        );
    }

    let stats = tree.stats();
    println!(
        "10K keys: total_kv_pairs={}, internal_nodes={}, leaves={}",
        stats.total_key_value_pairs, stats.num_internal_nodes, stats.num_leaves
    );
}

// ---------------------------------------------------------------------------
// Bulk insert then bulk delete
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn test_bulk_insert_then_bulk_delete() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    for i in 0..500 {
        store
            .insert(format!("d{i:03}").into_bytes(), b"val".to_vec())
            .unwrap();
    }
    store.commit("insert 500").unwrap();

    // Delete first 250
    for i in 0..250 {
        store.delete(format!("d{i:03}").as_bytes()).unwrap();
    }
    store.commit("delete 250").unwrap();

    // Verify: first 250 gone, last 250 present
    for i in 0..250 {
        assert!(
            store.get(format!("d{i:03}").as_bytes()).is_none(),
            "d{i:03} should be deleted"
        );
    }
    for i in 250..500 {
        assert!(
            store.get(format!("d{i:03}").as_bytes()).is_some(),
            "d{i:03} should still exist"
        );
    }

    std::mem::forget(_temp);
}

// ---------------------------------------------------------------------------
// Many small commits
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn test_many_small_commits() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    let mut commit_ids = Vec::new();
    for i in 0..100 {
        store
            .insert(
                format!("mc{i:03}").into_bytes(),
                format!("v{i}").into_bytes(),
            )
            .unwrap();
        let id = store.commit(&format!("commit {i}")).unwrap();
        commit_ids.push(id.to_string());
    }

    let log = store.log().unwrap();
    // 100 commits + initial commit from init
    assert!(
        log.len() >= 100,
        "expected >= 100 log entries, got {}",
        log.len()
    );

    // Historical snapshot at an early commit
    let early = &commit_ids[10];
    let keys = store.get_keys_at_ref(early).unwrap();
    // Should have at most 11 keys (mc000..mc010) plus the initial commit's keys
    assert!(
        keys.len() <= 15,
        "early commit should have few keys, got {}",
        keys.len()
    );

    std::mem::forget(_temp);
}

// ---------------------------------------------------------------------------
// Large values
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn test_large_values() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    let big_value = vec![0xABu8; 1024 * 1024]; // 1 MB
    store.insert(b"bigkey".to_vec(), big_value.clone()).unwrap();
    store.commit("big value").unwrap();

    let retrieved = store.get(b"bigkey").unwrap();
    assert_eq!(retrieved.len(), 1024 * 1024);
    assert_eq!(retrieved, big_value);

    std::mem::forget(_temp);
}

// ---------------------------------------------------------------------------
// Concurrent stress on thread-safe store
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn test_concurrent_stress_threadsafe() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let store = StoreFactory::git_threadsafe::<32, _>(&dataset).unwrap();

    let num_threads = 20;
    let keys_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let s = store.clone();
            std::thread::spawn(move || {
                for i in 0..keys_per_thread {
                    s.insert(
                        format!("t{t:02}_k{i:03}").into_bytes(),
                        format!("v{t}_{i}").into_bytes(),
                    )
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }

    store.commit("stress").unwrap();

    // Verify all keys present
    let keys = store.list_keys().unwrap();
    assert_eq!(
        keys.len(),
        num_threads * keys_per_thread,
        "expected {} keys, got {}",
        num_threads * keys_per_thread,
        keys.len()
    );

    std::mem::forget(_temp);
}
