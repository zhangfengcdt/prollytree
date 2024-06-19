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

use crate::digest::ValueDigest;
use crate::node::ProllyNode;

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
pub trait NodeStorage<const N: usize>: Send + Sync {
    /// Retrieves a node from storage by its hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - A reference to the `ValueDigest` representing the hash of the node to retrieve.
    ///
    /// # Returns
    ///
    /// The node associated with the given hash.
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> ProllyNode<N>;

    /// Inserts a node into storage.
    ///
    /// # Arguments
    ///
    /// * `hash` - The `ValueDigest` representing the hash of the node to insert.
    /// * `node` - The node to insert into storage.
    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>);

    /// Deletes a node from storage by its hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - A reference to the `ValueDigest` representing the hash of the node to delete.
    fn delete_node(&mut self, hash: &ValueDigest<N>);
}

/// An implementation of `NodeStorage2` that stores nodes in a filesystem.
///
/// # Type Parameters
///
/// - `N`: The size of the value digest.
pub struct FileSystemNodeStorage<const N: usize> {
    dir_path: PathBuf,
    _marker: PhantomData<ValueDigest<N>>,
}

impl<const N: usize> FileSystemNodeStorage<N> {
    /// Creates a new instance of `FileSystemNodeStorage2`.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the directory where node files will be stored.
    pub fn new(path: PathBuf) -> Self {
        fs::create_dir_all(&path).expect("Failed to create directory for FileSystemNodeStorage2");
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

impl<const N: usize> NodeStorage<N> for FileSystemNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> ProllyNode<N> {
        let file_path = self.get_file_path(hash);
        let mut file = File::open(file_path).expect("Node file not found");
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .expect("Failed to read node file");
        bincode::deserialize(&buffer).expect("Failed to deserialize node")
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) {
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

/// An implementation of `NodeStorage2` that stores nodes in a HashMap.
///
/// # Type Parameters
///
/// - `N`: The size of the value digest.
pub struct HashMapNodeStorage<const N: usize> {
    map: HashMap<ValueDigest<N>, ProllyNode<N>>,
}

impl<const N: usize> Default for HashMapNodeStorage<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> HashMapNodeStorage<N> {
    /// Creates a new instance of `HashMapNodeStorage2`.
    pub fn new() -> Self {
        HashMapNodeStorage {
            map: HashMap::new(),
        }
    }
}

impl<const N: usize> NodeStorage<N> for HashMapNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> ProllyNode<N> {
        self.map.get(hash).cloned().expect("Node not found")
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) {
        self.map.insert(hash, node);
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) {
        self.map.remove(hash);
    }
}
