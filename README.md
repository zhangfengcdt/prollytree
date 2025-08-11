# ProllyTree

[![Crates.io](https://img.shields.io/crates/v/prollytree.svg)](https://crates.io/crates/prollytree)
[![Documentation](https://docs.rs/prollytree/badge.svg)](https://docs.rs/prollytree)
[![License](https://img.shields.io/crates/l/prollytree.svg)](https://github.com/yourusername/prollytree/blob/main/LICENSE)
[![Downloads](https://img.shields.io/crates/d/prollytree.svg)](https://crates.io/crates/prollytree)

A **probabilistic B-tree** implementation in Rust that combines B-tree efficiency with Merkle tree cryptographic properties. Designed for distributed systems, version control, and verifiable data structures.

## Features

- **🚀 High Performance**: O(log n) operations with cache-friendly probabilistic balancing
- **🔐 Cryptographically Verifiable**: Merkle tree properties for data integrity and inclusion proofs
- **📦 Multiple Storage Backends**: In-memory, RocksDB, and Git-backed persistence
- **🌐 Distributed-Ready**: Efficient diff, sync, and three-way merge capabilities
- **🐍 Python Bindings**: Full API coverage via PyO3 with async support
- **📊 SQL Interface**: Query trees with SQL via GlueSQL integration
- **🤖 AI Agent Memory**: Purpose-built for LLM applications and agent systems

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
prollytree = "0.2.0"

# Optional features
prollytree = { version = "0.2.0", features = ["git", "sql", "rig"] }
```

## Examples

### Basic Tree Operations

```rust
use prollytree::tree::ProllyTree;
use prollytree::storage::InMemoryNodeStorage;

let storage = InMemoryNodeStorage::<32>::new();
let mut tree = ProllyTree::new(storage, Default::default());

// Insert data
tree.insert(b"user:alice".to_vec(), b"Alice Johnson".to_vec());
tree.insert(b"config:timeout".to_vec(), b"30".to_vec());

// Query data
let value = tree.find(b"user:alice")?;
println!("Found: {}", String::from_utf8(value)?);

// Generate cryptographic proof
let proof = tree.generate_proof(b"user:alice")?;
let is_valid = tree.verify_proof(&proof, b"user:alice", b"Alice Johnson");
```

### Git-backed Versioned Storage

```rust
use prollytree::git::GitVersionedKvStore;

let mut store = GitVersionedKvStore::init("./data")?;

// Version your data like Git
store.set(b"config/api_key", b"secret123")?;
store.commit("Initial config")?;

// Branch for experiments
store.checkout_new_branch("feature/optimization")?;
store.set(b"config/timeout", b"60")?;
store.commit("Increase timeout")?;

// Three-way merge back to main
store.checkout("main")?;
store.merge("feature/optimization")?;
```

### SQL Interface with Time Travel

```rust
use prollytree::sql::ProllyStorage;
use gluesql_core::prelude::Glue;

let storage = ProllyStorage::<32>::init("./data")?;
let mut glue = Glue::new(storage);

// Standard SQL operations
glue.execute("CREATE TABLE users (id INTEGER, name TEXT)").await?;
glue.execute("INSERT INTO users VALUES (1, 'Alice')").await?;

// Time travel queries
glue.storage.commit("v1.0").await?;
glue.execute("UPDATE users SET name = 'Alice Smith' WHERE id = 1").await?;

// Query historical data
let v1_data = glue.storage.query_at_commit("v1.0",
    "SELECT * FROM users WHERE id = 1").await?;
```

### AI Agent Memory

```rust
use prollytree::agent::{AgentMemorySystem, MemoryQuery};

let mut memory = AgentMemorySystem::init_with_thread_safe_git(
    "./agent_memory", "agent_001".to_string(), None
)?;

// Store conversation context
memory.short_term.store_conversation_turn(
    "session_123", "user", "What's the weather today?", None
).await?;

// Store persistent knowledge
memory.semantic.store_fact(
    "weather", "temperature",
    json!({"location": "Tokyo", "temp": "22°C"}),
    0.9, "weather_api"
).await?;

// Query and checkpoint
let memories = memory.semantic.query(MemoryQuery::text("Tokyo")).await?;
let checkpoint = memory.checkpoint("Weather conversation").await?;
```

## Feature Flags

```toml
[dependencies.prollytree]
version = "0.2.0"
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

- **[📖 Full API Documentation](https://docs.rs/prollytree)**
- **[💡 Use Cases & Examples](examples/README.md)** - AI agents, version control, distributed systems
- **[🐍 Python Bindings](python/README.md)** - Complete Python API
- **[⚡ Performance Guide](docs/performance.md)** - Optimization tips

## CLI Tool

```bash
# Install git-prolly CLI
cargo install prollytree --features git

# Use like Git for key-value data
git-prolly init my-data
git-prolly set "user:alice" "Alice Johnson"
git-prolly commit -m "Add user"
git-prolly checkout -b feature/updates
git-prolly merge main
```

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under the Apache License 2.0. See [LICENSE](LICENSE).
