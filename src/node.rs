#![allow(unused_variables)]
#![allow(dead_code)]

use crate::digest::ValueDigest;
use crate::storage::NodeStorage;
use serde::{Deserialize, Serialize};

const MAX_KEYS: usize = 4; // Maximum number of keys in a node before it splits
const ROOT_LEVEL: u8 = 0;

pub trait Node<const N: usize> {
    fn insert<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, storage: &mut S);
    fn update<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, storage: &mut S);
    fn delete<S: NodeStorage<N>>(&mut self, key: &[u8], storage: &mut S) -> bool;
    fn find<S: NodeStorage<N>>(&self, key: &[u8], storage: &S) -> Option<ProllyNode<N>>;
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

    fn sort_and_split_and_persist<S: NodeStorage<N>>(&mut self, storage: &mut S) {
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
        let new_node_hash = new_node.get_hash();
        storage.insert_node(new_node_hash.clone(), new_node);

        // If the current node is the root, create a new root
        if self.level == ROOT_LEVEL {
            // Save the current root node to storage and get its hash
            let original_root_hash = self.get_hash();
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
            let current_node_hash = self.get_hash();
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
            self.sort_and_split_and_persist(storage);
        } else {
            // The node is an internal (non-leaf) node, so find the child node to insert the key-value pair.

            // Find the child node to insert the key-value pair
            // by comparing the key with the keys in the node and finding the correct child index
            // assuming the keys are already sorted increasingly.
            let i = self.keys.iter().rposition(|k| key >= *k).unwrap_or(0);

            // Retrieve the child node using the stored hash
            let child_hash = self.values[i].clone();

            if let Some(mut child_node) =
                storage.get_node_by_hash(&ValueDigest::raw_hash(&child_hash))
            {
                child_node.insert(key.clone(), value.clone(), storage);
                // let new_node_hash = ValueDigest::<N>::new(&child_node.values.concat().clone()).as_bytes().to_vec();
                let new_node_hash = &child_node.get_hash().as_bytes().to_vec();
                // save the updated child node back to the storage
                storage.insert_node(child_node.get_hash(), child_node);
                // update this node's value with the new hash
                self.values[i].clone_from(new_node_hash);
            } else {
                // Handle the case when the child node is not found
                // For example, you can log an error message or return an error
                // throw exception
                // TODO: return error
                println!("Child node not found: {:?}", child_hash);
            }

            // Try to sort the keys and values and split the node if necessary
            self.sort_and_split_and_persist(storage);
        }
    }

    fn update<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, _storage: &mut S) {
        // TODO to be implemented
    }

    fn delete<S: NodeStorage<N>>(&mut self, key: &[u8], storage: &mut S) -> bool {
        // TODO to be implemented
        false
    }

    fn find<S: NodeStorage<N>>(&self, key: &[u8], storage: &S) -> Option<ProllyNode<N>> {
        if self.is_leaf {
            // If the node is a leaf, check if the key exists in this node
            if self.keys.iter().any(|k| k == key) {
                Some(self.clone())
            } else {
                None
            }
        } else {
            // The node is an internal (non-leaf) node, so find the child node to search the key
            let i = self.keys.iter().rposition(|k| key >= &k[..]).unwrap_or(0);

            // Retrieve the child node using the stored hash
            let child_hash = self.values[i].clone();

            if let Some(child_node) = storage.get_node_by_hash(&ValueDigest::raw_hash(&child_hash))
            {
                child_node.find(key, storage)
            } else {
                // Handle the case when the child node is not found
                None
            }
        }
    }
}

// implement get hash function of the ProllyNode
impl<const N: usize> ProllyNode<N> {
    pub fn get_hash(&self) -> ValueDigest<N> {
        let mut keys_and_values = self.keys.concat();
        keys_and_values.extend(&self.values.concat());
        ValueDigest::new(&keys_and_values)
    }
}

impl<const N: usize> ProllyNode<N> {
    pub fn children(&self, storage: &impl NodeStorage<N>) -> Vec<ProllyNode<N>> {
        // Create an empty vector to store the child nodes
        let mut children = Vec::new();

        // Iterate over the values vector, which stores the hashes of the child nodes
        if !self.is_leaf {
            for child_hash in &self.values {
                // Retrieve the child node from the storage using the hash
                if let Some(child_node) =
                    storage.get_node_by_hash(&ValueDigest::raw_hash(child_hash))
                {
                    // Add the child node to the result vector
                    children.push(child_node);
                } else {
                    // Handle the case when the child node is not found
                    // For example, you can log an error message or return an error
                    println!("Child node not found")
                }
            }
        }

        // Return the vector of child nodes
        children
    }

