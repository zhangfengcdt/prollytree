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

use crate::config::TreeConfig;
use crate::digest::ValueDigest;
use crate::encoding::EncodingType;
use crate::proof::Proof;
use crate::storage::NodeStorage;
use schemars::schema::RootSchema;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use twox_hash::XxHash64;

/// initial (leaf) level from which the prolly tree is built
const INIT_LEVEL: u8 = 0;
/// seed for the hash function
const HASH_SEED: u64 = 0;

// Chunking defaults (base, modulus, min_chunk_size, max_chunk_size, pattern) live
// on `TreeConfig::default()` in `crate::config`. `ProllyNode::default()` and
// `ProllyNodeBuilder::default()` read from there so there is one source of truth.

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

    /// Inserts multiple key-value pairs into the node in an optimized way.
    ///
    /// # Parameters
    /// - `keys`: The keys to insert.
    /// - `values`: The values associated with the keys.
    /// - `storage`: The storage to use for persisting nodes.
    /// - `parent_hash`: An optional hash of the parent node.
    fn insert_batch<S: NodeStorage<N>>(
        &mut self,
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
        storage: &mut S,
        path_hashes: Vec<ValueDigest<N>>,
    ) {
        for (key, value) in keys.iter().zip(values) {
            self.insert(key.clone(), value.clone(), storage, path_hashes.clone());
        }
    }

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

    /// Deletes multiple key-value pairs from the node.
    ///
    /// # Parameters
    /// - `keys`: The keys to delete.
    /// - `storage`: The storage to use for persisting nodes.
    /// - `parent_hash`: An optional hash of the parent node.
    fn delete_batch<S: NodeStorage<N>>(
        &mut self,
        keys: &[Vec<u8>],
        storage: &mut S,
        path_hashes: Vec<ValueDigest<N>>,
    ) {
        for key in keys {
            self.delete(key, storage, path_hashes.clone());
        }
    }

    /// Finds a key-value pair in the node.
    ///
    /// # Parameters
    /// - `key`: The key to find.
    /// - `storage`: The storage to use for persisting nodes.
    ///
    /// # Returns
    /// - `Some(ProllyNode<N>)` if the key was found, `None` otherwise.
    fn find<S: NodeStorage<N>>(&self, key: &[u8], storage: &S) -> Option<ProllyNode<N>>;

    /// Traverses the prolly tree and prints it in a directory-like structure.
    /// Each key in a node is printed on the same line.
    ///
    /// # Arguments
    ///
    /// * `storage` - A reference to the node storage containing the prolly tree nodes.
    fn print_tree<S: NodeStorage<N>>(&self, storage: &S);

    /// Prints the tree structure with proof path highlighted for a given key.
    /// This method visualizes the cryptographic proof path through the tree structure.
    ///
    /// # Arguments
    ///
    /// * `storage` - A reference to the node storage containing the prolly tree nodes.
    /// * `proof` - The proof object containing the path hashes.
    /// * `target_key` - The key for which the proof was generated.
    fn print_tree_with_proof<S: NodeStorage<N>>(
        &self,
        storage: &S,
        proof: &Proof<N>,
        target_key: &[u8],
    );
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProllyNode<const N: usize> {
    pub keys: Vec<Vec<u8>>,
    pub key_schema: Option<RootSchema>,
    pub values: Vec<Vec<u8>>,
    pub value_schema: Option<RootSchema>,
    pub is_leaf: bool,
    pub level: u8,
    pub base: u64,
    pub modulus: u64,
    pub min_chunk_size: usize,
    pub max_chunk_size: usize,
    pub pattern: u64,
    #[serde(skip)]
    pub split: bool,
    #[serde(skip)]
    pub merged: bool,
    pub encode_types: Vec<EncodingType>,
    pub encode_values: Vec<Vec<u8>>,
}

