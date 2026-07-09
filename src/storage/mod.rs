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

pub mod externalize;
pub mod file;
#[cfg(feature = "git")]
pub mod git;
pub mod memory;

pub use file::FileNodeStorage;
#[cfg(feature = "git")]
pub use git::GitNodeStorage;
pub use memory::InMemoryNodeStorage;

#[cfg(feature = "rocksdb_storage")]
pub use crate::rocksdb::RocksDBNodeStorage;

use crate::digest::ValueDigest;
use crate::node::ProllyNode;
use std::fmt::{Display, Formatter, LowerHex};
use std::sync::Arc;
use thiserror::Error;

/// Error type for node storage operations.
#[derive(Error, Debug)]
pub enum StorageError {
    /// An I/O error occurred during a storage operation.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A serialization error occurred while encoding a value.
    #[error("Serialization encode error: {0}")]
    SerializationEncode(#[from] bincode::error::EncodeError),

    /// A deserialization error occurred while decoding bytes.
    #[error("Serialization decode error: {0}")]
    SerializationDecode(#[from] bincode::error::DecodeError),

    /// A storage-specific error with a descriptive message.
    #[error("Storage error: {0}")]
    Other(String),
}

/// A trait for storage of nodes in the ProllyTree.
///
/// This trait defines the necessary operations for managing the storage
/// of nodes within a ProllyTree. Implementors of this trait can provide
/// custom storage backends, such as in-memory storage, database storage,
/// or any other form of persistent storage.
///
/// # Type Parameters
///
/// - `N`: The size of the value digest.
pub trait NodeStorage<const N: usize>: Send + Sync + Clone {
    /// Retrieves a node from storage by its hash.
    ///
    /// Returns an `Arc<ProllyNode<N>>` to avoid cloning entire nodes on every
    /// read. Callers that only need to inspect the node can dereference the
    /// `Arc` cheaply. Callers that need a mutable copy can use
    /// [`Arc::unwrap_or_clone`].
    ///
    /// # Arguments
    ///
    /// * `hash` - A reference to the `ValueDigest` representing the hash of the node to retrieve.
    ///
    /// # Returns
    ///
    /// The node associated with the given hash, wrapped in an `Arc`.
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<Arc<ProllyNode<N>>>;

    /// Inserts a node into storage.
    ///
    /// # Arguments
    ///
    /// * `hash` - The `ValueDigest` representing the hash of the node to insert.
    /// * `node` - The node to insert into storage.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the node could not be persisted.
    fn insert_node(
        &mut self,
        hash: ValueDigest<N>,
        node: ProllyNode<N>,
    ) -> Result<(), StorageError>;

    /// Deletes a node from storage by its hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - A reference to the `ValueDigest` representing the hash of the node to delete.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the node could not be deleted.
    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Result<(), StorageError>;

    /// Saves a configuration value.
    fn save_config(&self, key: &str, config: &[u8]);

    /// Retrieves a configuration value.
    fn get_config(&self, key: &str) -> Option<Vec<u8>>;

    /// Flush any in-memory bookkeeping that backends need on disk for a fresh
    /// handle to read what this handle wrote.
    ///
    /// Default: no-op. Most backends (in-memory, file, rocksdb) place every
    /// write durably on `insert_node` / `save_config` already. `GitNodeStorage`
    /// is the exception: it persists each node as a git blob immediately, but
    /// its `prolly_hash → git_object_id` mapping lives only in memory until
    /// the higher-level commit machinery writes the canonical
    /// `prolly_hash_mappings` snapshot. Callers that intend a fresh process
    /// to reload the data they just wrote (e.g. an explicit `persist()` on
    /// a higher-level index) should call this after they finish writing.
    fn sync(&self) -> Result<(), StorageError> {
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Blob API — PR 0a foundation for large-value externalization.
    // -----------------------------------------------------------------------
    //
    // Blobs are arbitrary opaque byte sequences stored under a caller-chosen
    // content hash (typically `ValueDigest::new(&bytes)`). They are NOT
    // ProllyNodes — they're just raw bytes. The motivating use case is
    // externalizing values too large to fit inline in a leaf node: the leaf
    // stores `(key, hash)` and the value bytes live in a blob.
    //
    // PR 0a only adds the API + per-backend implementations. The threshold-
    // based externalization logic in `VersionedKvStore::insert` lands in a
    // follow-up PR.
    //
    // Default impls return `BlobNotSupported` so a backend that doesn't yet
    // implement them fails loudly rather than silently dropping data.

    /// Persist raw bytes under `hash`. Idempotent when the existing bytes
    /// match. If `hash` already exists with different bytes, implementations
    /// must report an error instead of accepting the mismatch.
    fn insert_blob(&mut self, hash: ValueDigest<N>, bytes: &[u8]) -> Result<(), StorageError> {
        let _ = (hash, bytes);
        Err(StorageError::Other(
            "blob storage not supported by this backend".into(),
        ))
    }

    /// Retrieve previously-stored bytes by `hash`. Returns `None` if not
    /// found.
    fn get_blob(&self, hash: &ValueDigest<N>) -> Option<Vec<u8>> {
        let _ = hash;
        None
    }

    /// Remove a blob by `hash`. Idempotent — missing blob is success.
    fn delete_blob(&mut self, hash: &ValueDigest<N>) -> Result<(), StorageError> {
        let _ = hash;
        Err(StorageError::Other(
            "blob storage not supported by this backend".into(),
        ))
    }

    /// Enumerate every blob currently stored. Used by
    /// [`crate::git::versioned_store::NamespacedKvStore::gc_blobs`] to
    /// identify orphans (blobs no longer referenced by any namespace tree).
    ///
    /// Default returns an empty list — appropriate for backends that don't
    /// support blob storage. Backends that implement `insert_blob` / `get_blob`
    /// / `delete_blob` must also implement this for GC to work.
    fn list_blobs(&self) -> Result<Vec<ValueDigest<N>>, StorageError> {
        Ok(Vec::new())
    }
}

impl<const N: usize> Display for ValueDigest<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl<const N: usize> LowerHex for ValueDigest<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
