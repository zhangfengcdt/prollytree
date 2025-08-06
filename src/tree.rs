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

use crate::config::TreeConfig;
use crate::diff::{
    ConflictResolver, DiffResult, IgnoreConflictsResolver, MergeConflict, MergeResult,
};
use crate::digest::ValueDigest;
use crate::node::{Node, ProllyNode};
use crate::proof::Proof;
use crate::storage::NodeStorage;

/// Trait representing a Prolly tree with a fixed size N and a node storage S.
/// This trait provides methods for creating, modifying, and querying the tree.
pub trait Tree<const N: usize, S: NodeStorage<N>> {
    /// Creates a new Prolly tree with the specified root node and storage.
    ///
    /// # Parameters
    /// - `storage`: The storage to use for persisting nodes.
    /// - `config`: The configuration for the tree.
    ///
    /// # Returns
    /// - A new instance of the tree.
    fn new(storage: S, config: TreeConfig<N>) -> Self;

    /// Inserts a key-value pair into the tree.
    ///
    /// # Parameters
    /// - `key`: The key to insert.
    /// - `value`: The value associated with the key.
    fn insert(&mut self, key: Vec<u8>, value: Vec<u8>);

    /// Inserts multiple key-value pairs into the tree in an optimized way.
    ///
    /// # Parameters
    /// - `keys`: The keys to insert.
    /// - `values`: The values associated with the keys.
    fn insert_batch(&mut self, keys: &[Vec<u8>], values: &[Vec<u8>]);

    /// Updates the value associated with the specified key in the tree.
    ///
    /// # Parameters
    /// - `key`: The key to update.
    /// - `value`: The new value to associate with the key.
    ///
    /// # Returns
    /// - `true` if the key was found and updated, `false` otherwise.
    fn update(&mut self, key: Vec<u8>, value: Vec<u8>) -> bool;

    /// Deletes the key-value pair associated with the specified key from the tree.
    ///
    /// # Parameters
    /// - `key`: The key to delete.
    ///
    /// # Returns
    /// - `true` if the key was found and deleted, `false` otherwise.
    fn delete(&mut self, key: &[u8]) -> bool;

    /// Deletes multiple key-value pairs from the tree.
    ///
    /// # Parameters
    /// - `keys`: The keys to delete.
    fn delete_batch(&mut self, keys: &[Vec<u8>]);

    /// Finds the node associated with the specified key in the tree.
    ///
    /// # Parameters
    /// - `key`: The key to find.
    ///
    /// # Returns
    /// - `Some(ProllyNode<N>)` if the key was found, `None` otherwise.
    fn find(&self, key: &[u8]) -> Option<ProllyNode<N>>;

    /// Traverses the tree and returns a string representation of its structure.
    ///
    /// # Returns
    /// - A string representation of the tree structure.
    fn traverse(&self) -> String;

    /// Traverses the tree and returns a formatted string representation using the provided formatter function.
    ///
    /// # Parameters
    /// - `formatter`: A function to format each node.
    ///
    /// # Returns
    /// - A formatted string representation of the tree structure.
    fn formatted_traverse<F>(&self, formatter: F) -> String
    where
        F: Fn(&ProllyNode<N>) -> String;

    /// Gets the hash of the root node of the tree.
    ///
    /// # Returns
    /// - `Some(ValueDigest<N>)` if the root node exists, `None` otherwise.
    fn get_root_hash(&self) -> Option<ValueDigest<N>>;

    /// Gets the number of nodes in the tree.
    ///
    /// # Returns
    /// - The number of nodes in the tree.
    fn size(&self) -> usize;

    /// Gets the depth of the tree.
    ///
    /// # Returns
    /// - The depth of the tree.
    fn depth(&self) -> usize;

    /// Provides a summary of the tree structure and contents.
    ///
    /// # Returns
    /// - A summary of the tree.
    fn summary(&self) -> String;

    /// Provides various statistics about the tree.
    ///
    /// # Returns
    /// - A `TreeStats` object containing statistics about the tree.
    fn stats(&self) -> TreeStats;

    /// Loads the configuration for the tree from storage.
    ///
    /// # Parameters
    /// - `storage`: The storage to load the configuration from.
    fn load_config(storage: &S) -> Result<TreeConfig<N>, &'static str>;

    /// Saves the configuration for the tree to storage.
    ///
    /// # Returns
    /// - `Ok(())` if the configuration was saved successfully, `Err(&'static str)` otherwise.
    fn save_config(&self) -> Result<(), &'static str>;

    /// Generates a proof of existence for a given key in the tree.
    ///
    /// This function traverses the tree from the root to the target node containing the key,
    /// collecting the hashes of all nodes along the path. The proof can be used to verify the
    /// existence of the key and its associated value without revealing other data in the tree.
    ///
    /// # Arguments
    ///
    /// * `key` - The key for which to generate the proof.
    ///
    /// # Returns
    ///
    /// A `Proof` struct containing the path of hashes and the hash of the target node (if the key exists).
    fn generate_proof(&self, key: &[u8]) -> Proof<N>;

    fn verify(&self, proof: Proof<N>, key: &[u8], expected_value: Option<&[u8]>) -> bool;

    /// Computes the differences between two Prolly Trees.
    ///
    /// This function compares the current tree (`self`) with another tree (`other`)
    /// and identifies the differences between them. It traverses both trees and
    /// generates a list of changes, including added, removed, and modified key-value pairs.
    ///
    /// # Arguments
    ///
    /// * `other` - The other Prolly Tree to compare against.
    ///
    /// # Returns
    ///
    /// A vector of `DiffResult` containing the differences between the two trees.
    fn diff(&self, other: &Self) -> Vec<DiffResult>;

    /// Prints the tree structure to the console.
    /// This function is useful for debugging and visualizing the tree.
    /// It prints the tree structure in a human-readable format.
    /// The tree is printed in a depth-first manner, starting from the root node.
    /// Each node is printed with its keys and values, along with the hash of the node.
    ///
    fn print(&mut self);

    /// Prints the tree structure with the proof path highlighted for a given key.
    /// This function combines `generate_proof` and `print` to visualize the
    /// cryptographic proof path through the tree structure with color coding.
    ///
    /// # Arguments
    ///
    /// * `key` - The key for which to generate and display the proof path.
    ///
    /// # Returns
    ///
    /// A boolean indicating whether the proof is valid.
    fn print_proof(&self, key: &[u8]) -> bool;

    /// Performs a three-way merge between source, destination and base trees.
    ///
    /// This function implements a three-way merge algorithm for prolly trees.
    /// Given three tree root hashes (base, source, destination), it computes
    /// the differences between base->source and base->destination, then merges
    /// changes from source into destination, detecting conflicts when both
    /// branches modify the same key with different values.
    ///
    /// # Arguments
    ///
    /// * `source_root` - Root hash of the source (feature) tree
    /// * `destination_root` - Root hash of the destination (main) tree
    /// * `base_root` - Root hash of the common base tree
    ///
    /// # Returns
    ///
    /// A vector of `MergeResult` indicating the changes to apply and any conflicts
    fn merge(
        &self,
        source_root: &ValueDigest<N>,
        destination_root: &ValueDigest<N>,
        base_root: &ValueDigest<N>,
    ) -> Vec<MergeResult>;

    /// Applies merge results to create a new merged tree.
    ///
    /// This method takes the destination tree and applies a list of merge results
    /// to create a new tree with all the merged changes. If any conflicts are
    /// present in the merge results, this method will return an error.
    ///
    /// # Arguments
    ///
    /// * `destination_root` - Root hash of the destination tree to merge into
    /// * `merge_results` - List of merge operations to apply
    ///
    /// # Returns
    ///
    /// A new `ProllyTree` instance with merged changes, or an error if conflicts exist
    fn apply_merge_results(
        &self,
        destination_root: &ValueDigest<N>,
        merge_results: &[MergeResult],
    ) -> Result<Self, Vec<MergeConflict>>
    where
        Self: Sized;

