use async_trait::async_trait;
use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::traits::MemoryPersistence;
use crate::config::TreeConfig;
use crate::storage::InMemoryNodeStorage;
use crate::tree::{ProllyTree, Tree};

/// Prolly tree-based in-memory persistence for agent memory
/// This provides a more robust foundation than a simple HashMap
/// while maintaining thread safety and async compatibility
pub struct InMemoryPersistence {
    tree: Arc<RwLock<ProllyTree<32, InMemoryNodeStorage<32>>>>,
    namespace_prefix: String,
    commit_counter: Arc<RwLock<u64>>,
}

impl InMemoryPersistence {
    /// Initialize a new prolly tree-based memory persistence layer
    pub fn init<P: AsRef<Path>>(_path: P, namespace_prefix: &str) -> Result<Self, Box<dyn Error>> {
        let storage = InMemoryNodeStorage::new();
        let config = TreeConfig::default();
        let tree = ProllyTree::new(storage, config);

        Ok(Self {
            tree: Arc::new(RwLock::new(tree)),
            namespace_prefix: namespace_prefix.to_string(),
            commit_counter: Arc::new(RwLock::new(0)),
        })
    }

    /// Open an existing prolly tree-based memory persistence layer
    /// For in-memory storage, this is the same as init
    pub fn open<P: AsRef<Path>>(_path: P, namespace_prefix: &str) -> Result<Self, Box<dyn Error>> {
        Self::init(_path, namespace_prefix)
    }

    /// Get the full key with namespace prefix
    fn full_key(&self, key: &str) -> Vec<u8> {
        format!("{}/{}", self.namespace_prefix, key).into_bytes()
    }

    /// Generate next commit ID
    async fn next_commit_id(&self) -> String {
        let mut counter = self.commit_counter.write().await;
        *counter += 1;
        format!("prolly_commit_{:08}", *counter)
    }
}

#[async_trait]
impl MemoryPersistence for InMemoryPersistence {
    async fn save(&mut self, key: &str, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let full_key = self.full_key(key);
        let mut tree = self.tree.write().await;

        // Insert into prolly tree
        tree.insert(full_key, data.to_vec());

        Ok(())
    }

    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        let full_key = self.full_key(key);
        let tree = self.tree.read().await;

        // Get from prolly tree using find method
        let result = tree.find(&full_key).and_then(|node| {
            // Find the value in the node
            node.keys
                .iter()
                .position(|k| k == &full_key)
                .map(|index| node.values[index].clone())
        });

        Ok(result)
    }

    async fn delete(&mut self, key: &str) -> Result<(), Box<dyn Error>> {
        let full_key = self.full_key(key);
        let mut tree = self.tree.write().await;

        // Delete from prolly tree (returns bool indicating success)
        tree.delete(&full_key);

        Ok(())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let namespace_prefix_with_slash = format!("{}/", self.namespace_prefix);
        let tree = self.tree.read().await;

        // Get all keys and filter by prefix
        let all_keys = tree.collect_keys();

        let matching_keys: Vec<String> = all_keys
            .into_iter()
            .filter_map(|key| {
                // First convert to string and strip namespace
                String::from_utf8(key).ok().and_then(|s| {
                    s.strip_prefix(&namespace_prefix_with_slash)
                        .map(|s| s.to_string())
                })
            })
            .filter(|relative_key| relative_key.starts_with(prefix))
            .collect::<std::collections::HashSet<_>>() // Deduplicate
            .into_iter()
            .collect();

        Ok(matching_keys)
    }

    async fn checkpoint(&mut self, message: &str) -> Result<String, Box<dyn Error>> {
        let commit_id = self.next_commit_id().await;

        // For in-memory storage, we just generate a commit ID
        // In a real git-based implementation, this would create an actual commit
        println!("Prolly tree checkpoint: {} - {}", commit_id, message);

        Ok(commit_id)
    }
}

/// Additional methods specific to prolly tree persistence
impl InMemoryPersistence {
    /// Create a new branch (for in-memory, this is a no-op)
    pub async fn create_branch(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        println!("Created prolly tree branch: {name}");
        Ok(())
    }

    /// Switch to a branch or commit (for in-memory, this is a no-op)
    pub async fn checkout(&mut self, branch_or_commit: &str) -> Result<(), Box<dyn Error>> {
        println!("Checked out prolly tree: {branch_or_commit}");
        Ok(())
    }

    /// Get current branch name
    pub async fn current_branch(&self) -> String {
        "main".to_string()
    }

    /// List all branches
    pub async fn list_branches(&self) -> Result<Vec<String>, Box<dyn Error>> {
        Ok(vec!["main".to_string()])
    }

    /// Get status of staged changes
    pub async fn status(&self) -> Vec<(Vec<u8>, String)> {
        vec![]
    }

    /// Merge another branch (for in-memory, this is a no-op)
    pub async fn merge(&mut self, branch: &str) -> Result<String, Box<dyn Error>> {
        println!("Merged prolly tree branch: {branch}");
        // Use a simple timestamp instead of chrono for in-memory implementation
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Ok(format!("merge_result_{timestamp}"))
    }

    /// Get history of commits
    pub async fn history(&self, _limit: Option<usize>) -> Result<Vec<String>, Box<dyn Error>> {
        Ok(vec!["Initial prolly tree commit".to_string()])
    }

    /// Get prolly tree statistics
    pub async fn tree_stats(&self) -> Result<ProllyTreeStats, Box<dyn Error>> {
        let tree = self.tree.read().await;

        // Get tree statistics using existing methods
        let key_count = tree.size();
        let stats = tree.stats();

        // Estimate total size from tree stats
        let total_size_bytes = (stats.avg_node_size * stats.num_nodes as f64) as usize;

        Ok(ProllyTreeStats {
            key_count,
            total_size_bytes,
            namespace_prefix: self.namespace_prefix.clone(),
        })
    }

