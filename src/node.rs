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
#![allow(clippy::too_many_arguments)]

use crate::digest::ValueDigest;
use crate::storage::NodeStorage;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::hash::Hasher;
use twox_hash::XxHash64;

const INIT_LEVEL: u8 = 0;
const HASH_SEED: u64 = 0;
const DEFAULT_BASE: u64 = 257;
const DEFAULT_MOD: u64 = 1_000_000_007;
const DEFAULT_MIN_CHUNK_SIZE: usize = 2;
const DEFAULT_MAX_CHUNK_SIZE: usize = 16 * 1024;

/// The default pattern is 0b11, which is used to determine the split points
/// The number of bit 1 determines the probability of split,
/// e.g., 0b11 has a higher probability of split than 0b1111
const DEFAULT_PATTERN: u64 = 0b11;

/// Trait representing a node with a fixed size N.
/// This trait provides methods for inserting, deleting, and finding key-value pairs in the node.
pub trait Node<const N: usize> {
    /// Inserts a key-value pair into the node.
    ///
    /// # Parameters
    /// - `key`: The key to insert.
    /// - `value`: The value associated with the key.
    /// - `storage`: The storage to use for persisting nodes.
    /// - `parent_hash`: An optional hash of the parent node.
    fn insert<S: NodeStorage<N>>(
        &mut self,
        key: Vec<u8>,
        value: Vec<u8>,
        storage: &mut S,
        path_hashes: Vec<ValueDigest<N>>,
    );

    /// Deletes a key-value pair from the node.
    ///
    /// # Parameters
    /// - `key`: The key to delete.
    /// - `storage`: The storage to use for persisting nodes.
    /// - `parent_hash`: An optional hash of the parent node.
    ///
    /// # Returns
    /// - `true` if the key was successfully deleted, `false` otherwise.
    fn delete<S: NodeStorage<N>>(
        &mut self,
        key: &[u8],
        storage: &mut S,
        path_hashes: Vec<ValueDigest<N>>,
    ) -> bool;

    /// Finds a key-value pair in the node.
    ///
    /// # Parameters
    /// - `key`: The key to find.
    /// - `storage`: The storage to use for persisting nodes.
    ///
    /// # Returns
    /// - `Some(ProllyNode<N>)` if the key was found, `None` otherwise.
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
    pub split: bool,
    pub merged: bool,
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
            split: false,
            merged: false,
        }
    }
}

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

impl<const N: usize> Default for ProllyNodeBuilder<N> {
    fn default() -> Self {
        ProllyNodeBuilder {
            keys: Vec::new(),
            values: Vec::new(),
            is_leaf: true,
            level: INIT_LEVEL,
            base: DEFAULT_BASE,
            modulus: DEFAULT_MOD,
            min_chunk_size: DEFAULT_MIN_CHUNK_SIZE,
            max_chunk_size: DEFAULT_MAX_CHUNK_SIZE,
            pattern: DEFAULT_PATTERN,
        }
    }
}

impl<const N: usize> ProllyNodeBuilder<N> {
    pub fn keys(mut self, keys: Vec<Vec<u8>>) -> Self {
        self.keys = keys;
        self
    }

    pub fn values(mut self, values: Vec<Vec<u8>>) -> Self {
        self.values = values;
        self
    }

    pub fn leaf(mut self, leaf: bool) -> Self {
        self.is_leaf = leaf;
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
            ..Default::default()
        }
    }
}

impl<const N: usize> ProllyNode<N> {
    pub fn init_root(key: Vec<u8>, value: Vec<u8>) -> Self {
        ProllyNode {
            keys: vec![key],
            values: vec![value],
            is_leaf: true,
            level: INIT_LEVEL,
            ..Default::default()
        }
    }

    pub fn builder() -> ProllyNodeBuilder<N> {
        ProllyNodeBuilder::default()
    }

