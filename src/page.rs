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
#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::digest::ValueDigest;
use crate::node::Node;
use crate::visitor::Visitor;
use sha2::digest::FixedOutputReset;
use sha2::Digest;
use std::collections::VecDeque;

/// Represents a page in a prolly tree.
///
/// A page in a prolly tree is a collection of nodes that helps in organizing and managing
/// key-value pairs. Pages are used to create a hierarchical structure, enabling efficient
/// insertion, deletion, and lookup operations. Each page contains the following components:
///
/// - `nodes`: A vector of nodes, where each node contains a key, value hash, and pointers to
///   lower-level pages. The nodes within a page are kept sorted based on their keys to allow
///   efficient searches.
/// - `level`: The level of the page within the tree, indicating its depth. The root page has
///   the highest level, and the level decreases as you move down the tree.
///
/// Pages in the prolly tree support various operations to maintain the tree's structure and
/// balance:
///
/// - Insertion: Adding a new key-value pair to the page, ensuring that the nodes remain sorted.
/// - Deletion: Removing a key-value pair from the page.
/// - Balancing: Splitting and merging pages as necessary to maintain a balanced tree structure.
/// - Finding: Searching for a key within the page to retrieve the corresponding node.
///
/// The level of the page is used to manage the depth of the tree, and it plays a crucial role
/// in balancing operations, ensuring that the tree remains efficiently searchable.

#[cfg(feature = "prolly_balance_max_nodes")]
const MAX_NODES: usize = 100;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Page<const N: usize, K: AsRef<[u8]>> {
    pub nodes: Vec<Node<N, K>>, // A vector of nodes contained in this page
    pub level: usize,           // The level of the page within the tree
}

impl<const N: usize, K: Ord + Clone + AsRef<[u8]>> Page<N, K> {
    pub fn new(level: usize) -> Self {
        Page {
            nodes: Vec::new(),
            level,
        }
    }

    pub fn insert(&mut self, key: K, value_hash: ValueDigest<N>) {
        let new_node = Node::new(key, value_hash, self.level + 1);
        self.nodes.push(new_node);
        self.nodes.sort_by(|a, b| a.key().cmp(b.key()));
        self.balance();
    }

    pub fn delete(&mut self, key: &K) -> bool {
        if let Some(pos) = self.nodes.iter().position(|node| node.key() == key) {
            self.nodes.remove(pos);
            self.balance();
            true
        } else {
            false
        }
    }

    pub fn update(&mut self, key: K, value_hash: ValueDigest<N>) {
        if let Some(node) = self.nodes.iter_mut().find(|node| *node.key() == key) {
            node.set_value_hash(value_hash);
            // do not need to balance after update because the key is not changed
        }
    }

    #[cfg(feature = "prolly_balance_max_nodes")]
    pub fn balance(&mut self) {
        if self.nodes.len() > MAX_NODES {
            // Split the page into two
            let mid = self.nodes.len() / 2;
            let right_nodes = self.nodes.split_off(mid);
            let right_page = Page {
                nodes: right_nodes,
                level: self.level,
            };

            // Insert the middle key into the parent page
            let mid_node = self.nodes.pop().unwrap();
            let mut new_node = Node::new(
                mid_node.key().clone(),
                mid_node.value_hash().clone(),
                *mid_node.level() + 1,
            );
            new_node.set_lt_pointer(Some(Box::new(right_page)));

            let left_page = Page {
                nodes: self.nodes.clone(),
                level: self.level,
            };

            if let Some(ref mut last_node) = self.nodes.last_mut() {
                last_node.set_lt_pointer(Some(Box::new(left_page)));
            } else {
                // Handle the case where there are no nodes left after split
                self.nodes.push(new_node);
                return;
            }
            self.nodes.push(new_node);
        }
    }

    #[cfg(feature = "prolly_balance_rolling_hash")]
    fn rolling_hash_splitter(&self) -> VecDeque<usize> {
        let mut split_indices = VecDeque::new();
        let mut rolling_hash: u64 = 0;
        let mut chunk_size = 0;

        for (i, node) in self.nodes.iter().enumerate() {
            // Update the rolling hash with the current node's key and value hash
            for byte in node
                .key()
                .as_ref()
                .iter()
                .chain(node.value_hash().as_bytes().iter())
            {
                rolling_hash = (rolling_hash << 1).wrapping_add(*byte as u64);
            }

            // Increment the chunk size
            chunk_size += 1;

            // Decide whether to split the chunk
            // This means that as the chunk size grows, the target pattern shifts to make it easier to match.
            // For example,
            //   - If the chunk size is 4, the target pattern is 1 << 2, which equals 4 (binary 100).
            //   - If the chunk size is 8, the target pattern is 1 << 4, which equals 16 (binary 10000).
            let target_pattern = 1u64 << (chunk_size / 2);
            if rolling_hash & target_pattern == 0 {
                split_indices.push_back(i + 1);
                chunk_size = 0;
            }
        }

        split_indices
    }

