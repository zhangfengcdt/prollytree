# ProllyTree

[![Crates.io](https://img.shields.io/crates/v/prollytree.svg)](https://crates.io/crates/prollytree)
[![Documentation](https://docs.rs/prollytree/badge.svg)](https://docs.rs/prollytree)
[![License](https://img.shields.io/crates/l/prollytree.svg)](https://github.com/zhangfengcdt/prollytree/blob/main/LICENSE)

**ProllyTree** is a probabilistic B-tree that also behaves like a Merkle tree — combining the logarithmic locality of a B-tree with the content-addressed, cryptographically verifiable structure of a Merkle DAG. It is implemented in Rust with first-class Python bindings, and ships with Git-backed versioned storage, a SQL query layer, and a `git-prolly` CLI for Git-like key-value workflows.

## Why a "prolly" tree?

A classical B-tree balances on *size* — after N inserts it's rebalanced to keep branching uniform. A Merkle tree is *content-addressed* but shape-dependent — two peers that inserted the same keys in different orders end up with different trees and therefore different root hashes.

A prolly tree fixes both problems:

- **Shape depends only on content, not history.** Node boundaries are picked by a deterministic hash-based predicate over the keys, so any two trees holding the same key set converge to the same shape and the same root hash.
- **You keep B-tree ergonomics.** Operations are still O(log n). Range scans, point lookups, and batched writes all work the way you'd expect.
- **You keep Merkle guarantees.** Every node is content-addressed. You can prove inclusion of a key, sync two replicas by exchanging differing subtrees, and three-way merge by diffing Merkle subtrees instead of replaying edits.

The [Theory](theory/index.md) section walks through the construction in full, including the rolling-hash balancing rule, Merkle proof mechanics, and how versioning layers on top.

## Key features

- **O(log n) operations** with cache-friendly probabilistic balancing.
- **Cryptographically verifiable.** Merkle properties give you inclusion proofs and root-hash equality for free.
- **Multiple storage backends.** In-memory, File, RocksDB, and Git-backed persistence behind a single `NodeStorage` trait.
- **Git-backed versioning.** Branch, commit, diff, and three-way merge the key-value store with the `git-prolly` CLI or the `VersionedKvStore` API — uses real Git under the hood.
- **SQL interface.** Query your tree as relational tables via GlueSQL integration.
- **Python bindings.** Full API coverage via PyO3, including versioning, merging, and SQL.

## Quick example (Rust)

```rust
use prollytree::tree::{ProllyTree, Tree};
use prollytree::storage::InMemoryNodeStorage;

let storage = InMemoryNodeStorage::<32>::new();
let mut tree = ProllyTree::new(storage, Default::default());

tree.insert(b"user:alice".to_vec(), b"Alice Johnson".to_vec());
tree.insert(b"config:timeout".to_vec(), b"30".to_vec());

// Generate a cryptographic proof and verify it.
let proof = tree.generate_proof(b"user:alice");
assert!(tree.verify(proof, b"user:alice", Some(b"Alice Johnson")));
```

## Quick example (Python)

```python
from prollytree import VersionedKvStore, ConflictResolution

store = VersionedKvStore("./my_store")
store.insert(b"config:theme", b"light")
store.commit("Initial config")

store.create_branch("experiment")
store.update(b"config:theme", b"dark")
store.commit("Try dark mode")

store.checkout("main")
store.merge("experiment", ConflictResolution.TakeSource)
```

## Where to go next

- **[Installation](installation.md)** — add ProllyTree to a Rust or Python project.
- **[Quickstart](quickstart.md)** — a ten-minute tour in either language.
- **[Theory](theory/index.md)** — what a prolly tree actually is and why the construction works.
- **[CLI reference](cli.md)** — every `git-prolly` subcommand.
- **[Architecture](architecture.md)** — layered view of storage, tree, versioning, and SQL.