    /// Attempts to balance the node by merging the next (right) neighbor
    /// and then splitting it into smaller nodes if necessary.
    fn balance<S: NodeStorage<N>>(
        &mut self,
        storage: &mut S,
        is_root_node: bool,
        path_hashes: &[ValueDigest<N>],
    ) {
        // Sort the keys and values in the node before splitting
        // Only sort the last key-value pair because the rest are already sorted
        if let (Some(last_key), Some(last_value)) = (self.keys.pop(), self.values.pop()) {
            let pos = self.keys.binary_search(&last_key).unwrap_or_else(|e| e);
            self.keys.insert(pos, last_key);
            self.values.insert(pos, last_value);
        }

        // If the node is a leaf, check if it can be merged with its next sibling
        if let Some(next_sibling_hash) = self.get_next_sibling_hash(storage, path_hashes) {
            if let Some(mut next_sibling) =
                storage.get_node_by_hash(&ValueDigest::raw_hash(&next_sibling_hash))
            {
                // Try to merge the current node with the next sibling
                self.merge_with_next_sibling(&mut next_sibling);
            }
        }

        // Use chunk_content to determine split points
        let chunks = self.chunk_content();
        if chunks.len() <= 1 {
            // do not need to split the node
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
                split: self.split,
                merged: self.merged,
            };
            let sibling_hash = sibling.get_hash();
            storage.insert_node(sibling_hash.clone(), sibling.clone());
            siblings.push((sibling, sibling_hash));
        }

        // If the current node is the only node in this level
        // we need to create a new root at the next level
        if is_root_node {
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
                split: self.split,
                merged: self.merged,
            };
            *self = new_root;
        } else {
            // Otherwise, promote the first key of each sibling to the parent
            // siblings holds the new split nodes of the current node
            for (sibling, sibling_hash) in siblings {
                self.keys.push(sibling.keys[0].clone());
                self.values.push(sibling_hash.as_bytes().to_vec());
            }
            self.is_leaf = false;
            self.split = true;

            // Persist the current node
            let current_node_hash = self.get_hash();
            storage.insert_node(current_node_hash.clone(), self.clone());
        }
    }

    fn get_next_sibling_hash<S: NodeStorage<N>>(
        &self,
        storage: &S,
        path_hashes: &[ValueDigest<N>],
    ) -> Option<Vec<u8>> {
        if let Some(parent_hash) = path_hashes.last() {
            // Retrieve the parent node using the parent hash
            if let Some(parent_node) = storage.get_node_by_hash(parent_hash) {
                // Find the position of the next sibling using the condition
                let largest_key = &self.keys[self.keys.len() - 1];
                if let Some(pos) = parent_node.keys.iter().position(|k| k > largest_key) {
                    // Check if there is a next sibling
                    if pos < parent_node.values.len() {
                        // Return the next sibling's hash
                        return Some(parent_node.values[pos].clone());
                    }
                }
            }
        }
        None
    }

    fn merge_with_next_sibling(&mut self, next_sibling: &mut ProllyNode<N>) {
        // Combine the keys and values of the current node and the next sibling
        let mut combined_keys = self.keys.clone();
        let mut combined_values = self.values.clone();
        combined_keys.append(&mut next_sibling.keys.clone());
        combined_values.append(&mut next_sibling.values.clone());

        // Merge the current node with the next sibling
        self.keys.append(&mut next_sibling.keys);
        self.values.append(&mut next_sibling.values);
        self.merged = true;
    }
}

impl<const N: usize> NodeChunk for ProllyNode<N> {
    fn chunk_content(&self) -> Vec<(usize, usize)> {
        let mut chunks = Vec::new();
        let mut start = 0;
        let mut last_start = 0;

        while start < self.keys.len() {
            let mut end = start + self.min_chunk_size;

            // Ensure that 'end' does not exceed the length of the keys vector
            if end > self.keys.len() {
                end = self.keys.len();
            }

            // Initialize the rolling hash for the first window
            let mut hash = Self::initialize_rolling_hash(
                &self.keys[start..end],
                &self.values[start..end],
                self.base,
                self.modulus,
            );

            while end < self.keys.len() && end - start < self.max_chunk_size {
                // Check if the current hash matches the pattern
                if hash & self.pattern == self.pattern {
                    break;
                }

                // Slide the window by one element to the right
                if end < self.keys.len() {
                    hash = Self::update_rolling_hash(
                        hash,
                        &self.keys[start],
                        &self.values[start],
                        &self.keys[end],
                        &self.values[end],
                        self.base,
                        self.modulus,
                        (end - start) as u64,
                    );
                    start += 1;
                    end += 1;
                } else {
                    break;
                }
            }

            chunks.push((last_start, end));
            last_start = end;
            start = end;
        }

        chunks
    }

