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

#![allow(unused_variables)]
#![allow(dead_code)]

use crate::digest::ValueDigest;
use crate::storage::NodeStorage;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::hash::Hasher;
use twox_hash::XxHash64;

const ROOT_LEVEL: u8 = 0;
const HASH_SEED: u64 = 0;
const DEFAULT_BASE: u64 = 257;
const DEFAULT_MOD: u64 = 1_000_000_007;
const DEFAULT_MIN_CHUNK_SIZE: usize = 2;
const DEFAULT_MAX_CHUNK_SIZE: usize = 16*1024;
const DEFAULT_PATTERN: u64 = 0b11;

pub trait Node<const N: usize> {
    fn insert<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, storage: &mut S);
    fn delete<S: NodeStorage<N>>(&mut self, key: &[u8], storage: &mut S) -> bool;
    fn find<S: NodeStorage<N>>(&self, key: &[u8], storage: &S) -> Option<ProllyNode<N>>;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProllyNode<const N: usize> {
    pub keys: Vec<Vec<u8>>,
    pub values: Vec<Vec<u8>>,
    pub is_leaf: bool,
    pub level: u8,
    pub base: u64,
    pub modulus: u64,
    pub min_chunk_size: usize,
    pub max_chunk_size: usize,
    pub pattern: u64,
}

impl<const N: usize> Default for ProllyNode<N> {
    fn default() -> Self {
        ProllyNode {
            keys: Vec::new(),
            values: Vec::new(),
            is_leaf: true,
            level: 0,
            base: DEFAULT_BASE,
            modulus: DEFAULT_MOD,
            min_chunk_size: DEFAULT_MIN_CHUNK_SIZE,
            max_chunk_size: DEFAULT_MAX_CHUNK_SIZE,
            pattern: DEFAULT_PATTERN,
        }
    }
}

#[derive(Default)]
pub struct ProllyNodeBuilder<const N: usize> {
    keys: Vec<Vec<u8>>,
    values: Vec<Vec<u8>>,
    is_leaf: bool,
    level: u8,
    base: u64,
    modulus: u64,
    min_chunk_size: usize,
    max_chunk_size: usize,
    pattern: u64,
}

impl<const N: usize> ProllyNodeBuilder<N> {
    pub fn new() -> Self {
        ProllyNodeBuilder {
            keys: Vec::new(),
            values: Vec::new(),
            is_leaf: true,
            level: ROOT_LEVEL,
            base: DEFAULT_BASE,
            modulus: DEFAULT_MOD,
            min_chunk_size: DEFAULT_MIN_CHUNK_SIZE,
            max_chunk_size: DEFAULT_MAX_CHUNK_SIZE,
            pattern: DEFAULT_PATTERN,
        }
    }

    pub fn keys(mut self, keys: Vec<Vec<u8>>) -> Self {
        self.keys = keys;
        self
    }

    pub fn values(mut self, values: Vec<Vec<u8>>) -> Self {
        self.values = values;
        self
    }

    pub fn is_leaf(mut self, is_leaf: bool) -> Self {
        self.is_leaf = is_leaf;
        self
    }

    pub fn level(mut self, level: u8) -> Self {
        self.level = level;
        self
    }

    pub fn base(mut self, base: u64) -> Self {
        self.base = base;
        self
    }

    pub fn modulus(mut self, modulus: u64) -> Self {
        self.modulus = modulus;
        self
    }

    pub fn min_chunk_size(mut self, min_chunk_size: usize) -> Self {
        self.min_chunk_size = min_chunk_size;
        self
    }

    pub fn max_chunk_size(mut self, max_chunk_size: usize) -> Self {
        self.max_chunk_size = max_chunk_size;
        self
    }

    pub fn pattern(mut self, pattern: u64) -> Self {
        self.pattern = pattern;
        self
    }

    pub fn build(self) -> ProllyNode<N> {
        ProllyNode {
            keys: self.keys,
            values: self.values,
            is_leaf: self.is_leaf,
            level: self.level,
            base: self.base,
            modulus: self.modulus,
            min_chunk_size: self.min_chunk_size,
            max_chunk_size: self.max_chunk_size,
            pattern: self.pattern,
        }
    }
}

impl<const N: usize> ProllyNode<N> {
    pub fn init_root(key: Vec<u8>, value: Vec<u8>) -> Self {
        ProllyNode {
            keys: vec![key],
            values: vec![value],
            is_leaf: true,
            level: ROOT_LEVEL,
            ..Default::default()
        }
    }

