# Rust API

The Rust API is documented with `rustdoc` and auto-published to [docs.rs](https://docs.rs/prollytree). The canonical, always-up-to-date reference lives there.

👉 **[docs.rs/prollytree](https://docs.rs/prollytree)**

## Generating the docs locally

```bash
cargo doc --document-private-items --no-deps --open
```

The `--document-private-items` flag is helpful when you want to read the implementation details of things like `Node` and the balancing predicate.

## Where to start

If you're new to the crate, these modules are the useful entry points, roughly in layering order:

| Module | What lives there |
|---|---|
| [`prollytree::tree`](https://docs.rs/prollytree/latest/prollytree/tree/index.html) | `ProllyTree`, the core data structure |
| [`prollytree::storage`](https://docs.rs/prollytree/latest/prollytree/storage/index.html) | `NodeStorage` trait, `InMemoryNodeStorage`, `FileNodeStorage` |
| [`prollytree::rocksdb`](https://docs.rs/prollytree/latest/prollytree/rocksdb/index.html) (feature `rocksdb_storage`) | `RocksDBNodeStorage` |
| [`prollytree::git`](https://docs.rs/prollytree/latest/prollytree/git/index.html) (feature `git`) | `VersionedKvStore`, `StoreFactory`, `GitNodeStorage` |
| [`prollytree::sql`](https://docs.rs/prollytree/latest/prollytree/sql/index.html) (feature `sql`) | GlueSQL adapter |
| [`prollytree::diff`](https://docs.rs/prollytree/latest/prollytree/diff/index.html) | Diff types and `ConflictResolver` trait |
| [`prollytree::proof`](https://docs.rs/prollytree/latest/prollytree/proof/index.html) | Merkle inclusion/absence proofs |
| [`prollytree::config`](https://docs.rs/prollytree/latest/prollytree/config/index.html) | `TreeConfig` |

## Surface at a glance

```rust
use prollytree::{
    tree::{ProllyTree, Tree},
    storage::{InMemoryNodeStorage, FileNodeStorage},
    config::TreeConfig,
};

#[cfg(feature = "git")]
use prollytree::git::versioned_store::{StoreFactory, VersionedKvStore};

#[cfg(feature = "sql")]
use prollytree::sql::ProllySQLStore;
```

See [Quickstart](../quickstart.md) for the shortest working example and [Basic Usage](../basic_usage.md) for a longer walk.