    fn initialize_rolling_hash(
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
        base: u64,
        modulus: u64,
    ) -> u64 {
        let mut hash = 0;
        for (key, value) in keys.iter().zip(values) {
            hash = (hash * base
                + Self::hash_item(key, base, modulus)
                + Self::hash_item(value, base, modulus))
                % modulus;
        }
        hash
    }

    fn update_rolling_hash(
        old_hash: u64,
        old_key: &[u8],
        old_value: &[u8],
        new_key: &[u8],
        new_value: &[u8],
        base: u64,
        modulus: u64,
        window_size: u64,
    ) -> u64 {
        let old_key_hash = Self::hash_item(old_key, base, modulus);
        let old_value_hash = Self::hash_item(old_value, base, modulus);
        let new_key_hash = Self::hash_item(new_key, base, modulus);
        let new_value_hash = Self::hash_item(new_value, base, modulus);

        let base_exp_window_size = Self::mod_exp(base, window_size, modulus);

        let hash = (old_hash * base + new_key_hash + new_value_hash) % modulus;
        let hash = (hash + modulus - (old_key_hash * base_exp_window_size) % modulus) % modulus;

        (hash + modulus - (old_value_hash * base_exp_window_size) % modulus) % modulus
    }

    fn mod_exp(base: u64, exp: u64, modulus: u64) -> u64 {
        let mut result = 1;
        let mut base = base % modulus;
        let mut exp = exp;

        while exp > 0 {
            if exp % 2 == 1 {
                result = (result * base) % modulus;
            }
            exp >>= 1;
            base = (base * base) % modulus;
        }

        result
    }

    fn hash_item(item: &[u8], _base: u64, modulus: u64) -> u64 {
        let mut hasher = XxHash64::with_seed(HASH_SEED);
        item.hash(&mut hasher);
        hasher.finish() % modulus
    }
}

trait NodeChunk {
    fn chunk_content(&self) -> Vec<(usize, usize)>;
    fn initialize_rolling_hash(
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
        base: u64,
        modulus: u64,
    ) -> u64;
    fn update_rolling_hash(
        old_hash: u64,
        old_key: &[u8],
        old_value: &[u8],
        new_key: &[u8],
        new_value: &[u8],
        base: u64,
        modulus: u64,
        window_size: u64,
    ) -> u64;
    fn mod_exp(base: u64, exp: u64, modulus: u64) -> u64;
    fn hash_item(item: &[u8], base: u64, modulus: u64) -> u64;
}