    #[cfg(feature = "prolly_balance_rolling_hash")]
    pub fn balance(&mut self) {
        let mut split_indices = self.rolling_hash_splitter();
        let mut new_nodes = Vec::new();
        let mut current_page_nodes = Vec::new();

        for (i, node) in self.nodes.drain(..).enumerate() {
            current_page_nodes.push(node);

            if let Some(&split_index) = split_indices.front() {
                if i + 1 == split_index {
                    split_indices.pop_front();

                    let new_page = Page {
                        nodes: current_page_nodes.clone(),
                        level: self.level,
                    };

                    let mid_node = current_page_nodes.pop().unwrap();
                    let mut new_node = Node::new(
                        mid_node.key().clone(),
                        mid_node.value_hash().clone(),
                        *mid_node.level() + 1,
                    );
                    new_node.set_lt_pointer(Some(Box::new(new_page)));

                    new_nodes.push(new_node);
                    current_page_nodes.clear();
                }
            }
        }

        if !current_page_nodes.is_empty() {
            let new_page = Page {
                nodes: current_page_nodes,
                level: self.level,
            };

            let last_node = new_nodes.last_mut();
            if let Some(node) = last_node {
                node.set_lt_pointer(Some(Box::new(new_page)));
            } else {
                new_nodes.push(Node::new(
                    self.nodes.pop().unwrap().key().clone(),
                    self.nodes.pop().unwrap().value_hash().clone(),
                    self.level + 1,
                ));
            }
        }

        self.nodes = new_nodes;
    }

    pub fn find(&self, key: &K) -> Option<&Node<N, K>> {
        self.nodes.iter().find(|node| node.key() == key)
    }