    pub fn builder() -> ProllyNodeBuilder<N> {
        ProllyNodeBuilder::default()
    }

    fn sort_and_split_and_persist<S: NodeStorage<N>>(&mut self, storage: &mut S) {
        // Sort the keys and values in the node before splitting
        // Only sort the last key-value pair because the rest are already sorted
        if let (Some(last_key), Some(last_value)) = (self.keys.pop(), self.values.pop()) {
            let pos = self.keys.binary_search(&last_key).unwrap_or_else(|e| e);
            self.keys.insert(pos, last_key);
            self.values.insert(pos, last_value);
        }

        // Use chunk_content to determine split points
        let chunks = self.chunk_content();
        if chunks.len() <= 1 {
            return;
        }

        let mut siblings = Vec::new();
        let original_keys = std::mem::take(&mut self.keys);
        let original_values = std::mem::take(&mut self.values);

        for (start, end) in chunks {
            let sibling = ProllyNode {
                keys: original_keys[start..end].to_vec(),
                values: original_values[start..end].to_vec(),
                is_leaf: self.is_leaf,
                level: self.level,
                base: self.base,
                modulus: self.modulus,
                min_chunk_size: self.min_chunk_size,
                max_chunk_size: self.max_chunk_size,
                pattern: self.pattern,
            };
            let sibling_hash = sibling.get_hash();
            storage.insert_node(sibling_hash.clone(), sibling.clone());
            siblings.push((sibling, sibling_hash));
        }

        // If the current node is the root, create a new root
        if self.level == ROOT_LEVEL {
            // Save the current root node to storage and get its hash
            let original_root_hash = self.get_hash();
            storage.insert_node(original_root_hash.clone(), self.clone());

            // Create a new root node
            let new_root = ProllyNode {
                keys: siblings
                    .iter()
                    .map(|(sibling, _)| sibling.keys[0].clone())
                    .collect(),
                values: siblings
                    .iter()
                    .map(|(_, hash)| hash.as_bytes().to_vec())
                    .collect(),
                is_leaf: false,
                level: self.level + 1,
                base: self.base,
                modulus: self.modulus,
                min_chunk_size: self.min_chunk_size,
                max_chunk_size: self.max_chunk_size,
                pattern: self.pattern,
            };
            *self = new_root;
        } else {
            // Otherwise, promote the first key of each sibling to the parent
            for (sibling, sibling_hash) in siblings {
                self.keys.push(sibling.keys[0].clone());
                self.values.push(sibling_hash.as_bytes().to_vec());
            }

            // Persist the current node
            let current_node_hash = self.get_hash();
            storage.insert_node(current_node_hash.clone(), self.clone());
        }
    }

    fn rebalance<S: NodeStorage<N>>(&mut self, storage: &mut S) {
        // Use chunk_content to determine split points
        let chunks = self.chunk_content();

        // If chunks are valid and there are more than one chunk, split
        if chunks.len() > 1 {
            let mut siblings = Vec::new();
            let original_keys = std::mem::take(&mut self.keys);
            let original_values = std::mem::take(&mut self.values);

            for (start, end) in chunks {
                let sibling = ProllyNode {
                    keys: original_keys[start..end].to_vec(),
                    values: original_values[start..end].to_vec(),
                    is_leaf: self.is_leaf,
                    level: self.level,
                    base: self.base,
                    modulus: self.modulus,
                    min_chunk_size: self.min_chunk_size,
                    max_chunk_size: self.max_chunk_size,
                    pattern: self.pattern,
                };
                let sibling_hash = sibling.get_hash();
                storage.insert_node(sibling_hash.clone(), sibling.clone());
                siblings.push((sibling, sibling_hash));
            }

            // If the current node is the root, create a new root
            if self.level == ROOT_LEVEL {
                // Save the current root node to storage and get its hash
                let original_root_hash = self.get_hash();
                storage.insert_node(original_root_hash.clone(), self.clone());

                // Create a new root node
                let new_root = ProllyNode {
                    keys: siblings
                        .iter()
                        .map(|(sibling, _)| sibling.keys[0].clone())
                        .collect(),
                    values: siblings
                        .iter()
                        .map(|(_, hash)| hash.as_bytes().to_vec())
                        .collect(),
                    is_leaf: false,
                    level: self.level + 1,
                    base: self.base,
                    modulus: self.modulus,
                    min_chunk_size: self.min_chunk_size,
                    max_chunk_size: self.max_chunk_size,
                    pattern: self.pattern,
                };
                *self = new_root;
            } else {
                // Otherwise, promote the first key of each sibling to the parent
                for (sibling, sibling_hash) in siblings {
                    self.keys.push(sibling.keys[0].clone());
                    self.values.push(sibling_hash.as_bytes().to_vec());
                }

                // Persist the current node
                let current_node_hash = self.get_hash();
                storage.insert_node(current_node_hash.clone(), self.clone());
            }
        } else {
            // Attempt to merge with the next right neighbor if available
            if let Some(next_sibling_hash) = self.get_next_sibling_hash(storage) {
                if let Some(mut next_sibling) =
                    storage.get_node_by_hash(&ValueDigest::raw_hash(&next_sibling_hash))
                {
                    self.merge_with_next_sibling(&mut next_sibling, storage);
                }
            }
        }
    }

