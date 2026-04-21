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
use gix::prelude::*;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;

const DEFAULT_CACHE_SIZE: NonZeroUsize = NonZeroUsize::new(1000).unwrap();
use parking_lot::Mutex;
use std::sync::Arc;

use super::{NodeStorage, StorageError};

/// Git-backed storage for ProllyTree nodes
///
/// This storage implementation uses Git blobs to store serialized ProllyNode instances.
/// Each node is stored as a Git blob object, with the blob's SHA-1 hash serving as the
/// node's content-addressable identifier.
#[derive(Debug)]
pub struct GitNodeStorage<const N: usize> {
    _repository: Arc<Mutex<gix::Repository>>,
    cache: Mutex<LruCache<ValueDigest<N>, Arc<ProllyNode<N>>>>,
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
            cache: Mutex::new(LruCache::new(DEFAULT_CACHE_SIZE)),
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

    /// Get a copy of the current hash mappings
    pub fn get_hash_mappings(&self) -> HashMap<ValueDigest<N>, gix::ObjectId> {
        self.hash_to_object_id.lock().clone()
    }

    /// Merge additional hash mappings into this storage instance.
    ///
    /// This is used by [`NamespacedKvStore`] to consolidate hash mappings from
    /// namespace subtrees into the main storage before committing, so that
    /// `save_tree_config_to_git` writes all mappings.
    pub fn merge_hash_mappings(&self, other_mappings: HashMap<ValueDigest<N>, gix::ObjectId>) {
        let mut map = self.hash_to_object_id.lock();
        map.extend(other_mappings);
    }

    /// Create a new GitNodeStorage instance
    pub fn new(
        repository: gix::Repository,
        dataset_dir: std::path::PathBuf,
    ) -> Result<Self, GitKvError> {
        let cache_size = DEFAULT_CACHE_SIZE; // Default cache size

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
        let cache_size = NonZeroUsize::new(cache_size).unwrap_or(DEFAULT_CACHE_SIZE);

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
        let cache_size = DEFAULT_CACHE_SIZE; // Default cache size

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
        let repo = self._repository.lock();
        let blob = gix::objs::Blob { data: serialized };
        let blob_id = repo
            .objects
            .write(&blob)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to write blob: {e}")))?;

        Ok(blob_id)
    }

    /// Load a node from a Git blob
    fn load_node_from_blob(&self, blob_id: &gix::ObjectId) -> Result<ProllyNode<N>, GitKvError> {
        let repo = self._repository.lock();

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
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<Arc<ProllyNode<N>>> {
        // First check cache
        if let Some(node) = self.cache.lock().peek(hash) {
            return Some(Arc::clone(node));
        }

        // Check if we have a mapping for this hash
        let object_id = self.hash_to_object_id.lock().get(hash).cloned()?;

        // Load from Git blob and wrap in Arc
        let node = Arc::new(self.load_node_from_blob(&object_id).ok()?);

        // Cache the Arc for future lookups
        self.cache.lock().put(hash.clone(), Arc::clone(&node));

        Some(node)
    }

    fn insert_node(
        &mut self,
        hash: ValueDigest<N>,
        node: ProllyNode<N>,
    ) -> Result<(), StorageError> {
        // Store in cache
        self.cache.lock().put(hash.clone(), Arc::new(node.clone()));

        // Store as Git blob (this is durable — the blob lives in the git object db)
        let blob_id = self
            .store_node_as_blob(&node)
            .map_err(|e| StorageError::Other(e.to_string()))?;

        // Record the mapping in memory. We used to also append it to
        // `dataset_dir/prolly_hash_mappings` on every new insert, but doing so
        // added each transient intermediate node (including ones produced by
        // rebalances during `reload_tree_from_head`) to the working-tree file in
        // unsorted append order. The result: `git status` spuriously reported
        // the file as modified after any checkout, and cross-branch narrowing
        // via `git reset` dropped mappings that merge later needed (GH-161, GH-162).
        // The canonical on-disk snapshot is written atomically by
        // `save_tree_config_to_git` at commit time, in sorted order, from the
        // full in-memory map — so the per-insert write is redundant.
        self.hash_to_object_id.lock().insert(hash.clone(), blob_id);

        Ok(())
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Result<(), StorageError> {
        // Remove from cache
        self.cache.lock().pop(hash);

        // Remove from mapping
        self.hash_to_object_id.lock().remove(hash);

        // Note: Git doesn't really "delete" objects - they become unreachable
        // and will be garbage collected eventually.
        Ok(())
    }

    fn save_config(&self, key: &str, config: &[u8]) {
        // Store config in memory
        let mut configs = self.configs.lock();
        configs.insert(key.to_string(), config.to_vec());

        // The `tree_config` key is the canonical serialized TreeConfig used during
        // merge/checkout/commit. Its on-disk form lives in
        // `dataset_dir/prolly_config_tree_config` and is written by
        // `save_tree_config_to_git` with `serde_json::to_string_pretty`. The compact
        // `serde_json::to_vec` bytes handed in here are a byte-different
        // serialization of the same logical config — writing them would leave the
        // working tree file out of sync with what the last commit recorded and
        // cause `git status` to spuriously report the file as modified (see GH-161).
        // Skip the on-disk write for `tree_config` and keep only the in-memory copy.
        if key == "tree_config" {
            return;
        }

        let config_path = self.dataset_dir.join(format!("prolly_config_{key}"));
        let _ = std::fs::write(config_path, config);
    }

    fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        // First try to get from memory
        if let Some(config) = self.configs.lock().get(key).cloned() {
            return Some(config);
        }

        // If not in memory, try to load from filesystem
        let config_path = self.dataset_dir.join(format!("prolly_config_{key}"));
        if let Ok(config) = std::fs::read(config_path) {
            // Cache in memory for future use
            self.configs.lock().insert(key.to_string(), config.clone());
            return Some(config);
        }

        None
    }
}

impl<const N: usize> GitNodeStorage<N> {
    /// Load hash mappings from filesystem
    fn load_hash_mappings(&self) {
        let mapping_path = self.dataset_dir.join("prolly_hash_mappings");

        if let Ok(mappings) = std::fs::read_to_string(mapping_path) {
            let mut hash_map = self.hash_to_object_id.lock();

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
        storage.insert_node(hash.clone(), node.clone()).unwrap();

        // Test get
        let retrieved = storage.get_node_by_hash(&hash);
        assert!(retrieved.is_some());

        let retrieved_node = retrieved.unwrap();
        assert_eq!(retrieved_node.keys, node.keys);
        assert_eq!(retrieved_node.values, node.values);
        assert_eq!(retrieved_node.is_leaf, node.is_leaf);

        // Test delete
        storage.delete_node(&hash).unwrap();
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
        storage.insert_node(hash1.clone(), node1.clone()).unwrap();
        assert!(storage.cache.lock().contains(&hash1));

        // Get from cache
        let cached = storage.get_node_by_hash(&hash1);
        assert!(cached.is_some());
    }
}
