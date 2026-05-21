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

use crate::digest::ValueDigest;
use crate::node::ProllyNode;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use super::{NodeStorage, StorageError};

/// An implementation of `NodeStorage` that stores nodes in a HashMap.
///
/// # Type Parameters
///
/// - `N`: The size of the value digest.
#[derive(Debug)]
pub struct InMemoryNodeStorage<const N: usize> {
    map: HashMap<ValueDigest<N>, Arc<ProllyNode<N>>>,
    configs: RwLock<HashMap<String, Vec<u8>>>,
    blobs: RwLock<HashMap<ValueDigest<N>, Arc<Vec<u8>>>>,
}

impl<const N: usize> Clone for InMemoryNodeStorage<N> {
    fn clone(&self) -> Self {
        InMemoryNodeStorage {
            map: self.map.clone(),
            configs: RwLock::new(self.configs.read().clone()),
            blobs: RwLock::new(self.blobs.read().clone()),
        }
    }
}

impl<const N: usize> Default for InMemoryNodeStorage<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> InMemoryNodeStorage<N> {
    pub fn new() -> Self {
        InMemoryNodeStorage {
            map: HashMap::new(),
            configs: RwLock::new(HashMap::new()),
            blobs: RwLock::new(HashMap::new()),
        }
    }
}

impl<const N: usize> NodeStorage<N> for InMemoryNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<Arc<ProllyNode<N>>> {
        self.map.get(hash).cloned()
    }

    fn insert_node(
        &mut self,
        hash: ValueDigest<N>,
        mut node: ProllyNode<N>,
    ) -> Result<(), StorageError> {
        // Clear transient flags before storing so reads never see stale state.
        node.split = false;
        node.merged = false;
        self.map.insert(hash, Arc::new(node));
        Ok(())
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Result<(), StorageError> {
        self.map.remove(hash);
        Ok(())
    }

    fn save_config(&self, key: &str, config: &[u8]) {
        self.configs
            .write()
            .insert(key.to_string(), config.to_vec());
    }

    fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        self.configs.read().get(key).cloned()
    }

    fn insert_blob(&mut self, hash: ValueDigest<N>, bytes: &[u8]) -> Result<(), StorageError> {
        // Content-addressed: if the hash is already present, leave it alone.
        // `entry().or_insert_with()` avoids an unnecessary `to_vec()` clone
        // when the blob already exists.
        let mut blobs = self.blobs.write();
        blobs
            .entry(hash)
            .or_insert_with(|| Arc::new(bytes.to_vec()));
        Ok(())
    }

    fn get_blob(&self, hash: &ValueDigest<N>) -> Option<Vec<u8>> {
        self.blobs.read().get(hash).map(|arc| (**arc).clone())
    }

    fn delete_blob(&mut self, hash: &ValueDigest<N>) -> Result<(), StorageError> {
        self.blobs.write().remove(hash);
        Ok(())
    }

    fn list_blobs(&self) -> Result<Vec<ValueDigest<N>>, StorageError> {
        Ok(self.blobs.read().keys().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &[u8]) -> ValueDigest<32> {
        ValueDigest::<32>::new(s)
    }

    #[test]
    fn blob_round_trip() {
        let mut s = InMemoryNodeStorage::<32>::new();
        let h = d(b"payload");
        s.insert_blob(h.clone(), b"payload").unwrap();
        assert_eq!(s.get_blob(&h).as_deref(), Some(b"payload" as &[u8]));
    }

    #[test]
    fn blob_get_missing_returns_none() {
        let s = InMemoryNodeStorage::<32>::new();
        assert!(s.get_blob(&d(b"never_written")).is_none());
    }

    #[test]
    fn blob_insert_idempotent() {
        let mut s = InMemoryNodeStorage::<32>::new();
        let h = d(b"v");
        s.insert_blob(h.clone(), b"v").unwrap();
        // Second insert with same hash should be a no-op (content-addressed).
        s.insert_blob(h.clone(), b"v").unwrap();
        assert_eq!(s.get_blob(&h).as_deref(), Some(b"v" as &[u8]));
    }

    #[test]
    fn blob_delete_then_get_is_none() {
        let mut s = InMemoryNodeStorage::<32>::new();
        let h = d(b"to-be-removed");
        s.insert_blob(h.clone(), b"to-be-removed").unwrap();
        s.delete_blob(&h).unwrap();
        assert!(s.get_blob(&h).is_none());
    }

    #[test]
    fn blob_delete_missing_is_ok() {
        let mut s = InMemoryNodeStorage::<32>::new();
        // Deleting a hash that was never inserted is a no-op success.
        assert!(s.delete_blob(&d(b"phantom")).is_ok());
    }

    #[test]
    fn blobs_are_isolated_from_nodes() {
        let mut s = InMemoryNodeStorage::<32>::new();
        let h = d(b"shared-hash");
        // Insert a blob at this hash.
        s.insert_blob(h.clone(), b"blob-bytes").unwrap();
        // Insert a node at the SAME hash. Nodes and blobs share the hash
        // space (content-addressed) but are stored in separate maps, so they
        // shouldn't shadow each other.
        let node = ProllyNode::<32>::default();
        s.insert_node(h.clone(), node).unwrap();

        assert!(s.get_node_by_hash(&h).is_some());
        assert!(s.get_blob(&h).is_some());
    }

    #[test]
    fn blobs_survive_clone() {
        let mut a = InMemoryNodeStorage::<32>::new();
        a.insert_blob(d(b"original"), b"original").unwrap();
        let b = a.clone();
        assert!(b.get_blob(&d(b"original")).is_some());
    }
}
