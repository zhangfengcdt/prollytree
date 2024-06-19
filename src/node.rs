#![allow(unused_variables)]

use crate::digest::ValueDigest;
use crate::storage::NodeStorage;
use serde::{Deserialize, Serialize};

const MAX_KEYS: usize = 4; // Maximum number of keys in a node before it splits

pub trait Node<const N: usize> {
    fn insert<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, storage: &mut S);
    fn update<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, storage: &mut S);
    fn delete<S: NodeStorage<N>>(&mut self, key: &[u8], storage: &mut S) -> bool;
    fn find<S: NodeStorage<N>>(&self, key: &[u8], storage: &S) -> Option<&Self>;
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProllyNode<const N: usize> {
    keys: Vec<Vec<u8>>,
    values: Vec<Vec<u8>>, // Stores the hashes of child nodes when not a leaf
    is_leaf: bool,
    level: u8,
}

impl<const N: usize> ProllyNode<N> {
    pub fn new(keys: Vec<u8>, values: Vec<u8>, is_leaf: bool, level: u8) -> Self {
        ProllyNode {
            keys: vec![keys],
            values: vec![values],
            is_leaf,
            level,
        }
    }

    fn split_node<S: NodeStorage<N>>(&mut self, storage: &mut S) {
        let mid_index = self.keys.len() / 2;
        let mid_key = self.keys[mid_index].clone();

        // Create a new node for the right half
        let new_node = ProllyNode {
            keys: self.keys.split_off(mid_index + 1),
            values: self.values.split_off(mid_index + 1),
            is_leaf: self.is_leaf,
            level: self.level,
        };

        // Update the current node to hold the left half
        self.keys.truncate(mid_index);
        self.values.truncate(mid_index);

        // Save the new node to storage and get its hash
        let new_node_hash = ValueDigest::new(&new_node.values.concat());
        storage.insert_node(new_node_hash.clone(), new_node);

        // If the current node is the root, create a new root
        if self.level == 0 {
            let new_root = ProllyNode {
                keys: vec![mid_key],
                values: vec![new_node_hash.as_bytes().to_vec()],
                is_leaf: false,
                level: 0,
            };
            *self = new_root;
        } else {
            // Otherwise, promote the middle key to the parent
            self.keys.push(mid_key);
            self.values.push(new_node_hash.as_bytes().to_vec());
        }
    }
}

// implement the Node trait for ProllyNode
impl<const N: usize> Node<N> for ProllyNode<N> {
    fn insert<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, storage: &mut S) {
        if self.is_leaf {
            // Insert the key-value pair into the node
            self.keys.push(key);
            self.values.push(value);

            // Check if the node should be split
            if self.keys.len() > MAX_KEYS {
                self.split_node(storage);
            }
        } else {
            // Find the child node to insert the key-value pair
            let mut i = 0;
            while i < self.keys.len() && key > self.keys[i] {
                i += 1;
            }

            if i < self.values.len() {
                // Retrieve the child node using the stored hash
                let child_hash = &self.values[i];
                let mut child_node = storage.get_node_by_hash(&ValueDigest::new(child_hash));
                child_node.insert(key.clone(), value.clone(), storage);

                // Save the updated child node back to the storage
                let updated_child_hash = ValueDigest::new(&child_node.values.concat());
                storage.insert_node(updated_child_hash.clone(), child_node);
                self.values[i] = updated_child_hash.as_bytes().to_vec();
            } else {
                // If the child index is out of bounds, create a new child node
                let new_child = ProllyNode::new(key.clone(), value.clone(), true, self.level + 1);
                let new_child_hash = ValueDigest::new(&new_child.values.concat());
                storage.insert_node(new_child_hash.clone(), new_child);
                self.values.push(new_child_hash.as_bytes().to_vec());
            }

            // Check if the node should be split
            if self.keys.len() > MAX_KEYS {
                self.split_node(storage);
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::HashMapNodeStorage;

    #[test]
    fn test_insert() {
        let mut storage = HashMapNodeStorage::<32>::new();
        let mut node: ProllyNode<32> = ProllyNode::new(vec![1, 2, 3], vec![4, 5, 6], true, 1);

        let key = vec![4, 5, 6];
        let value = vec![7, 8, 9];
        node.insert(key.clone(), value.clone(), &mut storage);

        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.values.len(), 2);
    }
}