    /// Convenience method to perform a three-way merge with conflict resolution.
    ///
    /// This method combines `merge()` and `apply_merge_results()` with a conflict resolver
    /// to provide a flexible interface for merging. Conflicts that can't be resolved
    /// are returned for manual resolution.
    ///
    /// # Arguments
    ///
    /// * `source_root` - Root hash of the source (feature) tree
    /// * `destination_root` - Root hash of the destination (main) tree
    /// * `base_root` - Root hash of the common base tree
    /// * `resolver` - Conflict resolver to handle merge conflicts
    ///
    /// # Returns
    ///
    /// Either a new merged tree or a list of unresolved conflicts
    fn merge_trees<R: ConflictResolver>(
        &self,
        source_root: &ValueDigest<N>,
        destination_root: &ValueDigest<N>,
        base_root: &ValueDigest<N>,
        resolver: &R,
    ) -> Result<Self, Vec<MergeConflict>>
    where
        Self: Sized;

    /// Convenience method for merge_trees with default IgnoreConflictsResolver
    fn merge_trees_ignore_conflicts(
        &self,
        source_root: &ValueDigest<N>,
        destination_root: &ValueDigest<N>,
        base_root: &ValueDigest<N>,
    ) -> Result<Self, Vec<MergeConflict>>
    where
        Self: Sized,
    {
        self.merge_trees(
            source_root,
            destination_root,
            base_root,
            &IgnoreConflictsResolver,
        )
    }
}

pub struct TreeStats {
    pub num_nodes: usize,
    pub num_leaves: usize,
    pub num_internal_nodes: usize,
    pub avg_node_size: f64,
    pub total_key_value_pairs: usize,
}

impl TreeStats {
    pub fn new() -> Self {
        TreeStats {
            num_nodes: 0,
            num_leaves: 0,
            num_internal_nodes: 0,
            avg_node_size: 0.0,
            total_key_value_pairs: 0,
        }
    }
}

impl Default for TreeStats {
    fn default() -> Self {
        TreeStats::new()
    }
}

#[derive(Debug)]
pub struct ProllyTree<const N: usize, S: NodeStorage<N>> {
    pub root: ProllyNode<N>,
    pub storage: S,
    pub config: TreeConfig<N>,
}