impl<const N: usize> Default for ProllyNode<N> {
    fn default() -> Self {
        let cfg = TreeConfig::<N>::default();
        ProllyNode {
            keys: Vec::new(),
            key_schema: None,
            values: Vec::new(),
            value_schema: None,
            is_leaf: true,
            level: INIT_LEVEL,
            base: cfg.base,
            modulus: cfg.modulus,
            min_chunk_size: cfg.min_chunk_size,
            max_chunk_size: cfg.max_chunk_size,
            pattern: cfg.pattern,
            split: false,
            merged: false,
            encode_types: Vec::new(),
            encode_values: Vec::new(),
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
        let cfg = TreeConfig::<N>::default();
        ProllyNodeBuilder {
            keys: Vec::new(),
            values: Vec::new(),
            is_leaf: true,
            level: INIT_LEVEL,
            base: cfg.base,
            modulus: cfg.modulus,
            min_chunk_size: cfg.min_chunk_size,
            max_chunk_size: cfg.max_chunk_size,
            pattern: cfg.pattern,
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

/// Trait for balancing nodes in the tree.
/// This trait provides methods for splitting and merging nodes to maintain tree balance.
trait Balanced<const N: usize> {
    /// Balances the node by splitting or merging it as needed.
    fn balance<S: NodeStorage<N>>(
        &mut self,
        storage: &mut S,
        is_root_node: bool,
        path_hashes: &[ValueDigest<N>],
    );

    /// Gets the hash of the next sibling of the node.
    fn get_next_sibling_hash<S: NodeStorage<N>>(
        &self,
        storage: &S,
        path_hashes: &[ValueDigest<N>],
    ) -> Option<Vec<u8>>;

    /// Merges the node with its next sibling.
    fn merge_with_next_sibling(&mut self, next_sibling: &mut ProllyNode<N>);
}

impl<const N: usize> Balanced<N> for ProllyNode<N> {
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
            if let Some(arc_next) =
                storage.get_node_by_hash(&ValueDigest::raw_hash(&next_sibling_hash))
            {
                let mut next_sibling = Arc::unwrap_or_clone(arc_next);
                // Try to merge the current node with the next sibling
                self.merge_with_next_sibling(&mut next_sibling);
            }
        }

        // Use chunk_content to determine split points
        if self.keys.len() < self.min_chunk_size {
            return;
        }
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
                key_schema: self.key_schema.clone(),
                values: original_values[start..end].to_vec(),
                value_schema: self.value_schema.clone(),
                is_leaf: self.is_leaf,
                level: self.level,
                base: self.base,
                modulus: self.modulus,
                min_chunk_size: self.min_chunk_size,
                max_chunk_size: self.max_chunk_size,
                pattern: self.pattern,
                split: self.split,
                merged: self.merged,
                encode_types: self.encode_types.clone(),
                encode_values: self.encode_values.clone(),
            };
            let sibling_hash = sibling.get_hash();
            let _ = storage.insert_node(sibling_hash.clone(), sibling.clone());
            siblings.push((sibling, sibling_hash));
        }

        // If the current node is the only node in this level
        // we need to create a new root at the next level
        if is_root_node {
            // Save the current root node to storage and get its hash
            let original_root_hash = self.get_hash();
            let _ = storage.insert_node(original_root_hash.clone(), self.clone());

            // Create a new root node
            let new_root = ProllyNode {
                keys: siblings
                    .iter()
                    .map(|(sibling, _)| sibling.keys[0].clone())
                    .collect(),
                key_schema: self.key_schema.clone(),
                values: siblings
                    .iter()
                    .map(|(_, hash)| hash.as_bytes().to_vec())
                    .collect(),
                value_schema: self.value_schema.clone(),
                is_leaf: false,
                level: self.level + 1,
                base: self.base,
                modulus: self.modulus,
                min_chunk_size: self.min_chunk_size,
                max_chunk_size: self.max_chunk_size,
                pattern: self.pattern,
                split: self.split,
                merged: self.merged,
                encode_types: self.encode_types.clone(),
                encode_values: self.encode_values.clone(),
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
            let _ = storage.insert_node(current_node_hash.clone(), self.clone());
        }
    }

    fn get_next_sibling_hash<S: NodeStorage<N>>(
        &self,
        storage: &S,
        path_hashes: &[ValueDigest<N>],
    ) -> Option<Vec<u8>> {
        if let Some(last_parent_hash) = path_hashes.last() {
            // Retrieve the parent node using the parent hash
            if let Some(parent_node) = storage.get_node_by_hash(last_parent_hash) {
                if self.keys.is_empty() {
                    return None;
                }
                // Find the position of the next sibling using the condition
                let largest_key = &self.keys[self.keys.len() - 1];
                if let Some(pos) = parent_node.keys.iter().position(|k| k > largest_key) {
                    // Check if there is a next sibling
                    if pos < parent_node.values.len() {
                        // Return the next sibling's hash
                        return Some(parent_node.values[pos].clone());
                    } else {
                        // The current node is the last child of the parent
                        return None;
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

    pub fn formatted_traverse_3<F>(&self, storage: &impl NodeStorage<N>, formatter: F) -> String
    where
        F: Fn(&ProllyNode<N>, &str, bool) -> String,
    {
        fn traverse_node<const N: usize, S: NodeStorage<N>, F>(
            node: &ProllyNode<N>,
            storage: &S,
            formatter: &F,
            prefix: &str,
            is_last: bool,
            output: &mut String,
        ) where
            F: Fn(&ProllyNode<N>, &str, bool) -> String,
        {
            *output += &formatter(node, prefix, is_last);

            let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            let children = node.children(storage);
            for (i, child) in children.iter().enumerate() {
                traverse_node(
                    child,
                    storage,
                    formatter,
                    &new_prefix,
                    i == children.len() - 1,
                    output,
                );
            }
        }

        let mut output = String::new();
        traverse_node(self, storage, &formatter, "", true, &mut output);
        output
    }

    /// Build a canonical tree from a sorted sequence of (key, value) pairs
    /// using the batch chunker built on top of `chunk_content`.
    ///
    /// This is an alternate canonical builder kept as an independent
    /// oracle: the streaming chunker in `crate::streaming_chunker` is
    /// the production path used by `ProllyTree`, and the
    /// `matches_node_build_canonical_from_pairs` unit test compares the
    /// two builders' root hashes to catch regressions in either path.
    ///
    /// "Canonical" means the resulting tree depends only on the (key, value)
    /// set and the `TreeConfig`, never on the order operations were applied.
    /// The function chunks the sorted sequence with `chunk_content`, emits
    /// leaves at level 0, then repeats up the tree using each child's first
    /// key as the pivot and child hash as the value, until a single root
    /// node remains. Intermediate level nodes are persisted to `storage`
    /// so that the returned root's children are reachable.
    ///
    /// `pairs` MUST be sorted by key ascending and contain no duplicate keys.
    pub fn build_canonical_from_pairs<S: NodeStorage<N>>(
        pairs: Vec<(Vec<u8>, Vec<u8>)>,
        config: &TreeConfig<N>,
        storage: &mut S,
    ) -> ProllyNode<N> {
        let (keys, values): (Vec<Vec<u8>>, Vec<Vec<u8>>) = pairs.into_iter().unzip();
        let mut current = build_level(&keys, &values, true, INIT_LEVEL, config);

        let mut level = INIT_LEVEL;
        while current.len() > 1 {
            // Persist each child node before we promote pivots to the next level.
            for node in &current {
                let _ = storage.insert_node(node.get_hash(), node.clone());
            }
            level = level.saturating_add(1);

            let next_keys: Vec<Vec<u8>> = current
                .iter()
                .map(|n| n.keys.first().cloned().unwrap_or_default())
                .collect();
            let next_values: Vec<Vec<u8>> = current
                .iter()
                .map(|n| n.get_hash().as_bytes().to_vec())
                .collect();

            current = build_level(&next_keys, &next_values, false, level, config);
        }

        current
            .into_iter()
            .next()
            .unwrap_or_else(|| build_empty(config))
    }
}

/// Chunk a flat (keys, values) sequence with the config's chunker and emit
/// one `ProllyNode` per chunk. Used by `build_canonical_from_pairs`.
fn build_level<const N: usize>(
    keys: &[Vec<u8>],
    values: &[Vec<u8>],
    is_leaf: bool,
    level: u8,
    config: &TreeConfig<N>,
) -> Vec<ProllyNode<N>> {
    if keys.is_empty() {
        // Empty input: return a single empty node so the caller has something
        // to use as the root. Only happens when the tree itself is empty.
        return vec![build_empty(config)];
    }

    let mut buf = make_node(keys.to_vec(), values.to_vec(), is_leaf, level, config);

    let chunks = buf.chunk_content();
    if chunks.is_empty() || (chunks.len() == 1 && chunks[0] == (0, keys.len())) {
        // Either below min_chunk_size (chunks empty) or the entire sequence
        // is one chunk - return a single node holding everything.
        buf.split = false;
        return vec![buf];
    }

    chunks
        .into_iter()
        .map(|(start, end)| {
            make_node(
                keys[start..end].to_vec(),
                values[start..end].to_vec(),
                is_leaf,
                level,
                config,
            )
        })
        .collect()
}

fn make_node<const N: usize>(
    keys: Vec<Vec<u8>>,
    values: Vec<Vec<u8>>,
    is_leaf: bool,
    level: u8,
    config: &TreeConfig<N>,
) -> ProllyNode<N> {
    ProllyNode {
        keys,
        key_schema: config.key_schema.clone(),
        values,
        value_schema: config.value_schema.clone(),
        is_leaf,
        level,
        base: config.base,
        modulus: config.modulus,
        min_chunk_size: config.min_chunk_size,
        max_chunk_size: config.max_chunk_size,
        pattern: config.pattern,
        split: false,
        merged: false,
        encode_types: Vec::new(),
        encode_values: Vec::new(),
    }
}

fn build_empty<const N: usize>(config: &TreeConfig<N>) -> ProllyNode<N> {
    make_node(Vec::new(), Vec::new(), true, INIT_LEVEL, config)
}

impl<const N: usize> NodeChunk for ProllyNode<N> {
    fn chunk_content(&self) -> Vec<(usize, usize)> {
        if self.keys.len() < self.min_chunk_size {
            return Vec::new();
        }
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
        // set is root node based on parent hash
        let is_root_node = path_hashes.is_empty();

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

            if let Some(arc_child) = storage.get_node_by_hash(&ValueDigest::raw_hash(&child_hash)) {
                let mut child_node = Arc::unwrap_or_clone(arc_child);

                // Record the current node's hash in the path
                path_hashes.push(self.get_hash());

                // Insert the key-value pair into the child node retrieved from the storage
                child_node.insert(key.clone(), value.clone(), storage, path_hashes.clone());

                // Remove the current node's hash from the path
                path_hashes.pop();

                // Save the updated child node back to the storage
                let new_node_hash = child_node.get_hash().as_bytes().to_vec();
                let _ = storage.insert_node(child_node.get_hash(), child_node.clone());

                // Check if the child node has been merged into its parent's next sibling
                if child_node.merged {
                    // remove the next sibling from the parent node
                    if i + 1 < self.keys.len() {
                        self.keys.remove(i + 1);
                        self.values.remove(i + 1);
                    }
                }

                // Check if the child node has been split and needs to be updated in the current node
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
                println!("Child node not found: {child_hash:?}");
            }

            // Sort the keys and balance the node
            self.balance(storage, is_root_node, &path_hashes);
        }

        // Extra check / logic before returning
        // Check if the node is a non-leaf root node, and it has only one child
        // If so, merge the child node with the current node
        if !self.is_leaf && is_root_node && self.keys.len() == 1 && self.level > INIT_LEVEL + 1 {
            let child_hash = self.values[0].clone();
            if let Some(child_node) = storage.get_node_by_hash(&ValueDigest::raw_hash(&child_hash))
            {
                // Merge the child node with the current node
                self.keys.clone_from(&child_node.keys);
                self.values.clone_from(&child_node.values);
                self.is_leaf = child_node.is_leaf;
                self.level = child_node.level;
            }
        }
    }

    fn insert_batch<S: NodeStorage<N>>(
        &mut self,
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
        storage: &mut S,
        path_hashes: Vec<ValueDigest<N>>,
    ) {
        // Sort the keys and corresponding values
        let mut key_value_pairs: Vec<(Vec<u8>, Vec<u8>)> =
            keys.iter().cloned().zip(values.iter().cloned()).collect();
        key_value_pairs.sort_by(|a, b| a.0.cmp(&b.0));

        for (key, value) in key_value_pairs {
            self.insert(key, value, storage, path_hashes.clone());
        }
    }

    fn delete<S: NodeStorage<N>>(
        &mut self,
        key: &[u8],
        storage: &mut S,
        mut path_hashes: Vec<ValueDigest<N>>,
    ) -> bool {
        // set is root node based on parent hash
        let is_root_node = path_hashes.is_empty();

        if self.is_leaf {
            // If the node is a leaf, try to find and remove the key
            if let Some(pos) = self.keys.iter().position(|k| k == key) {
                self.keys.remove(pos);
                self.values.remove(pos);

                // Persist the current node after deletion
                let current_node_hash = self.get_hash();
                let _ = storage.insert_node(current_node_hash.clone(), self.clone());

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

            if let Some(arc_child) = storage.get_node_by_hash(&ValueDigest::raw_hash(&child_hash)) {
                let mut child_node = Arc::unwrap_or_clone(arc_child);

                // Record the current node's hash in the path
                path_hashes.push(self.get_hash());

                // Delete the key from the child node
                let is_deleted = child_node.delete(key, storage, path_hashes.clone());

                // Remove the current node's hash from the path
                path_hashes.pop();

                // If no key is deleted (e.g., key is not found), just return false
                if !is_deleted {
                    return false;
                }

                // Save the updated child node back to the storage
                let new_node_hash = child_node.get_hash().as_bytes().to_vec();
                let _ = storage.insert_node(child_node.get_hash(), child_node.clone());

                // Check if the child node has been merged into its parent's next sibling
                if child_node.merged {
                    // remove the next sibling from the parent node
                    if i + 1 < self.keys.len() {
                        self.keys.remove(i + 1);
                        self.values.remove(i + 1);
                    }
                }

                // Check if the child node has been split and needs to be updated in the current node
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

                true
            } else {
                // Handle the case when the child node is not found
                println!("Child node not found: {child_hash:?}");
                false
            }
        }
    }

    fn delete_batch<S: NodeStorage<N>>(
        &mut self,
        keys: &[Vec<u8>],
        storage: &mut S,
        path_hashes: Vec<ValueDigest<N>>,
    ) {
        // Sort the keys before deletion
        let mut sorted_keys = keys.to_vec();
        sorted_keys.sort();

        for key in sorted_keys {
            self.delete(&key, storage, path_hashes.clone());
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
            // The node is an internal (non-leaf) node, so find the child node to search the key.
            // After certain delete patterns an internal node can transiently end up with no
            // children to descend into; treat that as "key not present" rather than panicking
            // with an out-of-bounds index.
            if self.values.is_empty() {
                return None;
            }
            let i = self.keys.iter().rposition(|k| key >= &k[..]).unwrap_or(0);
            let i = i.min(self.values.len() - 1);

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

    fn print_tree<S: NodeStorage<N>>(&self, storage: &S) {
        println!("root:");
        let output = self.formatted_traverse_3(storage, |node, prefix, is_last| {
            let keys_str = node
                .keys
                .iter()
                .map(|key| {
                    key.iter()
                        .map(|byte| format!("{byte:0}"))
                        .collect::<Vec<String>>()
                        .join(" ")
                })
                .collect::<Vec<String>>()
                .join(", ");
            let hash =
                Self::initialize_rolling_hash(&self.keys, &self.values, self.base, self.modulus);
            if node.is_leaf {
                format!(
                    "{}{}[{}]\n",
                    prefix,
                    if is_last { "└── " } else { "├── " },
                    keys_str
                )
            } else {
                format!(
                    "{}{}#({:?})[{}]\n",
                    prefix,
                    if is_last { "└── " } else { "├── " },
                    hash,
                    keys_str
                )
            }
        });
        println!("{output}");
        println!("Note: #[keys] indicates internal node, [keys] indicates leaf node");
    }

    fn print_tree_with_proof<S: NodeStorage<N>>(
        &self,
        storage: &S,
        proof: &Proof<N>,
        target_key: &[u8],
    ) {
        let output = self.formatted_traverse_with_proof(
            storage,
            proof,
            target_key,
            |node, prefix, is_last, is_in_proof_path, node_hash| {
                let keys_str = node
                    .keys
                    .iter()
                    .map(|key| {
                        key.iter()
                            .map(|byte| format!("{byte:0}"))
                            .collect::<Vec<String>>()
                            .join(" ")
                    })
                    .collect::<Vec<String>>()
                    .join(", ");

                let branch_symbol = if is_last { "└── " } else { "├── " };
                let node_symbol = if node.is_leaf { "" } else { "*" };

                if is_in_proof_path {
                    // Color the proof path nodes in green with hash information
                    let hash_str = format!("{node_hash:x}")
                        .chars()
                        .take(16)
                        .collect::<String>();
                    format!(
                        "{prefix}\x1b[32m{branch_symbol}{node_symbol}[{keys_str}] (hash: {hash_str}...)\x1b[0m\n"
                    )
                } else {
                    // Regular formatting for non-proof nodes
                    format!("{prefix}{branch_symbol}{node_symbol}[{keys_str}]\n")
                }
            },
        );
        println!("{output}");
        println!("Note: \x1b[32mGreen nodes\x1b[0m are in the proof path, *[keys] indicates internal node, [keys] indicates leaf node");
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
    pub fn children(&self, storage: &impl NodeStorage<N>) -> Vec<Arc<ProllyNode<N>>> {
        let mut children = Vec::new();

        if !self.is_leaf {
            for child_hash in &self.values {
                if let Some(child_node) =
                    storage.get_node_by_hash(&ValueDigest::raw_hash(child_hash))
                {
                    children.push(child_node);
                } else {
                    println!("Child node not found")
                }
            }
        }

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
    ///
    /// # Returns
    /// A string representation of the tree nodes in a breadth-first order.
    pub fn formatted_traverse<F>(&self, storage: &impl NodeStorage<N>, formatter: F) -> String
    where
        F: Fn(&ProllyNode<N>) -> String,
    {
        let mut queue: std::collections::VecDeque<Arc<ProllyNode<N>>> =
            std::collections::VecDeque::new();
        queue.push_back(Arc::new(self.clone()));

        let mut output = String::new();

        while let Some(node) = queue.pop_front() {
            output += &formatter(&node);
            for child in node.children(storage) {
                queue.push_back(child);
            }
        }

        output
    }

    /// Traverse the tree in a depth-first manner with proof path highlighting.
    /// This method is similar to formatted_traverse_3 but includes proof path information.
    pub fn formatted_traverse_with_proof<F>(
        &self,
        storage: &impl NodeStorage<N>,
        proof: &Proof<N>,
        target_key: &[u8],
        formatter: F,
    ) -> String
    where
        F: Fn(&ProllyNode<N>, &str, bool, bool, ValueDigest<N>) -> String,
    {
        fn traverse_node<const N: usize, S: NodeStorage<N>, F>(
            node: &ProllyNode<N>,
            storage: &S,
            proof: &Proof<N>,
            _target_key: &[u8],
            formatter: &F,
            prefix: &str,
            is_last: bool,
            output: &mut String,
        ) where
            F: Fn(&ProllyNode<N>, &str, bool, bool, ValueDigest<N>) -> String,
        {
            let node_hash = node.get_hash();

            // Check if this node is in the proof path
            let is_in_proof_path = proof.path.contains(&node_hash);

            *output += &formatter(node, prefix, is_last, is_in_proof_path, node_hash);

            let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            let children = node.children(storage);

            for (i, child) in children.iter().enumerate() {
                let is_last_child = i == children.len() - 1;

                traverse_node(
                    child,
                    storage,
                    proof,
                    _target_key,
                    formatter,
                    &new_prefix,
                    is_last_child,
                    output,
                );
            }
        }

        let mut output = String::new();

        traverse_node(
            self,
            storage,
            proof,
            target_key,
            &formatter,
            "",
            true,
            &mut output,
        );

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryNodeStorage;
    use rand::prelude::StdRng;
    use rand::seq::SliceRandom;
    use rand::SeedableRng;

    /// This test verifies the insertion of key-value pairs into a ProllyNode and ensures
    /// that the keys are sorted correctly and the node splits based on the chunk content.
    /// The test also checks the tree structure by traversing the tree in a breadth-first manner.
    #[test]
    fn test_print_tree() {
        let mut storage = InMemoryNodeStorage::<32>::default();
        let value_for_all = vec![100];

        // initialize the prolly tree with multiple key-value pairs using the builder
        let mut node: ProllyNode<32> = ProllyNode::builder()
            .pattern(0b11)
            .min_chunk_size(2)
            .build();

        for i in 0..=100 {
            node.insert(vec![i], value_for_all.clone(), &mut storage, Vec::new());
            storage.insert_node(node.get_hash(), node.clone()).unwrap();
        }

        // Print the tree
        node.print_tree(&storage);
    }

    /// This test verifies the insertion of key-value pairs into a ProllyNode and ensures
    /// that the keys are sorted correctly and the node splits based on the chunk content.
    /// The test also checks the tree structure by traversing the tree in a breadth-first manner.
    /// The test uses a HashMapNodeStorage to store the nodes.
    #[test]
    fn test_insert_in_order() {
        let mut storage = InMemoryNodeStorage::<32>::default();

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
        assert_eq!(node.keys.len(), 7);
        assert_eq!(node.values.len(), 7);
        assert!(node.is_leaf);

        // insert more key-value pairs
        node.insert(vec![6], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![8], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![10], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![12], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![15], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![20], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![28], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![30], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![31], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![32], value_for_all.clone(), &mut storage, Vec::new());
        node.insert(vec![33], value_for_all.clone(), &mut storage, Vec::new());

        println!("{}", node.traverse(&storage));

        assert_eq!(
            node.traverse(&storage),
            "[L0:[[1], [2], [3], [4], [5], [6], [7], [8], [10], [12], [15], [20], [28], [30], [31], [32], [33]]]"
        );
    }

    #[test]
    fn test_insert_rev_order() {
        let mut storage = InMemoryNodeStorage::<32>::default();

        let value_for_all = vec![100];
        let max_key = 200;

        let mut storage_ref = InMemoryNodeStorage::<32>::default();

        // initialize a new root node with the first key-value pair
        let mut node_ref: ProllyNode<32> = ProllyNode::default();

        for i in 0..=max_key {
            node_ref.insert(vec![i], value_for_all.clone(), &mut storage_ref, Vec::new());
            storage
                .insert_node(node_ref.get_hash(), node_ref.clone())
                .unwrap();
        }
        println!("increasing order: {}", node_ref.traverse(&storage_ref));

        // initialize a new root node with the first key-value pair
        let mut node: ProllyNode<32> = ProllyNode::default();

        // each time an insert is done, the root node hash is updated
        for i in (0..=max_key).rev() {
            node.insert(vec![i], value_for_all.clone(), &mut storage, Vec::new());
            storage.insert_node(node.get_hash(), node.clone()).unwrap(); // save the updated root node hash to storage
                                                                         //println!("{}", node.traverse(&storage));
        }
        println!("decreasing order: {}", node.traverse(&storage));

        assert_eq!(node_ref.traverse(&storage_ref), node.traverse(&storage));
    }

    #[test]
    fn test_insert_alt_order() {
        let mut storage = InMemoryNodeStorage::<32>::default();
        let value_for_all = vec![100];
        let max_key = 200;

        // Initialize a new root node with the first key-value pair
        let mut node_ref: ProllyNode<32> = ProllyNode::default();

        // Insert elements in increasing order
        for i in 0..=max_key {
            node_ref.insert(vec![i], value_for_all.clone(), &mut storage, Vec::new());
            storage
                .insert_node(node_ref.get_hash(), node_ref.clone())
                .unwrap();
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
            storage.insert_node(node.get_hash(), node.clone()).unwrap(); // save the updated root node hash to storage
        }
        println!("alt order: {}", node.traverse(&storage));

        // Verify that both trees have the same structure
        assert_eq!(node_ref.traverse(&storage), node.traverse(&storage));
    }

    #[test]
    fn test_insert_rnd_order() {
        let mut storage = InMemoryNodeStorage::<32>::default();
        let value_for_all = vec![100];

        // Initialize a new root node
        let mut node_ref: ProllyNode<32> = ProllyNode::builder()
            .min_chunk_size(8)
            .pattern(0b1111111)
            .build();

        // Insert elements in increasing order
        for i in 1..=15 {
            node_ref.insert(vec![i], value_for_all.clone(), &mut storage, Vec::new());
            storage
                .insert_node(node_ref.get_hash(), node_ref.clone())
                .unwrap();
        }
        println!("inc order: {}", node_ref.traverse(&storage));

        // Initialize a new root node
        let mut node: ProllyNode<32> = ProllyNode::builder()
            .min_chunk_size(8)
            .pattern(0b1111111)
            .build();

        // Define custom order for keys
        let custom_keys = vec![
            3, 9, 15, 4, 11, 13, 1, 2, 12, 8, 6, 14, 10, 5, 7, // Add more keys as needed
        ];

        // Insert elements in custom order
        for key in custom_keys {
            node.insert(vec![key], value_for_all.clone(), &mut storage, Vec::new());
            storage.insert_node(node.get_hash(), node.clone()).unwrap(); // save the updated root node hash to storage
        }
        println!("ctm order: {}", node.traverse(&storage));

        // Verify that both trees have the same structure
        assert_eq!(node_ref.traverse(&storage), node.traverse(&storage));
    }

    /// This test verifies the history independence property of the ProllyNode data structure.
    /// The test generates different sequences of insertions and ensures that the resulting trees
    /// are the same regardless of the insertion order.
    #[test]
    fn test_history_independence() {
        let value = vec![100];
        let element_count = 100;

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
        let seed = [0u8; 32]; // fixed seed for deterministic behavior
        let mut rng = StdRng::from_seed(seed);
        sequence4.shuffle(&mut rng);

        let sequences = vec![sequence1, sequence2, sequence3, sequence4];

        let mut trees = Vec::new();

        for sequence in sequences {
            let mut storage = InMemoryNodeStorage::<32>::default();
            let mut node: ProllyNode<32> = ProllyNode::builder()
                .min_chunk_size(8)
                .pattern(0b1111111)
                .build();

            // print the sequence
            println!("Sequence: {:?}", sequence);

            for key in sequence {
                node.insert(vec![key as u8], value.clone(), &mut storage, Vec::new());
                storage.insert_node(node.get_hash(), node.clone()).unwrap();
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

    /// This test verifies the insertion and update of key-value pairs into a ProllyNode and ensures
    /// that the keys are sorted correctly and the node splits based on the chunk content.
    /// The test also checks the tree structure by traversing the tree in a breadth-first manner.
    /// The test uses a HashMapNodeStorage to store the nodes.
    #[test]
    fn test_insert_update() {
        let mut storage = InMemoryNodeStorage::<32>::default();

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
        let mut storage = InMemoryNodeStorage::<32>::default();

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
    fn test_delete() {
        let mut storage = InMemoryNodeStorage::<32>::default();
        let value_for_all = vec![100];

        let mut node: ProllyNode<32> = ProllyNode::builder()
            .pattern(0b11)
            .min_chunk_size(2)
            .build();

        assert_eq!(node.traverse(&storage), "[L0:[]]");

        // insert key-value pairs
        for i in 1..=10 {
            node.insert(vec![i], value_for_all.clone(), &mut storage, Vec::new());
            storage.insert_node(node.get_hash(), node.clone()).unwrap();
        }

        assert_eq!(
            node.traverse(&storage),
            "[L0:[[1], [2], [3], [4], [5], [6]]][L0:[[7], [8], [9], [10]]]"
        );

        println!("{}", node.traverse(&storage));

        // Test deleting existing keys
        assert!(node.delete(&[1], &mut storage, Vec::new()));
        storage.insert_node(node.get_hash(), node.clone()).unwrap();
        assert!(node.find(&[1], &storage).is_none());

        assert!(node.delete(&[2], &mut storage, Vec::new()));
        storage.insert_node(node.get_hash(), node.clone()).unwrap();
        assert!(node.find(&[2], &storage).is_none());

        assert!(node.delete(&[3], &mut storage, Vec::new()));
        storage.insert_node(node.get_hash(), node.clone()).unwrap();
        assert!(node.find(&[3], &storage).is_none());

        assert!(node.delete(&[4], &mut storage, Vec::new()));
        storage.insert_node(node.get_hash(), node.clone()).unwrap();
        assert!(node.find(&[4], &storage).is_none());

        assert!(node.delete(&[5], &mut storage, Vec::new()));
        storage.insert_node(node.get_hash(), node.clone()).unwrap();
        assert!(node.find(&[5], &storage).is_none());

        assert_eq!(node.traverse(&storage), "[L0:[[6], [7], [8], [9], [10]]]");

        // Test deleting a non-existing key
        assert!(node.delete(&[6], &mut storage, Vec::new()));
        assert_eq!(node.traverse(&storage), "[L0:[[7], [8], [9], [10]]]");

        // Insert more key-value pairs and delete them to verify tree consistency
        node.insert(vec![7], value_for_all.clone(), &mut storage, Vec::new());
        storage.insert_node(node.get_hash(), node.clone()).unwrap();
        node.insert(vec![8], value_for_all.clone(), &mut storage, Vec::new());
        storage.insert_node(node.get_hash(), node.clone()).unwrap();
        node.insert(vec![9], value_for_all.clone(), &mut storage, Vec::new());
        storage.insert_node(node.get_hash(), node.clone()).unwrap();

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

        assert_eq!(node.traverse(&storage), "[L0:[[17], [20], [32]]]");
    }

    #[test]
    fn test_chunk_content() {
        let mut storage = InMemoryNodeStorage::<32>::default();
        let value_for_all = vec![100];

        for size in 0..=20 {
            // Generate the keys vector using a loop
            let keys: Vec<Vec<u8>> = (1..=size).map(|i| vec![i]).collect();
            let values = vec![value_for_all.clone(); keys.len()];

            // Initialize the prolly tree with multiple key-value pairs using the builder
            let node: ProllyNode<32> = ProllyNode::builder().keys(keys).values(values).build();

            // Insert the node into storage
            storage.insert_node(node.get_hash(), node.clone()).unwrap();

            // Print chunk content
            println!("{:?}", node.chunk_content());
        }
    }

    #[test]
    fn test_chunk_content_rnd() {
        let mut storage = InMemoryNodeStorage::<32>::default();
        let value_for_all = vec![100];

        let keys: Vec<Vec<u8>> = vec![vec![17], vec![30], vec![32]];
        let values = vec![value_for_all.clone(); keys.len()];

        // initialize the prolly tree with multiple key-value pairs using the builder
        let node: ProllyNode<32> = ProllyNode::builder()
            .keys(keys)
            .values(values)
            .pattern(0b11)
            .min_chunk_size(2)
            .build();

        // Insert the node into storage
        storage.insert_node(node.get_hash(), node.clone()).unwrap();

        // Print chunk content
        println!("{:?}", node.chunk_content());
    }

    /// This test verifies the balancing of the tree after multiple insertions.
    /// The test checks the tree structure and ensures that the root node is split correctly
    /// and the keys are promoted to the parent node.
    #[test]
    fn test_balance_after_insertions() {
        let mut storage = InMemoryNodeStorage::<32>::default();
        let value_for_all = vec![100];

        // Initialize the prolly tree with a small chunk size to trigger splits
        let mut node: ProllyNode<32> = ProllyNode::builder()
            .pattern(0b1)
            .min_chunk_size(4)
            .max_chunk_size(8)
            .build();

        // Insert key-value pairs to trigger a split
        for i in 0..=10 {
            node.insert(vec![i], value_for_all.clone(), &mut storage, Vec::new());
            storage.insert_node(node.get_hash(), node.clone()).unwrap();
        }

        // After 11 insertions, the root should not be a leaf node
        assert!(!node.is_leaf);

        // Check that all keys can be found
        for i in 0..=10 {
            assert!(node.find(&[i], &storage).is_some());
        }

        // Insert one more key to trigger another split
        node.insert(vec![11], value_for_all.clone(), &mut storage, Vec::new());
        storage.insert_node(node.get_hash(), node.clone()).unwrap();

        // Check that all keys can still be found
        for i in 0..=11 {
            assert!(node.find(&[i], &storage).is_some());
        }
    }

    #[test]
    fn test_flags_reset_after_operations() {
        // Test that split/merged flags are reset after insert/delete operations
        let mut storage = InMemoryNodeStorage::<32>::default();
        let mut node: ProllyNode<32> = ProllyNode::builder()
            .pattern(0b1)
            .min_chunk_size(2)
            .max_chunk_size(4)
            .build();

        // Insert enough items to trigger splits
        for i in 0..6 {
            node.insert(vec![i], vec![i], &mut storage, Vec::new());
            storage.insert_node(node.get_hash(), node.clone()).unwrap();
            // Flags should be reset after each operation
            assert!(
                !node.split,
                "Split flag should be reset after insert operation {}",
                i
            );
            assert!(
                !node.merged,
                "Merged flag should be reset after insert operation {}",
                i
            );
        }

        // Test deletion as well
        assert!(node.delete(&[0], &mut storage, Vec::new()));
        assert!(
            !node.split,
            "Split flag should be reset after delete operation"
        );
        assert!(
            !node.merged,
            "Merged flag should be reset after delete operation"
        );
    }

    #[test]
    fn test_flags_not_serialized() {
        // Test that split/merged flags are not serialized
        let mut node = ProllyNode::<32>::default();
        node.split = true;
        node.merged = true;
        let bytes = crate::serde_bincode::serialize(&node).unwrap();
        let de: ProllyNode<32> = crate::serde_bincode::deserialize(&bytes).unwrap();
        assert!(
            !de.split && !de.merged,
            "Split/merged flags should not be serialized"
        );
    }

    #[test]
    fn test_print_tree_with_proof() {
        use crate::config::TreeConfig;
        use crate::tree::{ProllyTree, Tree};

        // Test the new print_tree_with_proof functionality using ProllyTree
        let storage = InMemoryNodeStorage::<32>::default();
        let config = TreeConfig {
            base: 131,
            modulus: 1_000_000_009,
            min_chunk_size: 2,
            max_chunk_size: 8,
            pattern: 0b11,
            root_hash: None,
            key_schema: None,
            value_schema: None,
            encode_types: vec![],
        };

        let mut tree = ProllyTree::new(storage, config);

        // Insert some test data
        for i in 0..10 {
            tree.insert(vec![i], vec![i * 10]);
        }

        // Test proof visualization for an existing key
        let test_key = vec![5];

        println!("Testing print_proof for key {:?}:", test_key);
        let is_valid = tree.print_proof(&test_key);

        // The proof should be valid for an existing key
        assert!(is_valid, "Proof should be valid for existing key");
    }

    // ------------------------------------------------------------------
    // History-independence tests at the ProllyNode layer.
    //
    // History independence at the public `ProllyTree` API is guaranteed
    // by the streaming-chunker pipeline in `crate::streaming_chunker`,
    // which `ProllyTree::apply_changes` drives on every mutation. The
    // integration tests in `tests/history_independence.rs` cover the
    // full matrix at that layer.
    //
    // The tests below exercise `ProllyNode` directly, bypassing the
    // streaming chunker. The legacy in-place `Balanced::balance` path
    // still has insertion-order-dependent behavior (chunker emits
    // different leaf boundaries for the same final keys under
    // non-default configs; deletes can leave empty trailing leaves;
    // internal node structure varies). The `#[ignore]`d tests here
    // document those gaps - run with `--include-ignored` to surface
    // them. They serve as the acceptance suite for a future rewrite
    // of `ProllyNode`'s primitive mutation API to route through the
    // streaming chunker.
    // ------------------------------------------------------------------

    /// Build the four insertion orders used by the node-level tests for a
    /// sequence of keys 0..n: ascending, descending, alternating-odd-then-even,
    /// and a deterministic shuffle.
    fn order_variants(n: u64) -> Vec<(&'static str, Vec<u64>)> {
        let asc: Vec<u64> = (0..n).collect();
        let desc: Vec<u64> = (0..n).rev().collect();
        let alt: Vec<u64> = (0..n).step_by(2).chain((1..n).step_by(2)).collect();
        let mut shuf: Vec<u64> = (0..n).collect();
        let mut rng = StdRng::from_seed([0u8; 32]);
        shuf.shuffle(&mut rng);
        vec![
            ("ascending", asc),
            ("descending", desc),
            ("alt-odd-even", alt),
            ("shuffled(0)", shuf),
        ]
    }

    /// TreeConfig profiles that vary the chunker tuning knobs.
    /// Each entry is (label, base, modulus, min, max, pattern).
    fn config_variants() -> Vec<(&'static str, u64, u64, usize, usize, u64)> {
        vec![
            ("default", 257, 1_000_000_007, 8, 4096, 0b11111111),
            ("tiny-chunks", 257, 1_000_000_007, 2, 16, 0b11),
            ("medium-chunks", 257, 1_000_000_007, 4, 64, 0b1111),
            ("alt-hash", 131, 1_000_000_009, 8, 4096, 0b11111111),
        ]
    }

    fn k8(i: u64) -> Vec<u8> {
        i.to_be_bytes().to_vec()
    }

    fn v16(i: u64) -> Vec<u8> {
        let mut v = Vec::with_capacity(16);
        v.extend_from_slice(&i.to_be_bytes());
        v.extend_from_slice(&(!i).to_be_bytes());
        v
    }

    fn build_node(
        cfg: &(&'static str, u64, u64, usize, usize, u64),
        order: &[u64],
        value_fn: impl Fn(u64) -> Vec<u8>,
    ) -> ValueDigest<32> {
        let mut storage = InMemoryNodeStorage::<32>::default();
        let mut node: ProllyNode<32> = ProllyNode::builder()
            .base(cfg.1)
            .modulus(cfg.2)
            .min_chunk_size(cfg.3)
            .max_chunk_size(cfg.4)
            .pattern(cfg.5)
            .build();
        for &i in order {
            node.insert(k8(i), value_fn(i), &mut storage, Vec::new());
            storage.insert_node(node.get_hash(), node.clone()).unwrap();
        }
        node.get_hash()
    }

    fn make_node(cfg: &(&'static str, u64, u64, usize, usize, u64)) -> ProllyNode<32> {
        ProllyNode::builder()
            .base(cfg.1)
            .modulus(cfg.2)
            .min_chunk_size(cfg.3)
            .max_chunk_size(cfg.4)
            .pattern(cfg.5)
            .build()
    }

    fn traversal_of(
        cfg: &(&'static str, u64, u64, usize, usize, u64),
        order: &[u64],
        value_fn: impl Fn(u64) -> Vec<u8>,
    ) -> String {
        let mut storage = InMemoryNodeStorage::<32>::default();
        let mut node = make_node(cfg);
        for &i in order {
            node.insert(k8(i), value_fn(i), &mut storage, Vec::new());
            storage.insert_node(node.get_hash(), node.clone()).unwrap();
        }
        node.traverse(&storage)
    }

    // ---- Always-on baseline (passes today) ----

    /// Pure inserts under the default TreeConfig converge at the leaf level
    /// (i.e. the `traverse()` string is identical across insertion orders)
    /// for 8-byte u64 keys at N=256. This is the same flavor of guarantee
    /// the legacy `test_history_independence` exercises with 1-byte keys
    /// and N=100.
    ///
    /// Note: this does NOT assert root-hash equality - see the `#[ignore]`d
    /// `*_root_hash_*` tests for the stronger guarantee. Today the chunker
    /// can build different internal-node structure for the same final keys
    /// even when the leaves agree, so the Merkle root is not yet a
    /// canonical fingerprint of the (key, value) set.
    #[test]
    fn test_history_independence_default_config_traversal() {
        const N: u64 = 256;
        let default_cfg = config_variants()[0];
        let orders = order_variants(N);
        let baseline_label = orders[0].0;
        let baseline_trav = traversal_of(&default_cfg, &orders[0].1, v16);
        for (label, order) in orders.iter().skip(1) {
            let t = traversal_of(&default_cfg, order, v16);
            assert_eq!(
                t, baseline_trav,
                "default config: order={} leaf content diverged from order={}",
                label, baseline_label
            );
        }
    }

    // ---- #[ignore]d: tests that currently expose chunker/balance bugs ----

    #[test]
    #[ignore = "chunker emits different leaf boundaries for the same final keys under non-default configs (e.g. tiny-chunks); see module header"]
    fn test_history_independence_traversal_varied_configs() {
        const N: u64 = 256;
        let orders = order_variants(N);
        for cfg in config_variants() {
            let traversals: Vec<(String, String)> = orders
                .iter()
                .map(|(label, order)| ((*label).to_string(), traversal_of(&cfg, order, v16)))
                .collect();
            let baseline = &traversals[0];
            for (label, t) in traversals.iter().skip(1) {
                assert_eq!(
                    t, &baseline.1,
                    "config={} order={} diverged from order={} at the leaf level",
                    cfg.0, label, baseline.0
                );
            }
        }
    }

    #[test]
    #[ignore = "overwrites do not always converge to the same leaf layout as a fresh tree built from the final values; see module header"]
    fn test_history_independence_traversal_after_updates() {
        // For each config: insert all keys with a placeholder, then overwrite
        // with the real value in two different orders. Leaf-level content must
        // match a baseline built from the final values in ascending order.
        const N: u64 = 256;
        for cfg in config_variants() {
            let asc: Vec<u64> = (0..N).collect();
            let baseline = traversal_of(&cfg, &asc, v16);

            let placeholder = |_i: u64| vec![0u8];

            // Variant A: ascending placeholders, then descending overwrites.
            let mut storage_a = InMemoryNodeStorage::<32>::default();
            let mut node_a = make_node(&cfg);
            for &i in &asc {
                node_a.insert(k8(i), placeholder(i), &mut storage_a, Vec::new());
            }
            for i in (0..N).rev() {
                node_a.insert(k8(i), v16(i), &mut storage_a, Vec::new());
            }
            let trav_a = node_a.traverse(&storage_a);

            // Variant B: shuffled placeholders, then shuffled (different seed) overwrites.
            let mut order1: Vec<u64> = (0..N).collect();
            order1.shuffle(&mut StdRng::from_seed([1u8; 32]));
            let mut order2: Vec<u64> = (0..N).collect();
            order2.shuffle(&mut StdRng::from_seed([2u8; 32]));
            let mut storage_b = InMemoryNodeStorage::<32>::default();
            let mut node_b = make_node(&cfg);
            for &i in &order1 {
                node_b.insert(k8(i), placeholder(i), &mut storage_b, Vec::new());
            }
            for &i in &order2 {
                node_b.insert(k8(i), v16(i), &mut storage_b, Vec::new());
            }
            let trav_b = node_b.traverse(&storage_b);

            assert_eq!(
                trav_a, baseline,
                "config={} traversal after asc/desc update diverged from baseline",
                cfg.0
            );
            assert_eq!(
                trav_b, baseline,
                "config={} traversal after shuf/shuf update diverged from baseline",
                cfg.0
            );
        }
    }

    #[test]
    #[ignore = "deletes can leave empty trailing leaves so the traversal does not match a tree that only ever held the survivors; see module header"]
    fn test_history_independence_traversal_after_deletes() {
        // Insert SURVIVE+EXTRA keys then delete the EXTRA. Leaf-level
        // content must match a baseline that only ever held the survivors.
        const SURVIVE: u64 = 256;
        const EXTRA: u64 = 256;
        for cfg in config_variants() {
            let survivors: Vec<u64> = (0..SURVIVE).collect();
            let extras: Vec<u64> = (SURVIVE..SURVIVE + EXTRA).collect();
            let baseline = traversal_of(&cfg, &survivors, v16);

            // Variant A: asc inserts, asc deletes.
            let mut storage_a = InMemoryNodeStorage::<32>::default();
            let mut node_a = make_node(&cfg);
            for &i in survivors.iter().chain(extras.iter()) {
                node_a.insert(k8(i), v16(i), &mut storage_a, Vec::new());
            }
            for &i in &extras {
                node_a.delete(&k8(i), &mut storage_a, Vec::new());
            }
            let trav_a = node_a.traverse(&storage_a);

            // Variant B: shuffled inserts (seed=3), shuffled deletes (seed=4).
            let mut all: Vec<u64> = (0..SURVIVE + EXTRA).collect();
            all.shuffle(&mut StdRng::from_seed([3u8; 32]));
            let mut del_order = extras.clone();
            del_order.shuffle(&mut StdRng::from_seed([4u8; 32]));
            let mut storage_b = InMemoryNodeStorage::<32>::default();
            let mut node_b = make_node(&cfg);
            for &i in &all {
                node_b.insert(k8(i), v16(i), &mut storage_b, Vec::new());
            }
            for &i in &del_order {
                node_b.delete(&k8(i), &mut storage_b, Vec::new());
            }
            let trav_b = node_b.traverse(&storage_b);

            assert_eq!(
                trav_a, baseline,
                "config={} traversal after asc/asc delete diverged from baseline",
                cfg.0
            );
            assert_eq!(
                trav_b, baseline,
                "config={} traversal after shuf/shuf delete diverged from baseline",
                cfg.0
            );
        }
    }

    // ---- Tier 2: full structural convergence (currently broken) ----
    //
    // These tests assert the property a content-addressed prolly tree
    // *should* satisfy: that the Merkle root hash is identical across all
    // insertion orders. They currently fail because the chunker / balance
    // path can leave internal-node structure that depends on the order in
    // which inserts arrived (specifically the merge-with-next-sibling
    // step in `Balanced::balance` does not always re-chunk to a
    // canonical layout). Run with `--include-ignored` to surface them.

    #[test]
    #[ignore = "exposes internal-node structural divergence under different insertion orders; see comment above"]
    fn test_history_independence_root_hash_varied_configs() {
        const N: u64 = 256;
        let orders = order_variants(N);
        for cfg in config_variants() {
            let hashes: Vec<(String, ValueDigest<32>)> = orders
                .iter()
                .map(|(label, order)| ((*label).to_string(), build_node(&cfg, order, v16)))
                .collect();
            let baseline = &hashes[0];
            for (label, h) in hashes.iter().skip(1) {
                assert_eq!(
                    h, &baseline.1,
                    "config={} order={} root hash diverged from order={}",
                    cfg.0, label, baseline.0
                );
            }
        }
    }

    #[test]
    #[ignore = "exposes internal-node structural divergence under update-replay; see comment above"]
    fn test_history_independence_root_hash_after_updates() {
        const N: u64 = 256;
        for cfg in config_variants() {
            let asc: Vec<u64> = (0..N).collect();
            let baseline = build_node(&cfg, &asc, v16);

            let placeholder = |_i: u64| vec![0u8];
            let mut storage_a = InMemoryNodeStorage::<32>::default();
            let mut node_a = make_node(&cfg);
            for &i in &asc {
                node_a.insert(k8(i), placeholder(i), &mut storage_a, Vec::new());
            }
            for i in (0..N).rev() {
                node_a.insert(k8(i), v16(i), &mut storage_a, Vec::new());
            }
            assert_eq!(
                node_a.get_hash(),
                baseline,
                "config={} update-then-final (asc/desc) root hash diverged from baseline",
                cfg.0
            );
        }
    }

    #[test]
    #[ignore = "exposes internal-node structural divergence under delete-replay; see comment above"]
    fn test_history_independence_root_hash_after_deletes() {
        const SURVIVE: u64 = 256;
        const EXTRA: u64 = 256;
        for cfg in config_variants() {
            let survivors: Vec<u64> = (0..SURVIVE).collect();
            let extras: Vec<u64> = (SURVIVE..SURVIVE + EXTRA).collect();
            let baseline = build_node(&cfg, &survivors, v16);

            let mut storage = InMemoryNodeStorage::<32>::default();
            let mut node = make_node(&cfg);
            for &i in survivors.iter().chain(extras.iter()) {
                node.insert(k8(i), v16(i), &mut storage, Vec::new());
            }
            for &i in &extras {
                node.delete(&k8(i), &mut storage, Vec::new());
            }
            assert_eq!(
                node.get_hash(),
                baseline,
                "config={} delete-then-final root hash diverged from baseline",
                cfg.0
            );
        }
    }
}
