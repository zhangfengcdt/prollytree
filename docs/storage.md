# ProllyTree Storage Backends Guide

ProllyTree supports multiple storage backends to meet different performance, persistence, and deployment requirements. This guide provides a comprehensive overview of each available storage backend, their characteristics, use cases, and configuration options.

## Overview

ProllyTree uses a pluggable storage architecture through the `NodeStorage` trait, allowing you to choose the appropriate backend for your specific needs:

- **InMemoryNodeStorage**: Fast, volatile storage for development and testing
- **FileNodeStorage**: Simple file-based persistence for local applications
- **RocksDBNodeStorage**: High-performance LSM-tree storage for production workloads
- **GitNodeStorage**: Git object store integration for development (experimental)

## InMemoryNodeStorage

### Description
The in-memory storage backend keeps all ProllyTree nodes in a `HashMap` in RAM. This provides the fastest access times but offers no persistence across application restarts.

### Characteristics
- **Performance**: Fastest read/write operations
- **Persistence**: None - data is lost when application terminates
- **Memory Usage**: Entire tree stored in RAM
- **Concurrency**: Thread-safe with internal locking
- **Storage Overhead**: Minimal (just HashMap overhead)

### Use Cases
- **Unit testing**: Fast test execution without I/O overhead
- **Development**: Quick prototyping and debugging
- **Caching layer**: Temporary storage for frequently accessed data
- **Small datasets**: When entire dataset fits comfortably in memory

### Usage Example
```rust
use prollytree::storage::InMemoryNodeStorage;
use prollytree::tree::{ProllyTree, Tree};
use prollytree::config::TreeConfig;

let storage = InMemoryNodeStorage::<32>::new();
let config = TreeConfig::<32>::default();
let mut tree = ProllyTree::new(storage, config);

// Data will be lost when `tree` goes out of scope
tree.insert(b"key".to_vec(), b"value".to_vec());
```

### Configuration
The in-memory storage is self-contained and requires no configuration. It automatically manages memory allocation and cleanup.

## FileNodeStorage

### Description
The file storage backend persists each ProllyTree node as a separate file on the filesystem using binary serialization. Configuration data is stored in separate files with a `config_` prefix.

### Characteristics
- **Performance**: Moderate - limited by filesystem I/O
- **Persistence**: Full persistence across application restarts
- **Storage Format**: Binary-serialized nodes (using bincode)
- **File Organization**: One file per node, named by hash
- **Platform Support**: Works on all platforms with filesystem access

### Use Cases
- **Local applications**: Desktop applications needing persistence
- **Development**: When you need persistence but don't want database setup
- **Small to medium datasets**: Up to thousands of nodes
- **Debugging**: Easy to inspect individual node files

### Usage Example
```rust
use prollytree::storage::FileNodeStorage;
use prollytree::tree::{ProllyTree, Tree};
use prollytree::config::TreeConfig;
use std::path::PathBuf;

let storage_dir = PathBuf::from("./prolly_data");
let storage = FileNodeStorage::<32>::new(storage_dir);
let config = TreeConfig::<32>::default();
let mut tree = ProllyTree::new(storage, config);

tree.insert(b"key".to_vec(), b"value".to_vec());
// Data persists in ./prolly_data/ directory
```

### File Structure
```
prolly_data/
├── a1b2c3d4e5f6... (node file - hex hash)
├── f6e5d4c3b2a1... (node file - hex hash)
├── config_tree_config (configuration file)
└── config_custom_key (custom configuration)
```

### Limitations
- **Scalability**: Performance degrades with large number of nodes
- **Atomicity**: No atomic updates across multiple nodes
- **Concurrent Access**: Not safe for concurrent writers

## RocksDBNodeStorage

### Description
RocksDB storage provides a production-ready, high-performance backend using Facebook's RocksDB LSM-tree implementation. It's optimized for ProllyTree's content-addressed, write-heavy workload patterns.

### Characteristics
- **Performance**: High throughput for both reads and writes
- **Persistence**: Durable storage with WAL (Write-Ahead Log)
- **Scalability**: Handles millions of nodes efficiently
- **Compression**: LZ4 for hot data, Zstd for cold data
- **Caching**: Multi-level caching (LRU cache + RocksDB block cache)
- **Compaction**: Background cleanup of obsolete data

### Architecture
```
Application
    ↓
LRU Cache (1000 nodes default)
    ↓
RocksDB
├── Write Buffer (128MB)
├── Block Cache (512MB)
├── Bloom Filters (10 bits/key)
└── SST Files (compressed)
```