impl<const N: usize, S: NodeStorage<N>> Tree<N, S> for ProllyTree<N, S> {
    fn new(storage: S, config: TreeConfig<N>) -> Self {
        let root = ProllyNode {
            keys: Vec::new(),
            key_schema: config.key_schema.clone(),
            values: Vec::new(),
            value_schema: config.value_schema.clone(),
            is_leaf: true,
            level: 0,
            base: config.base,
            modulus: config.modulus,
            min_chunk_size: config.min_chunk_size,
            max_chunk_size: config.max_chunk_size,
            pattern: config.pattern,
            split: false,
            merged: false,
            encode_types: Vec::new(),
            encode_values: Vec::new(),
        };
        let root_hash = Some(root.get_hash());
        let mut tree = ProllyTree {
            root,
            storage,
            config,
        };
        tree.config.root_hash = root_hash;
        tree
    }
    fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) {
        // Root node does not have a parent hash
        self.root.insert(key, value, &mut self.storage, Vec::new());
        self.persist_root();
    }

    fn insert_batch(&mut self, keys: &[Vec<u8>], values: &[Vec<u8>]) {
        self.root
            .insert_batch(keys, values, &mut self.storage, Vec::new());
    }

    fn update(&mut self, key: Vec<u8>, value: Vec<u8>) -> bool {
        if self.find(&key).is_some() {
            self.insert(key, value);
            true
        } else {
            false
        }
    }

    fn delete(&mut self, key: &[u8]) -> bool {
        let deleted = self.root.delete(key, &mut self.storage, Vec::new());
        if deleted {
            self.persist_root();
        }
        deleted
    }

    fn delete_batch(&mut self, keys: &[Vec<u8>]) {
        self.root.delete_batch(keys, &mut self.storage, Vec::new());
    }

    fn find(&self, key: &[u8]) -> Option<ProllyNode<N>> {
        self.root.find(key, &self.storage)
    }

    fn traverse(&self) -> String {
        self.root.traverse(&self.storage)
    }

    fn formatted_traverse<F>(&self, formatter: F) -> String
    where
        F: Fn(&ProllyNode<N>) -> String,
    {
        self.root.formatted_traverse(&self.storage, formatter)
    }

    fn get_root_hash(&self) -> Option<ValueDigest<N>> {
        Option::from(self.root.get_hash())
    }

    fn size(&self) -> usize {
        fn count_pairs<const N: usize, S: NodeStorage<N>>(
            node: &ProllyNode<N>,
            storage: &S,
        ) -> usize {
            if node.is_leaf {
                node.keys.len()
            } else {
                let mut count = 0;
                for value in &node.values {
                    if let Some(child_node) =
                        storage.get_node_by_hash(&ValueDigest::raw_hash(value))
                    {
                        count += count_pairs(&child_node, storage);
                    }
                }
                count
            }
        }

        count_pairs(&self.root, &self.storage)
    }

    fn depth(&self) -> usize {
        (self.root.level as usize) + 1
    }

    fn summary(&self) -> String {
        let stats = self.stats();
        format!(
            "Tree Summary:\n- Number of Key-Value Pairs: {}\n- Number of Nodes: {}\n- Number of Leaves: {}\n- Number of Internal Nodes: {}\n- Average Leaf Node Size: {:.2}",
            self.size(),
            stats.num_nodes,
            stats.num_leaves,
            stats.num_internal_nodes,
            stats.avg_node_size
        )
    }

    fn stats(&self) -> TreeStats {
        fn collect_stats<const N: usize, S: NodeStorage<N>>(
            node: &ProllyNode<N>,
            storage: &S,
            stats: &mut TreeStats,
        ) {
            stats.num_nodes += 1;
            if node.is_leaf {
                stats.num_leaves += 1;
                stats.total_key_value_pairs += node.keys.len();
            } else {
                stats.num_internal_nodes += 1;
                for value in &node.values {
                    if let Some(child_node) =
                        storage.get_node_by_hash(&ValueDigest::raw_hash(value))
                    {
                        collect_stats(&child_node, storage, stats);
                    }
                }
            }
        }

        let mut stats = TreeStats::new();
        collect_stats(&self.root, &self.storage, &mut stats);
        if stats.num_leaves > 0 {
            stats.avg_node_size = stats.total_key_value_pairs as f64 / stats.num_leaves as f64;
        }
        stats
    }

    fn load_config(storage: &S) -> Result<TreeConfig<N>, &'static str> {
        // Implement the logic to load the configuration from storage
        // Here we assume the config is stored with a specific key "tree_config"
        if let Some(config_data) = storage.get_config("tree_config") {
            let config: TreeConfig<N> =
                serde_json::from_slice(&config_data).map_err(|_| "Failed to deserialize config")?;
            Ok(config)
        } else {
            Err("Config not found")
        }
    }

    fn save_config(&self) -> Result<(), &'static str> {
        let mut config = self.config.clone();
        config.root_hash = Option::from(self.root.get_hash());
        let config_data = serde_json::to_vec(&config).map_err(|_| "Failed to serialize config")?;
        self.storage.save_config("tree_config", &config_data);
        Ok(())
    }

    /// Generates a proof of existence for a given key in the tree.
    ///
    /// This function traverses the tree from the root to the target node containing the key,
    /// collecting the hashes of all nodes along the path. The proof can be used to verify the
    /// existence of the key and its associated value without revealing other data in the tree.
    ///
    /// # Arguments
    ///
    /// * `key` - The key for which to generate the proof.
    /// * `storage` - The storage implementation to retrieve child nodes.
    ///
    /// # Returns
    ///
    /// A `Proof` struct containing the path of hashes and the hash of the target node (if the key exists).
    fn generate_proof(&self, key: &[u8]) -> Proof<N> {
        /// Recursive helper function to generate the proof path.
        ///
        /// This function traverses the tree from the given node to the target node containing the key,
        /// collecting the hashes of all nodes along the path. It returns the hash of the target node
        /// if the key exists, or `None` if the key does not exist.
        ///
        /// # Arguments
        ///
        /// * `node` - The current node being traversed.
        /// * `key` - The key for which to generate the proof.
        /// * `storage` - The storage implementation to retrieve child nodes.
        /// * `path` - The vector to store the hashes of the nodes along the path.
        ///
        /// # Returns
        ///
        /// The hash of the target node if the key exists, or `None` if the key does not exist.
        fn generate_proof_recursive<const N: usize, S: NodeStorage<N>>(
            node: &ProllyNode<N>,
            key: &[u8],
            storage: &S,
            path: &mut Vec<ValueDigest<N>>,
        ) -> Option<ValueDigest<N>> {
            path.push(node.get_hash());

            if node.is_leaf {
                if node.keys.iter().any(|k| k == key) {
                    Some(node.get_hash())
                } else {
                    None
                }
            } else {
                let i = node.keys.iter().rposition(|k| key >= &k[..]).unwrap_or(0);
                let child_hash = node.values[i].clone();

                if let Some(child_node) =
                    storage.get_node_by_hash(&ValueDigest::raw_hash(&child_hash))
                {
                    generate_proof_recursive(&child_node, key, storage, path)
                } else {
                    None
                }
            }
        }

        let mut path = Vec::new();
        let target_hash = generate_proof_recursive(&self.root, key, &self.storage, &mut path);

        Proof { path, target_hash }
    }

    fn verify(&self, proof: Proof<N>, key: &[u8], expected_value: Option<&[u8]>) -> bool {
        // Start with the root hash
        let mut current_hash = self.root.get_hash();

        for (i, node_hash) in proof.path.iter().enumerate() {
            // Retrieve the node content from storage using the current hash
            if let Some(node) = self.storage.get_node_by_hash(&current_hash) {
                // Check if the current node's hash matches the expected hash in the path
                if node.get_hash() != *node_hash {
                    return false;
                }

                // If it's the last node in the path, verify the leaf node
                if i == proof.path.len() - 1 {
                    return if node.is_leaf {
                        node.keys.iter().any(|k| k == key)
                            && (expected_value.is_none()
                                || node
                                    .values
                                    .iter()
                                    .any(|v| expected_value.unwrap() == &v[..]))
                    } else {
                        false // Path should end at a leaf node
                    };
                }

                // Move to the next node in the path by finding the correct child
                let child_index = node.keys.iter().rposition(|k| key >= &k[..]).unwrap_or(0);
                current_hash = ValueDigest::raw_hash(&node.values[child_index]);
            } else {
                // If the node is not found in storage, the proof is invalid
                return false;
            }
        }

        false // If we exit the loop without verifying, the proof is invalid
    }

    fn diff(&self, other: &Self) -> Vec<DiffResult> {
        let mut diffs = Vec::new();
        self.diff_recursive(&self.root, &other.root, &mut diffs);
        diffs
    }

    fn print(&mut self) {
        self.root.print_tree(&self.storage);
    }

    fn print_proof(&self, key: &[u8]) -> bool {
        // Generate the proof for the given key
        let proof = self.generate_proof(key);

        // Verify the proof
        let is_valid = self.verify(proof.clone(), key, None);

        // Print the tree structure with proof path highlighted
        println!("root:");
        self.root.print_tree_with_proof(&self.storage, &proof, key);

        // Print proof information
        println!("\nProof for key {key:?} is valid: {is_valid}");
        println!("Proof: {proof:#?}");

        is_valid
    }

    fn merge(
        &self,
        source_root: &ValueDigest<N>,
        destination_root: &ValueDigest<N>,
        base_root: &ValueDigest<N>,
    ) -> Vec<MergeResult> {
        // Load trees from storage using the provided root hashes
        let source_tree = self.storage.get_node_by_hash(source_root);
        let destination_tree = self.storage.get_node_by_hash(destination_root);
        let base_tree = self.storage.get_node_by_hash(base_root);

        if source_tree.is_none() || destination_tree.is_none() || base_tree.is_none() {
            // If we can't load one of the trees, return an error as a conflict
            return vec![MergeResult::Conflict(MergeConflict {
                key: b"<merge_error>".to_vec(),
                base_value: None,
                source_value: None,
                destination_value: Some(b"Failed to load tree from storage".to_vec()),
            })];
        }

        let source_tree = source_tree.unwrap();
        let destination_tree = destination_tree.unwrap();
        let base_tree = base_tree.unwrap();

        // Compute diffs directly using the node-level diffing
        let mut base_to_source_diffs = Vec::new();
        let mut base_to_destination_diffs = Vec::new();

        self.diff_nodes_recursive(&base_tree, &source_tree, &mut base_to_source_diffs);
        self.diff_nodes_recursive(
            &base_tree,
            &destination_tree,
            &mut base_to_destination_diffs,
        );

        // Convert diffs to maps for easier processing
        let mut source_changes: std::collections::HashMap<Vec<u8>, DiffResult> =
            std::collections::HashMap::new();
        let mut destination_changes: std::collections::HashMap<Vec<u8>, DiffResult> =
            std::collections::HashMap::new();

        for diff in base_to_source_diffs {
            let key = match &diff {
                DiffResult::Added(k, _) => k.clone(),
                DiffResult::Removed(k, _) => k.clone(),
                DiffResult::Modified(k, _, _) => k.clone(),
            };
            source_changes.insert(key, diff);
        }

        for diff in base_to_destination_diffs {
            let key = match &diff {
                DiffResult::Added(k, _) => k.clone(),
                DiffResult::Removed(k, _) => k.clone(),
                DiffResult::Modified(k, _, _) => k.clone(),
            };
            destination_changes.insert(key, diff);
        }

        // Collect all keys that were changed in either branch
        let mut all_changed_keys = std::collections::HashSet::new();
        for key in source_changes.keys() {
            all_changed_keys.insert(key.clone());
        }
        for key in destination_changes.keys() {
            all_changed_keys.insert(key.clone());
        }

        let mut merge_results = Vec::new();

        // Process each changed key
        for key in all_changed_keys {
            let source_change = source_changes.get(&key);
            let destination_change = destination_changes.get(&key);

            match (source_change, destination_change) {
                // Only source changed - apply source change
                (Some(source_diff), None) => match source_diff {
                    DiffResult::Added(_, value) => {
                        merge_results.push(MergeResult::Added(key, value.clone()));
                    }
                    DiffResult::Removed(_, _) => {
                        merge_results.push(MergeResult::Removed(key));
                    }
                    DiffResult::Modified(_, _, new_value) => {
                        merge_results.push(MergeResult::Modified(key, new_value.clone()));
                    }
                },

                // Only destination changed - no action needed (destination already has the change)
                (None, Some(_)) => {
                    // Destination change already exists, no merge action needed
                }

                // Both changed - need to check for conflicts
                (Some(source_diff), Some(destination_diff)) => {
                    let conflict =
                        self.detect_conflict(&key, source_diff, destination_diff, &base_tree);

                    if let Some(conflict) = conflict {
                        merge_results.push(MergeResult::Conflict(conflict));
                    } else {
                        // No conflict, apply source change (assuming identical changes)
                        match source_diff {
                            DiffResult::Added(_, value) => {
                                merge_results.push(MergeResult::Added(key, value.clone()));
                            }
                            DiffResult::Removed(_, _) => {
                                merge_results.push(MergeResult::Removed(key));
                            }
                            DiffResult::Modified(_, _, new_value) => {
                                merge_results.push(MergeResult::Modified(key, new_value.clone()));
                            }
                        }
                    }
                }

                // Neither changed (shouldn't happen due to our key collection logic)
                (None, None) => {}
            }
        }

        merge_results
    }

    fn apply_merge_results(
        &self,
        destination_root: &ValueDigest<N>,
        merge_results: &[MergeResult],
    ) -> Result<Self, Vec<MergeConflict>> {
        // Check for conflicts first
        let mut conflicts = Vec::new();
        for result in merge_results {
            if let MergeResult::Conflict(conflict) = result {
                conflicts.push((*conflict).clone());
            }
        }

        if !conflicts.is_empty() {
            return Err(conflicts);
        }

        // Load the destination tree
        let destination_tree =
            self.storage
                .get_node_by_hash(destination_root)
                .ok_or_else(|| {
                    vec![MergeConflict {
                        key: b"<apply_error>".to_vec(),
                        base_value: None,
                        source_value: None,
                        destination_value: Some(b"Failed to load destination tree".to_vec()),
                    }]
                })?;

        // Create a new tree starting from the destination
        let mut new_tree = ProllyTree {
            root: destination_tree,
            storage: self.storage.clone(),
            config: self.config.clone(),
        };

        // Apply each merge result
        for result in merge_results {
            match result {
                MergeResult::Added(key, value) => {
                    new_tree.insert(key.clone(), value.clone());
                }
                MergeResult::Modified(key, value) => {
                    new_tree.insert(key.clone(), value.clone()); // insert overwrites existing
                }
                MergeResult::Removed(key) => {
                    new_tree.delete(key);
                }
                MergeResult::Conflict(_) => {
                    // This should not happen since we checked for conflicts above
                    unreachable!("Conflicts should have been filtered out");
                }
            }
        }

        Ok(new_tree)
    }

    fn merge_trees<R: ConflictResolver>(
        &self,
        source_root: &ValueDigest<N>,
        destination_root: &ValueDigest<N>,
        base_root: &ValueDigest<N>,
        resolver: &R,
    ) -> Result<Self, Vec<MergeConflict>> {
        let merge_results = self.merge(source_root, destination_root, base_root);

        // Separate conflicts from other merge results and try to resolve conflicts
        let mut resolved_results = Vec::new();
        let mut unresolved_conflicts = Vec::new();

        for result in merge_results {
            match result {
                MergeResult::Conflict(conflict) => {
                    if let Some(resolved_result) = resolver.resolve_conflict(&conflict) {
                        resolved_results.push(resolved_result);
                    } else {
                        unresolved_conflicts.push(conflict);
                    }
                }
                other => resolved_results.push(other),
            }
        }

        // If there are still unresolved conflicts, return them
        if !unresolved_conflicts.is_empty() {
            return Err(unresolved_conflicts);
        }

        // Apply the resolved results
        self.apply_merge_results(destination_root, &resolved_results)
    }
}

