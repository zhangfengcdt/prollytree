# ProllyTree

[![Crates.io](https://img.shields.io/crates/v/prollytree.svg)](https://crates.io/crates/prollytree)
[![Documentation](https://docs.rs/prollytree/badge.svg)](https://docs.rs/prollytree)
[![License](https://img.shields.io/crates/l/prollytree.svg)](https://github.com/zhangfengcdt/prollytree/blob/main/LICENSE)
[![Downloads](https://img.shields.io/crates/d/prollytree.svg)](https://crates.io/crates/prollytree)

A **probabilistic B-tree** implementation in Rust that combines B-tree efficiency with Merkle tree cryptographic properties. Designed for distributed systems, version control, and verifiable data structures.

## Features

- **High Performance**: O(log n) operations with cache-friendly probabilistic balancing
- **Cryptographically Verifiable**: Merkle tree properties for data integrity and inclusion proofs
- **Multiple Storage Backends**: In-memory, File, RocksDB, and Git-backed persistence
- **Distributed-Ready**: Efficient diff, sync, and three-way merge with pluggable conflict resolvers
- **Python Bindings**: Full API coverage via PyO3 with async support
- **SQL Interface**: Query trees with SQL via GlueSQL integration

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
prollytree = "0.3.2-beta"

# Optional features
prollytree = { version = "0.3.2-beta", features = ["git", "sql"] }
```

## Examples

### Basic Tree Operations

```rust
use prollytree::tree::{ProllyTree, Tree};
use prollytree::storage::InMemoryNodeStorage;

let storage = InMemoryNodeStorage::<32>::new();
let mut tree = ProllyTree::new(storage, Default::default());

// Insert data
tree.insert(b"user:alice".to_vec(), b"Alice Johnson".to_vec());
tree.insert(b"config:timeout".to_vec(), b"30".to_vec());

// Query data - find returns a node, extract the value
if let Some(node) = tree.find(b"user:alice") {
    for (i, key) in node.keys.iter().enumerate() {
        if key == b"user:alice" {
            let value = &node.values[i];
            println!("Found: {}", String::from_utf8(value.clone())?);
            break;
        }
    }
}

// Generate cryptographic proof
let proof = tree.generate_proof(b"user:alice");
let is_valid = tree.verify(proof, b"user:alice", Some(b"Alice Johnson"));
```

### Git-backed Versioned Storage

```rust
use prollytree::git::versioned_store::StoreFactory;

// Create a Git-backed versioned key-value store
let mut store = StoreFactory::git::<32, _>("data")?;

// Insert and commit
store.insert(b"config/api_key".to_vec(), b"secret123".to_vec())?;
store.commit("Initial config")?;

// Retrieve data
if let Some(value) = store.get(b"config/api_key") {
    println!("Retrieved: {}", String::from_utf8(value)?);
}

// Create branches for parallel development
store.create_branch("experimental")?;

// Thread-safe variant for concurrent access
let ts_store = StoreFactory::git_threadsafe::<32, _>("data")?;
```

### Multiple Storage Backends

```rust
use prollytree::tree::{ProllyTree, Tree};
use prollytree::storage::{InMemoryNodeStorage, FileNodeStorage};

// In-memory storage (fast, temporary)
let mem_storage = InMemoryNodeStorage::<32>::new();
let mut mem_tree = ProllyTree::new(mem_storage, Default::default());
mem_tree.insert(b"session:abc123".to_vec(), b"active".to_vec());

// File-based storage (persistent)
let file_storage = FileNodeStorage::<32>::new("./tree_data".into())?;
let mut file_tree = ProllyTree::new(file_storage, Default::default());
file_tree.insert(b"user:alice".to_vec(), b"Alice Johnson".to_vec());

// Both trees support the same operations
if let Some(node) = mem_tree.find(b"session:abc123") {
    println!("Session found in memory storage");
}
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `git` | Git-backed versioned storage with branching, merging, and history | Yes |
| `sql` | SQL query interface via GlueSQL | Yes |
| `rocksdb_storage` | RocksDB persistent storage backend | No |
| `python` | Python bindings via PyO3 | No |
| `tracing` | Observability via the `tracing` crate | No |
| `digest_base64` | Base64 encoding for digests | Yes |

```toml
[dependencies.prollytree]
version = "0.3.2-beta"
features = ["git", "sql", "rocksdb_storage"]
```

## Performance

**Benchmarks (Apple M3 Pro, 18GB RAM):**
- Insert: ~8-21 us (scales O(log n))
- Lookup: ~1-3 us (sub-linear due to caching)
- Memory: ~100 bytes per key-value pair
- Batch operations: ~25% faster than individual ops

Run benchmarks: `cargo bench`

## Testing

### Unit Tests

```bash
# Run all unit tests with default features
cargo test

# Run with specific features
cargo test --features "git sql"

# Run a specific test
cargo test test_name -- --nocapture
```

### Integration Tests

The project includes a comprehensive integration test suite in `tests/`:

| Test file | Description |
|-----------|-------------|
| `cli_commands.rs` | End-to-end tests for all `git-prolly` CLI commands |
| `store_factory.rs` | `StoreFactory` API for all backend types |
| `versioned_store_cross_backend.rs` | Cross-backend consistency (Git vs InMemory vs File) |
| `sql_integration.rs` | Full SQL query lifecycle via GlueSQL |
| `conflict_resolvers.rs` | All conflict resolvers through real branch merges |
| `git_versioning_lifecycle.rs` | Branching, merging, history, and persistence |
| `worktree_integration.rs` | WorktreeManager concurrent operations |
| `storage_backends.rs` | InMemory and File `NodeStorage` CRUD |
| `error_recovery.rs` | Error paths: corrupted config, missing repos, bad branches |
| `scale_and_stress.rs` | Large datasets and concurrent stress (marked `#[ignore]`) |

```bash
# Run all integration tests
cargo test --features "git sql"

# Run a specific integration test file
cargo test --features "git sql" --test cli_commands

# Run scale/stress tests (slower, ignored by default)
cargo test --features "git sql" --test scale_and_stress -- --ignored

# Run only unit tests (skip integration)
cargo test --lib --features "git sql"
```

### Python Tests

```bash
# Build Python bindings first
./python/build_python.sh --all-features --install

# Run Python tests
python -m pytest python/tests/
```

## Documentation & Examples

- **[Full API Documentation](https://docs.rs/prollytree)**
- **[Python Documentation](https://prollytree.readthedocs.io/)**
- **[Use Cases & Examples](examples/)** - Version control, SQL, proofs, storage backends
- **[Python Bindings](python/README.md)** - Complete Python API

## CLI Tool

```bash
# Install git-prolly CLI
cargo install prollytree --features git

# Setup git repository and create dataset
git init my-repo && cd my-repo
mkdir my-data && git-prolly init my-data
cd my-data

# Key-value operations
git-prolly set "user:alice" "Alice Johnson"
git-prolly set "user:bob" "Bob Smith"
git-prolly commit -m "Add users"
git-prolly get "user:alice"
git-prolly list --values

# Branching and merging (uses git branches)
git checkout -b feature/updates
git-prolly set "user:alice" "Alice J."
git-prolly commit -m "Update alice"
git checkout main
git-prolly merge feature/updates

# SQL queries (requires sql feature)
git-prolly sql "CREATE TABLE users (id INTEGER, name TEXT)"
git-prolly sql "INSERT INTO users VALUES (1, 'Alice')"
git-prolly sql "SELECT * FROM users"

# History and inspection
git-prolly log
git-prolly history "user:alice"
git-prolly stats
git-prolly diff HEAD~1 HEAD
```

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under the Apache License 2.0. See [LICENSE](LICENSE).
