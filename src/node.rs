use crate::digest::ValueDigest;
use crate::storage::NodeStorage;
use serde::{Deserialize, Serialize};

const MAX_KEYS: usize = 4; // Maximum number of keys in a node before it splits

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Node<const N: usize> {
    keys: Vec<Vec<u8>>,
    values: Vec<Vec<u8>>, // Stores the hashes of child nodes when not a leaf
    is_leaf: bool,
    level: u8,
}

impl<const N: usize> Node<N> {
    pub fn new(keys: Vec<u8>, values: Vec<u8>, is_leaf: bool, level: u8) -> Self {
        Node {
            keys: vec![keys],
            values: vec![values],
            is_leaf,
            level,
        }
    }

    pub fn update_values(&mut self, new_values: Vec<u8>) {
        self.values = vec![new_values];
    }

    pub fn update_keys(&mut self, new_keys: Vec<u8>) {
        self.keys = vec![new_keys];
    }

    pub fn insert<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, storage: &mut S) {
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
                let new_child = Node::new(key.clone(), value.clone(), true, self.level + 1);
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

    pub fn split_node<S: NodeStorage<N>>(&mut self, storage: &mut S) {
        let mid_index = self.keys.len() / 2;
        let mid_key = self.keys[mid_index].clone();

        // Create a new node for the right half
        let new_node = Node {
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
            let new_root = Node {
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

    pub fn update(&mut self, _key: Vec<u8>, _value: Vec<u8>) {
        // Implement the logic to update the value for the given key
    }

    pub fn delete(&mut self, _key: &[u8]) -> bool {
        // Implement the logic to delete the key-value pair from the node
        true
    }

    pub fn search(&self, _key: &[u8]) -> Option<&Node<N>> {
        // Implement the logic to search for the key in the node
        Some(self)
    }

    pub fn save_to_storage<S: NodeStorage<N>>(&self, storage: &mut S, hash: ValueDigest<N>) {
        storage.insert_node(hash, self.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{HashMapNodeStorage, NodeStorage};

    #[test]
    fn test_update_values() {
        let mut node: Node<32> = Node::new(vec![1, 2, 3], vec![4, 5, 6], true, 1);

        let new_values = vec![7, 8, 9];
        node.update_values(new_values.clone());

        assert_eq!(node.values.len(), 1);
        assert_eq!(node.values[0], new_values);
    }

    #[test]
    fn test_update_keys() {
        let mut node: Node<32> = Node::new(vec![1, 2, 3], vec![4, 5, 6], true, 1);

        let new_keys = vec![7, 8, 9];
        node.update_keys(new_keys.clone());

        assert_eq!(node.keys.len(), 1);
        assert_eq!(node.keys[0], new_keys);
    }

    #[test]
    fn test_save_to_storage() {
        let mut storage = HashMapNodeStorage::<32>::new();
        let node: Node<32> = Node::new(vec![1, 2, 3], vec![4, 5, 6], true, 1);
        let hash = ValueDigest::new(b"test_hash");

        node.save_to_storage(&mut storage, hash.clone());

        let retrieved_node = storage.get_node_by_hash(&hash);
        assert_eq!(retrieved_node.keys, node.keys);
        assert_eq!(retrieved_node.values, node.values);
        assert_eq!(retrieved_node.is_leaf, node.is_leaf);
        assert_eq!(retrieved_node.level, node.level);
    }

    #[test]
    fn test_insert() {
        let mut storage = HashMapNodeStorage::<32>::new();
        let mut node: Node<32> = Node::new(vec![1, 2, 3], vec![4, 5, 6], true, 1);

        let key = vec![4, 5, 6];
        let value = vec![7, 8, 9];
        node.insert(key.clone(), value.clone(), &mut storage);

        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.values.len(), 2);
    }
}