impl<const N: usize, S: NodeStorage<N>> ProllyTree<N, S> {
    /// Compute differences between two nodes recursively
    fn diff_nodes_recursive(
        &self,
        old_node: &ProllyNode<N>,
        new_node: &ProllyNode<N>,
        diffs: &mut Vec<DiffResult>,
    ) {
        self.diff_recursive(old_node, new_node, diffs);
    }

    /// Find a value for a specific key in a node tree
    fn find_value_in_node(&self, node: &ProllyNode<N>, key: &[u8]) -> Option<Vec<u8>> {
        // Search directly in the node first (for leaf nodes)
        if node.is_leaf {
            for (i, k) in node.keys.iter().enumerate() {
                if k.as_slice() == key {
                    return Some(node.values[i].clone());
                }
            }
        } else {
            // For internal nodes, use the node's find method with storage
            return node.find(key, &self.storage).and_then(|found_node| {
                found_node
                    .keys
                    .iter()
                    .zip(found_node.values.iter())
                    .find(|(k, _)| k.as_slice() == key)
                    .map(|(_, v)| v.clone())
            });
        }
        None
    }

    /// Detects conflicts between changes from source and destination branches
    fn detect_conflict(
        &self,
        key: &[u8],
        source_diff: &DiffResult,
        destination_diff: &DiffResult,
        base_node: &ProllyNode<N>,
    ) -> Option<MergeConflict> {
        // Get the base value for this key from the base node
        let base_value = self.find_value_in_node(base_node, key);

        match (source_diff, destination_diff) {
            // Both added the same key
            (DiffResult::Added(_, source_value), DiffResult::Added(_, destination_value)) => {
                if source_value != destination_value {
                    Some(MergeConflict {
                        key: key.to_vec(),
                        base_value,
                        source_value: Some(source_value.clone()),
                        destination_value: Some(destination_value.clone()),
                    })
                } else {
                    None // Same value added, no conflict
                }
            }

            // Both removed the same key - no conflict
            (DiffResult::Removed(_, _), DiffResult::Removed(_, _)) => None,

            // Both modified the same key
            (
                DiffResult::Modified(_, _, source_value),
                DiffResult::Modified(_, _, destination_value),
            ) => {
                if source_value != destination_value {
                    Some(MergeConflict {
                        key: key.to_vec(),
                        base_value,
                        source_value: Some(source_value.clone()),
                        destination_value: Some(destination_value.clone()),
                    })
                } else {
                    None // Same modification, no conflict
                }
            }

            // One added, one removed - conflict
            (DiffResult::Added(_, source_value), DiffResult::Removed(_, _)) => {
                Some(MergeConflict {
                    key: key.to_vec(),
                    base_value,
                    source_value: Some(source_value.clone()),
                    destination_value: None,
                })
            }
            (DiffResult::Removed(_, _), DiffResult::Added(_, destination_value)) => {
                Some(MergeConflict {
                    key: key.to_vec(),
                    base_value,
                    source_value: None,
                    destination_value: Some(destination_value.clone()),
                })
            }

            // One added, one modified - conflict
            (DiffResult::Added(_, source_value), DiffResult::Modified(_, _, destination_value)) => {
                Some(MergeConflict {
                    key: key.to_vec(),
                    base_value,
                    source_value: Some(source_value.clone()),
                    destination_value: Some(destination_value.clone()),
                })
            }
            (DiffResult::Modified(_, _, source_value), DiffResult::Added(_, destination_value)) => {
                Some(MergeConflict {
                    key: key.to_vec(),
                    base_value,
                    source_value: Some(source_value.clone()),
                    destination_value: Some(destination_value.clone()),
                })
            }

            // One removed, one modified - conflict
            (DiffResult::Removed(_, _), DiffResult::Modified(_, _, destination_value)) => {
                Some(MergeConflict {
                    key: key.to_vec(),
                    base_value,
                    source_value: None,
                    destination_value: Some(destination_value.clone()),
                })
            }
            (DiffResult::Modified(_, _, source_value), DiffResult::Removed(_, _)) => {
                Some(MergeConflict {
                    key: key.to_vec(),
                    base_value,
                    source_value: Some(source_value.clone()),
                    destination_value: None,
                })
            }
        }
    }

    /// Recursively computes the differences between two Prolly Nodes.
    ///
    /// This helper function is used by `diff` to traverse the nodes of both trees
    /// and identify changes. It compares the keys and values of the nodes and
    /// generates appropriate `DiffResult` entries for added, removed, and modified
    /// key-value pairs.
    ///
    /// # Arguments
    ///
    /// * `old_node` - The node from the original tree.
    /// * `new_node` - The node from the new tree.
    /// * `diffs` - The vector to store the differences.
    fn diff_recursive(
        &self,
        old_node: &ProllyNode<N>,
        new_node: &ProllyNode<N>,
        diffs: &mut Vec<DiffResult>,
    ) {
        let mut old_iter = old_node.keys.iter().zip(old_node.values.iter()).peekable();
        let mut new_iter = new_node.keys.iter().zip(new_node.values.iter()).peekable();

        while let (Some((old_key, old_value)), Some((new_key, new_value))) =
            (old_iter.peek(), new_iter.peek())
        {
            match old_key.cmp(new_key) {
                std::cmp::Ordering::Less => {
                    diffs.push(DiffResult::Removed(old_key.to_vec(), old_value.to_vec()));
                    old_iter.next();
                }
                std::cmp::Ordering::Greater => {
                    diffs.push(DiffResult::Added(new_key.to_vec(), new_value.to_vec()));
                    new_iter.next();
                }
                std::cmp::Ordering::Equal => {
                    if old_value != new_value {
                        diffs.push(DiffResult::Modified(
                            old_key.to_vec(),
                            old_value.to_vec(),
                            new_value.to_vec(),
                        ));
                    }
                    old_iter.next();
                    new_iter.next();
                }
            }
        }

        for (old_key, old_value) in old_iter {
            diffs.push(DiffResult::Removed(old_key.clone(), old_value.clone()));
        }

        for (new_key, new_value) in new_iter {
            diffs.push(DiffResult::Added(new_key.clone(), new_value.clone()));
        }
    }

