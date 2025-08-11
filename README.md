# ProllyTree

[![Crates.io](https://img.shields.io/crates/v/prollytree.svg)](https://crates.io/crates/prollytree)
[![Documentation](https://docs.rs/prollytree/badge.svg)](https://docs.rs/prollytree)
[![License](https://img.shields.io/crates/l/prollytree.svg)](https://github.com/yourusername/prollytree/blob/main/LICENSE)
[![Downloads](https://img.shields.io/crates/d/prollytree.svg)](https://crates.io/crates/prollytree)

A **probabilistic B-tree** implementation in Rust that combines B-tree efficiency with Merkle tree cryptographic properties. Designed for distributed systems, version control, and verifiable data structures.

## Features

- **High Performance**: O(log n) operations with cache-friendly probabilistic balancing
- **Cryptographically Verifiable**: Merkle tree properties for data integrity and inclusion proofs
- **Multiple Storage Backends**: In-memory, RocksDB, and Git-backed persistence
- **Distributed-Ready**: Efficient diff, sync, and three-way merge capabilities
- **Python Bindings**: Full API coverage via PyO3 with async support
- **SQL Interface**: Query trees with SQL via GlueSQL integration
- **AI Agent Memory**: Purpose-built for LLM applications and agent systems

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
prollytree = "0.3.0"

# Optional features
prollytree = { version = "0.3.0", features = ["git", "sql", "rig"] }
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
use prollytree::git::GitVersionedKvStore;
use std::process::Command;
use std::fs;

// Setup: Create a temporary Git repository (in real use, you'd have an existing repo)
let repo_path = "/tmp/demo_git_repo";
fs::remove_dir_all(repo_path).ok(); // Clean up
fs::create_dir_all(repo_path)?;

// Initialize Git repository
Command::new("git").args(&["init"]).current_dir(repo_path).output()?;
Command::new("git").args(&["config", "user.name", "Demo"]).current_dir(repo_path).output()?;
Command::new("git").args(&["config", "user.email", "demo@example.com"]).current_dir(repo_path).output()?;

// Initial git commit
fs::write(format!("{}/README.md", repo_path), "# Demo")?;
Command::new("git").args(&["add", "."]).current_dir(repo_path).output()?;
Command::new("git").args(&["commit", "-m", "Initial"]).current_dir(repo_path).output()?;

// Switch to repo directory and create dataset
std::env::set_current_dir(repo_path)?;
fs::create_dir_all("data")?;
let mut store = GitVersionedKvStore::<32>::init("data")?;

// Now use Git-backed versioned storage
store.insert(b"config/api_key".to_vec(), b"secret123".to_vec())?;
store.commit("Initial config")?;

// Retrieve data
if let Some(value) = store.get(b"config/api_key") {
    println!("Retrieved: {}", String::from_utf8(value)?);
}

// Add more data and commit
store.insert(b"config/timeout".to_vec(), b"30".to_vec())?;
store.commit("Add timeout config")?;

// Create branches for parallel development
store.create_branch("experimental")?;
println!("Git-backed storage with full version control!");
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
let file_storage = FileNodeStorage::<32>::new("./tree_data".into());
let mut file_tree = ProllyTree::new(file_storage, Default::default());
file_tree.insert(b"user:alice".to_vec(), b"Alice Johnson".to_vec());

// Both trees support the same operations
if let Some(node) = mem_tree.find(b"session:abc123") {
    println!("Session found in memory storage");
}

// For SQL functionality, see examples/sql.rs
println!("Multiple storage backends working!");
```

## Feature Flags

```toml
[dependencies.prollytree]
version = "0.3.0"
features = [
    "git",              # Git-backed versioned storage
    "sql",              # SQL interface via GlueSQL
    "rig",              # Rig framework integration for AI
    "python",           # Python bindings via PyO3
    "rocksdb_storage",  # RocksDB persistent storage backend
]
```

## Performance

**Benchmarks (Apple M3 Pro, 18GB RAM):**
- Insert: ~8-21 µs (scales O(log n))
- Lookup: ~1-3 µs (sub-linear due to caching)
- Memory: ~100 bytes per key-value pair
- Batch operations: ~25% faster than individual ops

Run benchmarks: `cargo bench`

## Documentation & Examples

- **[Full API Documentation](https://docs.rs/prollytree)**
- **[Use Cases & Examples](examples/README.md)** - AI agents, version control, distributed systems
- **[Python Bindings](python/README.md)** - Complete Python API
- **[Performance Guide](docs/performance.md)** - Optimization tips

## CLI Tool

```bash
# Install git-prolly CLI
cargo install prollytree --features git

# Setup git repository and create dataset
git init my-repo && cd my-repo
mkdir my-data && git-prolly init my-data  # Create dataset directory
cd my-data
git-prolly set "user:alice" "Alice Johnson"
git-prolly commit -m "Add user"
git checkout -b feature/updates  # Use regular git for branching
git-prolly merge main
```

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under the Apache License 2.0. See [LICENSE](LICENSE).