    fn get_next_sibling_hash<S: NodeStorage<N>>(&self, storage: &S) -> Option<Vec<u8>> {
        // TODO: Implement get_next_sibling_hash method
        // Logic to get the next sibling hash
        // This is a placeholder and should be implemented based on the storage and node structure
        None
    }

    fn merge_with_next_sibling<S: NodeStorage<N>>(
        &mut self,
        next_sibling: &mut ProllyNode<N>,
        storage: &mut S,
    ) {
        // Merge the current node with the next sibling
        self.keys.append(&mut next_sibling.keys);
        self.values.append(&mut next_sibling.values);

        // Persist the merged node
        let merged_node_hash = self.get_hash();
        storage.insert_node(merged_node_hash.clone(), self.clone());

        // Remove the next sibling node from storage
        let next_sibling_hash = next_sibling.get_hash();
        // TODO: implement remove_node method in NodeStorage
        //storage.remove_node(&next_sibling_hash);

        // Update the parent node
        if let Some(parent_hash) = self.get_parent_hash(storage) {
            if let Some(mut parent_node) =
                storage.get_node_by_hash(&ValueDigest::raw_hash(&parent_hash))
            {
                parent_node.update_child_hash(&next_sibling_hash, &merged_node_hash, storage);
            }
        }
    }

    fn get_parent_hash<S: NodeStorage<N>>(&self, storage: &S) -> Option<Vec<u8>> {
        // Logic to get the parent hash
        // This is a placeholder and should be implemented based on the storage and node structure
        None
    }

    fn update_child_hash<S: NodeStorage<N>>(
        &mut self,
        old_child_hash: &ValueDigest<N>,
        new_child_hash: &ValueDigest<N>,
        storage: &mut S,
    ) {
        // Update the hash of the child node in the parent node
        if let Some(pos) = self
            .values
            .iter()
            .position(|v| v == old_child_hash.as_bytes())
        {
            self.values[pos] = new_child_hash.as_bytes().to_vec();
            let parent_node_hash = self.get_hash();
            storage.insert_node(parent_node_hash.clone(), self.clone());
        }
    }
}

impl<const N: usize> NodeChunk for ProllyNode<N> {
    fn polynomial_hash<T: Hash>(data: &[T], base: u64, modulus: u64) -> u64 {
        let mut hash = 0;
        for item in data {
            let mut hasher = XxHash64::with_seed(HASH_SEED);
            item.hash(&mut hasher);
            hash = (hash * base + hasher.finish()) % modulus;
        }
        hash
    }
    fn calculate_rolling_hash(
        &self,
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
        window_size: usize,
    ) -> u64 {
        let keys_hash = Self::polynomial_hash(&keys[..window_size], self.base, self.modulus);
        let values_hash = Self::polynomial_hash(&values[..window_size], self.base, self.modulus);
        (keys_hash + values_hash) % self.modulus
    }
    fn chunk_content(&self) -> Vec<(usize, usize)> {
        let mut chunks = Vec::new();
        let mut start = 0;

        while start < self.keys.len() {
            let mut end = start + self.min_chunk_size;

            // Ensure that 'end' does not exceed the length of the keys vector
            if end > self.keys.len() {
                end = self.keys.len();
            }

            while end < self.keys.len() && end - start < self.max_chunk_size {
                // Ensure that 'end' does not exceed the length of the keys and values vectors
                if end > self.keys.len() || end > self.values.len() {
                    end = self.keys.len().min(self.values.len());
                    break;
                }

                let hash = self.calculate_rolling_hash(
                    &self.keys[start..end],
                    &self.values[start..end],
                    end - start,
                );
                if hash & self.pattern == self.pattern {
                    break;
                }
                end += 1;

                // Ensure that 'end' does not exceed the length of the keys and values vectors
                if end > self.keys.len() || end > self.values.len() {
                    end = self.keys.len().min(self.values.len());
                    break;
                }
            }

            chunks.push((start, end));
            start = end;
        }

        chunks
    }
}