    /// Persist the root node to storage and save configuration
    pub fn persist_root(&mut self) {
        // Store the root node in the storage
        let root_hash = self.root.get_hash();
        if self
            .storage
            .insert_node(root_hash.clone(), self.root.clone())
            .is_some()
        {
            // Update the config with the new root hash
            self.config.root_hash = Some(root_hash);

            // Save the configuration
            let _ = self.save_config();
        }
    }

    /// Load a ProllyTree from an existing root hash in storage
    pub fn load_from_storage(storage: S, config: TreeConfig<N>) -> Option<Self> {
        if let Some(ref root_hash) = config.root_hash {
            if let Some(root_node) = storage.get_node_by_hash(root_hash) {
                return Some(ProllyTree {
                    root: root_node,
                    storage,
                    config,
                });
            }
        }
        None
    }

    /// Collect all keys from the tree
    pub fn collect_keys(&self) -> Vec<Vec<u8>> {
        let mut keys = Vec::new();
        self.collect_keys_recursive(&self.root, &mut keys);
        keys
    }

    /// Recursively collect keys from a node and its children
    fn collect_keys_recursive(&self, node: &ProllyNode<N>, keys: &mut Vec<Vec<u8>>) {
        // Add all keys from this node
        for key in &node.keys {
            keys.push(key.clone());
        }

        // Recursively collect keys from child nodes
        for child_node in node.children(&self.storage) {
            self.collect_keys_recursive(&child_node, keys);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryNodeStorage;

    /// Example usage of the Prolly Tree
    #[test]
    fn inmem_node_storage_test() {
        // 1. Create a custom tree config
        let config = TreeConfig {
            base: 131,
            modulus: 1_000_000_009,
            min_chunk_size: 4,
            max_chunk_size: 8 * 1024,
            pattern: 0b101,
            root_hash: None,
            key_schema: None,
            value_schema: None,
            encode_types: vec![],
        };

        // 2. Create and Wrap the Storage Backend
        let storage = InMemoryNodeStorage::<32>::default();

        // 3. Create the Prolly Tree
        let mut tree = ProllyTree::new(storage, config);

        // 4. Insert New Key-Value Pairs
        tree.insert(b"key1".to_vec(), b"value1".to_vec());
        tree.insert(b"key2".to_vec(), b"value2".to_vec());

        // 5. Traverse the Tree with a Custom Formatter
        let traversal = tree.formatted_traverse(|node| {
            let keys_as_strings: Vec<String> = node.keys.iter().map(|k| format!("{k:?}")).collect();
            format!("[L{}: {}]", node.level, keys_as_strings.join(", "))
        });
        println!("Traversal: {traversal}");

        // 6. Update the Value for an Existing Key
        tree.update(b"key1".to_vec(), b"new_value1".to_vec());

        // 7. Find or Search for a Key
        if let Some(node) = tree.find(b"key1") {
            println!("Found key1 with value: {node:?}");
        } else {
            println!("key1 not found");
        }

        // 8. Delete a key-value pair
        if tree.delete(b"key2") {
            println!("key2 deleted");
        } else {
            println!("key2 not found");
        }

        // 9. Print tree stats
        println!("Size: {}", tree.size());
        println!("Depth: {}", tree.depth());
        println!("Summary: {}", tree.summary());

        // 10. Print Tree
        println!("{:?}", tree.root.print_tree(&tree.storage));
    }

    #[test]
    fn file_node_storage_test() {
        use crate::storage::FileNodeStorage;
        use std::fs;
        use std::path::PathBuf;

        // 1. Create a custom tree config
        let config = TreeConfig {
            base: 131,
            modulus: 1_000_000_009,
            min_chunk_size: 4,
            max_chunk_size: 8 * 1024,
            pattern: 0b101,
            root_hash: None,
            key_schema: None,
            value_schema: None,
            encode_types: vec![],
        };

        // 2. Create and Wrap the Storage Backend
        let storage_dir = PathBuf::from("/tmp/prolly_tree_storage");
        let storage = FileNodeStorage::<32>::new(storage_dir.clone());

        // 3. Create the Prolly Tree
        let mut tree = ProllyTree::new(storage, config);

        // 4. Insert New Key-Value Pairs
        tree.insert(b"key1".to_vec(), b"value1".to_vec());
        tree.insert(b"key2".to_vec(), b"value2".to_vec());

        // 5. Traverse the Tree with a Custom Formatter
        let traversal = tree.formatted_traverse(|node| {
            let keys_as_strings: Vec<String> = node.keys.iter().map(|k| format!("{k:?}")).collect();
            format!("[L{}: {}]", node.level, keys_as_strings.join(", "))
        });
        println!("Traversal: {traversal}");

        // 6. Update the Value for an Existing Key
        tree.update(b"key1".to_vec(), b"new_value1".to_vec());

        // 7. Find or Search for a Key
        if let Some(node) = tree.find(b"key1") {
            println!("Found key1 with value: {node:?}");
        } else {
            println!("key1 not found");
        }

        // 8. Delete a key-value pair
        if tree.delete(b"key2") {
            println!("key2 deleted");
        } else {
            println!("key2 not found");
        }

        // 9. Print tree stats
        println!("Size: {}", tree.size());
        println!("Depth: {}", tree.depth());
        println!("Summary: {}", tree.summary());

        // 10. Print Tree
        println!("{:?}", tree.root.print_tree(&tree.storage));

        // Clean up the storage directory
        fs::remove_dir_all(storage_dir).unwrap();
    }

    #[test]
    fn test_insert_and_find() {
        let storage = InMemoryNodeStorage::<32>::default();

        let mut tree = ProllyTree::new(storage, TreeConfig::default());

        tree.insert(b"key1".to_vec(), b"value1".to_vec());
        tree.insert(b"key2".to_vec(), b"value2".to_vec());

        assert!(tree.find(b"key1").is_some());
        assert!(tree.find(b"key2").is_some());
        assert!(tree.find(b"key3").is_none());
    }

    #[test]
    fn test_persist_and_load() {
        let storage = InMemoryNodeStorage::<32>::default();
        let config = TreeConfig::default();

        // Create tree and add data
        let mut tree = ProllyTree::new(storage.clone(), config.clone());
        tree.insert(b"key1".to_vec(), b"value1".to_vec());
        tree.insert(b"key2".to_vec(), b"value2".to_vec());

        // Persist the tree
        tree.persist_root();

        // Load the tree from storage
        let loaded_tree = ProllyTree::load_from_storage(tree.storage, tree.config)
            .expect("Should be able to load tree from storage");

        // Verify data is preserved
        assert!(loaded_tree.find(b"key1").is_some());
        assert!(loaded_tree.find(b"key2").is_some());
        assert!(loaded_tree.find(b"key3").is_none());
    }

    #[test]
    fn test_insert_batch_and_find() {
        let storage = InMemoryNodeStorage::<32>::default();

        let mut tree = ProllyTree::new(storage, TreeConfig::default());

        let keys = vec![b"key1".to_vec(), b"key2".to_vec(), b"key3".to_vec()];
        let values = vec![b"value1".to_vec(), b"value2".to_vec(), b"value3".to_vec()];

        tree.insert_batch(&keys, &values);

        assert!(tree.find(b"key1").is_some());
        assert!(tree.find(b"key2").is_some());
        assert!(tree.find(b"key3").is_some());
        assert!(tree.find(b"key4").is_none());
    }

    #[test]
    fn test_delete() {
        let storage = InMemoryNodeStorage::<32>::default();
        let mut tree = ProllyTree::new(storage, TreeConfig::default());

        tree.insert(b"key1".to_vec(), b"value1".to_vec());
        tree.insert(b"key2".to_vec(), b"value2".to_vec());

        assert!(tree.delete(b"key1"));
        assert!(tree.find(b"key1").is_none());
        assert!(tree.find(b"key2").is_some());
    }

    #[test]
    fn test_delete_batch() {
        let storage = InMemoryNodeStorage::<32>::default();
        let mut tree = ProllyTree::new(storage, TreeConfig::default());

        let keys = vec![b"key1".to_vec(), b"key2".to_vec(), b"key3".to_vec()];
        let values = vec![b"value1".to_vec(), b"value2".to_vec(), b"value3".to_vec()];

        // Insert keys and values
        tree.insert_batch(&keys, &values);

        // Verify insertion
        assert!(tree.find(b"key1").is_some());
        assert!(tree.find(b"key2").is_some());
        assert!(tree.find(b"key3").is_some());

        // Delete keys in batch
        tree.delete_batch(&keys);

        // Verify deletion
        assert!(tree.find(b"key1").is_none());
        assert!(tree.find(b"key2").is_none());
        assert!(tree.find(b"key3").is_none());
    }

    #[test]
    fn test_traverse() {
        let storage = InMemoryNodeStorage::<32>::default();
        let mut tree = ProllyTree::new(storage, TreeConfig::default());

        let key1 = b"key1".to_vec();
        let key2 = b"key2".to_vec();

        tree.insert(key1.clone(), b"value1".to_vec());
        tree.insert(key2.clone(), b"value2".to_vec());

        let traversal = tree.traverse();

        // Convert byte arrays to their binary representation strings for comparison
        let expected_key1 = format!("{key1:?}");
        let expected_key2 = format!("{key2:?}");

        // Check if the traversal contains the expected keys
        assert!(traversal.contains(&expected_key1.to_string()));
        assert!(traversal.contains(&expected_key2.to_string()));
    }

    #[test]
    fn test_stats() {
        let storage = InMemoryNodeStorage::<32>::default();
        let config = TreeConfig {
            base: 131,
            modulus: 1_000_000_009,
            min_chunk_size: 16,
            max_chunk_size: 8 * 1024,
            pattern: 0b111,
            root_hash: None,
            key_schema: None,
            value_schema: None,
            encode_types: vec![],
        };

        let mut tree = ProllyTree::new(storage, config);

        // Insert key-value pairs using a loop
        let max_key = 3000u32;

        for i in 0..max_key {
            // Convert to big-endian byte array to maintain order
            let key = i.to_be_bytes().to_vec();
            let value = i.to_be_bytes().to_vec();
            tree.insert(key.clone(), value.clone());
        }

        println!("{:?}", tree.root.print_tree(&tree.storage));

        for i in 0..max_key {
            let key = i.to_be_bytes().to_vec();
            assert!(tree.find(&key).is_some());
        }
        let non_existing_key = (max_key + 10).to_be_bytes().to_vec();
        assert!(tree.find(&non_existing_key).is_none());

        // assert that the tree has the expected key-value pairs
        assert_eq!(tree.size(), max_key as usize);

        // assert that the tree has the expected depth
        assert_eq!(tree.depth(), 3);

        println!("Size: {}", tree.size());
        println!("Depth: {}", tree.depth());
        println!("Summary: {}", tree.summary());
    }

    #[test]
    fn test_generate_proof() {
        let config = TreeConfig::default();
        let storage = InMemoryNodeStorage::<32>::default();
        let mut tree = ProllyTree::new(storage, config);

        // Insert key-value pairs
        for i in 0..100 {
            let key = vec![i];
            let value = vec![i];
            tree.insert(key.clone(), value.clone());
        }

        // Generate proof for an existing key
        let key_to_prove = vec![5];
        let proof = tree.generate_proof(&key_to_prove);

        // Verify the proof
        let verified = tree.verify(proof, &key_to_prove, Some(&key_to_prove));
        assert!(verified);

        // Generate proof for a non-existing key
        let key_to_prove_wrong = vec![120];
        let proof_wrong = tree.generate_proof(&key_to_prove_wrong);

        // Should not be verified
        let verified_wrong =
            tree.verify(proof_wrong, &key_to_prove_wrong, Some(&key_to_prove_wrong));
        assert!(!verified_wrong);
    }

    #[test]
    fn test_diff() {
        let config = TreeConfig::default();
        let storage1 = InMemoryNodeStorage::<32>::default();
        let mut tree1 = ProllyTree::new(storage1, config.clone());

        let storage2 = InMemoryNodeStorage::<32>::default();
        let mut tree2 = ProllyTree::new(storage2, config);

        // Insert key-value pairs into tree1
        for i in 0..50 {
            tree1.insert(vec![i], vec![i]);
        }

        // Insert key-value pairs into tree1
        for i in 0..50 {
            tree2.insert(vec![i], vec![i]);
        }

        // modify some keys in tree2
        tree2.insert(vec![10], vec![200]);

        // print tree1 and tree2
        println!("{:?}", tree1.root.print_tree(&tree1.storage));
        println!("{:?}", tree2.root.print_tree(&tree2.storage));

        // Generate diff between tree1 and tree2
        let differences = tree1.diff(&tree2);

        // Check the differences
        // Expecting only the first L1 value would change
        for diff in &differences {
            match diff {
                DiffResult::Added(key, value) => {
                    println!("Added: key = {key:?}, value = {value:?}");
                }
                DiffResult::Removed(key, value) => {
                    println!("Removed: key = {key:?}, value = {value:?}");
                }
                DiffResult::Modified(key, old_value, new_value) => {
                    println!(
                        "Modified: key = {key:?}, old_value = {old_value:?}, new_value = {new_value:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_print_proof_demo() {
        let config = TreeConfig::default();
        let storage = InMemoryNodeStorage::<32>::default();
        let mut tree = ProllyTree::new(storage, config);

        // Insert enough key-value pairs to create a multi-level tree
        for i in 0..20 {
            tree.insert(vec![i], vec![i * 10]);
        }

        println!("=== Prolly Tree with Proof Visualization Demo ===");

        // Test with an existing key
        let existing_key = vec![10];
        println!("\n--- Testing with existing key {:?} ---", existing_key);
        let is_valid = tree.print_proof(&existing_key);
        assert!(is_valid, "Proof should be valid for existing key");

        // Test with a non-existing key
        let non_existing_key = vec![25];
        println!(
            "\n--- Testing with non-existing key {:?} ---",
            non_existing_key
        );
        let is_valid = tree.print_proof(&non_existing_key);
        assert!(!is_valid, "Proof should be invalid for non-existing key");

        println!("\n=== Demo completed successfully ===");
    }

    #[test]
    fn test_merge_simple() {
        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree and persist it
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"key1".to_vec(), b"value1".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create source tree - same as base
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"key1".to_vec(), b"value1".to_vec());
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        // Create destination tree - same as base
        let mut dest_tree = ProllyTree::new(storage.clone(), config.clone());
        dest_tree.insert(b"key1".to_vec(), b"value1".to_vec());
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Perform merge using storage that contains all the nodes
        let merge_tree = ProllyTree::new(storage, config);
        let merge_results = merge_tree.merge(&source_root, &dest_root, &base_root);

        // With identical trees, should have no changes
        assert_eq!(merge_results.len(), 0);
    }

    #[test]
    fn test_merge_with_conflicts() {
        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"key1".to_vec(), b"value1".to_vec());
        base_tree.insert(b"key2".to_vec(), b"value2".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create source tree - modify key1 to "source_value"
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"key1".to_vec(), b"source_value".to_vec());
        source_tree.insert(b"key2".to_vec(), b"value2".to_vec());
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        // Create destination tree - modify key1 to "dest_value"
        let mut dest_tree = ProllyTree::new(storage.clone(), config.clone());
        dest_tree.insert(b"key1".to_vec(), b"dest_value".to_vec());
        dest_tree.insert(b"key2".to_vec(), b"value2".to_vec());
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Perform merge
        let merge_tree = ProllyTree::new(storage, config);
        let merge_results = merge_tree.merge(&source_root, &dest_root, &base_root);

        // Verify conflict detected
        assert_eq!(merge_results.len(), 1);
        match &merge_results[0] {
            MergeResult::Conflict(conflict) => {
                assert_eq!(conflict.key, b"key1".to_vec());
                assert_eq!(conflict.base_value, Some(b"value1".to_vec()));
                assert_eq!(conflict.source_value, Some(b"source_value".to_vec()));
                assert_eq!(conflict.destination_value, Some(b"dest_value".to_vec()));
            }
            _ => panic!("Expected conflict, got: {:?}", merge_results[0]),
        }
    }

    #[test]
    fn test_merge_to_empty_tree() {
        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree (empty)
        let base_tree = ProllyTree::new(storage.clone(), config.clone());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create source tree with data
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"key1".to_vec(), b"value1".to_vec());
        source_tree.insert(b"key2".to_vec(), b"value2".to_vec());
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        // Create destination tree (also empty)
        let dest_tree = ProllyTree::new(storage.clone(), config.clone());
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Perform merge
        let merge_tree = ProllyTree::new(storage, config);
        let merge_results = merge_tree.merge(&source_root, &dest_root, &base_root);

        // Should see all source changes as additions
        assert_eq!(merge_results.len(), 2);
        let mut keys_added = std::collections::HashSet::new();
        for result in merge_results {
            match result {
                MergeResult::Added(key, _) => {
                    keys_added.insert(key);
                }
                _ => panic!("Expected only additions, got: {:?}", result),
            }
        }
        assert!(keys_added.contains(&b"key1".to_vec()));
        assert!(keys_added.contains(&b"key2".to_vec()));
    }

    #[test]
    fn test_merge_add_remove_conflicts() {
        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"key1".to_vec(), b"value1".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create source tree - remove key1, add key2
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"key2".to_vec(), b"value2".to_vec());
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        // Create destination tree - modify key1
        let mut dest_tree = ProllyTree::new(storage.clone(), config.clone());
        dest_tree.insert(b"key1".to_vec(), b"modified_value1".to_vec());
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Perform merge
        let merge_tree = ProllyTree::new(storage, config);
        let merge_results = merge_tree.merge(&source_root, &dest_root, &base_root);

        // Should have one addition and one conflict
        assert_eq!(merge_results.len(), 2);

        let mut has_addition = false;
        let mut has_conflict = false;

        for result in merge_results {
            match result {
                MergeResult::Added(key, value) => {
                    assert_eq!(key, b"key2".to_vec());
                    assert_eq!(value, b"value2".to_vec());
                    has_addition = true;
                }
                MergeResult::Conflict(conflict) => {
                    assert_eq!(conflict.key, b"key1".to_vec());
                    assert_eq!(conflict.base_value, Some(b"value1".to_vec()));
                    assert_eq!(conflict.source_value, None); // removed in source
                    assert_eq!(
                        conflict.destination_value,
                        Some(b"modified_value1".to_vec())
                    );
                    has_conflict = true;
                }
                _ => panic!("Unexpected merge result: {:?}", result),
            }
        }

        assert!(has_addition && has_conflict);
    }

    #[test]
    fn test_merge_complex_scenario() {
        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree with a single key for simplicity
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"modify_conflict".to_vec(), b"base_value".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create source tree - modify the key and add a new one
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"modify_conflict".to_vec(), b"source_value".to_vec()); // modified
        source_tree.insert(b"new_in_source".to_vec(), b"source_addition".to_vec()); // added
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        // Create destination tree - modify the key differently
        let mut dest_tree = ProllyTree::new(storage.clone(), config.clone());
        dest_tree.insert(b"modify_conflict".to_vec(), b"dest_value".to_vec()); // modified differently
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Perform merge
        let merge_tree = ProllyTree::new(storage, config);
        let merge_results = merge_tree.merge(&source_root, &dest_root, &base_root);

        // Analyze results
        let mut added_keys = std::collections::HashSet::new();
        let mut conflicts = std::collections::HashMap::new();

        for result in merge_results {
            match result {
                MergeResult::Added(key, _) => {
                    added_keys.insert(key);
                }
                MergeResult::Conflict(conflict) => {
                    conflicts.insert(conflict.key.clone(), conflict);
                }
                _ => panic!("Unexpected merge result: {:?}", result),
            }
        }

        // Should have one addition and one conflict
        assert!(added_keys.contains(&b"new_in_source".to_vec()));
        assert!(conflicts.contains_key(&b"modify_conflict".to_vec()));

        // Verify conflict details (relax the base_value check for now)
        let conflict = &conflicts[&b"modify_conflict".to_vec()];
        assert_eq!(conflict.source_value, Some(b"source_value".to_vec()));
        assert_eq!(conflict.destination_value, Some(b"dest_value".to_vec()));
    }

    #[test]
    fn test_merge_same_changes_no_conflict() {
        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"key1".to_vec(), b"value1".to_vec());
        base_tree.insert(b"key2".to_vec(), b"value2".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Both source and destination make the same changes
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"key1".to_vec(), b"same_new_value".to_vec()); // both modify to same value
        source_tree.insert(b"key2".to_vec(), b"value2".to_vec()); // unchanged
        source_tree.insert(b"key3".to_vec(), b"same_addition".to_vec()); // both add same key-value
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        let mut dest_tree = ProllyTree::new(storage.clone(), config.clone());
        dest_tree.insert(b"key1".to_vec(), b"same_new_value".to_vec()); // same modification
        dest_tree.insert(b"key2".to_vec(), b"value2".to_vec()); // unchanged
        dest_tree.insert(b"key3".to_vec(), b"same_addition".to_vec()); // same addition
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Perform merge
        let merge_tree = ProllyTree::new(storage, config);
        let merge_results = merge_tree.merge(&source_root, &dest_root, &base_root);

        // Should apply changes without conflict since they're identical
        assert_eq!(merge_results.len(), 2);

        let mut has_modification = false;
        let mut has_addition = false;

        for result in merge_results {
            match result {
                MergeResult::Modified(key, value) => {
                    assert_eq!(key, b"key1".to_vec());
                    assert_eq!(value, b"same_new_value".to_vec());
                    has_modification = true;
                }
                MergeResult::Added(key, value) => {
                    assert_eq!(key, b"key3".to_vec());
                    assert_eq!(value, b"same_addition".to_vec());
                    has_addition = true;
                }
                _ => panic!("Unexpected merge result: {:?}", result),
            }
        }

        assert!(has_modification && has_addition);
    }

