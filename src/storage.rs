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
use crate::encoding::EncodingType;
use crate::node::ProllyNode;
use std::collections::HashMap;

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
    fn set_encoding_type(&mut self, encoding_type: Option<EncodingType>);
}

/// An implementation of `NodeStorage` that stores nodes in a HashMap.
///
/// # Type Parameters
///
/// - `N`: The size of the value digest.
#[derive(Clone)]
pub struct InMemoryNodeStorage<const N: usize> {
    map: HashMap<ValueDigest<N>, ProllyNode<N>>,
    configs: HashMap<String, Vec<u8>>,
    encoding_type: Option<EncodingType>,
}

impl<const N: usize> Default for InMemoryNodeStorage<N> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<const N: usize> InMemoryNodeStorage<N> {
    pub fn new(encoding_type: Option<EncodingType>) -> Self {
        InMemoryNodeStorage {
            map: HashMap::new(),
            configs: HashMap::new(),
            encoding_type,
        }
    }
}

impl<const N: usize> NodeStorage<N> for InMemoryNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<ProllyNode<N>> {
        if let Some(node) = self.map.get(hash) {
            if let Some(encoding_type) = self.encoding_type {
                match encoding_type {
                    EncodingType::Json =>
                    // Add other encoding types here if needed
                    {
                        Some(node.clone())
                    }
                };
            } else {
                return Some(node.clone());
            }
        }
        None
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) -> Option<()> {
        let node_to_insert = if let Some(encoding_type) = self.encoding_type {
            match encoding_type {
                EncodingType::Json => {
                    // Add other encoding types here if needed
                    Some(node)
                } // Add other encoding types here if needed
            }
        } else {
            Some(node)
        };

        if let Some(valid_node) = node_to_insert {
            self.map.insert(hash, valid_node);
            Some(())
        } else {
            None
        }
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Option<()> {
        self.map.remove(hash);
        Some(())
    }

    fn save_config(&self, key: &str, config: &[u8]) {
        let mut configs = self.configs.clone();
        configs.insert(key.to_string(), config.to_vec());
    }

    fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        self.configs.get(key).cloned()
    }

    fn set_encoding_type(&mut self, encoding_type: Option<EncodingType>) {
        self.encoding_type = encoding_type;
    }
}