    /// Get the underlying tree (for advanced operations)
    pub async fn with_tree<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&ProllyTree<32, InMemoryNodeStorage<32>>) -> R,
    {
        let tree = self.tree.read().await;
        f(&tree)
    }

    /// Perform a range query on the prolly tree
    pub async fn range_query(
        &self,
        start_key: &str,
        end_key: &str,
    ) -> Result<Vec<(String, Vec<u8>)>, Box<dyn Error>> {
        let start_key_bytes = self.full_key(start_key);
        let end_key_bytes = self.full_key(end_key);
        let namespace_prefix_with_slash = format!("{}/", self.namespace_prefix);
        let tree = self.tree.read().await;

        // Get all entries and filter by range
        let all_keys = tree.collect_keys();

        // Use HashSet to deduplicate keys and then process
        let unique_keys: std::collections::HashSet<Vec<u8>> = all_keys.into_iter().collect();
        let mut result = Vec::new();

        for key_bytes in unique_keys {
            if key_bytes >= start_key_bytes && key_bytes < end_key_bytes {
                if let Some(node) = tree.find(&key_bytes) {
                    // Find the value in the node
                    if let Some(index) = node.keys.iter().position(|k| k == &key_bytes) {
                        let value = node.values[index].clone();
                        if let Ok(key_str) = String::from_utf8(key_bytes) {
                            if let Some(relative_key) =
                                key_str.strip_prefix(&namespace_prefix_with_slash)
                            {
                                result.push((relative_key.to_string(), value));
                            }
                        }
                    }
                }
            }
        }

        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }
}

/// Statistics about the prolly tree
#[derive(Debug, Clone)]
pub struct ProllyTreeStats {
    pub key_count: usize,
    pub total_size_bytes: usize,
    pub namespace_prefix: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_prolly_persistence_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let mut persistence = InMemoryPersistence::init(temp_dir.path(), "test_memories").unwrap();

        // Test save
        let key = "test_key";
        let data = b"test_data";
        persistence.save(key, data).await.unwrap();

        // Test load
        let loaded = persistence.load(key).await.unwrap();
        assert_eq!(loaded, Some(data.to_vec()));

        // Test update
        let new_data = b"updated_data";
        persistence.save(key, new_data).await.unwrap();
        let loaded = persistence.load(key).await.unwrap();
        assert_eq!(loaded, Some(new_data.to_vec()));

        // Test delete
        persistence.delete(key).await.unwrap();
        let loaded = persistence.load(key).await.unwrap();
        assert_eq!(loaded, None);
    }

    #[tokio::test]
    async fn test_prolly_persistence_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let mut persistence = InMemoryPersistence::init(temp_dir.path(), "test_memories").unwrap();

        // Save some data
        persistence.save("key1", b"data1").await.unwrap();
        persistence.save("key2", b"data2").await.unwrap();

        // Create checkpoint
        let commit_id = persistence.checkpoint("Test checkpoint").await.unwrap();
        assert!(commit_id.starts_with("prolly_commit_"));
    }

    #[tokio::test]
    async fn test_prolly_persistence_list_keys() {
        let temp_dir = TempDir::new().unwrap();
        let mut persistence = InMemoryPersistence::init(temp_dir.path(), "test_memories").unwrap();

        // Save data with different prefixes
        persistence.save("user/1", b"user1").await.unwrap();
        persistence.save("user/2", b"user2").await.unwrap();
        persistence.save("system/config", b"config").await.unwrap();

        // List keys with prefix
        let user_keys = persistence.list_keys("user").await.unwrap();
        assert_eq!(user_keys.len(), 2);
        assert!(user_keys.contains(&"user/1".to_string()));
        assert!(user_keys.contains(&"user/2".to_string()));

        let system_keys = persistence.list_keys("system").await.unwrap();
        assert_eq!(system_keys.len(), 1);
        assert!(system_keys.contains(&"system/config".to_string()));
    }

    #[tokio::test]
    async fn test_prolly_persistence_stats() {
        let temp_dir = TempDir::new().unwrap();
        let mut persistence = InMemoryPersistence::init(temp_dir.path(), "test_memories").unwrap();

        // Add some data
        persistence.save("key1", b"data1").await.unwrap();
        persistence
            .save("key2", b"longer_data_value")
            .await
            .unwrap();

        // Get stats
        let stats = persistence.tree_stats().await.unwrap();
        assert_eq!(stats.key_count, 2);
        assert!(stats.total_size_bytes > 0);
        assert_eq!(stats.namespace_prefix, "test_memories");
    }

    #[tokio::test]
    async fn test_prolly_persistence_range_query() {
        let temp_dir = TempDir::new().unwrap();
        let mut persistence = InMemoryPersistence::init(temp_dir.path(), "test_memories").unwrap();

        // Add some data with sortable keys
        persistence.save("key_a", b"data_a").await.unwrap();
        persistence.save("key_b", b"data_b").await.unwrap();
        persistence.save("key_c", b"data_c").await.unwrap();
        persistence.save("other_x", b"data_x").await.unwrap();

        // Range query
        let results = persistence.range_query("key_", "key_z").await.unwrap();
        assert_eq!(results.len(), 3);

        // Should be sorted
        assert_eq!(results[0].0, "key_a");
        assert_eq!(results[1].0, "key_b");
        assert_eq!(results[2].0, "key_c");
    }
}