// implement the Node trait for ProllyNode
impl<const N: usize> Node<N> for ProllyNode<N> {
    fn insert<S: NodeStorage<N>>(
        &mut self,
        key: Vec<u8>,
        value: Vec<u8>,
        storage: &mut S,
        mut path_hashes: Vec<ValueDigest<N>>,
    ) {
        // Record the current node's hash in the path
        path_hashes.push(self.get_hash());

        // set is root node based on parent hash
        let is_root_node = path_hashes.len() == 1;

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

            // Sort the keys and balance the node
            self.balance(storage, is_root_node, &path_hashes);
        } else {
            // The node is an internal (non-leaf) node, so find the child node to insert the key-value pair

            // Find the child node to insert the key-value pair
            // by comparing the key with the keys in the node and finding the correct child index
            // assuming the keys are already sorted increasingly
            let i = self.keys.iter().rposition(|k| key >= *k).unwrap_or(0);

            // Retrieve the child node using the stored hash
            // child node can be either leaf or internal node
            let child_hash = self.values[i].clone();

            if let Some(mut child_node) =
                storage.get_node_by_hash(&ValueDigest::raw_hash(&child_hash))
            {
                // if (key.clone() == vec![3]) {
                //     println!("Inserting key: {:?}, value: {:?}", key, value);
                // }

                // Insert the key-value pair into the child node retrieved from the storage
                child_node.insert(key.clone(), value.clone(), storage, path_hashes.clone());

                // Save the updated child node back to the storage
                let new_node_hash = child_node.get_hash().as_bytes().to_vec();
                storage.insert_node(child_node.get_hash(), child_node.clone());

                // Check if the child node has been merged into its parent's next sibling
                if child_node.merged {
                    // remove the next sibling from the parent node
                    if i + 1 < self.keys.len() {
                        self.keys.remove(i + 1);
                        self.values.remove(i + 1);
                    }
                }

                // Check if the child node has been split and needs to be updated in the current node
                //if !child_node.is_leaf && child_node.keys.len() > 1 {
                if child_node.split {
                    // Move the key-value pairs from the child node to the current node at position `i`
                    self.keys.remove(i);
                    self.values.remove(i);

                    for (j, (key, value)) in child_node
                        .keys
                        .into_iter()
                        .zip(child_node.values)
                        .enumerate()
                    {
                        self.keys.insert(i + j, key);
                        self.values.insert(i + j, value);
                    }
                } else {
                    // Update this node's value with the new hash
                    self.values[i] = new_node_hash;
                }
            } else {
                // Handle the case when the child node is not found
                println!("Child node not found: {:?}", child_hash);
            }

            // Sort the keys and balance the node
            self.balance(storage, is_root_node, &path_hashes);
        }
    }

    fn delete<S: NodeStorage<N>>(
        &mut self,
        key: &[u8],
        storage: &mut S,
        mut path_hashes: Vec<ValueDigest<N>>,
    ) -> bool {
        // Record the current node's hash in the path
        path_hashes.push(self.get_hash());

        // set is root node based on parent hash
        let is_root_node = path_hashes.len() == 1;

        if self.is_leaf {
            // If the node is a leaf, try to find and remove the key
            if let Some(pos) = self.keys.iter().position(|k| k == key) {
                self.keys.remove(pos);
                self.values.remove(pos);

                // Check if the leaf node is empty
                if self.keys.is_empty() {
                    return true; // Indicate that the node should be removed from the parent
                }

                // Persist the current node after deletion
                let current_node_hash = self.get_hash();
                storage.insert_node(current_node_hash.clone(), self.clone());

                // Sort the keys and balance the node
                self.balance(storage, is_root_node, &path_hashes);

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
                if child_node.delete(key, storage, path_hashes.clone()) {
                    // If the deletion was successful, check if the child node is empty
                    if child_node.keys.is_empty() {
                        // Remove the empty child node from the parent node's keys and values
                        self.keys.remove(i);
                        self.values.remove(i);

                        // Check if the current level is one above the init level and only one child node is left
                        if self.level == INIT_LEVEL + 1 && self.keys.len() == 1 {
                            // Retrieve the remaining child node using the stored hash
                            let remaining_child_hash = self.values[0].clone();
                            if let Some(mut remaining_child_node) = storage
                                .get_node_by_hash(&ValueDigest::raw_hash(&remaining_child_hash))
                            {
                                // Set the remaining child node as the new init node
                                remaining_child_node.level = INIT_LEVEL;
                                remaining_child_node.is_leaf = true;

                                // Persist the new root node
                                let new_root_hash = remaining_child_node.get_hash();
                                storage.insert_node(
                                    new_root_hash.clone(),
                                    remaining_child_node.clone(),
                                );

                                // Update the current node to become the new root node
                                *self = remaining_child_node;
                            }
                        }
                    } else {
                        // Update the current node's child hash
                        let new_child_hash = child_node.get_hash();
                        self.values[i] = new_child_hash.as_bytes().to_vec();
                    }

                    // Persist the current node after updating the child hash or removing the child node
                    let current_node_hash = self.get_hash();
                    storage.insert_node(current_node_hash.clone(), self.clone());

                    // Sort the keys and balance the node
                    self.balance(storage, is_root_node, &path_hashes);

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
    /// [L0:[key1, key2, ...]][L1:[key3, key4, ...]]
    /// where L0, L1, ... are the levels of the nodes, and key1, key2, ... are the keys in the nodes.
    pub fn traverse(&self, storage: &impl NodeStorage<N>) -> String {
        self.formatted_traverse(storage, |node| {
            if node.level == 0 {
                // return the keys for leaf nodes
                format!("[L{}:{:?}]", node.level, node.keys.to_vec())
            } else {
                // return empty string for non-leaf nodes
                "".to_string()
            }
        })
    }

    /// Traverse the tree in a breadth-first manner and return a string representation of the nodes.
    /// This method is useful for debugging and visualization purposes.
    /// The output string contains the level of each node, its keys, and whether it is a leaf node.
    /// The format of the output string is customizable using a closure.
    ///
    /// # Arguments
    /// * `storage` - The storage implementation to retrieve child nodes.
    /// * `formatter` - A closure that takes a reference to a node and returns a string representation of the node.
    ///
    /// # Returns
    /// A string representation of the tree nodes in a breadth-first order.
    pub fn formatted_traverse<F>(&self, storage: &impl NodeStorage<N>, formatter: F) -> String
    where
        F: Fn(&ProllyNode<N>) -> String,
    {
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(self.clone());

        let mut output = String::new();

        while let Some(node) = queue.pop_front() {
            output += &formatter(&node);
            for child in node.children(storage) {
                queue.push_back(child.clone());
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryNodeStorage;
    use rand::seq::SliceRandom;

    /// This test verifies the insertion of key-value pairs into a ProllyNode and ensures
    /// that the keys are sorted correctly and the node splits based on the chunk content.
    /// The test also checks the tree structure by traversing the tree in a breadth-first manner.
    /// The test uses a HashMapNodeStorage to store the nodes.
    #[test]
    fn test_insert_in_order() {
        let mut storage = InMemoryNodeStorage::<32>::new();

        let value_for_all = vec![100];

        // initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::init_root(vec![1], value_for_all.clone());

        // insert the 2nd key-value pair
        node.insert(vec![2], value_for_all.clone(), &mut storage, Vec::new());
        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.values.len(), 2);
        assert!(node.is_leaf);

        // insert the 3rd key-value pair
        node.insert(vec![3], value_for_all.clone(), &mut storage, Vec::new());
        assert_eq!(node.keys.len(), 3);
        assert_eq!(node.values.len(), 3);
        assert!(node.is_leaf);

        // insert the 4th key-value pair
        node.insert(vec![4], value_for_all.clone(), &mut storage, Vec::new());
        assert_eq!(node.keys.len(), 4);
        assert_eq!(node.values.len(), 4);
        assert!(node.is_leaf);

        // assert values are sorted by keys
        assert_eq!(node.keys, vec![vec![1], vec![2], vec![3], vec![4]]);

        // insert the 5th key-value pair
        node.insert(vec![5], value_for_all.clone(), &mut storage, Vec::new());
        // insert the 6th key-value pair, which should trigger a split
        node.insert(vec![6], value_for_all.clone(), &mut storage, Vec::new());
        // insert the 7th key-value pair, which should trigger a split
        node.insert(vec![7], value_for_all.clone(), &mut storage, Vec::new());

        // new root node should have 2 children nodes
        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.values.len(), 2);
        assert!(!node.is_leaf);

        // the 1st child node should have 2 key-value pairs
        let child1_hash = &node.values[0];
        let child1 = storage.get_node_by_hash(&ValueDigest::raw_hash(child1_hash));
        assert_eq!(child1.clone().unwrap().keys.len(), 6);
        assert_eq!(child1.clone().unwrap().values.len(), 6);
        assert!(child1.clone().unwrap().is_leaf);

        // the 2nd child node should have 3 key-value pairs
        let child2_hash = &node.values[1];
        let child2 = storage.get_node_by_hash(&ValueDigest::raw_hash(child2_hash));
        assert_eq!(child2.clone().unwrap().keys.len(), 1);
        assert_eq!(child2.clone().unwrap().values.len(), 1);
        assert!(child2.clone().unwrap().is_leaf);

        // assert tree structure by traversing the tree in a breadth-first manner
        assert_eq!(
            node.traverse(&storage),
            "[L0:[[1], [2], [3], [4], [5], [6]]][L0:[[7]]]"
        );

        // insert more key-value pairs
        node.insert(vec![6], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![8], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![10], value_for_all.clone(), &mut storage, Vec::new());

        assert_eq!(
            node.traverse(&storage),
            "[L0:[[1], [2], [3], [4], [5], [6]]][L0:[[7], [8], [10]]]"
        );

        node.insert(vec![12], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![15], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![20], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![28], value_for_all.clone(), &mut storage, Vec::new());
        // should trigger a split and create a new root node here
        node.insert(vec![30], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![31], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![32], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![33], value_for_all.clone(), &mut storage, Vec::new());

        println!("{}", node.traverse(&storage));

        assert_eq!(
            node.traverse(&storage),
            "[L0:[[1], [2], [3], [4], [5], [6]]][L0:[[7], [8], [10]]][L0:[[12], [15], [20], [28], [30]]][L0:[[31], [32], [33]]]"
        );
    }

    #[test]
    fn test_insert_rev_order() {
        let mut storage = InMemoryNodeStorage::<32>::new();

        let value_for_all = vec![100];
        let max_key = 100;

        let mut storage_ref = InMemoryNodeStorage::<32>::new();

        // initialize a new root node with the first key-value pair
        let mut node_ref: ProllyNode<32> = ProllyNode::default();

        // the keys equal to their index in the vector
        for i in 0..=max_key {
            node_ref.insert(vec![i], value_for_all.clone(), &mut storage_ref, Vec::new());
            storage.insert_node(node_ref.get_hash(), node_ref.clone());
        }
        println!("increasing order: {}", node_ref.traverse(&storage_ref));

        // initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::default();

        // each time an insert is done, the root node hash is updated
        for i in (0..=max_key).rev() {
            node.insert(vec![i], value_for_all.clone(), &mut storage, Vec::new());
            storage.insert_node(node.get_hash(), node.clone()); // save the updated root node hash to storage
                                                                // println!("{}", node.traverse(&storage));
        }
        println!("decreasing order: {}", node.traverse(&storage));

        assert_eq!(node_ref.traverse(&storage_ref), node.traverse(&storage));
    }

    #[test]
    fn test_insert_alt_order() {
        let mut storage = InMemoryNodeStorage::<32>::new();
        let value_for_all = vec![100];
        let max_key = 35;

        // Initialize a new root node with the first key-value pair
        let mut node_ref: ProllyNode<32> = ProllyNode::default();

        // Insert elements in increasing order
        for i in 0..=max_key {
            node_ref.insert(vec![i], value_for_all.clone(), &mut storage, Vec::new());
            storage.insert_node(node_ref.get_hash(), node_ref.clone());
        }
        println!("inc order: {}", node_ref.traverse(&storage));

        // Initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::default();

        // Generate keys in an alternating order (odd numbers first, then even numbers)
        let mut keys: Vec<u8> = (1..=max_key).step_by(2).collect(); // odd numbers
        keys.extend((0..=max_key).step_by(2)); // even numbers

        // Insert elements in alternating order
        for key in keys {
            node.insert(vec![key], value_for_all.clone(), &mut storage, Vec::new());
            storage.insert_node(node.get_hash(), node.clone()); // save the updated root node hash to storage
            println!("alt order: {}", node.traverse(&storage));
        }

        println!("alt order: {}", node.traverse(&storage));

        // Verify that both trees have the same structure
        assert_eq!(node_ref.traverse(&storage), node.traverse(&storage));
    }

    /// This test verifies the insertion and update of key-value pairs into a ProllyNode and ensures
    /// that the keys are sorted correctly and the node splits based on the chunk content.
    /// The test also checks the tree structure by traversing the tree in a breadth-first manner.
    /// The test uses a HashMapNodeStorage to store the nodes.
    #[test]
    fn test_insert_update() {
        let mut storage = InMemoryNodeStorage::<32>::new();

        let value1 = vec![100];
        let value2 = vec![200];

        // initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::init_root(vec![1], value1.clone());

        // insert the 2nd key-value pair
        node.insert(vec![2], value1.clone(), &mut storage, Vec::new());
        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.values.len(), 2);
        assert!(node.is_leaf);

        // insert the 3rd key-value pair
        node.insert(vec![3], value1.clone(), &mut storage, Vec::new());
        assert_eq!(node.keys.len(), 3);
        assert_eq!(node.values.len(), 3);
        assert!(node.is_leaf);

        // insert the 4th key-value pair
        node.insert(vec![4], value1.clone(), &mut storage, Vec::new());
        assert_eq!(node.keys.len(), 4);
        assert_eq!(node.values.len(), 4);
        assert!(node.is_leaf);

        // Update the value of an existing key
        node.insert(vec![3], value2.clone(), &mut storage, Vec::new());
        assert_eq!(node.values[2], value2);

        // insert more key-value pairs
        node.insert(vec![5], value1.clone(), &mut storage, Vec::new());
        node.insert(vec![6], value1.clone(), &mut storage, Vec::new());
        node.insert(vec![7], value1.clone(), &mut storage, Vec::new());

        // Update the value of another existing key
        node.insert(vec![6], value2.clone(), &mut storage, Vec::new());
        assert!(node.find(&[6], &storage).unwrap().values.contains(&value2));
    }

    /// This test verifies the deletion of key-value pairs from a ProllyNode and ensures
    /// that the keys are sorted correctly and the node balances based on the chunk content.
    /// The test uses a HashMapNodeStorage to store the nodes.
    #[test]
    fn test_find() {
        let mut storage = InMemoryNodeStorage::<32>::new();

        let value_for_all = vec![100];

        // initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::init_root(vec![1], value_for_all.clone());

        // insert key-value pairs
        node.insert(vec![2], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![3], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![4], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![5], value_for_all.clone(), &mut storage, Vec::new());

        // Test finding existing keys
        assert!(node.find(&[1], &storage).is_some());
        assert!(node.find(&[2], &storage).is_some());
        assert!(node.find(&[3], &storage).is_some());
        assert!(node.find(&[4], &storage).is_some());
        assert!(node.find(&[5], &storage).is_some());

        // Test finding a non-existing key
        assert!(node.find(&[6], &storage).is_none());

        // insert more key-value pairs
        node.insert(vec![6], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![7], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![8], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![9], value_for_all.clone(), &mut storage, Vec::new());

        // Test finding existing keys again after more insertions
        assert!(node.find(&[6], &storage).is_some());
        assert!(node.find(&[7], &storage).is_some());
        assert!(node.find(&[8], &storage).is_some());
        assert!(node.find(&[9], &storage).is_some());

        // Test finding a non-existing key
        assert!(node.find(&[10], &storage).is_none());
    }

    /// This test verifies the deletion of key-value pairs from a ProllyNode and ensures
    /// that the keys are sorted correctly and the node balances based on the chunk content.
    /// The test uses a HashMapNodeStorage to store the nodes.
    /// The test also checks the tree structure by traversing the tree in a breadth-first manner.
    #[test]
    #[ignore]
    fn test_delete() {
        let mut storage = InMemoryNodeStorage::<32>::new();
        let value_for_all = vec![100];

        let mut node: ProllyNode<32> = ProllyNode::builder().build();

        assert_eq!(node.traverse(&storage), "[L0:[]]");

        // insert key-value pairs
        for i in 1..=10 {
            node.insert(vec![i], value_for_all.clone(), &mut storage, Vec::new());
        }

        assert_eq!(
            node.traverse(&storage),
            "[L0:[[1], [2], [3], [4], [5], [6]]][L0:[[7], [8], [9], [10]]]"
        );

        // Test deleting existing keys
        assert!(node.delete(&[1], &mut storage, Vec::new()));
        assert!(node.find(&[1], &storage).is_none());
        assert!(node.delete(&[2], &mut storage, Vec::new()));
        assert!(node.find(&[2], &storage).is_none());
        assert!(node.delete(&[3], &mut storage, Vec::new()));
        assert!(node.find(&[3], &storage).is_none());
        assert!(node.delete(&[4], &mut storage, Vec::new()));
        assert!(node.find(&[4], &storage).is_none());
        assert!(node.delete(&[5], &mut storage, Vec::new()));
        assert!(node.find(&[5], &storage).is_none());

        assert_eq!(node.traverse(&storage), "[L0:[[6], [7], [8], [9], [10]]]");

        // Test deleting a non-existing key
        assert!(node.delete(&[6], &mut storage, Vec::new()));

        assert_eq!(node.traverse(&storage), "[L0:[[7], [8], [9]]][L0:[[10]]]");

        // Insert more key-value pairs and delete them to verify tree consistency
        node.insert(vec![7], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![8], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![9], value_for_all.clone(), &mut storage, Vec::new());

        assert!(node.delete(&[7], &mut storage, Vec::new()));
        assert!(node.find(&[7], &storage).is_none());
        assert!(node.delete(&[8], &mut storage, Vec::new()));
        assert!(node.find(&[8], &storage).is_none());
        assert!(node.delete(&[9], &mut storage, Vec::new()));
        assert!(node.find(&[9], &storage).is_none());

        assert_eq!(node.traverse(&storage), "[L0:[[10]]]");

        assert!(node.delete(&[10], &mut storage, Vec::new()));
        assert!(node.find(&[10], &storage).is_none());

        assert_eq!(node.traverse(&storage), "[L0:[]]");

        node.insert(vec![12], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![17], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![20], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![38], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![32], value_for_all.clone(), &mut storage, Vec::new());

        assert!(node.delete(&[12], &mut storage, Vec::new()));
        assert!(node.delete(&[38], &mut storage, Vec::new()));

        node.insert(vec![32], value_for_all.clone(), &mut storage, Vec::new());

        println!("{}", node.traverse(&storage));
    }

    #[test]
    fn test_chunk_content() {
        let mut storage = InMemoryNodeStorage::<32>::new();
        let value_for_all = vec![100];

        for size in 0..=20 {
            // Generate the keys vector using a loop
            let keys: Vec<Vec<u8>> = (1..=size).map(|i| vec![i]).collect();
            let values = vec![value_for_all.clone(); keys.len()];

            // Initialize the prolly tree with multiple key-value pairs using the builder
            let node: ProllyNode<32> = ProllyNode::builder().keys(keys).values(values).build();

            // Insert the node into storage
            storage.insert_node(node.get_hash(), node.clone());

            // Print chunk content
            println!("{:?}", node.chunk_content());
        }
    }

    #[test]
    fn test_chunk_content2() {
        let mut storage = InMemoryNodeStorage::<32>::new();
        let value_for_all = vec![100];

        let keys: Vec<Vec<u8>> =  vec![vec![29], vec![30], vec![31], vec![32], vec![33], vec![35]];
        let values = vec![value_for_all.clone(); keys.len()];

        // Initialize the prolly tree with multiple key-value pairs using the builder
        let node: ProllyNode<32> = ProllyNode::builder().keys(keys).values(values).build();

        // Insert the node into storage
        storage.insert_node(node.get_hash(), node.clone());

        // Print chunk content
        println!("{:?}", node.chunk_content());
    }

    #[test]
    fn test_history_independence_random() {
        let value = vec![100];
        let element_count = 15;

        // Generate different sequences of insertions
        // seq1. Insert elements in increasing order
        let sequence1 = (1..=element_count).collect::<Vec<_>>();
        // seq2. Insert elements in decreasing order
        let sequence2 = (1..=element_count).rev().collect::<Vec<_>>();
        // seq3. Insert elements in alternating order, e.g., (1, 3, 5, 7, 2, 4, 6, 8)
        let sequence3 = (1..=element_count)
            .step_by(2)
            .chain((2..=element_count).step_by(2))
            .collect::<Vec<_>>();
        // seq4. Insert elements in random order
        let mut sequence4 = (1..=element_count).collect::<Vec<_>>();
        sequence4.shuffle(&mut rand::thread_rng());

        let sequences = vec![sequence1, sequence2, sequence3, sequence4];

        let mut trees = Vec::new();

        for sequence in sequences {
            let mut storage = InMemoryNodeStorage::<32>::new();
            let mut node: ProllyNode<32> = ProllyNode::builder().build();

            for key in sequence {
                node.insert(vec![key as u8], value.clone(), &mut storage, Vec::new());
            }

            trees.push(node.traverse(&storage));
        }

        // Assert that all tree traversals are the same
        for i in 1..trees.len() {
            assert_eq!(
                trees[0],
                trees[i],
                "History independence failed for sequences: {} and {}",
                0,
                i + 1
            );
        }
    }
}
