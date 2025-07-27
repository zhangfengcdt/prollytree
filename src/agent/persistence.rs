use async_trait::async_trait;
use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::git::{GitKvError, GitVersionedKvStore};
use super::traits::{MemoryPersistence, MemoryError};

// Since GitVersionedKvStore doesn't implement Send/Sync due to gix::Repository limitations,
// we'll need to work around this. For now, let's use a simpler approach.

/// Prolly tree-based persistence for agent memory
pub struct ProllyMemoryPersistence {
    store: Arc<RwLock<GitVersionedKvStore<32>>>,
    namespace_prefix: String,
}

impl ProllyMemoryPersistence {
    /// Initialize a new prolly memory persistence layer
    pub fn init<P: AsRef<Path>>(path: P, namespace_prefix: &str) -> Result<Self, GitKvError> {
        let store = GitVersionedKvStore::<32>::init(path)?;
        Ok(Self {
            store: Arc::new(RwLock::new(store)),
            namespace_prefix: namespace_prefix.to_string(),
        })
    }

    /// Open an existing prolly memory persistence layer
    pub fn open<P: AsRef<Path>>(path: P, namespace_prefix: &str) -> Result<Self, GitKvError> {
        let store = GitVersionedKvStore::<32>::open(path)?;
        Ok(Self {
            store: Arc::new(RwLock::new(store)),
            namespace_prefix: namespace_prefix.to_string(),
        })
    }

    /// Get the full key with namespace prefix
    fn full_key(&self, key: &str) -> Vec<u8> {
        format!("{}/{}", self.namespace_prefix, key).into_bytes()
    }

    /// Convert GitKvError to Box<dyn Error>
    fn convert_error(err: GitKvError) -> Box<dyn Error> {
        Box::new(err) as Box<dyn Error>
    }
}

#[async_trait]
impl MemoryPersistence for ProllyMemoryPersistence {
    async fn save(&mut self, key: &str, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let mut store = self.store.write().await;
        let full_key = self.full_key(key);
        
        // Check if key exists to decide between insert and update
        match store.get(&full_key) {
            Some(_) => {
                store.update(full_key, data.to_vec())
                    .map_err(Self::convert_error)?;
            }
            None => {
                store.insert(full_key, data.to_vec())
                    .map_err(Self::convert_error)?;
            }
        }
        
        Ok(())
    }

    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        let store = self.store.read().await;
        let full_key = self.full_key(key);
        Ok(store.get(&full_key))
    }

    async fn delete(&mut self, key: &str) -> Result<(), Box<dyn Error>> {
        let mut store = self.store.write().await;
        let full_key = self.full_key(key);
        store.delete(&full_key).map_err(Self::convert_error)?;
        Ok(())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let store = self.store.read().await;
        let full_prefix = format!("{}/{}", self.namespace_prefix, prefix);
        let prefix_bytes = full_prefix.as_bytes();
        
        let keys = store.keys()
            .filter(|k| k.starts_with(prefix_bytes))
            .map(|k| {
                String::from_utf8_lossy(k)
                    .strip_prefix(&format!("{}/", self.namespace_prefix))
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        
        Ok(keys)
    }

    async fn checkpoint(&mut self, message: &str) -> Result<String, Box<dyn Error>> {
        let mut store = self.store.write().await;
        let commit_id = store.commit(message).map_err(Self::convert_error)?;
        Ok(commit_id.to_hex().to_string())
    }
}

/// Additional methods specific to git-based persistence
impl ProllyMemoryPersistence {
    /// Create a new branch
    pub async fn create_branch(&mut self, name: &str) -> Result<(), MemoryError> {
        let mut store = self.store.write().await;
        store.create_branch(name)
            .map_err(|e| MemoryError::StorageError(format!("Failed to create branch: {:?}", e)))
    }

    /// Switch to a branch or commit
    pub async fn checkout(&mut self, branch_or_commit: &str) -> Result<(), MemoryError> {
        let mut store = self.store.write().await;
        store.checkout(branch_or_commit)
            .map_err(|e| MemoryError::StorageError(format!("Failed to checkout: {:?}", e)))
    }

    /// Get current branch name
    pub async fn current_branch(&self) -> String {
        let store = self.store.read().await;
        store.current_branch().to_string()
    }

    /// List all branches
    pub async fn list_branches(&self) -> Result<Vec<String>, MemoryError> {
        let store = self.store.read().await;
        store.list_branches()
            .map_err(|e| MemoryError::StorageError(format!("Failed to list branches: {:?}", e)))
    }

    /// Get status of staged changes
    pub async fn status(&self) -> Vec<(Vec<u8>, String)> {
        let store = self.store.read().await;
        store.status()
    }

    /// Merge another branch
    pub async fn merge(&mut self, branch: &str) -> Result<String, MemoryError> {
        let mut store = self.store.write().await;
        let result = store.merge(branch)
            .map_err(|e| MemoryError::StorageError(format!("Failed to merge: {:?}", e)))?;
        Ok(format!("{:?}", result))
    }

    /// Get history of commits
    pub async fn history(&self, limit: Option<usize>) -> Result<Vec<String>, MemoryError> {
        // This would need to be implemented using git operations
        // For now, return a placeholder
        Ok(vec!["History not yet implemented".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_prolly_persistence_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let mut persistence = ProllyMemoryPersistence::init(
            temp_dir.path(),
            "test_memories"
        ).unwrap();

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
        let mut persistence = ProllyMemoryPersistence::init(
            temp_dir.path(),
            "test_memories"
        ).unwrap();

        // Save some data
        persistence.save("key1", b"data1").await.unwrap();
        persistence.save("key2", b"data2").await.unwrap();

        // Create checkpoint
        let commit_id = persistence.checkpoint("Test checkpoint").await.unwrap();
        assert!(!commit_id.is_empty());
    }

    #[tokio::test]
    async fn test_prolly_persistence_list_keys() {
        let temp_dir = TempDir::new().unwrap();
        let mut persistence = ProllyMemoryPersistence::init(
            temp_dir.path(),
            "test_memories"
        ).unwrap();

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
}