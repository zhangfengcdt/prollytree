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

use super::traits::MemoryPersistence;
use crate::git::{GitKvError, ThreadSafeGitVersionedKvStore};
use async_trait::async_trait;
use std::error::Error;
use std::path::Path;
use std::sync::Arc;

/// Thread-safe ProllyTree-based memory persistence using git-backed versioned storage
///
/// This is a thread-safe wrapper around the ProllyMemoryPersistence that can be
/// safely used in multi-threaded contexts. It uses Arc<Mutex<>> internally to
/// ensure thread safety while maintaining the same interface.
pub struct ThreadSafeVersionedPersistence {
    store: Arc<ThreadSafeGitVersionedKvStore<32>>,
    namespace_prefix: String,
}

impl ThreadSafeVersionedPersistence {
    /// Initialize a new thread-safe prolly tree-based memory persistence layer with git backend
    pub fn init<P: AsRef<Path>>(path: P, namespace_prefix: &str) -> Result<Self, Box<dyn Error>> {
        let store = ThreadSafeGitVersionedKvStore::init(path)?;
        Ok(Self {
            store: Arc::new(store),
            namespace_prefix: namespace_prefix.to_string(),
        })
    }

    /// Open an existing thread-safe prolly tree-based memory persistence layer
    pub fn open<P: AsRef<Path>>(path: P, namespace_prefix: &str) -> Result<Self, Box<dyn Error>> {
        let store = ThreadSafeGitVersionedKvStore::open(path)?;
        Ok(Self {
            store: Arc::new(store),
            namespace_prefix: namespace_prefix.to_string(),
        })
    }

    /// Get the full key with namespace prefix
    fn full_key(&self, key: &str) -> String {
        format!("{}:{}", self.namespace_prefix, key)
    }

    /// Get access to the underlying ThreadSafeGitVersionedKvStore (for git operations)
    pub fn git_store(&self) -> Arc<ThreadSafeGitVersionedKvStore<32>> {
        self.store.clone()
    }
}

#[async_trait]
impl MemoryPersistence for ThreadSafeVersionedPersistence {
    async fn save(&mut self, key: &str, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let full_key = self.full_key(key);

        // Save to git-backed prolly tree
        self.store.insert(full_key.into_bytes(), data.to_vec())?;

        Ok(())
    }

    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        let full_key = self.full_key(key);

