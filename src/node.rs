#![allow(unused_variables)]
#![allow(dead_code)]

use crate::digest::ValueDigest;
use crate::storage::NodeStorage;
use crate::visitor::Visitor;
use serde::{Deserialize, Serialize};

const MAX_KEYS: usize = 4; // Maximum number of keys in a node before it splits
const ROOT_LEVEL: u8 = 0;

pub trait Node<const N: usize> {
    fn insert<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, storage: &mut S);
    fn update<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, storage: &mut S);
    fn delete<S: NodeStorage<N>>(&mut self, key: &[u8], storage: &mut S) -> bool;
    fn find<S: NodeStorage<N>>(&self, key: &[u8], storage: &S) -> Option<&Self>;
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProllyNode<const N: usize> {
    pub keys: Vec<Vec<u8>>,
    pub values: Vec<Vec<u8>>,
    pub is_leaf: bool,
    pub level: u8,
}

impl<const N: usize> ProllyNode<N> {
    pub fn init_root(key: Vec<u8>, value: Vec<u8>) -> Self {
        ProllyNode {
            keys: vec![key],
            values: vec![value],
            is_leaf: true,
            level: ROOT_LEVEL,
        }
    }

    pub fn new(key: Vec<u8>, value: Vec<u8>, is_leaf: bool, level: u8) -> Self {
        ProllyNode {
            keys: vec![key],
            values: vec![value],
            is_leaf,
            level,
        }
    }

    fn sort_and_split<S: NodeStorage<N>>(&mut self, storage: &mut S) {
        // Sort the keys and values in the node before splitting
        // Only sort the last key-value pair because the rest are already sorted
        if let (Some(last_key), Some(last_value)) = (self.keys.pop(), self.values.pop()) {
            let pos = self.keys.binary_search(&last_key).unwrap_or_else(|e| e);
            self.keys.insert(pos, last_key);
            self.values.insert(pos, last_value);
        }

        // Check if the node should be split
        if self.keys.len() <= MAX_KEYS {
            return;
        }

        // Handle the last key-value pair separately for splitting
        let last_index = self.keys.len() - 1;
        let last_key = self.keys[last_index].clone();
        let last_value = self.values[last_index].clone();

        // Create a new node for the last key-value pair
        let new_node = ProllyNode {
            keys: vec![last_key.clone()],
            values: vec![last_value.clone()],
            is_leaf: self.is_leaf,
            level: self.level,
        };

        // Remove the last key-value pair from the current node
        self.keys.pop();
        self.values.pop();

        // Save the new node to storage and get its hash
        let new_node_hash = ValueDigest::new(&new_node.values.concat());
        storage.insert_node(new_node_hash.clone(), new_node);

        // If the current node is the root, create a new root
        if self.level == ROOT_LEVEL {
            // Save the current root node to storage and get its hash
            let original_root_hash = ValueDigest::new(&self.values.concat());
            storage.insert_node(original_root_hash.clone(), self.clone());

            // Create a new root node
            let new_root = ProllyNode {
                keys: vec![self.keys[0].clone(), last_key],
                values: vec![
                    original_root_hash.as_bytes().to_vec(),
                    new_node_hash.as_bytes().to_vec(),
                ],
                is_leaf: false,
                level: self.level + 1,
            };
            *self = new_root;
        } else {
            // Otherwise, promote the last key to the parent
            // Insert the new node's key and its hash into the parent node
            self.keys.push(last_key);
            self.values.push(new_node_hash.as_bytes().to_vec());

            // Persist the current node
            let current_node_hash = ValueDigest::new(&self.values.concat());
            storage.insert_node(current_node_hash.clone(), self.clone());
        }
    }
}

