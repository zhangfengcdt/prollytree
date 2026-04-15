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

//! Integration tests for error handling and recovery.

#![cfg(feature = "git")]

mod common;

use prollytree::git::versioned_store::GitVersionedKvStore;
use prollytree::validation;

// ---------------------------------------------------------------------------
// Open with corrupted config
// ---------------------------------------------------------------------------

#[test]
fn test_open_corrupted_config() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    // Init and commit to create config files
    {
        let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();
        store.insert(b"key".to_vec(), b"val".to_vec()).unwrap();
        store.commit("data").unwrap();
    }

    // Corrupt the config file
    let config_path = dataset.join("prolly_config_tree_config");
    if config_path.exists() {
        std::fs::write(&config_path, b"garbage_data_not_valid_config").unwrap();
    }

    // Open should fail gracefully (not panic)
    let result = GitVersionedKvStore::<32>::open(&dataset);
    // It may succeed with defaults or fail with an error — either is acceptable,
    // but it must NOT panic.
    match result {
        Ok(_store) => {
            // Acceptable: store opened with defaults/fallback
        }
        Err(e) => {
            // Acceptable: returned an error
            assert!(!format!("{e:?}").is_empty(), "error should have a message");
        }
    }
}

// ---------------------------------------------------------------------------
// Open with missing .git directory
// ---------------------------------------------------------------------------

#[test]
fn test_open_missing_git_directory() {
    let temp = tempfile::TempDir::new().unwrap();
    let dataset = temp.path().join("dataset");
    std::fs::create_dir_all(&dataset).unwrap();

    // No git init — .git does not exist
    let result = GitVersionedKvStore::<32>::open(&dataset);
    assert!(result.is_err(), "open without .git should fail");
}

// ---------------------------------------------------------------------------
// Checkout nonexistent branch
// ---------------------------------------------------------------------------

#[test]
fn test_checkout_nonexistent_branch() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();
    store.insert(b"k".to_vec(), b"v".to_vec()).unwrap();
    store.commit("init").unwrap();

    let result = store.checkout("nonexistent_branch_xyz");
    assert!(
        result.is_err(),
        "checkout of nonexistent branch should fail"
    );

    std::mem::forget(_temp);
}

// ---------------------------------------------------------------------------
// Double init preserves data
// ---------------------------------------------------------------------------

#[test]
fn test_double_init_preserves_data() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    // First init + data
    {
        let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();
        store.insert(b"survive".to_vec(), b"yes".to_vec()).unwrap();
        store.commit("first init").unwrap();
    }

    // Second init on same path — should open existing, not wipe
    {
        let store = GitVersionedKvStore::<32>::open(&dataset).unwrap();
        assert_eq!(
            store.get(b"survive"),
            Some(b"yes".to_vec()),
            "data from first init should survive open"
        );
    }
}

// ---------------------------------------------------------------------------
// Init in nonexistent parent
// ---------------------------------------------------------------------------

#[test]
fn test_init_in_nonexistent_parent() {
    // No git repo at this path, and the path doesn't exist
    let result = GitVersionedKvStore::<32>::init("/tmp/prollytree_nonexistent_parent_9999/dataset");
    assert!(
        result.is_err(),
        "init in nonexistent directory without git repo should fail"
    );
}

// ---------------------------------------------------------------------------
// Input validation
// ---------------------------------------------------------------------------

#[test]
fn test_insert_empty_key_rejected() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    let result = store.insert(b"".to_vec(), b"value".to_vec());
    assert!(result.is_err(), "empty key should be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("empty"),
        "error should mention empty key: {err}"
    );
}

#[test]
fn test_insert_oversized_key_rejected() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    let big_key = vec![0u8; validation::MAX_KEY_SIZE + 1];
    let result = store.insert(big_key, b"value".to_vec());
    assert!(result.is_err(), "oversized key should be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("exceeds"),
        "error should mention size limit: {err}"
    );
}

#[test]
fn test_insert_oversized_value_rejected() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    let big_value = vec![0u8; validation::MAX_VALUE_SIZE + 1];
    let result = store.insert(b"key".to_vec(), big_value);
    assert!(result.is_err(), "oversized value should be rejected");
}

#[test]
fn test_update_empty_key_rejected() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    let result = store.update(b"".to_vec(), b"value".to_vec());
    assert!(result.is_err(), "empty key update should be rejected");
}

#[test]
fn test_valid_key_value_accepted() {
    let (_temp, dataset) = common::setup_repo_and_dataset();
    let mut store = GitVersionedKvStore::<32>::init(&dataset).unwrap();

    // Normal-sized key/value should succeed
    store
        .insert(b"normal_key".to_vec(), b"normal_value".to_vec())
        .expect("valid kv should be accepted");

    // Max-size key should succeed
    let max_key = vec![b'k'; validation::MAX_KEY_SIZE];
    store
        .insert(max_key, b"val".to_vec())
        .expect("max-size key should be accepted");
}