### Use Cases
- **Production applications**: High-performance persistent storage
- **Large datasets**: Millions of nodes and frequent updates
- **Write-heavy workloads**: Frequent tree modifications
- **Distributed systems**: Building block for distributed storage

### Usage Example
```rust
use prollytree::storage::RocksDBNodeStorage;
use prollytree::tree::{ProllyTree, Tree};
use prollytree::config::TreeConfig;
use std::path::PathBuf;

// Basic usage
let db_path = PathBuf::from("./rocksdb_data");
let storage = RocksDBNodeStorage::<32>::new(db_path)?;
let config = TreeConfig::<32>::default();
let mut tree = ProllyTree::new(storage, config);

// Custom cache size
let storage = RocksDBNodeStorage::<32>::with_cache_size(db_path, 5000)?;

// Custom RocksDB options
let mut opts = RocksDBNodeStorage::<32>::default_options();
opts.set_write_buffer_size(256 * 1024 * 1024); // 256MB
let storage = RocksDBNodeStorage::<32>::with_options(db_path, opts)?;
```

### Configuration Options

#### Default Optimizations
- **Write Buffer**: 128MB for batching writes
- **Memory Tables**: Up to 4 concurrent memtables
- **Compression**: LZ4 for L0-L2, Zstd for bottom levels
- **Block Cache**: 512MB for frequently accessed data
- **Bloom Filters**: 10 bits per key for faster lookups

#### Performance Tuning
```rust
use rocksdb::{Options, DBCompressionType, BlockBasedOptions, Cache};

let mut opts = Options::default();

// Increase write buffer for high write throughput
opts.set_write_buffer_size(256 * 1024 * 1024);

// More aggressive compression for storage efficiency
opts.set_compression_type(DBCompressionType::Zstd);

// Larger block cache for read-heavy workloads
let cache = Cache::new_lru_cache(1024 * 1024 * 1024); // 1GB
let mut block_opts = BlockBasedOptions::default();
block_opts.set_block_cache(&cache);
opts.set_block_based_table_factory(&block_opts);
```

### Batch Operations
RocksDB storage supports efficient batch operations:

```rust
let nodes = vec![
    (hash1, node1),
    (hash2, node2),
    (hash3, node3),
];

// Atomic batch insert
storage.batch_insert_nodes(nodes)?;

// Atomic batch delete
storage.batch_delete_nodes(&[hash1, hash2])?;
```

### Monitoring and Maintenance
- **Statistics**: RocksDB provides detailed performance metrics
- **Compaction**: Automatic background compaction
- **Backup**: Use RocksDB backup utilities for data safety
- **Tuning**: Monitor write amplification and adjust settings

## GitNodeStorage

### Description
The Git storage backend stores ProllyTree nodes as Git blob objects in a Git repository. This experimental backend is designed for development workflows where you want to leverage Git's content-addressable storage.

### ⚠️ Important Limitations

**Development Use Only**: GitNodeStorage should only be used for local development and experimentation. It is not suitable for production use due to several important limitations:

1. **Dangling Objects**: ProllyTree nodes are stored as Git blob objects but are **not committed** to any branch or tag. These objects exist as "dangling" or "unreachable" objects in Git's object database.

2. **Garbage Collection Risk**: Git's garbage collector (`git gc`) will **delete these dangling objects** during cleanup operations. This can happen:
   - When running `git gc` manually
   - Automatically during Git operations (push, pull, repack, etc.)
   - When Git's automatic garbage collection triggers

3. **Data Loss**: Since the objects are not referenced by any commit, branch, or tag, they will be permanently lost when garbage collected. There is no recovery mechanism.

### Characteristics
- **Storage Format**: Git blob objects (binary serialized nodes)
- **Content Addressing**: Leverages Git's SHA-1 content addressing
- **Persistence**: Temporary - objects can be garbage collected
- **Integration**: Works with existing Git repositories
- **Caching**: LRU cache for performance

### Use Cases (Development Only)
- **Git Integration Experiments**: Testing Git-based storage concepts
- **Development Workflows**: Temporary storage during development
- **Learning**: Understanding content-addressable storage
- **Prototyping**: Rapid prototyping with Git infrastructure

### Usage Example
```rust
// Only available with "git" feature
#[cfg(feature = "git")]
use prollytree::git::GitNodeStorage;

let repo = gix::open(".")?;
let dataset_dir = std::path::PathBuf::from("./git_data");
let storage = GitNodeStorage::<32>::new(repo, dataset_dir)?;

// ⚠️ WARNING: Data may be lost during git gc!
let config = TreeConfig::<32>::default();
let mut tree = ProllyTree::new(storage, config);
tree.insert(b"key".to_vec(), b"value".to_vec());
```