// implement the Node trait for ProllyNode
impl<const N: usize> Node<N> for ProllyNode<N> {
    fn insert<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, storage: &mut S) {
        if self.is_leaf {
            // The node is a leaf node, so insert the key-value pair directly.

            // TODO: Check if the key already exists in the node
            // insert the key-value pair into the node
            self.keys.push(key);
            self.values.push(value);

            // sort the keys and values and split the node if necessary
            self.sort_and_split(storage);
        } else {
            // The node is an internal (non-leaf) node, so find the child node to insert the key-value pair.

            // Find the child node to insert the key-value pair
            // by comparing the key with the keys in the node and finding the correct child index
            // assuming the keys are already sorted increasingly.
            let mut i = 0;
            while i < self.keys.len() && key > self.keys[i] {
                i += 1;
            }

            if i < self.values.len() {
                // If the child index is within bounds, insert the key-value pair into the child node

                // Retrieve the child node using the stored hash
                let child_hash = &self.values[i];
                let child_node = storage.get_node_by_hash(&ValueDigest::new(child_hash));

                if let Some(mut child_node) =
                    storage.get_node_by_hash(&ValueDigest::new(child_hash))
                {
                    child_node.insert(key.clone(), value.clone(), storage);
                } else {
                    // Handle the case when the child node is not found
                    // For example, you can log an error message or return an error
                }

                // Save the updated child node back to the storage
                if let Some(child_node) = storage.get_node_by_hash(&ValueDigest::new(child_hash)) {
                    let keys = &child_node.keys;
                    let values = &child_node.values;
                    let is_leaf = child_node.is_leaf;
                    // Now you can use keys, values, and is_leaf in your code
                } else {
                    // Handle the case when the child node is not found
                    // For example, you can log an error message or return an error
                }
            } else {
                // If the child index is out of bounds, create a new child node

                // If the child index is out of bounds, create a new child node
                let new_child = ProllyNode::new(key.clone(), value.clone(), true, self.level + 1);
                let new_child_hash = ValueDigest::new(&new_child.values.concat());

                // Save the new child node to the storage
                storage.insert_node(new_child_hash.clone(), new_child);

                // Insert the key and the new child node's hash into the current node
                self.keys.push(key);
                self.values.push(new_child_hash.as_bytes().to_vec());
            }

            // Try to sort the keys and values and split the node if necessary
            self.sort_and_split(storage);
        }
    }

    fn update<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, _storage: &mut S) {
        // TODO to be implemented
    }

    fn delete<S: NodeStorage<N>>(&mut self, key: &[u8], storage: &mut S) -> bool {
        // TODO to be implemented
        false
    }

    fn find<S: NodeStorage<N>>(&self, key: &[u8], storage: &S) -> Option<&Self> {
        // TODO to be implemented
        Some(self)
    }
}

