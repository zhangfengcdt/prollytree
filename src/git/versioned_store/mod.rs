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

//! Versioned key-value store backed by Git and ProllyTree.
//!
//! # Async/Sync Architecture
//!
//! This module provides a **synchronous** API by design. All public methods on
//! [`VersionedKvStore`] and [`ThreadSafeVersionedKvStore`] are blocking because
//! the underlying Git object database operations (reads/writes via `gix`) perform
//! file I/O that cannot be made async without significant complexity.
//!
//! ## Using from async code
//!
//! When calling these APIs from an async context (e.g., a Tokio runtime), wrap
//! calls in [`tokio::task::spawn_blocking`] to avoid blocking the async executor:
//!
//! ```rust,ignore
//! let store = ThreadSafeGitVersionedKvStore::<32>::open(path)?;
//! let value = tokio::task::spawn_blocking({
//!     let store = store.clone();
//!     move || store.get(b"my-key")
//! }).await?;
//! ```
//!
//! [`ThreadSafeVersionedKvStore`] implements `Clone` (via `Arc`), `Send`, and
//! `Sync`, so it can be safely shared across `spawn_blocking` boundaries and
//! between async tasks.
//!
//! ## Thread safety
//!
//! [`ThreadSafeVersionedKvStore`] wraps the inner store in `Arc<parking_lot::Mutex<..>>`.
//! `parking_lot::Mutex` is used instead of `std::sync::Mutex` to avoid lock
//! poisoning — see the struct documentation for rationale.
//!
//! ## Integration points
//!
//! - **SQL layer** ([`crate::sql::ProllyStorage`]): Implements GlueSQL's async
//!   `Store`/`StoreMut`/`Transaction` traits by offloading sync store operations
//!   to `spawn_blocking`.
//! - **Python bindings** ([`crate::python`]): Bridges Python ↔ Rust ↔ async via
//!   `py.allow_threads()` + `tokio::runtime::Runtime::block_on()`.
//! - **CLI** (`git-prolly`): Creates a Tokio runtime in `main()` and uses
//!   `block_on()` for SQL commands only; all other commands run synchronously.

mod backends;
mod core;
mod history;
pub mod namespaced;
#[cfg(test)]
mod namespaced_tests;
#[cfg(test)]
mod tests;
mod thread_safe;

/// Global mutex to serialize CWD access in tests.
///
/// Many tests change the process-wide working directory via `CwdGuard`.
/// Without serialization, parallel tests race on `std::env::set_current_dir`
/// and fail intermittently. Both `tests.rs` and `namespaced_tests.rs` share
/// this mutex to prevent those races.
#[cfg(test)]
pub(super) static CWD_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(super) fn cwd_lock() -> &'static std::sync::Mutex<()> {
    CWD_LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

use crate::git::metadata::{GitMetadataBackend, MetadataBackend};
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

/// A versioned key-value store backed by Git and ProllyTree with configurable storage.
///
/// This combines the efficient tree operations of ProllyTree with
/// version control capabilities, providing a full-featured versioned
/// key-value store with branching, merging, and history.
///
/// The metadata backend `M` controls how version-control metadata
/// (commits, branches, history) is stored. The default is
/// [`GitMetadataBackend`] which uses a real git repository.
///
/// All methods are **synchronous** and may perform file I/O through the Git
/// object database. When calling from an async context, use the thread-safe
/// wrapper [`ThreadSafeVersionedKvStore`] with [`tokio::task::spawn_blocking`].
/// See the [module-level documentation](self) for details.
pub struct VersionedKvStore<
    const N: usize,
    S: NodeStorage<N>,
    M: MetadataBackend = GitMetadataBackend,
