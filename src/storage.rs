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

use crate::digest::ValueDigest;
use crate::node::ProllyNode;
use std::collections::HashMap;
use std::fmt::{Display, Formatter, LowerHex};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::RwLock;

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
pub trait NodeStorage<const N: usize>: Send + Sync + Clone {
    /// Retrieves a node from storage by its hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - A reference to the `ValueDigest` representing the hash of the node to retrieve.
    ///
    /// # Returns
    ///
    /// The node associated with the given hash.
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<ProllyNode<N>>;

    /// Inserts a node into storage.
    ///
    /// # Arguments
    ///
    /// * `hash` - The `ValueDigest` representing the hash of the node to insert.
    /// * `node` - The node to insert into storage.
    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) -> Option<()>;

    /// Deletes a node from storage by its hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - A reference to the `ValueDigest` representing the hash of the node to delete.
    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Option<()>;

    fn save_config(&self, key: &str, config: &[u8]);
    fn get_config(&self, key: &str) -> Option<Vec<u8>>;
}

/// An implementation of `NodeStorage` that stores nodes in a HashMap.
///
/// # Type Parameters
///
/// - `N`: The size of the value digest.
#[derive(Debug)]
pub struct InMemoryNodeStorage<const N: usize> {
    map: HashMap<ValueDigest<N>, ProllyNode<N>>,
    configs: RwLock<HashMap<String, Vec<u8>>>,
}

impl<const N: usize> Clone for InMemoryNodeStorage<N> {
    fn clone(&self) -> Self {
        InMemoryNodeStorage {
            map: self.map.clone(),
            configs: RwLock::new(self.configs.read().unwrap().clone()),
        }
    }
}

impl<const N: usize> Default for InMemoryNodeStorage<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> InMemoryNodeStorage<N> {
    pub fn new() -> Self {
        InMemoryNodeStorage {
            map: HashMap::new(),
            configs: RwLock::new(HashMap::new()),
        }
    }
}

impl<const N: usize> NodeStorage<N> for InMemoryNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<ProllyNode<N>> {
        self.map.get(hash).cloned().map(|mut node| {
            node.split = false;
            node.merged = false;
            node
        })
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) -> Option<()> {
        self.map.insert(hash, node);
        Some(())
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Option<()> {
        self.map.remove(hash);
        Some(())
    }

    fn save_config(&self, key: &str, config: &[u8]) {
        self.configs
            .write()
            .unwrap()
            .insert(key.to_string(), config.to_vec());
    }

    fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        self.configs.read().unwrap().get(key).cloned()
    }
}

#[derive(Clone, Debug)]
pub struct FileNodeStorage<const N: usize> {
    storage_dir: PathBuf,
}

impl<const N: usize> FileNodeStorage<N> {
    pub fn new(storage_dir: PathBuf) -> Self {
        fs::create_dir_all(&storage_dir).unwrap();
        FileNodeStorage { storage_dir }
    }

    fn node_path(&self, hash: &ValueDigest<N>) -> PathBuf {
        self.storage_dir.join(format!("{hash:x}"))
    }

    fn config_path(&self, key: &str) -> PathBuf {
        self.storage_dir.join(format!("config_{key}"))
    }
}

impl<const N: usize> Display for ValueDigest<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl<const N: usize> LowerHex for ValueDigest<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl<const N: usize> NodeStorage<N> for FileNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<ProllyNode<N>> {
        let path = self.node_path(hash);
        if path.exists() {
            let mut file = File::open(path).unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            let mut node: ProllyNode<N> = bincode::deserialize(&data).unwrap();
            node.split = false;
            node.merged = false;
            Some(node)
        } else {
            None
        }
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) -> Option<()> {
        let path = self.node_path(&hash);
        let data = bincode::serialize(&node).unwrap();
        let mut file = File::create(path).unwrap();
        file.write_all(&data).unwrap();
        Some(())
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Option<()> {
        let path = self.node_path(hash);
        if path.exists() {
            fs::remove_file(path).unwrap();
            Some(())
        } else {
            None
        }
    }

    fn save_config(&self, key: &str, config: &[u8]) {
        let path = self.config_path(key);
        let mut file = File::create(path).unwrap();
        file.write_all(config).unwrap();
    }

    fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.config_path(key);
        if path.exists() {
            let mut file = File::open(path).unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            Some(data)
        } else {
            None
        }
    }
}

#[cfg(feature = "rocksdb_storage")]
pub use crate::rocksdb::RocksDBNodeStorage;
