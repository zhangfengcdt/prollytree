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

//! Integration tests for `InMemoryNodeStorage` and `FileNodeStorage`.
//!
//! These two backends have zero unit tests. This file exercises them through
//! the `NodeStorage` trait and through `ProllyTree` operations.

use prollytree::config::TreeConfig;
use prollytree::node::ProllyNode;
use prollytree::storage::{FileNodeStorage, InMemoryNodeStorage, NodeStorage};
use prollytree::tree::{ProllyTree, Tree};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// InMemory: basic CRUD
// ---------------------------------------------------------------------------

#[test]
fn test_inmemory_insert_get_delete() {
    let mut storage = InMemoryNodeStorage::<32>::new();
    let node = ProllyNode::<32>::default();
    let hash = node.get_hash();

    storage.insert_node(hash.clone(), node.clone()).unwrap();

    let retrieved = storage.get_node_by_hash(&hash);
    assert!(retrieved.is_some(), "node should be retrievable");
    assert_eq!(retrieved.unwrap().keys, node.keys);

    storage.delete_node(&hash).unwrap();
    assert!(
        storage.get_node_by_hash(&hash).is_none(),
        "node should be deleted"
    );
}

// ---------------------------------------------------------------------------
// File: basic CRUD
// ---------------------------------------------------------------------------

#[test]
fn test_file_insert_get_delete() {
    let temp = TempDir::new().unwrap();
    let mut storage = FileNodeStorage::<32>::new(temp.path().to_path_buf()).unwrap();
    let node = ProllyNode::<32>::default();
    let hash = node.get_hash();

    storage.insert_node(hash.clone(), node.clone()).unwrap();

    let retrieved = storage.get_node_by_hash(&hash);
    assert!(retrieved.is_some(), "file node should be retrievable");

    storage.delete_node(&hash).unwrap();
    assert!(storage.get_node_by_hash(&hash).is_none());
}

// ---------------------------------------------------------------------------
// File: persistence across instances
// ---------------------------------------------------------------------------

#[test]
fn test_file_persistence_across_instances() {
    let temp = TempDir::new().unwrap();
    let node = ProllyNode::<32>::default();
    let hash = node.get_hash();

    {
        let mut storage = FileNodeStorage::<32>::new(temp.path().to_path_buf()).unwrap();
        storage.insert_node(hash.clone(), node.clone()).unwrap();
    }

    // New instance at the same path should find the node
    let storage = FileNodeStorage::<32>::new(temp.path().to_path_buf()).unwrap();
    assert!(
        storage.get_node_by_hash(&hash).is_some(),
        "node should persist across file storage instances"
    );
}

// ---------------------------------------------------------------------------
// Config roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_config_roundtrip_inmemory() {
    let storage = InMemoryNodeStorage::<32>::new();
    storage.save_config("test_key", b"test_value");
    assert_eq!(storage.get_config("test_key"), Some(b"test_value".to_vec()));
    assert_eq!(storage.get_config("nonexistent"), None);
}

#[test]
fn test_config_roundtrip_file() {
    let temp = TempDir::new().unwrap();
    let storage = FileNodeStorage::<32>::new(temp.path().to_path_buf()).unwrap();
    storage.save_config("cfg_key", b"cfg_data");
    assert_eq!(storage.get_config("cfg_key"), Some(b"cfg_data".to_vec()));
    assert_eq!(storage.get_config("missing"), None);
}

// ---------------------------------------------------------------------------
// Large node count
// ---------------------------------------------------------------------------

#[test]
fn test_large_node_storage() {
    let mut mem = InMemoryNodeStorage::<32>::new();

    let temp = TempDir::new().unwrap();
    let mut file = FileNodeStorage::<32>::new(temp.path().to_path_buf()).unwrap();

    let mut hashes = Vec::new();
    for i in 0..100 {
        let mut node = ProllyNode::<32>::default();
        node.keys = vec![format!("key{i}").into_bytes()];
        node.values = vec![format!("val{i}").into_bytes()];
        let hash = node.get_hash();
        hashes.push(hash.clone());

        mem.insert_node(hash.clone(), node.clone()).unwrap();
        file.insert_node(hash, node).unwrap();
    }

    for hash in &hashes {
        assert!(mem.get_node_by_hash(hash).is_some());
        assert!(file.get_node_by_hash(hash).is_some());
    }
}

// ---------------------------------------------------------------------------
// ProllyTree with FileNodeStorage
// ---------------------------------------------------------------------------

#[test]
fn test_tree_with_file_backend() {
    let temp = TempDir::new().unwrap();
    let storage = FileNodeStorage::<32>::new(temp.path().to_path_buf()).unwrap();
    let config = TreeConfig::<32>::default();
    let mut tree = ProllyTree::new(storage, config);

    // Insert
    for i in 0..20 {
        tree.insert(vec![i], vec![i + 100]);
    }

    // Find
    for i in 0..20 {
        let found = tree.find(&[i]);
        assert!(found.is_some(), "key {i} should be findable");
    }

    // Delete
    assert!(tree.delete(&[5]));
    assert!(tree.find(&[5]).is_none());
}
