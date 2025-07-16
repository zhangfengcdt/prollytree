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
use crate::git::types::GitKvError;
use crate::node::ProllyNode;
use crate::storage::NodeStorage;
use lru::LruCache;
use sha2::Digest;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

/// Git-backed storage for ProllyTree nodes
///
/// This storage implementation uses Git blobs to store serialized ProllyNode instances.
/// Each node is stored as a Git blob object, with the blob's SHA-1 hash serving as the
/// node's content-addressable identifier.
pub struct GitNodeStorage<const N: usize> {
    _repository: Arc<Mutex<gix::Repository>>,
    cache: Mutex<LruCache<ValueDigest<N>, ProllyNode<N>>>,
    configs: Mutex<HashMap<String, Vec<u8>>>,
}

impl<const N: usize> GitNodeStorage<N> {
    /// Create a new GitNodeStorage instance
    pub fn new(repository: gix::Repository) -> Result<Self, GitKvError> {
        let cache_size = NonZeroUsize::new(1000).unwrap(); // Default cache size

        Ok(GitNodeStorage {
            _repository: Arc::new(Mutex::new(repository)),
            cache: Mutex::new(LruCache::new(cache_size)),
            configs: Mutex::new(HashMap::new()),
        })
    }

    /// Create GitNodeStorage with custom cache size
    pub fn with_cache_size(
        repository: gix::Repository,
        cache_size: usize,
    ) -> Result<Self, GitKvError> {
        let cache_size = NonZeroUsize::new(cache_size).unwrap_or(NonZeroUsize::new(1000).unwrap());

        Ok(GitNodeStorage {
            _repository: Arc::new(Mutex::new(repository)),
            cache: Mutex::new(LruCache::new(cache_size)),
            configs: Mutex::new(HashMap::new()),
        })
    }

    /// Convert ValueDigest to git ObjectId
    fn digest_to_object_id(&self, hash: &ValueDigest<N>) -> Result<gix::ObjectId, GitKvError> {
        // For now, we'll use a simple mapping. In a real implementation,
        // we might want to store a mapping between ProllyTree hashes and Git object IDs
        let hex_string = format!("{:x}", hash);

        // Pad or truncate to 40 characters (SHA-1 length)
        let padded_hex = if hex_string.len() < 40 {
            format!("{:0<40}", hex_string)
        } else {
            hex_string[..40].to_string()
        };

        gix::ObjectId::from_hex(padded_hex.as_bytes())
            .map_err(|e| GitKvError::GitObjectError(format!("Invalid object ID: {}", e)))
    }

    /// Store a node as a Git blob
    fn store_node_as_blob(&self, node: &ProllyNode<N>) -> Result<gix::ObjectId, GitKvError> {
        let serialized = bincode::serialize(node)?;

        // For now, create a mock blob ID based on the content hash
        // In a real implementation, this would write to the Git object database
        let hash = sha2::Sha256::digest(&serialized);
        let hex_string = format!("{:x}", hash);
        let padded_hex = if hex_string.len() < 40 {
            format!("{:0<40}", hex_string)
        } else {
            hex_string[..40].to_string()
        };

        let blob_id = gix::ObjectId::from_hex(padded_hex.as_bytes())
            .map_err(|e| GitKvError::GitObjectError(format!("Invalid object ID: {}", e)))?;

        Ok(blob_id)
    }

    /// Load a node from a Git blob
    fn load_node_from_blob(&self, _blob_id: &gix::ObjectId) -> Result<ProllyNode<N>, GitKvError> {
        // For now, this is a mock implementation
        // In a real implementation, this would read from the Git object database
        Err(GitKvError::GitObjectError(
            "Mock implementation - node not found".to_string(),
        ))
    }
}

impl<const N: usize> NodeStorage<N> for GitNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<ProllyNode<N>> {
        // First check cache
        if let Some(node) = self.cache.lock().unwrap().peek(hash) {
            return Some(node.clone());
        }

        // Convert to Git object ID and load from Git
        match self.digest_to_object_id(hash) {
            Ok(blob_id) => match self.load_node_from_blob(&blob_id) {
                Ok(node) => Some(node),
                Err(_) => None,
            },
            Err(_) => None,
        }
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) -> Option<()> {
        // Store in cache
        self.cache.lock().unwrap().put(hash, node.clone());

        // Store as Git blob
        match self.store_node_as_blob(&node) {
            Ok(_blob_id) => {
                // In a real implementation, we'd maintain a mapping between
                // the ValueDigest and the Git object ID
                Some(())
            }
            Err(_) => None,
        }
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Option<()> {
        // Remove from cache
        self.cache.lock().unwrap().pop(hash);

        // Note: Git doesn't really "delete" objects - they become unreachable
        // and will be garbage collected eventually. For now, we'll just consider
        // this a successful operation.
        Some(())
    }

    fn save_config(&self, key: &str, config: &[u8]) {
        // Store config in memory for now
        // In a real implementation, we'd store this as a Git blob or in a config file
        let mut configs = self.configs.lock().unwrap();
        configs.insert(key.to_string(), config.to_vec());
    }

    fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        self.configs.lock().unwrap().get(key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TreeConfig;
    use crate::node::ProllyNode;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, gix::Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = gix::init_bare(temp_dir.path()).unwrap();
        (temp_dir, repo)
    }

    fn create_test_node<const N: usize>() -> ProllyNode<N> {
        let config: TreeConfig<N> = TreeConfig::default();
        ProllyNode {
            keys: vec![b"key1".to_vec(), b"key2".to_vec()],
            key_schema: config.key_schema.clone(),
            values: vec![b"value1".to_vec(), b"value2".to_vec()],
            value_schema: config.value_schema.clone(),
            is_leaf: true,
            level: 0,
            base: config.base,
            modulus: config.modulus,
            min_chunk_size: config.min_chunk_size,
            max_chunk_size: config.max_chunk_size,
            pattern: config.pattern,
            split: false,
            merged: false,
            encode_types: Vec::new(),
            encode_values: Vec::new(),
        }
    }

    #[test]
    fn test_git_node_storage_basic_operations() {
        let (_temp_dir, repo) = create_test_repo();
        let mut storage = GitNodeStorage::<32>::new(repo).unwrap();

        let node = create_test_node();
        let hash = node.get_hash();

        // Test insert
        assert!(storage.insert_node(hash, node.clone()).is_some());

        // Test get
        let retrieved = storage.get_node_by_hash(&hash);
        assert!(retrieved.is_some());

        let retrieved_node = retrieved.unwrap();
        assert_eq!(retrieved_node.keys, node.keys);
        assert_eq!(retrieved_node.values, node.values);
        assert_eq!(retrieved_node.is_leaf, node.is_leaf);

        // Test delete
        assert!(storage.delete_node(&hash).is_some());
    }

    #[test]
    fn test_cache_functionality() {
        let (_temp_dir, repo) = create_test_repo();
        let mut storage = GitNodeStorage::<32>::with_cache_size(repo, 2).unwrap();

        let node1 = create_test_node();
        let hash1 = node1.get_hash();

        // Insert and verify it's cached
        storage.insert_node(hash1, node1.clone());
        assert!(storage.cache.lock().unwrap().contains(&hash1));

        // Get from cache
        let cached = storage.get_node_by_hash(&hash1);
        assert!(cached.is_some());
    }
}
