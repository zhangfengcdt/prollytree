use async_trait::async_trait;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::traits::MemoryPersistence;

/// Simple in-memory persistence for demonstration
/// In production, this would be replaced with a proper prolly tree implementation
/// that handles the Send/Sync requirements properly
pub struct SimpleMemoryPersistence {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    namespace_prefix: String,
}

impl SimpleMemoryPersistence {
    /// Initialize a new simple memory persistence layer
    pub fn init<P: AsRef<Path>>(_path: P, namespace_prefix: &str) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            namespace_prefix: namespace_prefix.to_string(),
        })
    }

    /// Open an existing simple memory persistence layer (same as init for this implementation)
    pub fn open<P: AsRef<Path>>(_path: P, namespace_prefix: &str) -> Result<Self, Box<dyn Error>> {
        Self::init(_path, namespace_prefix)
    }

    /// Get the full key with namespace prefix
    fn full_key(&self, key: &str) -> String {
        format!("{}/{}", self.namespace_prefix, key)
    }
}

#[async_trait]
impl MemoryPersistence for SimpleMemoryPersistence {
    async fn save(&mut self, key: &str, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let full_key = self.full_key(key);
        let mut store = self.data.write().await;
        store.insert(full_key, data.to_vec());
        Ok(())
    }

    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        let full_key = self.full_key(key);
        let store = self.data.read().await;
        Ok(store.get(&full_key).cloned())
    }

    async fn delete(&mut self, key: &str) -> Result<(), Box<dyn Error>> {
        let full_key = self.full_key(key);
        let mut store = self.data.write().await;
        store.remove(&full_key);
        Ok(())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let full_prefix = format!("{}/{}", self.namespace_prefix, prefix);
        let store = self.data.read().await;

        let keys = store
            .keys()
            .filter_map(|k| {
                if k.starts_with(&full_prefix) {
                    k.strip_prefix(&format!("{}/", self.namespace_prefix))
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(keys)
    }

    async fn checkpoint(&mut self, message: &str) -> Result<String, Box<dyn Error>> {
        // For simple implementation, just return a mock commit ID
        let commit_id = format!("simple_commit_{}", chrono::Utc::now().timestamp());
        println!("Created checkpoint: {} - {}", commit_id, message);
        Ok(commit_id)
    }
}

/// Additional methods for the simple persistence
impl SimpleMemoryPersistence {
    /// Create a new branch (no-op for simple implementation)
    pub async fn create_branch(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        println!("Created branch: {name}");
        Ok(())
    }

    /// Switch to a branch or commit (no-op for simple implementation)
    pub async fn checkout(&mut self, branch_or_commit: &str) -> Result<(), Box<dyn Error>> {
        println!("Checked out: {branch_or_commit}");
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

    /// Merge another branch
    pub async fn merge(&mut self, branch: &str) -> Result<String, Box<dyn Error>> {
        println!("Merged branch: {branch}");
        Ok(format!("merge_result_{}", chrono::Utc::now().timestamp()))
    }

    /// Get history of commits
    pub async fn history(&self, _limit: Option<usize>) -> Result<Vec<String>, Box<dyn Error>> {
        Ok(vec!["Initial commit".to_string()])
    }

    /// Get data size for statistics
    pub async fn data_size(&self) -> usize {
        let store = self.data.read().await;
        store.values().map(|v| v.len()).sum()
    }

    /// Get key count
    pub async fn key_count(&self) -> usize {
        let store = self.data.read().await;
        store.len()
    }
}
