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
use crate::diff::DiffResult;
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
}

impl<const N: usize, S: NodeStorage<N>> ProllyTree<N, S> {
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
}