trait NodeChunk {
    fn polynomial_hash<T: Hash>(data: &[T], base: u64, modulus: u64) -> u64;
    fn calculate_rolling_hash(
        &self,
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
        window_size: usize,
    ) -> u64;
    fn chunk_content(&self) -> Vec<(usize, usize)>;
}

// implement the Node trait for ProllyNode
impl<const N: usize> Node<N> for ProllyNode<N> {
    fn insert<S: NodeStorage<N>>(&mut self, key: Vec<u8>, value: Vec<u8>, storage: &mut S) {
        if self.is_leaf {
            // Check if the key already exists in the node
            if let Some(pos) = self.keys.iter().position(|k| k == &key) {
                // If the key exists, update its value
                self.values[pos] = value;
            } else {
                // Otherwise, insert the key-value pair into the node
                self.keys.push(key);
                self.values.push(value);
            }

            // Sort the keys and values and split the node if necessary
            self.sort_and_split_and_persist(storage);
        } else {
            // The node is an internal (non-leaf) node, so find the child node to insert the key-value pair

            // Find the child node to insert the key-value pair
            // by comparing the key with the keys in the node and finding the correct child index
            // assuming the keys are already sorted increasingly
            let i = self.keys.iter().rposition(|k| key >= *k).unwrap_or(0);

            // Retrieve the child node using the stored hash
            let child_hash = self.values[i].clone();

            if let Some(mut child_node) =
                storage.get_node_by_hash(&ValueDigest::raw_hash(&child_hash))
            {
                child_node.insert(key.clone(), value.clone(), storage);
                let new_node_hash = child_node.get_hash().as_bytes().to_vec();

                // Save the updated child node back to the storage
                storage.insert_node(child_node.get_hash(), child_node);

                // Update this node's value with the new hash
                self.values[i] = new_node_hash;
            } else {
                // Handle the case when the child node is not found
                println!("Child node not found: {:?}", child_hash);
            }

            // Sort the keys and values and split the node if necessary
            self.sort_and_split_and_persist(storage);
        }
    }

    fn delete<S: NodeStorage<N>>(&mut self, key: &[u8], storage: &mut S) -> bool {
        if self.is_leaf {
            // If the node is a leaf, try to find and remove the key
            if let Some(pos) = self.keys.iter().position(|k| k == key) {
                self.keys.remove(pos);
                self.values.remove(pos);

                // Persist the current node after deletion
                let current_node_hash = self.get_hash();
                storage.insert_node(current_node_hash.clone(), self.clone());

                // Rebalance if necessary
                self.rebalance(storage);

                true
            } else {
                false
            }
        } else {
            // The node is an internal (non-leaf) node, so find the child node to delete the key
            let i = self.keys.iter().rposition(|k| key >= &k[..]).unwrap_or(0);

            // Retrieve the child node using the stored hash
            let child_hash = self.values[i].clone();

            if let Some(mut child_node) =
                storage.get_node_by_hash(&ValueDigest::raw_hash(&child_hash))
            {
                if child_node.delete(key, storage) {
                    // If the deletion was successful, update the current node's child hash
                    let new_child_hash = child_node.get_hash();
                    self.values[i] = new_child_hash.as_bytes().to_vec();

                    // Persist the current node after updating the child hash
                    let current_node_hash = self.get_hash();
                    storage.insert_node(current_node_hash.clone(), self.clone());

                    // Check if the child node needs rebalancing
                    self.rebalance(storage);

                    true
                } else {
                    false
                }
            } else {
                // Handle the case when the child node is not found
                println!("Child node not found: {:?}", child_hash);
                false
            }
        }
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
    /// that the keys are sorted correctly and the node splits based on the chunk content.
    /// The test also checks the tree structure by traversing the tree in a breadth-first manner.
    /// The test uses a HashMapNodeStorage to store the nodes.
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
        assert!(node.is_leaf);

        // insert the 3rd key-value pair
        node.insert(vec![3], value_for_all.clone(), &mut storage);
        assert_eq!(node.keys.len(), 3);
        assert_eq!(node.values.len(), 3);
        assert!(node.is_leaf);

        // insert the 4th key-value pair
        node.insert(vec![4], value_for_all.clone(), &mut storage);
        assert_eq!(node.keys.len(), 4);
        assert_eq!(node.values.len(), 4);
        assert!(node.is_leaf);

        // assert values are sorted by keys
        assert_eq!(node.keys, vec![vec![1], vec![2], vec![3], vec![4]]);

        // insert the 5th key-value pair
        node.insert(vec![5], value_for_all.clone(), &mut storage);
        // insert the 6th key-value pair, which should trigger a split
        node.insert(vec![6], value_for_all.clone(), &mut storage);

        // new root node should have 2 children nodes
        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.values.len(), 2);
        assert!(!node.is_leaf);

