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

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use crate::digest::ValueDigest;
use crate::node::Node;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A trait for storage of nodes in the ProllyTree.
///
/// This trait defines the necessary operations for managing the storage
/// of nodes within a ProllyTree. Implementors of this trait can provide
/// custom storage backends, such as in-memory storage, database storage,
/// or any other form of persistent storage.
///
/// # Type Parameters
///
/// - `N`: The size of the value digest.
/// - `K`: The type of the key used in the nodes, which must implement
///        `AsRef<[u8]>`, `Clone`, `PartialEq`, and `From<Vec<u8>>`.
pub trait NodeStorage<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> {
    /// Retrieves a node from storage by its hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - A reference to the `ValueDigest` representing the hash of the node to retrieve.
    ///
    /// # Returns
    ///
    /// The node associated with the given hash.
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Node<N, K>;

    /// Inserts a node into storage.
    ///
    /// # Arguments
    ///
    /// * `hash` - The `ValueDigest` representing the hash of the node to insert.
    /// * `node` - The node to insert into storage.
    fn insert_node(&mut self, hash: ValueDigest<N>, node: Node<N, K>);

    /// Deletes a node from storage by its hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - A reference to the `ValueDigest` representing the hash of the node to delete.
    fn delete_node(&mut self, hash: &ValueDigest<N>);
}

/// A storage backend for ProllyTree nodes using an in-memory HashMap.
///
/// This struct provides an in-memory implementation of the `NodeStorage` trait
/// using a `HashMap` to store nodes. Each node is indexed by its hash, allowing
/// efficient retrieval, insertion, and deletion of nodes.
///
/// # Type Parameters
///
/// - `N`: The size of the value digest (e.g., 32 for a 256-bit hash).
/// - `K`: The type of the key used in the nodes, which must implement
///        `AsRef<[u8]>`, `Clone`, `PartialEq`, and `From<Vec<u8>>`.
#[derive(Debug, Clone)]
pub struct HashMapNodeStorage<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> {
    /// The underlying HashMap storing the nodes.
    ///
    /// The key is a `ValueDigest` representing the hash of the node,
    /// and the value is the node itself.
    map: HashMap<ValueDigest<N>, Node<N, K>>,
}

impl<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> Default
    for HashMapNodeStorage<N, K>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> HashMapNodeStorage<N, K> {
    pub fn new() -> Self {
        HashMapNodeStorage {
            map: HashMap::new(),
        }
    }
}

impl<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>> + 'static> NodeStorage<N, K>
    for HashMapNodeStorage<N, K>
{
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Node<N, K> {
        self.map.get(hash).cloned().unwrap_or_else(|| {
            // Create a default node if the hash is not found
            Node {
                key: Vec::new().into(),
                value_hash: ValueDigest::<N>([0; N]),
                children_hash: None,
                parent_hash: None,
                level: 0,
                is_leaf: true,
                subtree_counts: None,
                storage: Arc::new(Mutex::new(self.clone())),
            }
        })
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: Node<N, K>) {
        self.map.insert(hash, node);
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) {
        self.map.remove(hash);
    }
}

/// A storage backend for ProllyTree nodes using the file system.
///
/// This struct provides a file system-based implementation of the `NodeStorage` trait.
/// Each node is stored in a separate file, identified by its hash, allowing
/// efficient retrieval, insertion, and deletion of nodes.
///
/// # Type Parameters
///
/// - `N`: The size of the value digest (e.g., 32 for a 256-bit hash).
/// - `K`: The type of the key used in the nodes, which must implement
///        `AsRef<[u8]>`, `Clone`, `PartialEq`, and `From<Vec<u8>>`.
#[derive(Debug)]
pub struct FileSystemNodeStorage<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>>
{
    dir_path: PathBuf,
    _marker: PhantomData<K>,
}

impl<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>>
    FileSystemNodeStorage<N, K>
{
    /// Creates a new instance of `FileSystemNodeStorage`.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the directory where node files will be stored.
    pub fn new(path: PathBuf) -> Self {
        fs::create_dir_all(&path).expect("Failed to create directory for FileSystemNodeStorage");
        FileSystemNodeStorage {
            dir_path: path,
            _marker: PhantomData,
        }
    }

    /// Gets the file path for a given node hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - The hash of the node.
    ///
    /// # Returns
    ///
    /// The file path where the node is stored.
    fn get_file_path(&self, hash: &ValueDigest<N>) -> PathBuf {
        let hash_str = hex::encode(hash.as_ref());
        self.dir_path.join(hash_str)
    }
}

impl<
        const N: usize,
        K: AsRef<[u8]>
            + Clone
            + PartialEq
            + From<Vec<u8>>
            + Default
            + Serialize
            + for<'de> Deserialize<'de>
            + 'static,
    > NodeStorage<N, K> for FileSystemNodeStorage<N, K>
{
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Node<N, K> {
        let file_path = self.get_file_path(hash);
        let mut file = File::open(file_path).expect("Node file not found");
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .expect("Failed to read node file");
        bincode::deserialize(&buffer).expect("Failed to deserialize node")
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: Node<N, K>) {
        let file_path = self.get_file_path(&hash);
        let mut file = File::create(file_path).expect("Failed to create node file");
        let buffer = bincode::serialize(&node).expect("Failed to serialize node");
        file.write_all(&buffer).expect("Failed to write node file");
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) {
        let file_path = self.get_file_path(hash);
        fs::remove_file(file_path).expect("Failed to delete node file");
    }
}