impl<const N: usize> ProllyNode<N> {
    fn traverse<'a, V, S>(&'a self, visitor: &mut V, storage: &S)
    where
        V: Visitor<'a, N, S>,
        S: NodeStorage<N>,
    {
        if visitor.pre_visit_node(self, storage) {
            if visitor.visit_node(self, storage) && !self.is_leaf {
                for value in &self.values {
                    let child_hash = ValueDigest::raw_hash(value);
                    let child_node = storage.get_node_by_hash(&child_hash);
                    // FIXME: This is a recursive call
                    // error[E0597]: `child_node` does not live long enough
                    //child_node.traverse(visitor, storage);
                }
            }
            visitor.post_visit_node(self, storage);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::HashMapNodeStorage;

    /// This test verifies the insertion of key-value pairs into a ProllyNode and ensures
    /// that the keys are sorted correctly and the node splits as expected when the maximum
    /// number of keys is exceeded.
    ///
    /// Steps:
    /// 1. Initialize a new root node with the first key-value pair.
    /// 2. Insert subsequent key-value pairs and verify that the node's keys and values
    ///    are updated correctly without splitting.
    /// 3. After inserting the 4th key-value pair, ensure that the keys are sorted correctly.
    /// 4. Insert the 5th key-value pair and verify that the node splits:
    ///    - The new root node should have 2 children.
    ///    - The first child should have 2 key-value pairs.
    ///    - The second child should have 1 key-value pair.
    #[test]
    fn test_insert_in_order() {
        let mut storage = HashMapNodeStorage::<32>::new();

        // initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::init_root(vec![1], vec![1]);

        // insert the 2nd key-value pair
        let key = vec![2];
        let value = vec![2];
        node.insert(key.clone(), value.clone(), &mut storage);
        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.values.len(), 2);
        assert_eq!(node.is_leaf, true);

        // insert the 3rd key-value pair
        let key = vec![3];
        let value = vec![3];
        node.insert(key.clone(), value.clone(), &mut storage);
        assert_eq!(node.keys.len(), 3);
        assert_eq!(node.values.len(), 3);
        assert_eq!(node.is_leaf, true);

        // insert the 4th key-value pair
        let key = vec![4];
        let value = vec![4];
        node.insert(key.clone(), value.clone(), &mut storage);
        assert_eq!(node.keys.len(), 4);
        assert_eq!(node.values.len(), 4);
        assert_eq!(node.is_leaf, true);

        // assert values are sorted by keys
        assert_eq!(node.keys, vec![vec![1], vec![2], vec![3], vec![4]]);

        // insert the 5th key-value pair
        // expect the node to be split
        let key = vec![5];
        let value = vec![5];
        node.insert(key.clone(), value.clone(), &mut storage);

        // new root node should have 2 children nodes
        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.values.len(), 2);
        assert_eq!(node.is_leaf, false);

        // the 1st child node should have 2 key-value pairs
        let child1_hash = &node.values[0];
        let child1 = storage.get_node_by_hash(&ValueDigest::raw_hash(child1_hash));
        assert_eq!(child1.clone().unwrap().keys.len(), 4);
        assert_eq!(child1.clone().unwrap().values.len(), 4);
        assert_eq!(child1.clone().unwrap().is_leaf, true);

        // the 2nd child node should have 3 key-value pairs
        let child2_hash = &node.values[1];
        let child2 = storage.get_node_by_hash(&ValueDigest::raw_hash(child2_hash));
        assert_eq!(child2.clone().unwrap().keys.len(), 1);
        assert_eq!(child2.clone().unwrap().values.len(), 1);
        assert_eq!(child2.clone().unwrap().is_leaf, true);
    }

    #[test]
    fn test_insert_in_order_2() {
        let mut storage = HashMapNodeStorage::<32>::new();

        // Initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::init_root(vec![1], vec![1]);

        // Insert multiple key-value pairs and assert their properties
        let test_cases = vec![(vec![2], vec![2]), (vec![3], vec![3]), (vec![4], vec![4])];

        for (i, (key, value)) in test_cases.iter().enumerate() {
            node.insert(key.clone(), value.clone(), &mut storage);
            if i < 3 {
                assert_eq!(node.keys.len(), i + 2);
                assert_eq!(node.values.len(), i + 2);
                assert_eq!(node.is_leaf, true);
            }
        }

        // Assert values are sorted by keys
        assert_eq!(node.keys, vec![vec![1], vec![2], vec![3], vec![4]]);

        // Insert the 5th key-value pair and expect the node to be split
        node.insert(vec![5], vec![5], &mut storage);

        // New root node should have 2 children nodes
        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.values.len(), 2);
        assert_eq!(node.is_leaf, false);

        // The 1st child node should have 2 key-value pairs
        let child1_hash = &node.values[0];
        let child1 = storage.get_node_by_hash(&ValueDigest::raw_hash(child1_hash));
        assert_eq!(child1.clone().unwrap().keys.len(), 4);
        assert_eq!(child1.clone().unwrap().values.len(), 4);
        assert_eq!(child1.clone().unwrap().is_leaf, true);

        // The 2nd child node should have 3 key-value pairs
        let child2_hash = &node.values[1];
        let child2 = storage.get_node_by_hash(&ValueDigest::raw_hash(child2_hash));
        assert_eq!(child2.clone().unwrap().keys.len(), 1);
        assert_eq!(child2.clone().unwrap().values.len(), 1);
        assert_eq!(child2.clone().unwrap().is_leaf, true);
    }
}
