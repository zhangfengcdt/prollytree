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

#![allow(static_mut_refs)]
#![allow(dead_code)]

use crate::digest::ValueDigest;
use crate::node::{Node, ProllyNode};
use crate::storage::NodeStorage;

// TODO: extract to ProllyTree trait

pub struct ProllyTree<const N: usize, S: NodeStorage<N>> {
    root: ProllyNode<N>,
    root_hash: Option<ValueDigest<N>>,
    storage: S,
}

impl<const N: usize, S: NodeStorage<N>> ProllyTree<N, S> {
    /// Creates a new `ProllyTree` instance with a default hasher.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the tree.
    /// * `storage` - The storage for the tree nodes.
    ///
    /// # Returns
    ///
    /// A new `ProllyTree` instance.
    pub fn new(root: ProllyNode<N>, storage: S) -> Self {
        ProllyTree {
            root,
            root_hash: None,
            storage,
        }
    }

    /// Creates a new `ProllyTree` instance with a custom hasher.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the tree.
    /// * `storage` - The storage for the tree nodes.
    ///
    /// # Returns
    ///
    /// A new `ProllyTree` instance with the specified hasher.
    pub fn new_with_hasher(root: ProllyNode<N>, storage: S) -> Self {
        ProllyTree {
            root,
            root_hash: None,
            storage,
        }
    }

    /// Inserts a new key-value pair into the tree.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to insert.
    /// * `value` - The value to insert.
    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.root.insert(key, value, &mut self.storage);
        self.root_hash = None; // Invalidate the cached root hash
    }

    /// Deletes a key-value pair from the tree.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to delete.
    ///
    /// # Returns
    ///
    /// `true` if the key was found and deleted, `false` otherwise.
    pub fn delete(&mut self, key: &[u8]) -> bool {
        let deleted = self.root.delete(key, &mut self.storage);
        if deleted {
            self.root_hash = None; // Invalidate the cached root hash
        }
        deleted
    }

    /// Calculates and returns the root hash of the tree.
    ///
    /// # Returns
    ///
    /// A reference to the cached root hash, calculating it if necessary.
    pub fn root_hash(&mut self) -> &Option<ValueDigest<N>> {
        &self.root_hash
    }

    /// Searches for a key in the tree and returns the corresponding node if found.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to search for.
    ///
    /// # Returns
    ///
    /// An `Option` containing the node if found, or `None` if not found.
    pub fn find(&self, key: &[u8]) -> Option<ProllyNode<N>> {
        self.root.find(key, &self.storage)
    }
}