        let data = self.store.get(full_key.as_bytes());
        Ok(data)
    }

    async fn delete(&mut self, key: &str) -> Result<(), Box<dyn Error>> {
        let full_key = self.full_key(key);

        // Delete from git-backed prolly tree
        self.store.delete(full_key.as_bytes())?;

        Ok(())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let full_prefix = self.full_key(prefix);

        // Get all keys from git-backed store and filter by prefix
        let all_keys = self.store.list_keys()?;
        let filtered_keys: Vec<String> = all_keys
            .into_iter()
            .filter_map(|key_bytes| {
                let key_str = String::from_utf8(key_bytes).ok()?;
                if key_str.starts_with(&full_prefix) {
                    // Remove the namespace prefix from returned keys
                    key_str
                        .strip_prefix(&format!("{}:", self.namespace_prefix))
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(filtered_keys)
    }

    async fn checkpoint(&mut self, message: &str) -> Result<String, Box<dyn Error>> {
        // Create a git commit with the provided message
        let commit_id = self.store.commit(message)?;

        Ok(format!("{commit_id}"))
    }
}

impl ThreadSafeVersionedPersistence {
    /// Create a new branch (git branch)
    pub async fn create_branch(&self, name: &str) -> Result<(), Box<dyn Error>> {
        self.store.create_branch(name)?;
        Ok(())
    }

    /// Switch to a different branch
    pub async fn checkout_branch(&self, name: &str) -> Result<(), Box<dyn Error>> {
        self.store.checkout(name)?;
        Ok(())
    }

    /// Get git log history
    pub async fn get_git_log(&self) -> Result<Vec<crate::git::CommitInfo>, GitKvError> {
        self.store.log()
    }

    /// Get memory statistics including git information
    pub async fn get_stats(&self) -> Result<ThreadSafeProllyMemoryStats, Box<dyn Error>> {
        // Get git log to count commits
        let commits = self.store.log().unwrap_or_default();
        let commit_count = commits.len();

        // Get current branch info
        let current_branch = self
            .store
            .current_branch()
            .unwrap_or_else(|_| "main".to_string());

        // Count total keys with our namespace
        let all_keys = self.store.list_keys()?;
        let namespace_keys: Vec<_> = all_keys
            .into_iter()
            .filter(|key| {
                String::from_utf8_lossy(key).starts_with(&format!("{}:", self.namespace_prefix))
            })
            .collect();

        Ok(ThreadSafeProllyMemoryStats {
            total_keys: namespace_keys.len(),
            namespace_prefix: self.namespace_prefix.clone(),
            commit_count,
            current_branch,
            latest_commit: commits.first().map(|c| c.id.to_string()),
        })
    }
}

/// Statistics about thread-safe ProllyTree memory persistence
#[derive(Debug, Clone)]
pub struct ThreadSafeProllyMemoryStats {
    pub total_keys: usize,
    pub namespace_prefix: String,
    pub commit_count: usize,
    pub current_branch: String,
    pub latest_commit: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use tempfile::TempDir;
    use tokio::runtime::Runtime;

    #[tokio::test]
    async fn test_thread_safe_prolly_memory_persistence_basic() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repository
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to initialize git repository");

        // Create a subdirectory for the dataset
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir(&dataset_dir).unwrap();

        let mut persistence =
            ThreadSafeVersionedPersistence::init(&dataset_dir, "test_memories").unwrap();

        // Test save
        let key = "test_key";
        let data = b"test_data";
        persistence.save(key, data).await.unwrap();

        // Test load
        let loaded = persistence.load(key).await.unwrap();
        assert_eq!(loaded, Some(data.to_vec()));

        // Test list keys
        let keys = persistence.list_keys("test").await.unwrap();
        assert!(keys.contains(&key.to_string()));
    }

    #[tokio::test]
    async fn test_thread_safe_prolly_memory_persistence_checkpoint() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repository
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to initialize git repository");

        // Create a subdirectory for the dataset
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir(&dataset_dir).unwrap();

        let mut persistence =
            ThreadSafeVersionedPersistence::init(&dataset_dir, "test_memories").unwrap();

        // Save some data
        persistence.save("key1", b"data1").await.unwrap();
        persistence.save("key2", b"data2").await.unwrap();

        // Create checkpoint
        let commit_id = persistence.checkpoint("Test checkpoint").await.unwrap();
        assert!(!commit_id.is_empty());

        // Verify we can get git log
        let git_log = persistence.get_git_log().await.unwrap();
        assert!(!git_log.is_empty());
        assert_eq!(git_log[0].message, "Test checkpoint");
    }

    #[test]
    fn test_thread_safe_prolly_memory_persistence_multithreaded() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repository
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to initialize git repository");

        // Create a subdirectory for the dataset
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir(&dataset_dir).unwrap();

        let persistence =
            Arc::new(ThreadSafeVersionedPersistence::init(&dataset_dir, "test_memories").unwrap());

        // Test concurrent access
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let persistence_clone = Arc::clone(&persistence);
                thread::spawn(move || {
                    let rt = Runtime::new().unwrap();
                    rt.block_on(async {
                        let key = format!("key{}", i);

                        // Note: We can't call save because it requires &mut self
                        // This demonstrates that the read operations work in multithreaded contexts
                        let loaded = persistence_clone.load(&key).await.unwrap();
                        assert_eq!(loaded, None); // Should be None since we didn't save
                    });
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }
}
