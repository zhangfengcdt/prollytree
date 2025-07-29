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
use gix::prelude::*;
use lru::LruCache;
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
    // Maps ProllyTree hashes to Git object IDs
    hash_to_object_id: Mutex<HashMap<ValueDigest<N>, gix::ObjectId>>,
    // Directory where this dataset's config and mapping files are stored
    dataset_dir: std::path::PathBuf,
}

impl<const N: usize> Clone for GitNodeStorage<N> {
    fn clone(&self) -> Self {
        let cloned = Self {
            _repository: self._repository.clone(),
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())),
            configs: Mutex::new(HashMap::new()),
            hash_to_object_id: Mutex::new(HashMap::new()),
            dataset_dir: self.dataset_dir.clone(),
        };

        // Load the hash mappings for the cloned instance
        cloned.load_hash_mappings();

        cloned
    }
}

impl<const N: usize> GitNodeStorage<N> {
    /// Get the dataset directory path
    pub fn dataset_dir(&self) -> &std::path::Path {
        &self.dataset_dir
    }

    /// Create a new GitNodeStorage instance
    pub fn new(
        repository: gix::Repository,
        dataset_dir: std::path::PathBuf,
    ) -> Result<Self, GitKvError> {
        let cache_size = NonZeroUsize::new(1000).unwrap(); // Default cache size

        let storage = GitNodeStorage {
            _repository: Arc::new(Mutex::new(repository)),
            cache: Mutex::new(LruCache::new(cache_size)),
            configs: Mutex::new(HashMap::new()),
            hash_to_object_id: Mutex::new(HashMap::new()),
            dataset_dir,
        };

        // Load existing hash mappings
        storage.load_hash_mappings();

        Ok(storage)
    }

    /// Create GitNodeStorage with custom cache size
    pub fn with_cache_size(
        repository: gix::Repository,
        dataset_dir: std::path::PathBuf,
        cache_size: usize,
    ) -> Result<Self, GitKvError> {
        let cache_size = NonZeroUsize::new(cache_size).unwrap_or(NonZeroUsize::new(1000).unwrap());

        let storage = GitNodeStorage {
            _repository: Arc::new(Mutex::new(repository)),
            cache: Mutex::new(LruCache::new(cache_size)),
            configs: Mutex::new(HashMap::new()),
            hash_to_object_id: Mutex::new(HashMap::new()),
            dataset_dir,
        };

        // Load existing hash mappings
        storage.load_hash_mappings();

        Ok(storage)
    }

    /// Create GitNodeStorage with pre-loaded hash mappings
    pub fn with_mappings(
        repository: gix::Repository,
        dataset_dir: std::path::PathBuf,
        hash_mappings: HashMap<ValueDigest<N>, gix::ObjectId>,
    ) -> Result<Self, GitKvError> {
        let cache_size = NonZeroUsize::new(1000).unwrap(); // Default cache size

        let storage = GitNodeStorage {
            _repository: Arc::new(Mutex::new(repository)),
            cache: Mutex::new(LruCache::new(cache_size)),
            configs: Mutex::new(HashMap::new()),
            hash_to_object_id: Mutex::new(hash_mappings),
            dataset_dir,
        };

        Ok(storage)
    }

    /// Store a node as a Git blob
    fn store_node_as_blob(&self, node: &ProllyNode<N>) -> Result<gix::ObjectId, GitKvError> {
        let serialized = bincode::serialize(node)?;

        // Write the serialized node as a Git blob
        let repo = self._repository.lock().unwrap();
        let blob = gix::objs::Blob { data: serialized };
        let blob_id = repo
            .objects
            .write(&blob)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to write blob: {e}")))?;

        Ok(blob_id)
    }

    /// Load a node from a Git blob
    fn load_node_from_blob(&self, blob_id: &gix::ObjectId) -> Result<ProllyNode<N>, GitKvError> {
        let repo = self._repository.lock().unwrap();

        // Find the blob object
        let mut buffer = Vec::new();
        let object = repo.objects.find(blob_id, &mut buffer).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to find blob {blob_id}: {e}"))
        })?;

        // Deserialize the blob data back to a ProllyNode
        let node: ProllyNode<N> =
            bincode::deserialize(object.data).map_err(GitKvError::SerializationError)?;

        Ok(node)
    }
}

impl<const N: usize> NodeStorage<N> for GitNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<ProllyNode<N>> {
        // First check cache
        if let Some(node) = self.cache.lock().unwrap().peek(hash) {
            return Some(node.clone());
        }

        // Check if we have a mapping for this hash
        let object_id = self.hash_to_object_id.lock().unwrap().get(hash).cloned()?;

