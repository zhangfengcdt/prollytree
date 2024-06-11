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

#![allow(dead_code)]
use sha2::digest::{FixedOutputReset, HashMarker};
use sha2::Sha256;
use std::marker::PhantomData;

use crate::digest::ValueDigest;
use crate::node::Node;
use crate::page::Page;

/// Represents a prolly tree with probabilistic balancing.
/// The tree is designed to be efficient and support operations like insertion,
/// deletion, and balancing, which maintain the probabilistic properties of the tree.
#[derive(Debug, Clone)]
pub struct ProllyTree<const N: usize, K: AsRef<[u8]>, V, H = Sha256> {
    root: Page<N, K>,
    root_hash: Option<Vec<u8>>,
    _value_type: PhantomData<V>,
    hasher: H,
}

impl<const N: usize, K, V, H> Default for ProllyTree<N, K, V, H>
where
    K: Ord + Clone + AsRef<[u8]>,
    V: Clone + AsRef<[u8]>,
    H: Default + FixedOutputReset + HashMarker,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, K, V, H> ProllyTree<N, K, V, H>
where
    K: Ord + Clone + AsRef<[u8]>,
    V: Clone + AsRef<[u8]>,
    H: Default + FixedOutputReset + HashMarker,
{
    /// Creates a new `ProllyTree` instance.
    /// The tree is initialized with an empty root page and no root hash.
    /// The hash function used is the default hash function (SHA-256).
    /// # Returns
    /// A new `ProllyTree` instance.
    /// # Example
    /// ```
    /// use prollytree::tree::ProllyTree;
    /// let tree = ProllyTree::<32, String, String>::new();
    /// ```
    /// This example creates a new `ProllyTree` instance with a key size of 32 bytes,
    /// storing `String` values.
    /// The tree is initialized with an empty root page and no root hash.
    /// The default hash function (SHA-256) is used.
    ///
    /// ```
    /// use sha2::Sha256;
    /// use prollytree::tree::ProllyTree;
    /// let tree = ProllyTree::<32, String, String, Sha256>::new();
    /// ```
    /// This example creates a new `ProllyTree` instance with a key size of 32 bytes,
    /// storing `String` values.
    /// The tree is initialized with an empty root page and no root hash.
    /// The SHA-256 hash function is explicitly specified.
    pub fn new() -> Self {
        ProllyTree {
            root: Page::new(0),
            root_hash: None,
            _value_type: PhantomData,
            hasher: H::default(),
        }
    }

    /// Creates a new `ProllyTree` instance with a custom hash function.
    /// The tree is initialized with an empty root page and no root hash.
    /// # Arguments
    /// * `hasher` - A custom hash function implementing the `Digest` trait.
    /// # Returns
    /// A new `ProllyTree` instance.
    pub fn new_with_hasher(hasher: H) -> Self {
        ProllyTree {
            root: Page::new(0),
            root_hash: None,
            _value_type: PhantomData,
            hasher,
        }
    }

    /// Creates a new `ProllyTree` instance with a custom hash function.
    /// The tree is initialized with an empty root page and no root hash.
    /// # Arguments
    /// * `hasher` - A custom hash function implementing the `Digest` trait.
    /// # Returns
    /// A new `ProllyTree` instance.
    /// # Example
    /// ```
    /// use sha2::Sha256;
    /// use prollytree::tree::ProllyTree;
    /// let mut tree = ProllyTree::<32, String, String, Sha256>::new();
    /// tree.insert("key".to_string(), "value".to_string());
    /// ```
    /// This example creates a new `ProllyTree` instance with a key size of 32 bytes,
    /// storing `String` values.
    /// The tree is initialized with an empty root page and no root hash.
    /// The SHA-256 hash function is explicitly specified.
    /// The `insert` method is then called to insert a key-value pair into the tree.
    pub fn insert(&mut self, key: K, value: V) {
        let value_hash = ValueDigest::new(value.as_ref());
        self.root.insert(key, value_hash);
        self.root_hash = None; // Invalidate the cached root hash
    }

    /// Updates the value associated with a key in the tree.
    /// # Arguments
    /// * `key` - The key to update.
    /// * `value` - The new value to associate with the key.
    /// # Example
    /// ```
    /// use prollytree::tree::ProllyTree;
    /// let mut tree = ProllyTree::<32, String, String>::new();
    /// tree.insert("key".to_string(), "value".to_string());
    /// tree.update("key".to_string(), "new value".to_string());
    /// ```
    /// This example inserts a key-value pair into the tree and then updates the value associated with the key.
    /// The `insert` method is used to insert the key-value pair into the tree.
    /// The `update` method is then called to update the value associated with the key.
    /// The `update` method replaces the existing value with the new value.
    /// The key remains the same.
    pub fn update(&mut self, key: K, value: V) {
        let value_hash = ValueDigest::new(value.as_ref());
        self.root.update(key, value_hash);
        self.root_hash = None; // Invalidate the cached root hash
    }

    /// Deletes a key from the tree.
    /// # Arguments
    /// * `key` - The key to delete.
    /// # Returns
    /// A boolean value indicating whether the key was successfully deleted.
    ///
    /// # Example
    /// ```
    /// use prollytree::tree::ProllyTree;
    /// let mut tree = ProllyTree::<32, String, String>::new();
    /// tree.insert("key".to_string(), "value".to_string());
    /// let deleted = tree.delete(&"key".to_string());
    /// ```
    /// This example inserts a key-value pair into the tree and then deletes the key.
    /// The `insert` method is used to insert the key-value pair into the tree.
    /// The `delete` method is then called to delete the key from the tree.
    /// The `delete` method returns a boolean value indicating whether the key was successfully deleted.
    pub fn delete(&mut self, key: &K) -> bool {
        let result = self.root.delete(key);
        if result {
            self.root_hash = None; // Invalidate the cached root hash
        }
        result
    }

    /// Returns the root hash of the tree.
    /// The root hash is calculated by hashing the root page of the tree.
    /// The root hash is cached and only recalculated if the root page has changed.
    /// # Returns
    /// A reference to the root hash of the tree.
    pub fn root_hash(&mut self) -> &Option<Vec<u8>> {
        if self.root_hash.is_none() {
            self.root_hash = Some(self.root.calculate_hash(&mut self.hasher));
        }
        &self.root_hash
    }

    /// Finds a key in the tree and returns the associated value.
    /// # Arguments
    /// * `key` - The key to find.
    /// # Returns
    /// An optional reference to the value associated with the key.
    /// If the key is found, the method returns a reference to the value.
    /// If the key is not found, the method returns `None`.
    pub fn find(&self, key: &K) -> Option<&Node<N, K>> {
        self.root.find(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Sha256;

    #[test]
    fn test_insert_and_find() {
        let mut tree = ProllyTree::<32, String, String, Sha256>::new();
        tree.insert("key".to_string(), "value".to_string());
        let node = tree.find(&"key".to_string());
        let new_hash = ValueDigest::<32>::new(b"value");

        assert!(node.is_some());
        assert_eq!(node.unwrap().value_hash().as_bytes(), new_hash.as_bytes());
    }

    #[test]
    fn test_update() {
        let mut tree = ProllyTree::<32, String, String, Sha256>::new();
        tree.insert("key".to_string(), "value".to_string());
        tree.update("key".to_string(), "new value".to_string());
        let node = tree.find(&"key".to_string());
        let new_hash = ValueDigest::<32>::new(b"new value");

        assert!(node.is_some());
        assert_eq!(node.unwrap().value_hash().as_bytes(), new_hash.as_bytes());
    }
}
