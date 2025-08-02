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

    async fn checkpoint(&mut self, _message: &str) -> Result<String, Box<dyn Error>> {
        let commit_id = self.next_commit_id().await;

        // For in-memory storage, we just generate a commit ID
        // In a real git-based implementation, this would create an actual commit
        Ok(commit_id)
    }
}

/// Additional methods specific to prolly tree persistence
impl InMemoryPersistence {
    /// Create a new branch (for in-memory, this is a no-op)
    pub async fn create_branch(&mut self, _name: &str) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Switch to a branch or commit (for in-memory, this is a no-op)
    pub async fn checkout(&mut self, _branch_or_commit: &str) -> Result<(), Box<dyn Error>> {
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
    pub async fn merge(&mut self, _branch: &str) -> Result<String, Box<dyn Error>> {
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
    use crate::agent::mem_store::BaseMemoryStore;
    use crate::agent::traits::MemoryStore;
    use crate::agent::types::*;
    use chrono::Utc;
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

    // ========================================================================
    // Tests for Thread-Safe Versioned Storage Backends
    // ========================================================================

    #[tokio::test]
    async fn test_thread_safe_git_backend_basic_operations() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repository
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to initialize git repository");

        // Create a subdirectory for the dataset (git-backed stores require subdirectories)
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir_all(&dataset_dir).unwrap();

        let mut store = BaseMemoryStore::init_with_thread_safe_git(
            &dataset_dir,
            "test_agent".to_string(),
            None,
        )
        .unwrap();

        // Create a test memory document
        let memory = create_test_memory("test1", "Hello Git Backend!");

        // Test store operation
        let memory_id = store.store(memory.clone()).await.unwrap();
        assert_eq!(memory_id, "test1");

        // Test retrieve operation
        let retrieved = store.get(&memory_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_memory = retrieved.unwrap();
        assert_eq!(retrieved_memory.id, memory_id);
        assert_eq!(retrieved_memory.content, memory.content);

        // Test update operation
        let mut updated_memory = memory.clone();
        updated_memory.content = serde_json::json!({"message": "Updated Git Backend!"});
        store
            .update(&memory_id, updated_memory.clone())
            .await
            .unwrap();

        let retrieved_updated = store.get(&memory_id).await.unwrap().unwrap();
        assert_eq!(retrieved_updated.content, updated_memory.content);

        // Test delete operation
        store.delete(&memory_id).await.unwrap();
        let deleted = store.get(&memory_id).await.unwrap();
        assert!(deleted.is_none());
    }

    #[tokio::test]
    async fn test_thread_safe_inmemory_backend_basic_operations() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repository for InMemory backend
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to initialize git repository");

        let mut store = BaseMemoryStore::init_with_thread_safe_inmemory(
            temp_dir.path(),
            "test_agent".to_string(),
            None,
        )
        .unwrap();

        // Create a test memory document
        let memory = create_test_memory("test2", "Hello InMemory Backend!");

        // Test store operation
        let memory_id = store.store(memory.clone()).await.unwrap();
        assert_eq!(memory_id, "test2");

        // Test retrieve operation
        let retrieved = store.get(&memory_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_memory = retrieved.unwrap();
        assert_eq!(retrieved_memory.id, memory_id);
        assert_eq!(retrieved_memory.content, memory.content);

        // Test multiple entries
        let memory2 = create_test_memory("test2_second", "Second memory");
        store.store(memory2).await.unwrap();

        // Test query functionality
        let query = MemoryQuery {
            namespace: None,
            memory_types: Some(vec![MemoryType::ShortTerm]),
            tags: None,
            time_range: None,
            text_query: None,
            semantic_query: None,
            limit: None,
            include_expired: false,
        };

        let results = store.query(query).await.unwrap();
        assert!(results.len() >= 2);
    }

    #[tokio::test]
    async fn test_thread_safe_file_backend_basic_operations() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repository for File backend
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to initialize git repository");

        let mut store = BaseMemoryStore::init_with_thread_safe_file(
            temp_dir.path(),
            "test_agent".to_string(),
            None,
        )
        .unwrap();

        // Create a test memory document
        let memory = create_test_memory("test3", "Hello File Backend!");

        // Test store operation
        let memory_id = store.store(memory.clone()).await.unwrap();
        assert_eq!(memory_id, "test3");

        // Test retrieve operation
        let retrieved = store.get(&memory_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_memory = retrieved.unwrap();
        assert_eq!(retrieved_memory.id, memory_id);
        assert_eq!(retrieved_memory.content, memory.content);

        // Test persistence across instances
        drop(store);

        // Reopen the store
        let store_reopened = BaseMemoryStore::open_with_thread_safe_file(
            temp_dir.path(),
            "test_agent".to_string(),
            None,
        )
        .unwrap();

        let retrieved_after_reopen = store_reopened.get(&memory_id).await.unwrap();
        assert!(retrieved_after_reopen.is_some());
        assert_eq!(retrieved_after_reopen.unwrap().content, memory.content);
    }

    #[tokio::test]
    async fn test_thread_safe_prolly_backend_basic_operations() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repository for Prolly backend
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to initialize git repository");

        // Create a subdirectory for the dataset (git-backed stores require subdirectories)
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir_all(&dataset_dir).unwrap();

        let mut store = BaseMemoryStore::init_with_thread_safe_prolly(
            &dataset_dir,
            "test_agent".to_string(),
            None,
        )
        .unwrap();

        // Create a test memory document
        let memory = create_test_memory("test4", "Hello Prolly Backend!");

        // Test store operation
        let memory_id = store.store(memory.clone()).await.unwrap();
        assert_eq!(memory_id, "test4");

        // Test retrieve operation
        let retrieved = store.get(&memory_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_memory = retrieved.unwrap();
        assert_eq!(retrieved_memory.id, memory_id);
        assert_eq!(retrieved_memory.content, memory.content);
    }

    #[tokio::test]
    async fn test_versioned_backend_checkpoint_operations() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repository
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to initialize git repository");

        // Create a subdirectory for the dataset (git-backed stores require subdirectories)
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir_all(&dataset_dir).unwrap();

        let mut store = BaseMemoryStore::init_with_thread_safe_git(
            &dataset_dir,
            "test_agent".to_string(),
            None,
        )
        .unwrap();

        // Store some memories
        let memory1 = create_test_memory("checkpoint_test1", "First memory for checkpoint");
        let memory2 = create_test_memory("checkpoint_test2", "Second memory for checkpoint");

        store.store(memory1).await.unwrap();
        store.store(memory2).await.unwrap();

        // Create a checkpoint
        let commit_id = store
            .commit("Test checkpoint with multiple memories")
            .await
            .unwrap();
        assert!(!commit_id.is_empty());

        // Verify memories are still accessible after checkpoint
        let retrieved1 = store.get("checkpoint_test1").await.unwrap();
        let retrieved2 = store.get("checkpoint_test2").await.unwrap();
        assert!(retrieved1.is_some());
        assert!(retrieved2.is_some());
    }

    #[tokio::test]
    async fn test_versioned_backend_branch_operations() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repository
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to initialize git repository");

        // Create a subdirectory for the dataset (git-backed stores require subdirectories)
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir_all(&dataset_dir).unwrap();

        let mut store = BaseMemoryStore::init_with_thread_safe_git(
            &dataset_dir,
            "test_agent".to_string(),
            None,
        )
        .unwrap();

        // Store initial memory
        let memory = create_test_memory("branch_test", "Initial memory");
        store.store(memory).await.unwrap();

        // Create initial commit
        let initial_commit = store.commit("Initial commit").await.unwrap();
        assert!(!initial_commit.is_empty());

        // Create a new branch
        store.create_branch("feature_branch").await.unwrap();

        // Switch to the new branch
        store.checkout("feature_branch").await.unwrap();

        // Add memory on the feature branch
        let feature_memory = create_test_memory("feature_test", "Feature branch memory");
        store.store(feature_memory).await.unwrap();

        let feature_commit = store.commit("Feature branch commit").await.unwrap();
        assert!(!feature_commit.is_empty());
        assert_ne!(initial_commit, feature_commit);

        // Verify memory exists on feature branch
        let retrieved = store.get("feature_test").await.unwrap();
        assert!(retrieved.is_some());

        // Switch back to main branch
        store.checkout("main").await.unwrap();

        // Verify feature memory doesn't exist on main branch
        let not_found = store.get("feature_test").await.unwrap();
        assert!(not_found.is_none());

        // But original memory should still exist
        let original = store.get("branch_test").await.unwrap();
        assert!(original.is_some());
    }

