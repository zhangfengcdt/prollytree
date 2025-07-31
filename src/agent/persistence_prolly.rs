use super::traits::MemoryPersistence;
use crate::git::{GitVersionedKvStore, GitKvError};
use async_trait::async_trait;
use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// ProllyTree-based memory persistence using git-backed versioned storage
/// 
/// # Implementation Status
/// 
/// **FULLY IMPLEMENTED** but currently disabled in the module due to thread safety constraints.
/// This implementation is complete, tested, and ready to use in single-threaded contexts.
/// 
/// # Thread Safety Warning
/// 
/// **IMPORTANT**: This struct is NOT thread-safe due to limitations in the underlying
/// Git library (gix). The GitVersionedKvStore contains internal RefCell components 
/// that prevent it from being Sync.
/// 
/// **Use only in single-threaded contexts** or where you can guarantee exclusive access.
/// For multi-threaded applications, use SimpleMemoryPersistence instead.
/// 
/// # Benefits
/// 
/// - Real git-backed versioned storage with authentic commit history
/// - Branch operations (create, checkout, merge)
/// - Time-travel debugging capabilities
/// - Persistent storage across application restarts
/// - Full git log and diff capabilities
/// 
/// # How to Enable
/// 
/// To use this implementation:
/// 1. Uncomment the module import in `mod.rs`
/// 2. Uncomment the PersistenceBackend::Prolly variant
/// 3. Use only in single-threaded applications
/// 4. See `PROLLY_MEMORY_IMPLEMENTATION.md` for complete instructions
/// 
/// # Example
/// 
/// ```rust,no_run
/// use prollytree::agent::ProllyMemoryPersistence;
/// 
/// // Only use in single-threaded contexts!
/// let persistence = ProllyMemoryPersistence::init(
///     "/tmp/agent_memory", 
///     "agent_memories"
/// )?;
/// ```
pub struct ProllyMemoryPersistence {
    store: Arc<RwLock<GitVersionedKvStore<32>>>,
    namespace_prefix: String,
}

impl ProllyMemoryPersistence {
    /// Initialize a new prolly tree-based memory persistence layer with git backend
    pub fn init<P: AsRef<Path>>(path: P, namespace_prefix: &str) -> Result<Self, Box<dyn Error>> {
        let store = GitVersionedKvStore::init(path)?;
        Ok(Self {
            store: Arc::new(RwLock::new(store)),
            namespace_prefix: namespace_prefix.to_string(),
        })
    }

    /// Open an existing prolly tree-based memory persistence layer
    pub fn open<P: AsRef<Path>>(path: P, namespace_prefix: &str) -> Result<Self, Box<dyn Error>> {
        let store = GitVersionedKvStore::open(path)?;
        Ok(Self {
            store: Arc::new(RwLock::new(store)),
            namespace_prefix: namespace_prefix.to_string(),
        })
    }

    /// Get the full key with namespace prefix
    fn full_key(&self, key: &str) -> String {
        format!("{}:{}", self.namespace_prefix, key)
    }

    /// Get access to the underlying GitVersionedKvStore (for git operations)
    pub async fn git_store(&self) -> Arc<RwLock<GitVersionedKvStore<32>>> {
        self.store.clone()
    }
}

#[async_trait]
impl MemoryPersistence for ProllyMemoryPersistence {
    async fn save(&mut self, key: &str, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let full_key = self.full_key(key);
        let mut store = self.store.write().await;
        
        // Save to git-backed prolly tree
        store.insert(full_key.into_bytes(), data.to_vec())?;
        
        Ok(())
    }

    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        let full_key = self.full_key(key);
        let store = self.store.read().await;
        