        // the 1st child node should have 2 key-value pairs
        let child1_hash = &node.values[0];
        let child1 = storage.get_node_by_hash(&ValueDigest::raw_hash(child1_hash));
        assert_eq!(child1.clone().unwrap().keys.len(), 5);
        assert_eq!(child1.clone().unwrap().values.len(), 5);
        assert!(child1.clone().unwrap().is_leaf);

        // the 2nd child node should have 3 key-value pairs
        let child2_hash = &node.values[1];
        let child2 = storage.get_node_by_hash(&ValueDigest::raw_hash(child2_hash));
        assert_eq!(child2.clone().unwrap().keys.len(), 1);
        assert_eq!(child2.clone().unwrap().values.len(), 1);
        assert!(child2.clone().unwrap().is_leaf);

        // assert tree structure by traversing the tree in a breadth-first manner
        assert_eq!(
            node.breadth_first_traverse(&storage),
            "[L0:[[1], [2], [3], [4], [5]]][L0:[[6]]]"
        );

        // insert more key-value pairs
        node.insert(vec![6], value_for_all.clone(), &mut storage);
        node.insert(vec![8], value_for_all.clone(), &mut storage);
        node.insert(vec![10], value_for_all.clone(), &mut storage);

        assert_eq!(
            node.breadth_first_traverse(&storage),
            "[L0:[[1], [2], [3], [4], [5]]][L0:[[6], [8], [10]]]"
        );

        node.insert(vec![12], value_for_all.clone(), &mut storage);
        node.insert(vec![15], value_for_all.clone(), &mut storage);
        node.insert(vec![20], value_for_all.clone(), &mut storage);
        node.insert(vec![28], value_for_all.clone(), &mut storage);
        // should trigger a split and create a new root node here
        node.insert(vec![30], value_for_all.clone(), &mut storage);
        node.insert(vec![31], value_for_all.clone(), &mut storage);
        node.insert(vec![32], value_for_all.clone(), &mut storage);
        node.insert(vec![33], value_for_all.clone(), &mut storage);

        println!("{}", node.breadth_first_traverse(&storage));

