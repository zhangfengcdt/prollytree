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

use super::{
    CommitInfo, GitKvError, GitVersionedKvStore, ThreadSafeFileVersionedKvStore,
    ThreadSafeGitVersionedKvStore, ThreadSafeInMemoryVersionedKvStore, ThreadSafeVersionedKvStore,
    TreeConfigSaver, VersionedKvStore,
};
use crate::git::metadata::MetadataBackend;
use crate::storage::{FileNodeStorage, InMemoryNodeStorage, NodeStorage};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::Arc;

// ==============================================================================
// Thread-Safe Wrapper Implementation
// ==============================================================================

impl<const N: usize> ThreadSafeGitVersionedKvStore<N> {
    /// Initialize a new thread-safe git-backed versioned key-value store
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = GitVersionedKvStore::init(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    /// Open an existing thread-safe git-backed versioned key-value store
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = GitVersionedKvStore::open(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }
}

impl<const N: usize, S: NodeStorage<N>, M: MetadataBackend> ThreadSafeVersionedKvStore<N, S, M>
where
    VersionedKvStore<N, S, M>: TreeConfigSaver<N>,
{
    /// Insert a key-value pair (stages the change)
    pub fn insert(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), GitKvError> {
        let mut store = self.inner.lock();
        store.insert(key, value)
    }

    /// Update an existing key-value pair (stages the change)
    pub fn update(&self, key: Vec<u8>, value: Vec<u8>) -> Result<bool, GitKvError> {
        let mut store = self.inner.lock();
        store.update(key, value)
    }

    /// Delete a key-value pair (stages the change)
    pub fn delete(&self, key: &[u8]) -> Result<bool, GitKvError> {
        let mut store = self.inner.lock();
        store.delete(key)
    }

    /// Get a value by key (checks staging area first, then committed data)
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let store = self.inner.lock();
        store.get(key)
    }

    /// List all keys (includes staged changes)
    pub fn list_keys(&self) -> Result<Vec<Vec<u8>>, GitKvError> {
        let store = self.inner.lock();
        Ok(store.list_keys())
    }

    /// Show current staging area status
    pub fn status(&self) -> Result<Vec<(Vec<u8>, String)>, GitKvError> {
        let store = self.inner.lock();
        Ok(store.status())
    }

    /// Commit staged changes
    pub fn commit(&self, message: &str) -> Result<gix::ObjectId, GitKvError> {
        let mut store = self.inner.lock();
        store.commit(message)
    }

    /// Create a new branch
    pub fn create_branch(&self, name: &str) -> Result<(), GitKvError> {
        let mut store = self.inner.lock();
        store.create_branch(name)
    }

    /// Get commit history
    pub fn log(&self) -> Result<Vec<CommitInfo>, GitKvError> {
        let store = self.inner.lock();
        store.log()
    }

    /// Get current branch name
    pub fn current_branch(&self) -> Result<String, GitKvError> {
        let store = self.inner.lock();
        Ok(store.current_branch().to_string())
    }
}

impl<const N: usize> ThreadSafeGitVersionedKvStore<N> {
    /// Get the underlying git repository reference (creates a clone)
    pub fn git_repo(&self) -> Result<gix::Repository, GitKvError> {
        let store = self.inner.lock();
        Ok(store.git_repo().clone())
    }
    /// Switch to a different branch - Git-specific implementation
    pub fn checkout(&self, name: &str) -> Result<(), GitKvError> {
        let mut store = self.inner.lock();
        store.checkout(name)
    }
}

impl<const N: usize> ThreadSafeInMemoryVersionedKvStore<N> {
    /// Initialize a new thread-safe in-memory versioned key-value store
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = VersionedKvStore::<N, InMemoryNodeStorage<N>>::init(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    /// Open an existing thread-safe in-memory versioned key-value store
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = VersionedKvStore::<N, InMemoryNodeStorage<N>>::open(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }
}

impl<const N: usize> ThreadSafeFileVersionedKvStore<N> {
    /// Initialize a new thread-safe file-based versioned key-value store
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = VersionedKvStore::<N, FileNodeStorage<N>>::init(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    /// Open an existing thread-safe file-based versioned key-value store
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = VersionedKvStore::<N, FileNodeStorage<N>>::open(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }
}

impl<const N: usize, S: NodeStorage<N>, M: MetadataBackend> Clone
    for ThreadSafeVersionedKvStore<N, S, M>
{
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

// Implement Send and Sync for the thread-safe wrapper
unsafe impl<const N: usize, S: NodeStorage<N>, M: MetadataBackend> Send
    for ThreadSafeVersionedKvStore<N, S, M>
where
    S: Send,
    M: Send,
{
}
unsafe impl<const N: usize, S: NodeStorage<N>, M: MetadataBackend> Sync
    for ThreadSafeVersionedKvStore<N, S, M>
where
    S: Send,
    M: Send,
{
}