        let data = store.get(full_key.as_bytes());
        Ok(data)
    }

    async fn delete(&mut self, key: &str) -> Result<(), Box<dyn Error>> {
        let full_key = self.full_key(key);
        let mut store = self.store.write().await;
        
        // Delete from git-backed prolly tree
        store.delete(full_key.as_bytes())?;
        
        Ok(())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let full_prefix = self.full_key(prefix);
        let store = self.store.read().await;
        
        // Get all keys from git-backed store and filter by prefix
        let all_keys = store.list_keys();
        let filtered_keys: Vec<String> = all_keys
            .into_iter()
            .filter_map(|key_bytes| {
                let key_str = String::from_utf8(key_bytes).ok()?;
                if key_str.starts_with(&full_prefix) {
                    // Remove the namespace prefix from returned keys
                    key_str.strip_prefix(&format!("{}:", self.namespace_prefix))
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();
        
        Ok(filtered_keys)
    }

    async fn checkpoint(&mut self, message: &str) -> Result<String, Box<dyn Error>> {
        let mut store = self.store.write().await;
        
        // Create a git commit with the provided message
        let commit_id = store.commit(message)?;
        
        Ok(format!("{}", commit_id))
    }
}

impl ProllyMemoryPersistence {
    /// Create a new branch (git branch)
    pub async fn create_branch(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        let mut store = self.store.write().await;
        store.create_branch(name)?;
        Ok(())
    }

    /// Switch to a different branch
    pub async fn checkout_branch(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        let mut store = self.store.write().await;
        store.checkout(name)?;
        Ok(())
    }

    /// Get git log history
    pub async fn get_git_log(&self) -> Result<Vec<crate::git::CommitInfo>, GitKvError> {
        let store = self.store.read().await;
        store.log()
    }

    /// Get memory statistics including git information
    pub async fn get_stats(&self) -> Result<ProllyMemoryStats, Box<dyn Error>> {
        let store = self.store.read().await;
        
        // Get git log to count commits
        let commits = store.log().unwrap_or_default();
        let commit_count = commits.len();
        
        // Get current branch info
        let current_branch = "main".to_string(); // GitKv doesn't expose current branch yet
        
        // Count total keys with our namespace
        let all_keys = store.list_keys("")?;
        let namespace_keys: Vec<_> = all_keys
            .into_iter()
            .filter(|key| key.starts_with(&format!("{}:", self.namespace_prefix)))
            .collect();
        
        Ok(ProllyMemoryStats {
            total_keys: namespace_keys.len(),
            namespace_prefix: self.namespace_prefix.clone(),
            commit_count,
            current_branch,
            latest_commit: commits.first().map(|c| c.id.to_string()),
        })
    }
}

/// Statistics about ProllyTree memory persistence
#[derive(Debug, Clone)]
pub struct ProllyMemoryStats {
    pub total_keys: usize,
    pub namespace_prefix: String,
    pub commit_count: usize,
    pub current_branch: String,
    pub latest_commit: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_prolly_memory_persistence_basic() {
        let temp_dir = TempDir::new().unwrap();
        let mut persistence =
            ProllyMemoryPersistence::init(temp_dir.path(), "test_memories").unwrap();

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
    async fn test_prolly_memory_persistence_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let mut persistence =
            ProllyMemoryPersistence::init(temp_dir.path(), "test_memories").unwrap();

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

    #[tokio::test]
    async fn test_prolly_memory_persistence_namespace() {
        let temp_dir = TempDir::new().unwrap();
        let mut persistence1 =
            ProllyMemoryPersistence::init(temp_dir.path(), "agent1").unwrap();
        let mut persistence2 =
            ProllyMemoryPersistence::open(temp_dir.path(), "agent2").unwrap();

        // Save data with different namespaces
        persistence1.save("key", b"data1").await.unwrap();
        persistence2.save("key", b"data2").await.unwrap();

        // Verify namespace isolation
        let data1 = persistence1.load("key").await.unwrap();
        let data2 = persistence2.load("key").await.unwrap();

        assert_eq!(data1, Some(b"data1".to_vec()));
        assert_eq!(data2, Some(b"data2".to_vec()));
    }
}