        assert_eq!(
            node.breadth_first_traverse(&storage),
            "[L0:[[1], [2], [3], [4], [5]]][L0:[[6], [8], [10], [12], [15], [20], [28]]][L0:[[30], [31], [32], [33]]]"
        );
    }

    /// This test verifies the insertion and update of key-value pairs into a ProllyNode and ensures
    /// that the keys are sorted correctly and the node splits based on the chunk content.
    /// The test also checks the tree structure by traversing the tree in a breadth-first manner.
    /// The test uses a HashMapNodeStorage to store the nodes.
    #[test]
    fn test_insert_update() {
        let mut storage = HashMapNodeStorage::<32>::new();

        let value1 = vec![100];
        let value2 = vec![200];

        // initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::init_root(vec![1], value1.clone());

        // insert the 2nd key-value pair
        node.insert(vec![2], value1.clone(), &mut storage);
        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.values.len(), 2);
        assert!(node.is_leaf);

        // insert the 3rd key-value pair
        node.insert(vec![3], value1.clone(), &mut storage);
        assert_eq!(node.keys.len(), 3);
        assert_eq!(node.values.len(), 3);
        assert!(node.is_leaf);

        // insert the 4th key-value pair
        node.insert(vec![4], value1.clone(), &mut storage);
        assert_eq!(node.keys.len(), 4);
        assert_eq!(node.values.len(), 4);
        assert!(node.is_leaf);

        // Update the value of an existing key
        node.insert(vec![3], value2.clone(), &mut storage);
        assert_eq!(node.values[2], value2);

        // insert more key-value pairs
        node.insert(vec![5], value1.clone(), &mut storage);
        node.insert(vec![6], value1.clone(), &mut storage);
        node.insert(vec![7], value1.clone(), &mut storage);

        // Update the value of another existing key
        node.insert(vec![6], value2.clone(), &mut storage);
        assert!(node.find(&[6], &storage).unwrap().values.contains(&value2));
    }

    /// This test verifies the deletion of key-value pairs from a ProllyNode and ensures
    /// that the keys are sorted correctly and the node rebalances based on the chunk content.
    /// The test uses a HashMapNodeStorage to store the nodes.
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

    /// This test verifies the deletion of key-value pairs from a ProllyNode and ensures
    /// that the keys are sorted correctly and the node rebalances based on the chunk content.
    /// The test uses a HashMapNodeStorage to store the nodes.
    /// The test also checks the tree structure by traversing the tree in a breadth-first manner.
    #[test]
    fn test_delete() {
        let mut storage = HashMapNodeStorage::<32>::new();

        let value_for_all = vec![100];

        // initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::init_root(vec![1], value_for_all.clone());

        // insert key-value pairs
        node.insert(vec![2], value_for_all.clone(), &mut storage);
        node.insert(vec![3], value_for_all.clone(), &mut storage);
        node.insert(vec![4], value_for_all.clone(), &mut storage);
        node.insert(vec![5], value_for_all.clone(), &mut storage);

        // Test deleting existing keys
        assert!(node.delete(&[1], &mut storage));
        assert!(node.find(&[1], &storage).is_none());

        assert!(node.delete(&[2], &mut storage));
        assert!(node.find(&[2], &storage).is_none());

        assert!(node.delete(&[3], &mut storage));
        assert!(node.find(&[3], &storage).is_none());

        assert!(node.delete(&[4], &mut storage));
        assert!(node.find(&[4], &storage).is_none());

        assert!(node.delete(&[5], &mut storage));
        assert!(node.find(&[5], &storage).is_none());

        // Test deleting a non-existing key
        assert!(!node.delete(&[6], &mut storage));

        // Insert more key-value pairs and delete them to verify tree consistency
        node.insert(vec![7], value_for_all.clone(), &mut storage);
        node.insert(vec![8], value_for_all.clone(), &mut storage);
        node.insert(vec![9], value_for_all.clone(), &mut storage);

        assert!(node.delete(&[7], &mut storage));
        assert!(node.find(&[7], &storage).is_none());

        assert!(node.delete(&[8], &mut storage));
        assert!(node.find(&[8], &storage).is_none());

        assert!(node.delete(&[9], &mut storage));
        assert!(node.find(&[9], &storage).is_none());
    }

    #[test]
    fn test_chunk_content() {
        let mut storage = HashMapNodeStorage::<32>::new();
        let value_for_all = vec![100];
        let mut node: ProllyNode<32> = ProllyNode::default();

        // Insert multiple key-value pairs
        node.insert(vec![1], value_for_all.clone(), &mut storage);
        assert_eq!(node.chunk_content().len(), 1);
        node.insert(vec![2], value_for_all.clone(), &mut storage);
        assert_eq!(node.chunk_content().len(), 1);
        node.insert(vec![3], value_for_all.clone(), &mut storage);
        assert_eq!(node.chunk_content().len(), 1);
        node.insert(vec![4], value_for_all.clone(), &mut storage);
        assert_eq!(node.chunk_content().len(), 1);
        node.insert(vec![5], value_for_all.clone(), &mut storage);
        // The node is supposed to split into 2 chunks after this insertion.
        node.insert(vec![6], value_for_all.clone(), &mut storage);
        assert_eq!(node.chunk_content().len(), 1);
        node.insert(vec![7], value_for_all.clone(), &mut storage);
        assert_eq!(node.chunk_content().len(), 1);
        node.insert(vec![8], value_for_all.clone(), &mut storage);
    }
}