    /// Traverse the page in an in-order fashion, calling the visitor for each node.
    /// If `visit_lt` is true, the visitor is called for the LT pointer of each node.
    /// Returns true if the traversal was successful, or false if the visitor returned false.
    /// The traversal stops if the visitor returns false.
    /// The visitor is called in the following order:
    /// - LT pointer (if `visit_lt` is true)
    /// - Node
    /// The traversal is depth-first, visiting the LT pointer first, then the node.
    /// The traversal is recursive, visiting the LT pointer first, then the node.
    pub fn in_order_traversal<'a, T>(&'a self, visitor: &mut T, visit_lt: bool) -> bool
    where
        T: Visitor<'a, N, K>,
    {
        for node in &self.nodes {
            if let Some(lt_pointer) = node.lt_pointer() {
                if !lt_pointer.in_order_traversal(visitor, visit_lt) {
                    return false;
                }
            }
            if visit_lt && !visitor.visit_node(node) {
                return false;
            }
        }
        true
    }

    pub fn calculate_hash<D: Digest + FixedOutputReset>(&self, hasher: &mut D) -> Vec<u8> {
        for node in &self.nodes {
            Digest::update(hasher, node.key().as_ref());
            Digest::update(hasher, node.value_hash().as_bytes());
        }
        let result = hasher.finalize_reset();
        result.to_vec()
    }

    pub fn min_subtree_key(&self) -> &K {
        self.nodes.first().map(|node| node.key()).unwrap()
    }

    pub fn max_subtree_key(&self) -> &K {
        self.nodes.last().map(|node| node.key()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::ValueDigest;

    #[test]
    fn test_page_new() {
        let page = Page::<32, String>::new(0);
        assert!(page.nodes.is_empty());
        assert_eq!(page.level, 0);
    }

    #[test]
    fn test_page_insert() {
        let mut page = Page::<32, String>::new(0);
        let key = "key1".to_string();
        let value = b"value1";
        let hash = ValueDigest::<32>::new(value);

        page.insert(key.clone(), hash.clone());

        assert_eq!(page.nodes.len(), 1);
        assert_eq!(page.nodes[0].key(), &key);
        assert_eq!(page.nodes[0].value_hash(), &hash);
    }

    #[test]
    fn test_page_delete() {
        let mut page = Page::<32, String>::new(0);
        let key = "key1".to_string();
        let value = b"value1";
        let hash = ValueDigest::<32>::new(value);

        page.insert(key.clone(), hash.clone());
        assert_eq!(page.nodes.len(), 1);

        let deleted = page.delete(&key);
        assert!(deleted);
        assert!(page.nodes.is_empty());
    }

    #[test]
    fn test_page_find() {
        let mut page = Page::<32, String>::new(0);
        let key = "key1".to_string();
        let value = b"value1";
        let hash = ValueDigest::<32>::new(value);

        page.insert(key.clone(), hash.clone());

        let found_node = page.find(&key);
        assert!(found_node.is_some());
        assert_eq!(found_node.unwrap().key(), &key);
    }

    #[cfg(feature = "prolly_balance_max_nodes")]
    #[test]
    fn test_page_balance() {
        let mut page = Page::<32, String>::new(0);

        for i in 1..7 {
            let key = format!("key{}", i);
            let value = format!("value{}", i);
            let hash = ValueDigest::<32>::new(value.as_bytes());
            page.insert(key.clone(), hash);
        }

        page.balance();

        let nodes = &page.nodes;
        assert!(nodes.windows(2).all(|w| w[0].key() <= w[1].key()));
    }

    #[cfg(feature = "prolly_balance_rolling_hash")]
    #[test]
    fn test_page_balance() {
        let mut page = Page::<32, String>::new(0);

        for i in 1..20 {
            let key = format!("key{}", i);
            let value = format!("value{}", i);
            let hash = ValueDigest::<32>::new(value.as_bytes());
            page.insert(key.clone(), hash);
        }

        page.balance();

        let nodes = &page.nodes;
        assert!(nodes.windows(2).all(|w| w[0].key() <= w[1].key()));

        // Check if the nodes are reasonably balanced
        let mut chunk_sizes = Vec::new();
        let mut current_chunk_size = 0;
        for node in nodes {
            current_chunk_size += 1;
            if let Some(ref lt_pointer) = node.lt_pointer() {
                chunk_sizes.push(current_chunk_size);
                current_chunk_size = 0;
            }
        }
        if current_chunk_size > 0 {
            chunk_sizes.push(current_chunk_size);
        }

        // Check if chunk sizes are within a reasonable range
        let mean_chunk_size = chunk_sizes.iter().sum::<usize>() as f64 / chunk_sizes.len() as f64;
        let max_chunk_size = chunk_sizes.iter().max().unwrap_or(&0);
        let min_chunk_size = chunk_sizes.iter().min().unwrap_or(&0);

        assert!(mean_chunk_size > 1.0);
        assert!(*max_chunk_size <= 10);
        assert!(*min_chunk_size >= 1);
    }

    #[cfg(feature = "prolly_balance_rolling_hash")]
    #[test]
    fn test_page_balance_large_inserts() {
        let mut page = Page::<32, String>::new(0);

        for i in 1..=200 {
            let key = format!("key{}", i);
            let value = format!("value{}", i);
            let hash = ValueDigest::<32>::new(value.as_bytes());
            page.insert(key.clone(), hash);
        }

        page.balance();

        let nodes = &page.nodes;
        assert!(nodes.windows(2).all(|w| w[0].key() <= w[1].key()));

        // Check if the nodes are reasonably balanced
        let mut chunk_sizes = Vec::new();
        let mut current_chunk_size = 0;
        for node in nodes {
            current_chunk_size += 1;
            if let Some(ref lt_pointer) = node.lt_pointer() {
                chunk_sizes.push(current_chunk_size);
                current_chunk_size = 0;
            }
        }
        if current_chunk_size > 0 {
            chunk_sizes.push(current_chunk_size);
        }

        // Check if chunk sizes are within a reasonable range
        let mean_chunk_size = chunk_sizes.iter().sum::<usize>() as f64 / chunk_sizes.len() as f64;
        let max_chunk_size = chunk_sizes.iter().max().unwrap_or(&0);
        let min_chunk_size = chunk_sizes.iter().min().unwrap_or(&0);

        assert!(mean_chunk_size > 1.0);
        assert!(*max_chunk_size <= 20);
        assert!(*min_chunk_size >= 1);
    }
}
