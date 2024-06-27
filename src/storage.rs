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

use crate::digest::ValueDigest;
use crate::node::ProllyNode;

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
}

/// An implementation of `NodeStorage` that stores nodes in a HashMap.
///
/// # Type Parameters
///
/// - `N`: The size of the value digest.
pub struct InMemoryNodeStorage<const N: usize> {
    map: HashMap<ValueDigest<N>, ProllyNode<N>>,
}

impl<const N: usize> Default for InMemoryNodeStorage<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> InMemoryNodeStorage<N> {
    /// Creates a new instance of `HashMapNodeStorage2`.
    pub fn new() -> Self {
        InMemoryNodeStorage {
            map: HashMap::new(),
        }
    }
}

impl<const N: usize> NodeStorage<N> for InMemoryNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<ProllyNode<N>> {
        self.map.get(hash).cloned()
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) -> Option<()> {
        self.map.insert(hash, node);
        Some(())
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Option<()> {
        self.map.remove(hash);
        Some(())
    }
}
