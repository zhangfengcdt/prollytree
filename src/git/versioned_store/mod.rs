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

mod backends;
mod core;
mod history;
#[cfg(test)]
mod tests;
mod thread_safe;

use crate::git::types::*;
use crate::storage::{FileNodeStorage, InMemoryNodeStorage, NodeStorage};
use crate::tree::ProllyTree;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use crate::storage::GitNodeStorage;

#[cfg(feature = "rocksdb_storage")]
use crate::storage::RocksDBNodeStorage;

/// Trait for accessing historical state from version control
pub trait HistoricalAccess<const N: usize> {
    /// Get all key-value pairs at a specific reference (commit, branch, etc.)
    fn get_keys_at_ref(&self, reference: &str) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError>;
}

/// Trait for accessing commit history and tracking changes to specific keys
pub trait TreeConfigSaver<const N: usize> {
    fn save_tree_config_to_git_internal(&self) -> Result<(), GitKvError>;
}

pub trait HistoricalCommitAccess<const N: usize> {
    /// Get all commits that contain changes to a specific key
    /// Returns commits in reverse chronological order (newest first)
    fn get_commits_for_key(&self, key: &[u8]) -> Result<Vec<CommitInfo>, GitKvError>;

    /// Get the commit history for the repository
    /// Returns commits in reverse chronological order (newest first)
    fn get_commit_history(&self) -> Result<Vec<CommitInfo>, GitKvError>;
}

/// A versioned key-value store backed by Git and ProllyTree with configurable storage
///
/// This combines the efficient tree operations of ProllyTree with Git's
/// version control capabilities, providing a full-featured versioned
/// key-value store with branching, merging, and history.
pub struct VersionedKvStore<const N: usize, S: NodeStorage<N>> {
    pub(crate) tree: ProllyTree<N, S>,
    pub(crate) git_repo: gix::Repository,
    pub(crate) staging_area: HashMap<Vec<u8>, Option<Vec<u8>>>, // None = deleted
    pub(crate) current_branch: String,
    pub(crate) storage_backend: StorageBackend,
    /// Dataset directory for storing config and other metadata for all backends
    /// (File/RocksDB/InMemory/Git). Git init/open paths set this from
    /// `storage.dataset_dir()`, and commit()/merge rely on this field being `Some`.
    pub(crate) dataset_dir: Option<std::path::PathBuf>,
}

/// Type alias for backward compatibility (Git storage)
pub type GitVersionedKvStore<const N: usize> = VersionedKvStore<N, GitNodeStorage<N>>;

/// Type alias for InMemory storage
pub type InMemoryVersionedKvStore<const N: usize> = VersionedKvStore<N, InMemoryNodeStorage<N>>;

/// Type alias for File storage
pub type FileVersionedKvStore<const N: usize> = VersionedKvStore<N, FileNodeStorage<N>>;

/// Type alias for RocksDB storage
#[cfg(feature = "rocksdb_storage")]
pub type RocksDBVersionedKvStore<const N: usize> = VersionedKvStore<N, RocksDBNodeStorage<N>>;

/// Thread-safe wrapper for VersionedKvStore
///
/// This wrapper provides thread-safe access to the underlying VersionedKvStore by using
/// `Arc<Mutex<>>` internally. All operations are synchronized, making it safe to use
/// across multiple threads.
///
/// Uses `parking_lot::Mutex` which does not support lock poisoning. If a thread panics
/// while holding the lock, subsequent lock acquisitions will succeed and the data
/// remains accessible. This is intentional: the previous `std::sync::Mutex` approach
/// also panicked (via `.unwrap()`) on poisoned locks, so callers had no recovery path
/// either way. With `parking_lot`, the application at least has a chance to continue.
pub struct ThreadSafeVersionedKvStore<const N: usize, S: NodeStorage<N>> {
    pub(crate) inner: Arc<Mutex<VersionedKvStore<N, S>>>,
}

/// Type alias for thread-safe Git storage
pub type ThreadSafeGitVersionedKvStore<const N: usize> =
    ThreadSafeVersionedKvStore<N, GitNodeStorage<N>>;

/// Type alias for thread-safe InMemory storage
pub type ThreadSafeInMemoryVersionedKvStore<const N: usize> =
    ThreadSafeVersionedKvStore<N, InMemoryNodeStorage<N>>;

/// Type alias for thread-safe File storage
pub type ThreadSafeFileVersionedKvStore<const N: usize> =
    ThreadSafeVersionedKvStore<N, FileNodeStorage<N>>;

/// Type alias for thread-safe RocksDB storage
#[cfg(feature = "rocksdb_storage")]
pub type ThreadSafeRocksDBVersionedKvStore<const N: usize> =
    ThreadSafeVersionedKvStore<N, RocksDBNodeStorage<N>>;
