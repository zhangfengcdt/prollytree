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
use crate::storage::NodeStorage;
use lru::LruCache;
use rocksdb::{
    BlockBasedOptions, Cache, DBCompressionType, Options, SliceTransform, WriteBatch, DB,
};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const CONFIG_PREFIX: &[u8] = b"config:";
const NODE_PREFIX: &[u8] = b"node:";

/// RocksDB-backed storage for ProllyTree nodes
///
/// This storage implementation uses RocksDB as the persistent storage backend,
/// with an LRU cache for frequently accessed nodes to improve performance.
pub struct RocksDBNodeStorage<const N: usize> {
    db: Arc<DB>,
    cache: Arc<Mutex<LruCache<ValueDigest<N>, ProllyNode<N>>>>,
}

impl<const N: usize> Clone for RocksDBNodeStorage<N> {
    fn clone(&self) -> Self {
        RocksDBNodeStorage {
            db: self.db.clone(),
            cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap()))),
        }
    }
}

impl<const N: usize> RocksDBNodeStorage<N> {
    /// Create a new RocksDBNodeStorage instance with default options
    pub fn new(path: PathBuf) -> Result<Self, rocksdb::Error> {
        let opts = Self::default_options();
        let db = DB::open(&opts, path)?;

        Ok(RocksDBNodeStorage {
            db: Arc::new(db),
            cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(1000).unwrap(),
            ))),
        })
    }

    /// Create RocksDBNodeStorage with custom options
    pub fn with_options(path: PathBuf, opts: Options) -> Result<Self, rocksdb::Error> {
        let db = DB::open(&opts, path)?;

        Ok(RocksDBNodeStorage {
            db: Arc::new(db),
            cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(1000).unwrap(),
            ))),
        })
    }

    /// Create RocksDBNodeStorage with custom cache size
    pub fn with_cache_size(path: PathBuf, cache_size: usize) -> Result<Self, rocksdb::Error> {
        let opts = Self::default_options();
        let db = DB::open(&opts, path)?;

        Ok(RocksDBNodeStorage {
            db: Arc::new(db),
            cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(cache_size).unwrap_or(NonZeroUsize::new(1000).unwrap()),
            ))),
        })
    }

    /// Get default RocksDB options optimized for ProllyTree
    pub fn default_options() -> Options {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Optimize for ProllyTree's write-heavy workload
        opts.set_write_buffer_size(128 * 1024 * 1024); // 128MB memtable
        opts.set_max_write_buffer_number(4);
        opts.set_min_write_buffer_number_to_merge(2);

        // Enable compression for storage efficiency
        opts.set_compression_type(DBCompressionType::Lz4);
        opts.set_bottommost_compression_type(DBCompressionType::Zstd);

        // Bloom filters for faster lookups
        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_bloom_filter(10.0, false);

        // Block cache for frequently accessed nodes
        let cache = Cache::new_lru_cache(512 * 1024 * 1024); // 512MB block cache
        block_opts.set_block_cache(&cache);

        opts.set_block_based_table_factory(&block_opts);

        // Use prefix extractor for efficient scans
        let prefix_len = NODE_PREFIX.len() + N;
        opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(prefix_len));

        opts
    }

    /// Create a key for storing a node
    fn node_key(hash: &ValueDigest<N>) -> Vec<u8> {
        let mut key = Vec::with_capacity(NODE_PREFIX.len() + N);
        key.extend_from_slice(NODE_PREFIX);
        key.extend_from_slice(&hash.0);
        key
    }

    /// Create a key for storing config
    fn config_key(key: &str) -> Vec<u8> {
        let mut result = Vec::with_capacity(CONFIG_PREFIX.len() + key.len());
        result.extend_from_slice(CONFIG_PREFIX);
        result.extend_from_slice(key.as_bytes());
        result
    }
}

impl<const N: usize> NodeStorage<N> for RocksDBNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<ProllyNode<N>> {
        // Check cache first
        if let Some(node) = self.cache.lock().unwrap().get(hash) {
            return Some(node.clone());
        }

