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

use crate::git::storage::GitNodeStorage;
use crate::git::types::*;
use crate::git::versioned_store::TreeConfigSaver;
use crate::storage::NodeStorage;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// A thread-safe versioned key-value store backed by Git and ProllyTree
///
/// This wraps the non-thread-safe GitVersionedKvStore to provide thread-safe
/// access using Arc<Mutex<>>. While this serializes access, it allows the
/// store to be used in multi-threaded contexts.
pub struct ThreadSafeVersionedKvStore<const N: usize, S: NodeStorage<N>> {
    inner: Arc<Mutex<crate::git::versioned_store::VersionedKvStore<N, S>>>,
}

/// Type alias for thread-safe Git storage
pub type ThreadSafeGitVersionedKvStore<const N: usize> = ThreadSafeVersionedKvStore<N, GitNodeStorage<N>>;

impl<const N: usize> ThreadSafeGitVersionedKvStore<N> {
    /// Initialize a new thread-safe git-backed versioned key-value store
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = crate::git::versioned_store::GitVersionedKvStore::init(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    /// Open an existing thread-safe git-backed versioned key-value store
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = crate::git::versioned_store::GitVersionedKvStore::open(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    /// Switch to a different branch - Git-specific implementation
    pub fn checkout(&self, name: &str) -> Result<(), GitKvError> {
        let mut store = self.inner.lock().map_err(|_| {
            GitKvError::GitObjectError("Failed to acquire lock on store".to_string())
        })?;
        store.checkout(name)
    }
}

impl<const N: usize, S: NodeStorage<N>> ThreadSafeVersionedKvStore<N, S>
where
    crate::git::versioned_store::VersionedKvStore<N, S>: TreeConfigSaver<N>,
{
    /// Insert a key-value pair (stages the change)
    pub fn insert(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), GitKvError> {
        let mut store = self.inner.lock().map_err(|_| {
            GitKvError::GitObjectError("Failed to acquire lock on store".to_string())
        })?;
        store.insert(key, value)
    }

    /// Update an existing key-value pair (stages the change)
    pub fn update(&self, key: Vec<u8>, value: Vec<u8>) -> Result<bool, GitKvError> {
        let mut store = self.inner.lock().map_err(|_| {
            GitKvError::GitObjectError("Failed to acquire lock on store".to_string())
        })?;
        store.update(key, value)
    }

    /// Delete a key-value pair (stages the change)
    pub fn delete(&self, key: &[u8]) -> Result<bool, GitKvError> {
        let mut store = self.inner.lock().map_err(|_| {
            GitKvError::GitObjectError("Failed to acquire lock on store".to_string())
        })?;
        store.delete(key)
    }

    /// Get a value by key (checks staging area first, then committed data)
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let store = self.inner.lock().ok()?;
        store.get(key)
    }

    /// List all keys (includes staged changes)
    pub fn list_keys(&self) -> Result<Vec<Vec<u8>>, GitKvError> {
        let store = self.inner.lock().map_err(|_| {
            GitKvError::GitObjectError("Failed to acquire lock on store".to_string())
        })?;
        Ok(store.list_keys())
    }

    /// Show current staging area status
    pub fn status(&self) -> Result<Vec<(Vec<u8>, String)>, GitKvError> {
        let store = self.inner.lock().map_err(|_| {
            GitKvError::GitObjectError("Failed to acquire lock on store".to_string())
        })?;
        Ok(store.status())
    }

    /// Commit staged changes
    pub fn commit(&self, message: &str) -> Result<gix::ObjectId, GitKvError> {
        let mut store = self.inner.lock().map_err(|_| {
            GitKvError::GitObjectError("Failed to acquire lock on store".to_string())
        })?;
        store.commit(message)
    }

    /// Create a new branch
    pub fn create_branch(&self, name: &str) -> Result<(), GitKvError> {
        let mut store = self.inner.lock().map_err(|_| {
            GitKvError::GitObjectError("Failed to acquire lock on store".to_string())
        })?;
        store.create_branch(name)
    }


    /// Get commit history
    pub fn log(&self) -> Result<Vec<CommitInfo>, GitKvError> {
        let store = self.inner.lock().map_err(|_| {
            GitKvError::GitObjectError("Failed to acquire lock on store".to_string())
        })?;
        store.log()
    }

    /// Get current branch name
    pub fn current_branch(&self) -> Result<String, GitKvError> {
        let store = self.inner.lock().map_err(|_| {
            GitKvError::GitObjectError("Failed to acquire lock on store".to_string())
        })?;
        Ok(store.current_branch().to_string())
    }

    /// Get the underlying git repository reference
    pub fn git_repo(&self) -> Result<gix::Repository, GitKvError> {
        let store = self.inner.lock().map_err(|_| {
            GitKvError::GitObjectError("Failed to acquire lock on store".to_string())
        })?;
        Ok(store.git_repo().clone())
    }
}

impl<const N: usize, S: NodeStorage<N>> Clone for ThreadSafeVersionedKvStore<N, S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

// Implement Send and Sync for the thread-safe wrapper
unsafe impl<const N: usize, S: NodeStorage<N>> Send for ThreadSafeVersionedKvStore<N, S> where S: Send {}
unsafe impl<const N: usize, S: NodeStorage<N>> Sync for ThreadSafeVersionedKvStore<N, S> where S: Send {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_thread_safe_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let store = ThreadSafeGitVersionedKvStore::<32>::init(temp_dir.path()).unwrap();

        // Test basic operations
        store.insert(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        assert_eq!(store.get(b"key1"), Some(b"value1".to_vec()));

        // Commit changes
        store.commit("Initial commit").unwrap();

        // Update key
        store.update(b"key1".to_vec(), b"value2".to_vec()).unwrap();
        assert_eq!(store.get(b"key1"), Some(b"value2".to_vec()));
    }

    #[test]
    fn test_thread_safe_concurrent_access() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ThreadSafeGitVersionedKvStore::<32>::init(temp_dir.path()).unwrap());

        // Test concurrent reads and writes
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let store_clone = Arc::clone(&store);
                thread::spawn(move || {
                    let key = format!("key{}", i).into_bytes();
                    let value = format!("value{}", i).into_bytes();
                    store_clone.insert(key.clone(), value.clone()).unwrap();
                    assert_eq!(store_clone.get(&key), Some(value));
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all keys were inserted
        store.commit("Concurrent insertions").unwrap();
        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 10);
    }
}