    #[tokio::test]
    async fn test_backend_performance_comparison() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repository
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to initialize git repository");

        // Test with different backends
        let backends = vec![
            ("Git", temp_dir.path().join("git")),
            ("InMemory", temp_dir.path().join("inmemory")),
            ("File", temp_dir.path().join("file")),
        ];

        for (backend_name, backend_path) in backends {
            std::fs::create_dir_all(&backend_path).unwrap();

            // Create subdirectory for git-backed stores
            let actual_path = if backend_name == "Git" {
                let dataset_dir = backend_path.join("dataset");
                std::fs::create_dir_all(&dataset_dir).unwrap();
                dataset_dir
            } else {
                backend_path
            };

            let start_time = std::time::Instant::now();

            let mut store = match backend_name {
                "Git" => BaseMemoryStore::init_with_thread_safe_git(
                    &actual_path,
                    "perf_test".to_string(),
                    None,
                )
                .unwrap(),
                "InMemory" => BaseMemoryStore::init_with_thread_safe_inmemory(
                    &actual_path,
                    "perf_test".to_string(),
                    None,
                )
                .unwrap(),
                "File" => BaseMemoryStore::init_with_thread_safe_file(
                    &actual_path,
                    "perf_test".to_string(),
                    None,
                )
                .unwrap(),
                _ => panic!("Unknown backend"),
            };

            // Store multiple memories (create them with matching agent_id)
            for i in 0..10 {
                let memory = create_test_memory_for_agent(
                    &format!("perf_test_{}", i),
                    &format!("Performance test memory {}", i),
                    "perf_test",
                );
                store.store(memory).await.unwrap();
            }

            // Commit changes
            store
                .commit(&format!("Performance test for {} backend", backend_name))
                .await
                .unwrap();

            let duration = start_time.elapsed();

            // Verify all memories were stored
            for i in 0..10 {
                let retrieved = store.get(&format!("perf_test_{}", i)).await.unwrap();
                assert!(retrieved.is_some());
            }
        }
    }

    #[tokio::test]
    async fn test_backend_error_handling() {
        let temp_dir = TempDir::new().unwrap();

        // Test Simple backend (no branch operations)
        let mut simple_store =
            BaseMemoryStore::init(temp_dir.path(), "test_agent".to_string(), None).unwrap();

        // Branch operations should fail on Simple backend
        let branch_result = simple_store.create_branch("test_branch").await;
        assert!(branch_result.is_err());

        let checkout_result = simple_store.checkout("main").await;
        assert!(checkout_result.is_err());

        // Initialize git repository for versioned backends
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to initialize git repository");

        // Test InMemory backend (branch operations should also fail)
        let inmemory_path = temp_dir.path().join("inmemory");
        std::fs::create_dir_all(&inmemory_path).unwrap();
        let mut inmemory_store = BaseMemoryStore::init_with_thread_safe_inmemory(
            &inmemory_path,
            "test_agent".to_string(),
            None,
        )
        .unwrap();

        let inmemory_branch_result = inmemory_store.create_branch("test_branch").await;
        assert!(inmemory_branch_result.is_err());

        // Test File backend (branch operations should also fail)
        let file_path = temp_dir.path().join("file");
        std::fs::create_dir_all(&file_path).unwrap();
        let mut file_store =
            BaseMemoryStore::init_with_thread_safe_file(&file_path, "test_agent".to_string(), None)
                .unwrap();

        let file_branch_result = file_store.create_branch("test_branch").await;
        assert!(file_branch_result.is_err());

        // Test Git backend (branch operations should succeed)
        let git_path = temp_dir.path().join("git");
        std::fs::create_dir_all(&git_path).unwrap();
        // Create subdirectory for git-backed store
        let git_dataset_dir = git_path.join("dataset");
        std::fs::create_dir_all(&git_dataset_dir).unwrap();
        let mut git_store = BaseMemoryStore::init_with_thread_safe_git(
            &git_dataset_dir,
            "test_agent".to_string(),
            None,
        )
        .unwrap();

        let git_branch_result = git_store.create_branch("test_branch").await;
        assert!(git_branch_result.is_ok());

        let git_checkout_result = git_store.checkout("test_branch").await;
        assert!(git_checkout_result.is_ok());
    }

    // Helper function to create test memory documents
    fn create_test_memory(id: &str, message: &str) -> MemoryDocument {
        create_test_memory_for_agent(id, message, "test_agent")
    }

    // Helper function to create test memory documents with specific agent_id
    fn create_test_memory_for_agent(id: &str, message: &str, agent_id: &str) -> MemoryDocument {
        MemoryDocument {
            id: id.to_string(),
            namespace: MemoryNamespace::new(agent_id.to_string(), MemoryType::ShortTerm),
            memory_type: MemoryType::ShortTerm,
            content: serde_json::json!({"message": message}),
            embeddings: None,
            metadata: MemoryMetadata {
                created_at: Utc::now(),
                updated_at: Utc::now(),
                agent_id: agent_id.to_string(),
                thread_id: Some("test_thread".to_string()),
                tags: vec![],
                ttl: None,
                access_count: 0,
                last_accessed: None,
                source: "test".to_string(),
                confidence: 1.0,
                related_memories: vec![],
            },
        }
    }
}