        // Fetch from RocksDB
        let key = Self::node_key(hash);
        match self.db.get(&key) {
            Ok(Some(data)) => {
                match bincode::deserialize::<ProllyNode<N>>(&data) {
                    Ok(mut node) => {
                        // Reset transient flags
                        node.split = false;
                        node.merged = false;

                        // Update cache
                        self.cache.lock().unwrap().put(hash.clone(), node.clone());

                        Some(node)
                    }
                    Err(_) => None,
                }
            }
            _ => None,
        }
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) -> Option<()> {
        // Update cache
        self.cache.lock().unwrap().put(hash.clone(), node.clone());

        // Serialize and store in RocksDB
        match bincode::serialize(&node) {
            Ok(data) => {
                let key = Self::node_key(&hash);
                match self.db.put(&key, data) {
                    Ok(_) => Some(()),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Option<()> {
        // Remove from cache
        self.cache.lock().unwrap().pop(hash);

        // Delete from RocksDB
        let key = Self::node_key(hash);
        match self.db.delete(&key) {
            Ok(_) => Some(()),
            Err(_) => None,
        }
    }

    fn save_config(&self, key: &str, config: &[u8]) {
        let db_key = Self::config_key(key);
        let _ = self.db.put(&db_key, config);
    }

    fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        let db_key = Self::config_key(key);
        self.db.get(&db_key).ok().flatten()
    }
}

/// Batch operations for RocksDBNodeStorage
impl<const N: usize> RocksDBNodeStorage<N> {
    /// Insert multiple nodes in a single batch operation
    pub fn batch_insert_nodes(
        &mut self,
        nodes: Vec<(ValueDigest<N>, ProllyNode<N>)>,
    ) -> Result<(), rocksdb::Error> {
        let mut batch = WriteBatch::default();
        let mut cache = self.cache.lock().unwrap();

        for (hash, node) in nodes {
            // Update cache
            cache.put(hash.clone(), node.clone());

            // Add to batch
            match bincode::serialize(&node) {
                Ok(data) => {
                    let key = Self::node_key(&hash);
                    batch.put(&key, data);
                }
                Err(_) => {
                    // Skip this entry if serialization fails
                    continue;
                }
            }
        }

        self.db.write(batch)
    }

    /// Delete multiple nodes in a single batch operation
    pub fn batch_delete_nodes(&mut self, hashes: &[ValueDigest<N>]) -> Result<(), rocksdb::Error> {
        let mut batch = WriteBatch::default();
        let mut cache = self.cache.lock().unwrap();

        for hash in hashes {
            // Remove from cache
            cache.pop(hash);

            // Add to batch
            let key = Self::node_key(hash);
            batch.delete(&key);
        }

        self.db.write(batch)
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<(), rocksdb::Error> {
        self.db.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TreeConfig;
    use tempfile::TempDir;

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
    fn test_rocksdb_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = RocksDBNodeStorage::<32>::new(temp_dir.path().to_path_buf()).unwrap();

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
        assert!(storage.get_node_by_hash(&hash).is_none());
    }

    #[test]
    fn test_config_operations() {
        let temp_dir = TempDir::new().unwrap();
        let storage = RocksDBNodeStorage::<32>::new(temp_dir.path().to_path_buf()).unwrap();

        let config_data = b"test config data";
        storage.save_config("test_key", config_data);

        let retrieved = storage.get_config("test_key");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), config_data);

        // Test non-existent config
        assert!(storage.get_config("non_existent").is_none());
    }

    #[test]
    fn test_batch_operations() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = RocksDBNodeStorage::<32>::new(temp_dir.path().to_path_buf()).unwrap();

        // Create multiple nodes
        let mut nodes = Vec::new();
        for i in 0..10 {
            let mut node = create_test_node();
            node.keys[0] = format!("key{}", i).into_bytes();
            let hash = node.get_hash();
            nodes.push((hash, node));
        }

        // Batch insert
        let hashes: Vec<_> = nodes.iter().map(|(h, _)| h.clone()).collect();
        assert!(storage.batch_insert_nodes(nodes.clone()).is_ok());

        // Verify all inserted
        for (hash, _) in &nodes {
            assert!(storage.get_node_by_hash(hash).is_some());
        }

        // Batch delete
        assert!(storage.batch_delete_nodes(&hashes).is_ok());

        // Verify all deleted
        for hash in &hashes {
            assert!(storage.get_node_by_hash(hash).is_none());
        }
    }

    #[test]
    fn test_cache_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage =
            RocksDBNodeStorage::<32>::with_cache_size(temp_dir.path().to_path_buf(), 2).unwrap();

        let node1 = create_test_node();
        let hash1 = node1.get_hash();

        // Insert and verify it's cached
        storage.insert_node(hash1.clone(), node1.clone());

        // Accessing should be from cache (we can't directly test this, but it should be fast)
        assert!(storage.get_node_by_hash(&hash1).is_some());
    }
}