> {
    pub(crate) tree: ProllyTree<N, S>,
    pub(crate) metadata: M,
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

/// Thread-safe wrapper for [`VersionedKvStore`].
///
/// Provides thread-safe access to the underlying store via `Arc<Mutex<>>`.
/// All operations are synchronized, making it safe to use across multiple
/// threads and async tasks.
///
/// # Cloning
///
/// `Clone` produces a new handle to the **same** underlying store (via
/// `Arc::clone`). This is cheap and is the intended way to share a store
/// across `spawn_blocking` closures or async tasks.
///
/// # Mutex choice
///
/// Uses `parking_lot::Mutex` which does not support lock poisoning. If a
/// thread panics while holding the lock, subsequent lock acquisitions will
/// succeed and the data remains accessible. This is intentional: the
/// previous `std::sync::Mutex` approach also panicked (via `.unwrap()`) on
/// poisoned locks, so callers had no recovery path either way. With
/// `parking_lot`, the application at least has a chance to continue.
///
/// # Async usage
///
/// All methods are synchronous. When calling from an async context, clone
/// the store and move it into [`tokio::task::spawn_blocking`]:
///
/// ```rust,ignore
/// let store: ThreadSafeGitVersionedKvStore<32> = /* ... */;
/// let result = tokio::task::spawn_blocking({
///     let store = store.clone();
///     move || store.get(b"key")
/// }).await?;
/// ```
pub struct ThreadSafeVersionedKvStore<
    const N: usize,
    S: NodeStorage<N>,
    M: MetadataBackend = GitMetadataBackend,
> {
    pub(crate) inner: Arc<Mutex<VersionedKvStore<N, S, M>>>,
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

// ---------------------------------------------------------------------------
// Re-export namespaced types
// ---------------------------------------------------------------------------

pub use namespaced::{
    FileNamespacedKvStore, GitNamespacedKvStore, InMemoryNamespacedKvStore, MigrationReport,
    NamespaceEntry, NamespaceHandle, NamespacedKvStore, StoreFormatVersion,
    ThreadSafeGitNamespacedKvStore, ThreadSafeInMemoryNamespacedKvStore,
    ThreadSafeNamespacedKvStore, DEFAULT_NAMESPACE,
};

#[cfg(feature = "rocksdb_storage")]
pub use namespaced::RocksDBNamespacedKvStore;

// ---------------------------------------------------------------------------
// StoreFactory — simplified API for creating versioned stores
// ---------------------------------------------------------------------------

use std::path::Path;

/// Factory for creating versioned key-value stores with different backends.
///
/// `StoreFactory` provides a single entry-point for store construction,
/// hiding the generic type aliases behind descriptive method names.
///
/// # Examples
///
/// ```rust,ignore
/// use prollytree::git::versioned_store::StoreFactory;
///
/// // In-memory (volatile, fastest) — great for testing or caching
/// let store = StoreFactory::memory::<32>("/tmp/my-repo/data")?;
///
/// // Git-backed (persistent, versioned) — production use
/// let store = StoreFactory::git::<32>("/tmp/my-repo/data")?;
///
/// // Thread-safe Git-backed — multi-threaded / async access
/// let store = StoreFactory::git_threadsafe::<32>("/tmp/my-repo/data")?;
/// ```
pub struct StoreFactory;

impl StoreFactory {
    /// Create an **in-memory** versioned store (volatile, fastest).
    ///
    /// Data lives only in process memory and is lost on drop.
    /// Use for: testing, caching, ephemeral workloads.
    pub fn memory<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<InMemoryVersionedKvStore<N>, GitKvError> {
        InMemoryVersionedKvStore::init(path)
    }

    /// Open an existing **in-memory** versioned store.
    pub fn memory_open<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<InMemoryVersionedKvStore<N>, GitKvError> {
        InMemoryVersionedKvStore::open(path)
    }

    /// Create a **file-backed** versioned store (persistent, no git history).
    ///
    /// Nodes are stored as individual files on disk.
    /// Use for: simple persistence without version control.
    pub fn file<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<FileVersionedKvStore<N>, GitKvError> {
        FileVersionedKvStore::init(path)
    }

    /// Open an existing **file-backed** versioned store.
    pub fn file_open<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<FileVersionedKvStore<N>, GitKvError> {
        FileVersionedKvStore::open(path)
    }

    /// Create a **Git-backed** versioned store (persistent, versioned).
    ///
    /// Nodes are stored as Git blob objects with full version history.
    /// Use for: production workloads requiring versioning and branching.
    pub fn git<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<GitVersionedKvStore<N>, GitKvError> {
        GitVersionedKvStore::init(path)
    }

    /// Open an existing **Git-backed** versioned store.
    pub fn git_open<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<GitVersionedKvStore<N>, GitKvError> {
        GitVersionedKvStore::open(path)
    }

    /// Create a **thread-safe Git-backed** versioned store.
    ///
    /// Wraps the store in `Arc<Mutex<..>>` for safe multi-threaded access.
    /// Use for: multi-threaded applications, async runtimes, shared state.
    pub fn git_threadsafe<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<ThreadSafeGitVersionedKvStore<N>, GitKvError> {
        ThreadSafeGitVersionedKvStore::init(path)
    }

    /// Open an existing **thread-safe Git-backed** versioned store.
    pub fn git_threadsafe_open<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<ThreadSafeGitVersionedKvStore<N>, GitKvError> {
        ThreadSafeGitVersionedKvStore::open(path)
    }

    /// Create a **thread-safe in-memory** versioned store.
    ///
    /// Use for: multi-threaded testing or caching scenarios.
    pub fn memory_threadsafe<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<ThreadSafeInMemoryVersionedKvStore<N>, GitKvError> {
        ThreadSafeInMemoryVersionedKvStore::init(path)
    }

    /// Open an existing **thread-safe in-memory** versioned store.
    pub fn memory_threadsafe_open<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<ThreadSafeInMemoryVersionedKvStore<N>, GitKvError> {
        ThreadSafeInMemoryVersionedKvStore::open(path)
    }

    /// Create a **thread-safe file-backed** versioned store.
    ///
    /// Use for: multi-threaded applications with simple file persistence.
    pub fn file_threadsafe<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<ThreadSafeFileVersionedKvStore<N>, GitKvError> {
        ThreadSafeFileVersionedKvStore::init(path)
    }

    /// Open an existing **thread-safe file-backed** versioned store.
    pub fn file_threadsafe_open<const N: usize, P: AsRef<Path>>(
        path: P,
    ) -> Result<ThreadSafeFileVersionedKvStore<N>, GitKvError> {
        ThreadSafeFileVersionedKvStore::open(path)
    }
}
