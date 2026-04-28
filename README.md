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
prollytree = "0.3.4-beta"

# Optional features
prollytree = { version = "0.3.4-beta", features = ["git", "sql"] }
```

## Examples

### Verifiable key-value store

A ProllyTree is a Merkle tree, so any key-value pair comes with a cryptographic
inclusion proof.

```rust
use prollytree::tree::{ProllyTree, Tree};
use prollytree::storage::InMemoryNodeStorage;

let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::new(), Default::default());
tree.insert(b"user:alice".to_vec(), b"Alice".to_vec());

let proof = tree.generate_proof(b"user:alice");
assert!(tree.verify(proof, b"user:alice", Some(b"Alice")));
```

### Git-backed versioning

The `git` feature stores tree nodes as Git objects, so commits, branches, and merges
work natively on key-value state.

```rust
use prollytree::git::versioned_store::StoreFactory;

let mut store = StoreFactory::git::<32, _>("data")?;
store.insert(b"config/api_key".to_vec(), b"v1".to_vec())?;
store.commit("Initial config")?;

store.create_branch("experimental")?;
store.insert(b"config/api_key".to_vec(), b"v2".to_vec())?;
store.commit("Try new key")?;
// → diff, merge, history available; see the user guide
```

See [`examples/`](examples/) for SQL queries, additional storage backends, and agent
memory patterns.

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
version = "0.3.4-beta"
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

```bash
# Rust tests
cargo test --features "git sql"

# Python tests (build bindings first)
./python/build_python.sh --all-features --install
python -m pytest python/tests/
```

## Documentation & Examples

- **[User Guide & Theory](https://zhangfengcdt.github.io/prollytree/)** – mkdocs site with the full tour (theory, CLI, Python, examples)
- **[Rust API Reference](https://docs.rs/prollytree)** – auto-generated from source
- **[Use Cases & Examples](examples/)** – version control, SQL, proofs, storage backends
- **[Python Bindings](python/README.md)** – Python-specific quickstart

## CLI Tool

```bash
cargo install prollytree --features git
git-prolly --help
```

See the [user guide](https://zhangfengcdt.github.io/prollytree/) for a full CLI walkthrough.

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under the Apache License 2.0. See [LICENSE](LICENSE).
