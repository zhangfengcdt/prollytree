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

use crate::digest::ValueDigest;
use crate::page::Page;
use rand::Rng;

/// Represents a node in a prolly tree.
///
/// A prolly tree is a data structure used for efficient storage and retrieval of key-value pairs,
/// providing probabilistic balancing to ensure good performance characteristics. Each node in the prolly tree
/// contains the following components:
///
/// - `key`: The key of the node, which is used to organize and retrieve nodes within the tree.
/// - `value_hash`: A cryptographic hash of the value associated with the key. This ensures data integrity
///   and can be used for quick comparisons without storing the full value.
/// - `lt_pointer`: A pointer to a lower-level page (subtree) that contains nodes with keys strictly less than
///   the key of this node. This allows the tree to maintain a hierarchical structure.
/// - `level`: The level of the node within the tree, indicating its depth. The root node has the highest level,
///   and the level decreases as you move down the tree.
///
///
/// Nodes in the prolly tree are designed to be efficient and support operations like insertion, deletion, and
/// balancing, which maintain the probabilistic properties of the tree.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Node<const N: usize, K: AsRef<[u8]>> {
    key: K,                     // The key of the node
    value_hash: ValueDigest<N>, // The hash of the value associated with the key

    /// A pointer to a page with a strictly lower tree level, and containing
    /// strictly smaller/less-than keys when compared to "key".
    lt_pointer: Option<Box<Page<N, K>>>, // Pointer to a lower-level page

    /// Additional fields for probabilistic balancing
    level: usize,
}

impl<const N: usize, K: Ord + Clone + AsRef<[u8]>> Node<N, K> {
    pub fn new(key: K, value_hash: ValueDigest<N>, level: usize) -> Self {
        Self {
            key,
            value_hash,
            lt_pointer: None,
            level,
        }
    }

    // Getter for key
    pub fn key(&self) -> &K {
        &self.key
    }

    // Getter for value_hash
    pub fn value_hash(&self) -> &ValueDigest<N> {
        &self.value_hash
    }

    // Getter for lt_pointer
    pub fn lt_pointer(&self) -> &Option<Box<Page<N, K>>> {
        &self.lt_pointer
    }

    // Setter for lt_pointer
    pub fn set_lt_pointer(&mut self, lt_pointer: Option<Box<Page<N, K>>>) {
        self.lt_pointer = lt_pointer;
    }

    // Getter for level
    pub fn level(&self) -> &usize {
        &self.level
    }

    // Insert, delete, and balancing functions
    pub fn insert(&mut self, key: K, value_hash: ValueDigest<N>) {
        if key < self.key {
            if let Some(ref mut lt_pointer) = self.lt_pointer {
                lt_pointer.insert(key, value_hash);
            } else {
                self.lt_pointer = Some(Box::new(Page::new(self.level + 1)));
                self.lt_pointer.as_mut().unwrap().insert(key, value_hash);
            }
        } else {
            // Inserting in the current page (since it's a simple example)
            let mut page = Page::new(self.level + 1);
            page.insert(key, value_hash);
            self.lt_pointer = Some(Box::new(page));
        }
        self.balance();
    }

    pub fn delete(&mut self, key: &K) -> bool {
        if key < &self.key {
            if let Some(ref mut lt_pointer) = self.lt_pointer {
                lt_pointer.delete(key)
            } else {
                false
            }
        } else if key == &self.key {
            // For simplicity, we are not handling the deletion of the root node here.
            false
        } else if let Some(ref mut lt_pointer) = self.lt_pointer {
            lt_pointer.delete(key)
        } else {
            false
        }
    }

    pub fn balance(&mut self) {
        // Implementing a simple probabilistic balancing using random insertion
        if let Some(ref mut lt_pointer) = self.lt_pointer {
            let mut rng = rand::thread_rng();
            if rng.gen_bool(0.5) {
                // Randomly decide to balance
                // Placeholder for actual balancing logic
                lt_pointer.nodes.sort_by(|a, b| a.key().cmp(b.key()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::ValueDigest;

    #[test]
    fn test_node_new() {
        let key = "key1";
        let value = b"value1";
        let hash = ValueDigest::<32>::new(value);
        let level = 0;
        let node = Node::new(key.to_string(), hash.clone(), level);

        assert_eq!(node.key(), &key.to_string());
        assert_eq!(node.value_hash(), &hash);
        assert_eq!(node.level(), &level);
        assert!(node.lt_pointer().is_none());
    }

    #[test]
    fn test_node_insert() {
        let key = "key1";
        let value = b"value1";
        let hash = ValueDigest::<32>::new(value);
        let mut node = Node::new(key.to_string(), hash.clone(), 0);

        let new_key = "key2";
        let new_value = b"value2";
        let new_hash = ValueDigest::<32>::new(new_value);
        node.insert(new_key.to_string(), new_hash.clone());

        assert!(node.lt_pointer().is_some());
        if let Some(ref lt_pointer) = node.lt_pointer() {
            assert_eq!(
                lt_pointer.find(&new_key.to_string()).unwrap().key(),
                &new_key.to_string()
            );
        }
    }

    #[test]
    fn test_node_delete() {
        let key = "key1";
        let value = b"value1";
        let hash = ValueDigest::<32>::new(value);
        let mut node = Node::new(key.to_string(), hash.clone(), 0);

        let new_key = "key2";
        let new_value = b"value2";
        let new_hash = ValueDigest::<32>::new(new_value);
        node.insert(new_key.to_string(), new_hash.clone());

        assert!(node.delete(&new_key.to_string()));
        assert!(node.lt_pointer().is_some());
        if let Some(ref lt_pointer) = node.lt_pointer() {
            assert!(lt_pointer.find(&new_key.to_string()).is_none());
        }
    }

    #[test]
    fn test_node_balance() {
        let key = "key1";
        let value = b"value1";
        let hash = ValueDigest::<32>::new(value);
        let mut node = Node::new(key.to_string(), hash.clone(), 0);

        let new_key1 = "key2";
        let new_value1 = b"value2";
        let new_hash1 = ValueDigest::<32>::new(new_value1);
        node.insert(new_key1.to_string(), new_hash1.clone());

        let new_key2 = "key3";
        let new_value2 = b"value3";
        let new_hash2 = ValueDigest::<32>::new(new_value2);
        node.insert(new_key2.to_string(), new_hash2.clone());

        node.balance();

        if let Some(ref lt_pointer) = node.lt_pointer() {
            let nodes = &lt_pointer.nodes;
            assert!(nodes.windows(2).all(|w| w[0].key() <= w[1].key()));
        }
    }
}