        // Load from Git blob
        self.load_node_from_blob(&object_id).ok()
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) -> Option<()> {
        // Store in cache
        self.cache.lock().unwrap().put(hash.clone(), node.clone());

        // Store as Git blob
        match self.store_node_as_blob(&node) {
            Ok(blob_id) => {
                // Store the mapping between ProllyTree hash and Git object ID
                self.hash_to_object_id
                    .lock()
                    .unwrap()
                    .insert(hash.clone(), blob_id);

                // Persist the mapping to filesystem
                self.save_hash_mapping(&hash, &blob_id);

                Some(())
            }
            Err(_) => None,
        }
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Option<()> {
        // Remove from cache
        self.cache.lock().unwrap().pop(hash);

        // Remove from mapping
        self.hash_to_object_id.lock().unwrap().remove(hash);

        // Note: Git doesn't really "delete" objects - they become unreachable
        // and will be garbage collected eventually. For now, we'll just consider
        // this a successful operation.
        Some(())
    }

    fn save_config(&self, key: &str, config: &[u8]) {
        // Store config in memory
        let mut configs = self.configs.lock().unwrap();
        configs.insert(key.to_string(), config.to_vec());

        // Also persist to filesystem for durability in the dataset directory
        let config_path = self.dataset_dir.join(format!("prolly_config_{key}"));
        let _ = std::fs::write(config_path, config);
    }

    fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        // First try to get from memory
        if let Some(config) = self.configs.lock().unwrap().get(key).cloned() {
            return Some(config);
        }

        // If not in memory, try to load from filesystem
        let config_path = self.dataset_dir.join(format!("prolly_config_{key}"));
        if let Ok(config) = std::fs::read(config_path) {
            // Cache in memory for future use
            self.configs
                .lock()
                .unwrap()
                .insert(key.to_string(), config.clone());
            return Some(config);
        }

        None
    }
}

impl<const N: usize> GitNodeStorage<N> {
    /// Save hash mapping to filesystem
    fn save_hash_mapping(&self, hash: &ValueDigest<N>, object_id: &gix::ObjectId) {
        let mapping_path = self.dataset_dir.join("prolly_hash_mappings");

        // Read existing mappings
        let mut mappings = if mapping_path.exists() {
            std::fs::read_to_string(&mapping_path).unwrap_or_default()
        } else {
            String::new()
        };

        // Add new mapping - use simple format for now without hex dependency
        let hash_bytes: Vec<String> = hash.0.iter().map(|b| format!("{b:02x}")).collect();
        let hash_hex = hash_bytes.join("");
        let object_hex = object_id.to_hex().to_string();
        mappings.push_str(&format!("{hash_hex}:{object_hex}\n"));

        // Write back
        let _ = std::fs::write(mapping_path, mappings);
    }

    /// Load hash mappings from filesystem
    fn load_hash_mappings(&self) {
        let mapping_path = self.dataset_dir.join("prolly_hash_mappings");

        if let Ok(mappings) = std::fs::read_to_string(mapping_path) {
            let mut hash_map = self.hash_to_object_id.lock().unwrap();

            for line in mappings.lines() {
                if let Some((hash_hex, object_hex)) = line.split_once(':') {
                    // Parse hex string manually
                    if hash_hex.len() == N * 2 {
                        let mut hash_bytes = Vec::new();
                        for i in 0..N {
                            if let Ok(byte) = u8::from_str_radix(&hash_hex[i * 2..i * 2 + 2], 16) {
                                hash_bytes.push(byte);
                            } else {
                                break;
                            }
                        }

                        if hash_bytes.len() == N {
                            if let Ok(object_id) = gix::ObjectId::from_hex(object_hex.as_bytes()) {
                                let mut hash_array = [0u8; N];
                                hash_array.copy_from_slice(&hash_bytes);
                                let hash = ValueDigest(hash_array);
                                hash_map.insert(hash, object_id);
                            }
                        }
                    }
                }
            }
        }
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
        let (temp_dir, repo) = create_test_repo();
        let mut storage = GitNodeStorage::<32>::new(repo, temp_dir.path().to_path_buf()).unwrap();

        let node = create_test_node();
        let hash = node.get_hash();

        // Test insert
        assert!(storage.insert_node(hash.clone(), node.clone()).is_some());

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
        let (temp_dir, repo) = create_test_repo();
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir_all(&dataset_dir).unwrap();
        let mut storage = GitNodeStorage::<32>::with_cache_size(repo, dataset_dir, 2).unwrap();

        let node1 = create_test_node();
        let hash1 = node1.get_hash();

        // Insert and verify it's cached
        storage.insert_node(hash1.clone(), node1.clone());
        assert!(storage.cache.lock().unwrap().contains(&hash1));

        // Get from cache
        let cached = storage.get_node_by_hash(&hash1);
        assert!(cached.is_some());
    }
}
