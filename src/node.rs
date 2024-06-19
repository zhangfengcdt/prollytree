use crate::digest::ValueDigest;
use crate::storage::NodeStorage;
use serde::{Deserialize, Serialize};

const MAX_KEYS: usize = 4; // Maximum number of keys in a node before it splits

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Node<const N: usize> {
    keys: Vec<Vec<u8>>,
    values: Vec<ValueDigest<N>>,
    children: Vec<Box<Node<N>>>, // child nodes
    is_leaf: bool,
    level: u8,
}

impl<const N: usize> Node<N> {
    pub fn new(keys: Vec<u8>, values: Vec<u8>, is_leaf: bool, level: u8) -> Self {
        Node {
            keys: vec![keys],
            values: vec![ValueDigest::new(&values)],
            children: Vec::new(),
            is_leaf,
            level,
        }
    }

    pub fn update_values(&mut self, new_values: Vec<u8>) {
        self.values = vec![ValueDigest::new(&new_values)];
    }

    pub fn update_keys(&mut self, new_keys: Vec<u8>) {
        self.keys = vec![new_keys];
    }

    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) {
        if self.is_leaf {
            // Insert the key-value pair into the node
            let value_digest = ValueDigest::new(&value);
            self.keys.push(key);
            self.values.push(value_digest);

            // Check if the node should be split
            if self.keys.len() > MAX_KEYS {
                self.split_node();
            }
        } else {
            // Find the child node to insert the key-value pair
            let mut i = 0;
            while i < self.keys.len() && key > self.keys[i] {
                i += 1;
            }

            if i < self.children.len() {
                self.children[i].insert(key, value);
            } else {
                // If the child index is out of bounds, create a new child node
                let new_child = Node::new(key.clone(), value.clone(), true, self.level + 1);
                self.children.push(Box::new(new_child));
            }

            // Check if the node should be split
            if self.keys.len() > MAX_KEYS {
                self.split_node();
            }
        }
    }

    pub fn split_node(&mut self) {
        let mid_index = self.keys.len() / 2;
        let mid_key = self.keys[mid_index].clone();

        // Create a new node for the right half
        let new_node = Node {
            keys: self.keys.split_off(mid_index + 1),
            values: self.values.split_off(mid_index + 1),
            children: self.children.split_off(mid_index + 1),
            is_leaf: self.is_leaf,
            level: self.level,
        };

        // Update the current node to hold the left half
        self.keys.truncate(mid_index);
        self.values.truncate(mid_index);
        self.children.truncate(mid_index + 1);

        // If the current node is the root, create a new root
        if self.level == 0 {
            let new_root = Node {
                keys: vec![mid_key],
                values: vec![ValueDigest::new(b"")],
                children: vec![Box::new(self.clone()), Box::new(new_node)],
                is_leaf: false,
                level: 0,
            };
            *self = new_root;
        } else {
            // Otherwise, promote the middle key to the parent
            // Assuming the parent method handles the promotion
        }
    }

    pub fn update(&mut self, _key: Vec<u8>, _value: Vec<u8>) {
        // Implement the logic to update the value for the given key
    }

    pub fn delete(&mut self, _key: &Vec<u8>) -> bool {
        // Implement the logic to delete the key-value pair from the node
        true
    }

    pub fn search(&self, _key: &Vec<u8>) -> Option<&Node<N>> {
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
        assert_eq!(node.values[0], ValueDigest::new(&new_values));
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
}
