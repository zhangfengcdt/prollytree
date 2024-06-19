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
use crate::node::Node;

pub struct ProllyTree<const N: usize> {
    root: Node<N>,
    root_hash: Option<ValueDigest<N>>,
}

impl<const N: usize> ProllyTree<N> {
    /// Creates a new `ProllyTree` instance with a default hasher.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the tree.
    ///
    /// # Returns
    ///
    /// A new `ProllyTree` instance.
    pub fn new(root: Node<N>) -> Self {
        ProllyTree {
            root,
            root_hash: None,
        }
    }

    /// Creates a new `ProllyTree` instance with a custom hasher.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the tree.
    ///
    /// # Returns
    ///
    /// A new `ProllyTree` instance with the specified hasher.
    pub fn new_with_hasher(root: Node<N>) -> Self {
        ProllyTree {
            root,
            root_hash: None,
        }
    }

    /// Inserts a new key-value pair into the tree.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to insert.
    /// * `value` - The value to insert.
    pub fn insert<V>(&mut self, key: Vec<u8>, value: Vec<u8>)
    where
        V: AsRef<[u8]>,
    {
        self.root.insert(key, value);
        self.root_hash = None; // Invalidate the cached root hash
    }

    /// Updates the value associated with a key in the tree.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to update.
    /// * `value` - The new value to update.
    pub fn update<V>(&mut self, key: Vec<u8>, value: Vec<u8>)
    where
        V: AsRef<[u8]>,
    {
        self.root.update(key, value);
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
    pub fn delete(&mut self, key: &Vec<u8>) -> bool {
        self.root.delete(key)
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
    pub fn find(&self, key: &Vec<u8>) -> Option<&Node<N>> {
        self.root.search(key)
    }
}
