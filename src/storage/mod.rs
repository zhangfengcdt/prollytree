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
    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) -> Option<()>;

    /// Deletes a node from storage by its hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - A reference to the `ValueDigest` representing the hash of the node to delete.
    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Option<()>;

    fn save_config(&self, key: &str, config: &[u8]);
    fn get_config(&self, key: &str) -> Option<Vec<u8>>;
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