    /// Traverse the tree in a breadth-first manner and return a string representation of the nodes.
    /// This method is useful for debugging and visualization purposes.
    /// The output string contains the level of each node, its keys, and whether it is a leaf node.
    /// The format of the output string is as follows:
    /// "L<level>:[<key1>, <key2>, ...]"
    /// where:
    /// - <level> is the level of the node.
    /// - <key1>, <key2>, ... are the keys of the node.
    /// # Arguments
    /// * `storage` - The storage implementation to retrieve child nodes.
    /// # Returns
    /// A string representation of the tree nodes in a breadth-first order.
    pub fn breadth_first_traverse(&self, storage: &impl NodeStorage<N>) -> String {
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(self.clone());

        let mut output = String::new();

        while let Some(node) = queue.pop_front() {
            if node.is_leaf {
                output += &node.format_node();
            }
            for child in node.children(storage) {
                queue.push_back(child.clone());
            }
        }

        output
    }

    /// Format the node as a string representation.
    /// The format of the output string is as follows:
    /// "L<level>:[<key1>, <key2>, ...]"
    /// where:
    /// - <level> is the level of the node.
    /// - <key1>, <key2>, ... are the keys of the node.
    /// # Returns
    /// A string representation of the node.
    fn format_node(&self) -> String {
        format!("[L{:?}:{:?}]", self.level, self.keys)
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

        let value_for_all = vec![100];

        // initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::init_root(vec![1], value_for_all.clone());

        // insert the 2nd key-value pair
        node.insert(vec![2], value_for_all.clone(), &mut storage);
        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.values.len(), 2);
        assert_eq!(node.is_leaf, true);

        // insert the 3rd key-value pair
        node.insert(vec![3], value_for_all.clone(), &mut storage);
        assert_eq!(node.keys.len(), 3);
        assert_eq!(node.values.len(), 3);
        assert_eq!(node.is_leaf, true);

        // insert the 4th key-value pair
        node.insert(vec![4], value_for_all.clone(), &mut storage);
        assert_eq!(node.keys.len(), 4);
        assert_eq!(node.values.len(), 4);
        assert_eq!(node.is_leaf, true);

        // assert values are sorted by keys
        assert_eq!(node.keys, vec![vec![1], vec![2], vec![3], vec![4]]);

        // insert the 5th key-value pair
        // expect the node to be split
        node.insert(vec![5], value_for_all.clone(), &mut storage);

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

        assert_eq!(
            node.breadth_first_traverse(&storage),
            "[L0:[[1], [2], [3], [4]]][L0:[[5]]]"
        );

        // insert more key-value pairs
        node.insert(vec![6], value_for_all.clone(), &mut storage);
        node.insert(vec![8], value_for_all.clone(), &mut storage);
        node.insert(vec![10], value_for_all.clone(), &mut storage);

        assert_eq!(
            node.breadth_first_traverse(&storage),
            "[L0:[[1], [2], [3], [4]]][L0:[[5], [6], [8], [10]]]"
        );

        node.insert(vec![12], value_for_all.clone(), &mut storage);
        node.insert(vec![15], value_for_all.clone(), &mut storage);
        node.insert(vec![20], value_for_all.clone(), &mut storage);
        node.insert(vec![28], value_for_all.clone(), &mut storage);
        println!("{}", node.breadth_first_traverse(&storage));

        // assert_eq!(
        //     node.breadth_first_traverse(&storage),
        //     "[L0:[[5], [6], [8], [10]]][L0:[[5], [6], [8], [10]]][L0:[[12]]]"
        // );
    }

    #[test]
    fn test_find() {
        let mut storage = HashMapNodeStorage::<32>::new();

        let value_for_all = vec![100];

        // initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::init_root(vec![1], value_for_all.clone());

        // insert key-value pairs
        node.insert(vec![2], value_for_all.clone(), &mut storage);
        node.insert(vec![3], value_for_all.clone(), &mut storage);
        node.insert(vec![4], value_for_all.clone(), &mut storage);
        node.insert(vec![5], value_for_all.clone(), &mut storage);

        // Test finding existing keys
        assert!(node.find(&[1], &storage).is_some());
        assert!(node.find(&[2], &storage).is_some());
        assert!(node.find(&[3], &storage).is_some());
        assert!(node.find(&[4], &storage).is_some());
        assert!(node.find(&[5], &storage).is_some());

        // Test finding a non-existing key
        assert!(node.find(&[6], &storage).is_none());

        // insert more key-value pairs
        node.insert(vec![6], value_for_all.clone(), &mut storage);
        node.insert(vec![7], value_for_all.clone(), &mut storage);
        node.insert(vec![8], value_for_all.clone(), &mut storage);
        node.insert(vec![9], value_for_all.clone(), &mut storage);

        // Test finding existing keys again after more insertions
        assert!(node.find(&[6], &storage).is_some());
        assert!(node.find(&[7], &storage).is_some());
        assert!(node.find(&[8], &storage).is_some());
        assert!(node.find(&[9], &storage).is_some());

        // Test finding a non-existing key
        assert!(node.find(&[10], &storage).is_none());
    }
}
