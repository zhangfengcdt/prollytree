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
    root: ProllyNode<N>,
    storage: S,
    config: TreeConfig<N>,
}

impl<const N: usize, S: NodeStorage<N>> Tree<N, S> for ProllyTree<N, S> {
    fn new(storage: S, config: TreeConfig<N>) -> Self {
        let root = ProllyNode {
            keys: Vec::new(),
            values: Vec::new(),
            is_leaf: true,
            level: 0,
            base: config.base,
            modulus: config.modulus,
            min_chunk_size: config.min_chunk_size,
            max_chunk_size: config.max_chunk_size,
            pattern: config.pattern,
            promoted: false,
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
        self.root.insert(key, value, &mut self.storage, None);
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
        self.root.delete(key, &mut self.storage, None)
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

    fn generate_proof(&self, key: &[u8]) -> Proof<N> {
        let mut path = Vec::new();
        let mut current_node = self.root.clone();

        loop {
            path.push(current_node.get_hash());

            if current_node.is_leaf {
                if current_node.keys.contains(&key.to_vec()) {
                    return Proof {
                        path,
                        target_hash: Some(current_node.get_hash()),
                    };
                } else {
                    return Proof {
                        path,
                        target_hash: None,
                    };
                }
            } else {
                let mut found = false;
                for i in 0..current_node.keys.len() {
                    if current_node.keys[i] >= key.to_vec() {
                        let child_hash = ValueDigest::raw_hash(&current_node.values[i]);
                        if let Some(child_node) = self.storage.get_node_by_hash(&child_hash) {
                            current_node = child_node.clone();
                            found = true;
                            break;
                        }
                    }
                }
                if !found {
                    return Proof {
                        path,
                        target_hash: None,
                    };
                }
            }
        }
    }

    fn diff(&self, other: &Self) -> Vec<DiffResult> {
        let mut diffs = Vec::new();
        self.diff_recursive(&self.root, &other.root, &mut diffs);
        diffs
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryNodeStorage;

    #[test]
    fn test_insert_and_find() {
        let storage = InMemoryNodeStorage::<32>::new();

        let mut tree = ProllyTree::new(storage, TreeConfig::default());

        tree.insert(b"key1".to_vec(), b"value1".to_vec());
        tree.insert(b"key2".to_vec(), b"value2".to_vec());

        assert!(tree.find(b"key1").is_some());
        assert!(tree.find(b"key2").is_some());
        assert!(tree.find(b"key3").is_none());
    }

    #[test]
    fn test_delete() {
        let storage = InMemoryNodeStorage::<32>::new();
        let mut tree = ProllyTree::new(storage, TreeConfig::default());

        tree.insert(b"key1".to_vec(), b"value1".to_vec());
        tree.insert(b"key2".to_vec(), b"value2".to_vec());

        assert!(tree.delete(b"key1"));
        assert!(tree.find(b"key1").is_none());
        assert!(tree.find(b"key2").is_some());
    }

    #[test]
    fn test_traverse() {
        let storage = InMemoryNodeStorage::<32>::new();
        let mut tree = ProllyTree::new(storage, TreeConfig::default());

        let key1 = b"key1".to_vec();
        let key2 = b"key2".to_vec();

        tree.insert(key1.clone(), b"value1".to_vec());
        tree.insert(key2.clone(), b"value2".to_vec());

        let traversal = tree.traverse();

        // Convert byte arrays to their binary representation strings for comparison
        let expected_key1 = format!("{:?}", key1);
        let expected_key2 = format!("{:?}", key2);

        // Check if the traversal contains the expected keys
        assert!(traversal.contains(&expected_key1.to_string()));
        assert!(traversal.contains(&expected_key2.to_string()));
    }

    #[test]
    fn test_all() {
        let storage = InMemoryNodeStorage::<32>::new();
        let config = TreeConfig {
            base: 131,
            modulus: 1_000_000_009,
            min_chunk_size: 4,
            max_chunk_size: 8 * 1024,
            pattern: 0b101,
            root_hash: None,
        };

        let mut tree = ProllyTree::new(storage, config);

        // Insert key-value pairs using a loop
        for i in 0..10 {
            tree.insert(vec![i], vec![i]);
            if true {
                let traversal = tree.formatted_traverse(|node| {
                    format!("[L{}:{:?}]\n", node.level, node.keys)
                    // if node.is_leaf {
                    //     format!("[L{}:{:?}]", node.level, node.keys)
                    // } else {
                    //     "".to_string()
                    // }
                });
                println!("Traversal #{}:\n{}", i, traversal);
            }
        }

        // for i in 0..100 {
        //     assert!(tree.find(&[i]).is_some());
        // }
        // assert!(tree.find(&[110]).is_none());

        // //assert that the tree has 100 key-value pairs
        // assert_eq!(tree.size(), 100);
        //
        // // assert that the tree has a depth of 2
        // assert_eq!(tree.depth(), 2);
        //
        // println!("Size: {}", tree.size());
        // println!("Depth: {}", tree.depth());
        // println!("Summary: {}", tree.summary());
        //
        // println!("Traversal: {}", tree.traverse());
        //
        // assert!(tree.find(&[1]).is_some());
        // assert!(tree.find(&[2]).is_some());
        // assert!(tree.find(&[110]).is_none());
    }

    #[test]
    fn main_test() {
        // 1. Create a custom tree config
        let config = TreeConfig {
            base: 131,
            modulus: 1_000_000_009,
            min_chunk_size: 4,
            max_chunk_size: 8 * 1024,
            pattern: 0b101,
            root_hash: None,
        };

        // 2. Create and Wrap the Storage Backend
        let storage = InMemoryNodeStorage::<32>::new();

        // 3. Create the Prolly Tree
        let mut tree = ProllyTree::new(storage, config);

        // 4. Insert New Key-Value Pairs
        tree.insert(b"key1".to_vec(), b"value1".to_vec());
        tree.insert(b"key2".to_vec(), b"value2".to_vec());

        // 5. Traverse the Tree with a Custom Formatter
        let traversal = tree.formatted_traverse(|node| {
            let keys_as_strings: Vec<String> =
                node.keys.iter().map(|k| format!("{:?}", k)).collect();
            format!("[L{}: {}]", node.level, keys_as_strings.join(", "))
        });
        println!("Traversal: {}", traversal);

        // 6. Update the Value for an Existing Key
        tree.update(b"key1".to_vec(), b"new_value1".to_vec());

        // 7. Find or Search for a Key
        if let Some(node) = tree.find(b"key1") {
            println!("Found key1 with value: {:?}", node);
        } else {
            println!("key1 not found");
        }

        // 8. Delete a Key-Value Pair
        if tree.delete(b"key2") {
            println!("key2 deleted");
        } else {
            println!("key2 not found");
        }
    }
}