### Data Safety Measures

If you must use GitNodeStorage for development, consider these safety measures:

1. **Disable Automatic GC**:
   ```bash
   git config gc.auto 0
   git config gc.autopacklimit 0
   ```

2. **Create Temporary Commits** (advanced):
   ```bash
   # Periodically commit to preserve objects
   git add -A
   git commit -m "temp: preserve prolly objects"
   ```

3. **Use Separate Repository**:
   Create a dedicated Git repository just for ProllyTree storage to avoid conflicts.

### Architecture
```
ProllyTree Node
    ↓
Bincode Serialization
    ↓
Git Blob Object (dangling)
    ↓
Git Object Database
    ↓
⚠️ git gc → Deletion
```

## Storage Backend Comparison

| Feature | InMemory | File | RocksDB | Git |
|---------|----------|------|---------|-----|
| **Persistence** | None | Full | Full | Temporary⚠️ |
| **Performance** | Fastest | Moderate | High | Moderate |
| **Scalability** | RAM-limited | Poor | Excellent | Poor |
| **Setup Complexity** | None | None | Low | Medium |
| **Production Ready** | No | Limited | Yes | No⚠️ |
| **Concurrent Access** | Limited | No | Yes | Limited |
| **Storage Overhead** | None | High | Low | Medium |
| **Backup/Recovery** | N/A | File copy | RocksDB tools | Git tools |

## Choosing the Right Backend

### Development & Testing
- **Unit Tests**: InMemoryNodeStorage
- **Integration Tests**: FileNodeStorage or InMemoryNodeStorage
- **Local Development**: FileNodeStorage or RocksDBNodeStorage

### Production Deployments
- **Small Applications**: FileNodeStorage (with careful consideration)
- **High-Performance Applications**: RocksDBNodeStorage
- **Distributed Systems**: RocksDBNodeStorage as foundation

### Experimental
- **Git Integration Research**: GitNodeStorage (development only)

## Performance Benchmarks

Run the storage comparison benchmarks to understand performance characteristics:

```bash
# Compare all available backends
cargo bench --bench storage_bench --features rocksdb_storage

# Run specific benchmark
cargo bench --bench storage_bench storage_insert
```

## Migration Between Backends

Currently, there's no built-in migration tool between storage backends. To migrate:

1. **Export Data**: Iterate through the old storage and collect all key-value pairs
2. **Create New Storage**: Initialize the target storage backend
3. **Import Data**: Insert all data into the new storage
4. **Validate**: Verify data integrity after migration

Example migration pattern:
```rust
// Export from old storage
let old_tree = ProllyTree::load_from_storage(old_storage, config.clone())?;
let mut data = Vec::new();
// ... collect all key-value pairs

// Import to new storage
let mut new_tree = ProllyTree::new(new_storage, config);
for (key, value) in data {
    new_tree.insert(key, value);
}
```

## Best Practices

### General
- Choose the simplest backend that meets your requirements
- Always benchmark with your specific data patterns
- Consider backup and recovery procedures
- Plan for data growth and scaling needs

### InMemoryNodeStorage
- Monitor memory usage to prevent OOM conditions
- Use for temporary data only
- Consider data loss implications

### FileNodeStorage
- Ensure adequate disk space and I/O performance
- Implement application-level locking for concurrent access
- Regular filesystem maintenance and monitoring

### RocksDBNodeStorage
- Monitor RocksDB metrics for performance tuning
- Configure appropriate cache sizes for your workload
- Plan for disk space and compaction overhead
- Use batch operations for bulk updates

### GitNodeStorage
- **Never use in production**
- Disable automatic garbage collection during development
- Use dedicated Git repositories
- Regularly backup important data to commits
- Understand that data can be lost without warning

## Troubleshooting

### Common Issues

#### OutOfMemory with InMemoryNodeStorage
- Reduce dataset size or switch to persistent storage
- Monitor heap usage and tune JVM/runtime parameters

#### Poor Performance with FileNodeStorage
- Check filesystem performance and available disk space
- Consider switching to RocksDBNodeStorage for better performance
- Reduce concurrent access patterns

#### RocksDB Compilation Issues
- Ensure proper build tools (cmake, C++ compiler)
- Check RocksDB system dependencies
- Use pre-built binaries if available

#### Git Storage Data Loss
- This is expected behavior - objects are not committed
- Disable garbage collection or switch to persistent storage
- Create periodic commits to preserve important data

For additional help, consult the project documentation or open an issue on the GitHub repository.
