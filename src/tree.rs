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
use crate::node::Node;

pub struct ProllyTree<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> {
    root: Node<N, K>,
    root_hash: Option<ValueDigest<N>>,
}

impl<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> ProllyTree<N, K> {
    /// Creates a new `ProllyTree` instance with a default hasher.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the tree.
    ///
    /// # Returns
    ///
    /// A new `ProllyTree` instance.
    pub fn new(root: Node<N, K>) -> Self {
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
    pub fn new_with_hasher(root: Node<N, K>) -> Self {
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
    pub fn insert<V>(&mut self, key: K, value: V)
    where
        V: AsRef<[u8]>,
    {
        let value_hash = ValueDigest::new(value.as_ref());
        self.root.insert(key, value_hash);
        self.root_hash = None; // Invalidate the cached root hash
    }

    /// Updates the value associated with a key in the tree.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to update.
    /// * `value` - The new value to update.
    pub fn update<V>(&mut self, key: K, value: V)
    where
        V: AsRef<[u8]>,
    {
        let value_hash = ValueDigest::new(value.as_ref());
        self.root.update(key, value_hash);
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
    pub fn delete(&mut self, key: &K) {
        self.root.delete(key)
    }

    /// Calculates and returns the root hash of the tree.
    ///
    /// # Returns
    ///
    /// A reference to the cached root hash, calculating it if necessary.
    pub fn root_hash(&mut self) -> &Option<ValueDigest<N>> {
        if self.root_hash.is_none() {
            self.root_hash = Some(self.root.calculate_hash());
        }
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
    pub fn find(&self, key: &K) -> Option<Node<N, K>> {
        self.root.search(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Node;
    use crate::storage::HashMapNodeStorage;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_prolly_tree_insert_update_delete() {
        const N: usize = 32;
        type K = Vec<u8>;

        // Create a root node
        let key = "root_key".as_bytes().to_vec();
        let value = "root_value".as_bytes().to_vec();
        let root = Node::new(
            key.clone(),
            value,
            true,
            Arc::new(Mutex::new(HashMapNodeStorage::<N, K>::new())),
        );

        // Initialize the ProllyTree
        let mut tree = ProllyTree::new(root);

        // Test insert
        let new_key = "new_key".as_bytes().to_vec();
        let new_value = "new_value".as_bytes().to_vec();
        tree.insert(new_key.clone(), new_value.clone());
        assert!(
            tree.find(&new_key).is_some(),
            "Key should be present after insert"
        );

        // Test update
        let updated_value = "updated_value".as_bytes().to_vec();
        tree.update(new_key.clone(), updated_value.clone());
        // TODO: fix it
        // let found_node = tree.find(&new_key).unwrap();
        // let found_value_hash = ValueDigest::<32>::new("updated_value".as_bytes());
        // assert_eq!(
        //     found_node.value_hash,
        //     found_value_hash,
        //     "Value hash should be updated"
        // );

        // Test delete
        tree.delete(&new_key);
        assert!(
            tree.find(&new_key).is_none(),
            "Key should not be present after delete"
        );
    }
}