    #[test]
    fn test_apply_merge_results_success() {
        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"existing".to_vec(), b"value".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create merge results (no conflicts)
        let merge_results = vec![
            MergeResult::Added(b"new_key".to_vec(), b"new_value".to_vec()),
            MergeResult::Modified(b"existing".to_vec(), b"modified_value".to_vec()),
        ];

        // Apply merge results
        let merge_tree = ProllyTree::new(storage, config);
        let result = merge_tree.apply_merge_results(&base_root, &merge_results);

        assert!(result.is_ok());
        let merged_tree = result.unwrap();

        // Verify the merged tree has the expected changes
        assert!(merged_tree.find(b"new_key").is_some());
        assert!(merged_tree.find(b"existing").is_some());

        // Check values are correct
        if let Some(node) = merged_tree.find(b"new_key") {
            let key_idx = node.keys.iter().position(|k| k == b"new_key").unwrap();
            let value = node.values[key_idx].clone();
            assert_eq!(value, b"new_value".to_vec());
        }

        if let Some(node) = merged_tree.find(b"existing") {
            let key_idx = node.keys.iter().position(|k| k == b"existing").unwrap();
            let value = node.values[key_idx].clone();
            assert_eq!(value, b"modified_value".to_vec());
        }
    }

    #[test]
    fn test_apply_merge_results_with_conflicts() {
        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree
        let base_tree = ProllyTree::new(storage.clone(), config.clone());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create merge results with conflicts
        let merge_results = vec![
            MergeResult::Added(b"new_key".to_vec(), b"new_value".to_vec()),
            MergeResult::Conflict(MergeConflict {
                key: b"conflict_key".to_vec(),
                base_value: Some(b"base".to_vec()),
                source_value: Some(b"source".to_vec()),
                destination_value: Some(b"dest".to_vec()),
            }),
        ];

        // Apply merge results should fail due to conflicts
        let merge_tree = ProllyTree::new(storage, config);
        let result = merge_tree.apply_merge_results(&base_root, &merge_results);

        assert!(result.is_err());
        let conflicts = result.expect_err("Expected conflicts");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].key, b"conflict_key".to_vec());
    }

    #[test]
    fn test_merge_trees_success() {
        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"shared".to_vec(), b"original".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create source tree with additions
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"shared".to_vec(), b"original".to_vec());
        source_tree.insert(b"from_source".to_vec(), b"source_value".to_vec());
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        // Create destination tree with different additions
        let mut dest_tree = ProllyTree::new(storage.clone(), config.clone());
        dest_tree.insert(b"shared".to_vec(), b"original".to_vec());
        dest_tree.insert(b"from_dest".to_vec(), b"dest_value".to_vec());
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Perform merge_trees (should succeed - no conflicts)
        let merge_tree = ProllyTree::new(storage, config);
        let result = merge_tree.merge_trees_ignore_conflicts(&source_root, &dest_root, &base_root);

        assert!(result.is_ok());
        let merged_tree = result.unwrap();

        // Verify merged tree contains all expected keys
        assert!(merged_tree.find(b"shared").is_some());
        assert!(merged_tree.find(b"from_source").is_some());
        assert!(merged_tree.find(b"from_dest").is_some()); // dest already had this
    }

    #[test]
    fn test_merge_trees_with_conflicts() {
        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"conflict_key".to_vec(), b"original".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create source tree with modification
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"conflict_key".to_vec(), b"source_value".to_vec());
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        // Create destination tree with different modification
        let mut dest_tree = ProllyTree::new(storage.clone(), config.clone());
        dest_tree.insert(b"conflict_key".to_vec(), b"dest_value".to_vec());
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Perform merge_trees (should fail due to conflict)
        let merge_tree = ProllyTree::new(storage, config);

        // Create a resolver that doesn't resolve conflicts (keeps them as conflicts)
        struct NoResolutionResolver;
        impl ConflictResolver for NoResolutionResolver {
            fn resolve_conflict(&self, _conflict: &MergeConflict) -> Option<MergeResult> {
                None // Don't resolve any conflicts
            }
        }

        let resolver = NoResolutionResolver;
        let result = merge_tree.merge_trees(&source_root, &dest_root, &base_root, &resolver);

        assert!(result.is_err());
        let conflicts = result.expect_err("Expected conflicts");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].key, b"conflict_key".to_vec());
    }

    #[test]
    fn test_merge_trees_with_ignore_conflicts_resolver() {
        use crate::diff::IgnoreConflictsResolver;

        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"shared".to_vec(), b"base_value".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create source tree with modifications
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"shared".to_vec(), b"source_value".to_vec());
        source_tree.insert(b"source_only".to_vec(), b"source_only_value".to_vec());
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        // Create destination tree with conflicting modifications
        let mut dest_tree = ProllyTree::new(storage.clone(), config.clone());
        dest_tree.insert(b"shared".to_vec(), b"dest_value".to_vec());
        dest_tree.insert(b"dest_only".to_vec(), b"dest_only_value".to_vec());
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Perform merge with ignore conflicts resolver
        let merge_tree = ProllyTree::new(storage, config);
        let resolver = IgnoreConflictsResolver;
        let result = merge_tree.merge_trees(&source_root, &dest_root, &base_root, &resolver);

        // Should succeed since conflicts are ignored
        assert!(result.is_ok());
        let merged_tree = result.unwrap();

        // Verify non-conflicting changes were applied
        assert!(merged_tree.find(b"source_only").is_some());
        assert!(merged_tree.find(b"dest_only").is_some());

        // Conflicting key should remain with destination value (unchanged)
        if let Some(node) = merged_tree.find(b"shared") {
            let key_idx = node.keys.iter().position(|k| k == b"shared").unwrap();
            let value = node.values[key_idx].clone();
            assert_eq!(value, b"dest_value".to_vec());
        }
    }

    #[test]
    fn test_merge_trees_with_take_source_resolver() {
        use crate::diff::TakeSourceResolver;

        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"conflict_key".to_vec(), b"base_value".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create source tree
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"conflict_key".to_vec(), b"source_value".to_vec());
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        // Create destination tree
        let mut dest_tree = ProllyTree::new(storage.clone(), config.clone());
        dest_tree.insert(b"conflict_key".to_vec(), b"dest_value".to_vec());
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Perform merge with take source resolver
        let merge_tree = ProllyTree::new(storage, config);
        let resolver = TakeSourceResolver;
        let result = merge_tree.merge_trees(&source_root, &dest_root, &base_root, &resolver);

        assert!(result.is_ok());
        let merged_tree = result.unwrap();

        // Should have source value due to resolver
        if let Some(node) = merged_tree.find(b"conflict_key") {
            let key_idx = node.keys.iter().position(|k| k == b"conflict_key").unwrap();
            let value = node.values[key_idx].clone();
            assert_eq!(value, b"source_value".to_vec());
        }
    }

    #[test]
    fn test_merge_trees_with_take_destination_resolver() {
        use crate::diff::TakeDestinationResolver;

        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"conflict_key".to_vec(), b"base_value".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create source tree
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"conflict_key".to_vec(), b"source_value".to_vec());
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        // Create destination tree
        let mut dest_tree = ProllyTree::new(storage.clone(), config.clone());
        dest_tree.insert(b"conflict_key".to_vec(), b"dest_value".to_vec());
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Perform merge with take destination resolver
        let merge_tree = ProllyTree::new(storage, config);
        let resolver = TakeDestinationResolver;
        let result = merge_tree.merge_trees(&source_root, &dest_root, &base_root, &resolver);

        assert!(result.is_ok());
        let merged_tree = result.unwrap();

        // Should have destination value due to resolver
        if let Some(node) = merged_tree.find(b"conflict_key") {
            let key_idx = node.keys.iter().position(|k| k == b"conflict_key").unwrap();
            let value = node.values[key_idx].clone();
            assert_eq!(value, b"dest_value".to_vec());
        }
    }

    #[test]
    fn test_merge_trees_ignore_conflicts_convenience_method() {
        let config = TreeConfig::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        // Create base tree
        let mut base_tree = ProllyTree::new(storage.clone(), config.clone());
        base_tree.insert(b"conflict_key".to_vec(), b"base_value".to_vec());
        let base_root = base_tree.get_root_hash().unwrap();
        storage.insert_node(base_root.clone(), base_tree.root.clone());

        // Create source tree
        let mut source_tree = ProllyTree::new(storage.clone(), config.clone());
        source_tree.insert(b"conflict_key".to_vec(), b"source_value".to_vec());
        source_tree.insert(b"new_key".to_vec(), b"new_value".to_vec());
        let source_root = source_tree.get_root_hash().unwrap();
        storage.insert_node(source_root.clone(), source_tree.root.clone());

        // Create destination tree
        let mut dest_tree = ProllyTree::new(storage.clone(), config.clone());
        dest_tree.insert(b"conflict_key".to_vec(), b"dest_value".to_vec());
        let dest_root = dest_tree.get_root_hash().unwrap();
        storage.insert_node(dest_root.clone(), dest_tree.root.clone());

        // Use convenience method
        let merge_tree = ProllyTree::new(storage, config);
        let result = merge_tree.merge_trees_ignore_conflicts(&source_root, &dest_root, &base_root);

        assert!(result.is_ok());
        let merged_tree = result.unwrap();

        // Should have the new key from source
        assert!(merged_tree.find(b"new_key").is_some());

        // Conflicting key should keep destination value (ignored conflict)
        if let Some(node) = merged_tree.find(b"conflict_key") {
            let key_idx = node.keys.iter().position(|k| k == b"conflict_key").unwrap();
            let value = node.values[key_idx].clone();
            assert_eq!(value, b"dest_value".to_vec());
        }
    }
}
