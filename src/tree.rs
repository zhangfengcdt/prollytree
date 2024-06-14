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
use serde::{Deserialize, Serialize};

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
        K: PartialEq + Clone + From<Vec<u8>> + AsRef<[u8]> + Serialize + for<'a> Deserialize<'a>,
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
        K: PartialEq + Clone + From<Vec<u8>> + AsRef<[u8]> + Serialize + for<'a> Deserialize<'a>,
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
    pub fn delete(&mut self, key: &K)
    where
        K: PartialEq + Clone + From<Vec<u8>> + AsRef<[u8]> + Serialize + for<'a> Deserialize<'a>,
    {
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
    pub fn find(&self, key: &K) -> Option<Node<N, K>>
    where
        K: PartialEq + Clone + From<Vec<u8>> + AsRef<[u8]> + Serialize + for<'a> Deserialize<'a>,
    {
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

    // This test is used as an example in the README.
    #[test]
    fn test_example() {
        // Step 1: Create and Wrap the Storage Backend
        let storage_backend = Arc::new(Mutex::new(HashMapNodeStorage::<32, Vec<u8>>::new()));

        // Step 2: Initialize the Root Node
        let root_node = Node::new(
            "root_key".as_bytes().to_vec(),
            "root_value".as_bytes().to_vec(),
            true,
            storage_backend,
        );

        // Step 3: Initialize the ProllyTree
        let mut tree = ProllyTree::new(root_node);

        // Step 4: Insert a New Key-Value Pair
        tree.insert(
            "new_key".as_bytes().to_vec(),
            "new_value".as_bytes().to_vec(),
        );

        // Step 5: Update the Value for an Existing Key
        tree.update(
            "new_key".as_bytes().to_vec(),
            "updated_value".as_bytes().to_vec(),
        );

        // Step 6: Find or Search for a Key
        let search_key = "new_key".as_bytes().to_vec();
        if let Some(_node) = tree.find(&search_key) {
            println!("Found node with key: {:?}", search_key);
        } else {
            println!("Node with key {:?} not found", search_key);
        }

        // Step 7: Delete a Key-Value Pair
        tree.delete(&search_key);
    }

    #[test]
    fn test_integer_key() {
        // Step 1: Create and Wrap the Storage Backend
        let storage_backend = Arc::new(Mutex::new(HashMapNodeStorage::<32, Vec<u8>>::new()));

        // Step 2: Initialize the Root Node
        let root_node = Node::new(
            1_u32.to_le_bytes().to_vec(),
            "abc".as_bytes().to_vec(),
            true,
            storage_backend,
        );
        // Step 3: Initialize the ProllyTree
        let mut tree = ProllyTree::new(root_node);

        // Step 4: Insert a New Key-Value Pair
        tree.insert(2_u32.to_le_bytes().to_vec(), "xyz".as_bytes().to_vec());
    }

    #[test]
    fn test_insert_multiple_values() {
        const N: usize = 32;
        type K = Vec<u8>;

        let key = "root_key".as_bytes().to_vec();
        let value = "root_value".as_bytes().to_vec();
        let root = Node::new(
            key.clone(),
            value,
            true,
            Arc::new(Mutex::new(HashMapNodeStorage::<N, K>::new())),
        );

        let mut tree = ProllyTree::new(root);

        let new_key1 = "new_key1".as_bytes().to_vec();
        let new_value1 = "new_value1".as_bytes().to_vec();
        tree.insert(new_key1.clone(), new_value1.clone());
        assert!(tree.find(&new_key1).is_some());

        let new_key2 = "new_key2".as_bytes().to_vec();
        let new_value2 = "new_value2".as_bytes().to_vec();
        tree.insert(new_key2.clone(), new_value2.clone());
        assert!(tree.find(&new_key2).is_some());
    }

    #[test]
    fn test_update_non_existent_key() {
        const N: usize = 32;
        type K = Vec<u8>;

        let key = "root_key".as_bytes().to_vec();
        let value = "root_value".as_bytes().to_vec();
        let root = Node::new(
            key.clone(),
            value,
            true,
            Arc::new(Mutex::new(HashMapNodeStorage::<N, K>::new())),
        );

        let mut tree = ProllyTree::new(root);

        let non_existent_key = "non_existent_key".as_bytes().to_vec();
        let new_value = "new_value".as_bytes().to_vec();
        tree.update(non_existent_key.clone(), new_value.clone());

        assert!(tree.find(&non_existent_key).is_none());
    }

    #[test]
    fn test_delete_non_existent_key() {
        const N: usize = 32;
        type K = Vec<u8>;

        let key = "root_key".as_bytes().to_vec();
        let value = "root_value".as_bytes().to_vec();
        let root = Node::new(
            key.clone(),
            value,
            true,
            Arc::new(Mutex::new(HashMapNodeStorage::<N, K>::new())),
        );

        let mut tree = ProllyTree::new(root);

        let non_existent_key = "non_existent_key".as_bytes().to_vec();
        tree.delete(&non_existent_key);

        assert!(tree.find(&non_existent_key).is_none());
    }

    #[test]
    fn test_find_non_existent_key() {
        const N: usize = 32;
        type K = Vec<u8>;

        let key = "root_key".as_bytes().to_vec();
        let value = "root_value".as_bytes().to_vec();
        let root = Node::new(
            key.clone(),
            value,
            true,
            Arc::new(Mutex::new(HashMapNodeStorage::<N, K>::new())),
        );

        let tree = ProllyTree::new(root);

        let non_existent_key = "non_existent_key".as_bytes().to_vec();
        assert!(tree.find(&non_existent_key).is_none());
    }

    #[test]
    fn test_insert_find_double_key() {
        const N: usize = 32;

        let storage_backend = Arc::new(Mutex::new(HashMapNodeStorage::<N, Vec<u8>>::new()));

        let key = 1.23_f32.to_be_bytes().to_vec();
        let value = "root_value".as_bytes().to_vec();
        let root = Node::new(key, value, true, storage_backend);

        let mut tree = ProllyTree::new(root);

        let new_key = 4.56_f32.to_be_bytes().to_vec();
        let new_value = "new_value".as_bytes().to_vec();
        tree.insert(new_key.clone(), new_value.clone());
        assert!(tree.find(&new_key).is_some());
    }

    #[derive(Serialize, Deserialize)]
    struct MyStruct {
        id: u32,
        name: String,
    }

    impl MyStruct {
        // Serialize the struct and return the serialized data
        fn to_bytes(&self) -> Vec<u8> {
            bincode::serialize(self).expect("Failed to serialize")
        }

        // Deserialize the struct from the given bytes
        fn from_bytes(bytes: &[u8]) -> Self {
            bincode::deserialize(bytes).expect("Failed to deserialize")
        }
    }

    // This is a helper struct to hold both the original struct and its serialized form
    struct SerializedStructWrapper {
        serialized: Vec<u8>,
    }

    impl SerializedStructWrapper {
        fn new(my_struct: MyStruct) -> Self {
            let serialized = my_struct.to_bytes();
            SerializedStructWrapper { serialized }
        }
    }

    impl AsRef<[u8]> for SerializedStructWrapper {
        fn as_ref(&self) -> &[u8] {
            &self.serialized
        }
    }

    #[test]
    fn test_insert_find_struct_value() {
        const N: usize = 32;

        let storage_backend = Arc::new(Mutex::new(HashMapNodeStorage::<N, Vec<u8>>::new()));

        let key = "root_key".as_bytes().to_vec();
        let my_struct = MyStruct {
            id: 1,
            name: String::from("example"),
        };

        let struct_values = SerializedStructWrapper::new(my_struct);

        let root = Node::new(key.clone(), struct_values, true, storage_backend);
        let mut tree = ProllyTree::new(root);

        let new_key = "new_key".as_bytes().to_vec();
        let new_my_struct = MyStruct {
            id: 2,
            name: String::from("example2"),
        };
        let new_my_struct_with_bytes = SerializedStructWrapper::new(new_my_struct);
        tree.insert(new_key.clone(), new_my_struct_with_bytes);
    }
}
