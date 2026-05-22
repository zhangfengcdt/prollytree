# ProllyTree

[![Crates.io](https://img.shields.io/crates/v/prollytree.svg)](https://crates.io/crates/prollytree)
[![Documentation](https://docs.rs/prollytree/badge.svg)](https://docs.rs/prollytree)
[![License](https://img.shields.io/crates/l/prollytree.svg)](https://github.com/zhangfengcdt/prollytree/blob/main/LICENSE)
[![Downloads](https://img.shields.io/crates/d/prollytree.svg)](https://crates.io/crates/prollytree)

A **probabilistic B-tree with Merkle properties** — a content-addressed, Git-versioned key-value store with branching, three-way merge, cryptographic proofs, optional SQL, and an optional vector / text-search index. Written in Rust with first-class Python bindings.

A prolly tree's shape is a deterministic function of its contents, so two replicas holding the same key-value set converge to the same root hash regardless of insertion order. That property is what makes the rest — Git-style versioning, efficient diff/sync between replicas, and verifiable subtree sharing across history — fall out for free.

## Features

| Capability | What it gives you |
|---|---|
| **Versioned KV store** | Git-backed branch / commit / diff / three-way merge on raw key-value state |
| **Namespaced KV store** | Many isolated prolly trees in one Git repo, atomic across namespaces |
| **Text / vector search** | Optional versioned ANN index inside any namespace; bundled MiniLM, hash, and callable embedders |
| **Multi-chunk indexing** | Split docs into chunks at index time, dedup on search by document |
| **Cascade mode** | One primary write auto-mirrors into every registered text index |
| **Large-value externalization** | Values above a threshold land in content-addressed blobs; `gc_blobs()` reclaims them |
| **Cryptographic proofs** | Merkle inclusion / absence proofs on every value |
| **Multiple storage backends** | In-memory, File, RocksDB, Git-backed |
| **SQL interface** | Query the tree as relational tables via GlueSQL |
| **Python bindings** | Full surface via PyO3 — versioning, namespaces, text search, SQL |
| **`git-prolly` CLI** | Git-style command surface over the versioning + SQL layers |

## Quick start

### Rust

```toml
[dependencies]
prollytree = { version = "0.4.1-beta", features = ["git", "sql"] }
# Add `proximity` for the text-search surface, `proximity_text` for bundled MiniLM.
```

### Python

```bash
pip install prollytree   # ships with git, sql, proximity, proximity_text by default
```

## Examples

### Verifiable key-value store

The raw `ProllyTree` ships a Merkle inclusion proof for every key — useful when data crosses trust boundaries.

```rust
use prollytree::tree::{ProllyTree, Tree};
use prollytree::storage::InMemoryNodeStorage;

let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::new(), Default::default());
tree.insert(b"user:alice".to_vec(), b"Alice".to_vec());

let proof = tree.generate_proof(b"user:alice");
assert!(tree.verify(proof, b"user:alice", Some(b"Alice")));
```

### Git-backed versioning

The `git` feature stores tree nodes as Git objects, so commits, branches, and merges work natively on key-value state.

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

### Multiple namespaces in one store

`NamespacedKvStore` holds many independent prolly trees in one Git repo. Each namespace owns its own key space and (optionally) its own search indexes; one commit covers them all.

```python
from prollytree import NamespacedKvStore

store = NamespacedKvStore("./data")
store.ns_insert("users",    b"u:alice", b"Alice")
store.ns_insert("settings", b"theme",   b"dark")
store.commit("seed users + settings")

store.branch("experiment")
store.ns_insert("settings", b"theme", b"light")
store.commit("flip theme on experiment")

store.checkout("main")
store.ns_get("settings", b"theme")   # b"dark" — main is unchanged
```

### Optional text / vector search

A namespace can host one or more text indexes that ride on the same storage as the primary tree. Every search hit is just an id; resolve back to the original bytes via the primary tree.

```python
from prollytree import NamespacedKvStore, MiniLmEmbedder

store = NamespacedKvStore("./data")
store.text_index_open("docs", "by_body", MiniLmEmbedder())
store.set_cascade("docs", ["by_body"])      # primary writes auto-index

store.ns_insert("docs", b"doc:1", b"the quick brown fox")
store.ns_insert("docs", b"doc:2", b"a lazy dog asleep on the mat")
store.commit("seed corpus")

for doc_id, distance in store.text_index_search("docs", "by_body", "vulpine animal", k=3):
    print(doc_id, distance, store.ns_get("docs", doc_id))
```

See [`examples/`](examples/) (Rust) and [`python/examples/`](python/examples/) for the full set: namespaces, text search, cascade, merge resolvers, SQL, blob GC.

## Good fits

The combination of content-addressed Merkle structure + Git-style versioning + optional semantic search makes ProllyTree a natural fit for a few non-trivial use cases:

- **Auditable application state.** Anywhere you'd otherwise reach for "an event log + a current-state snapshot" — config systems, feature-flag rollout state, policy rules — gets a real Git history with diff, blame, rollback, and proofs for free.
- **Distributed / multi-replica data.** Two peers that hold the same keys converge to the same root hash. Subtree sharing makes diff and sync `O(changes)`, not `O(corpus)`.
- **AI agent memory.** Per-agent namespaces give isolated key spaces in one store; commits make every memory mutation auditable; branches isolate speculative reasoning; the optional text index gives semantic recall without a separate vector database. The [text-search guide](https://zhangfengcdt.github.io/prollytree/text_search/) walks through this pattern in detail.
- **Versioned analytical datasets.** SQL over a Git-tracked KV store — `git checkout` a historical commit and run the same query against the data as it existed then. See the [SQL guide](https://zhangfengcdt.github.io/prollytree/sql/).
- **Content-addressed indexes.** Any place a Merkle tree already makes sense (verifiable logs, proof systems, gossip-friendly indexes) — ProllyTree gives you the data-structure ergonomics of a B-tree on top.

## Embedders (when you use the text-search feature)

| Embedder | Pulls in | Use it for |
|---|---|---|
| `HashEmbedder` | nothing extra | Tests, demos, exact-match recall |
| `MiniLmEmbedder` | Candle (pure Rust) + ~90 MB weights | Real semantic search, offline-friendly |
| `CallableEmbedder` | your callable | OpenAI, Cohere, sentence-transformers, your own model |

Embedder identity (`id` + `version`) is persisted with the index. Reopening with a mismatched embedder surfaces a clear error — no silent mixing of vectors from different models.

## Feature flags

| Feature | Description | Default |
|---|---|---|
| `git` | Git-backed versioned storage with branching, merging, history | Yes |
| `sql` | SQL query interface via GlueSQL | Yes |
| `proximity` | Vector index + text-search infrastructure (ML-free) | No |
| `proximity_text` | Bundled Candle + all-MiniLM-L6-v2 embedder | No |
| `rocksdb_storage` | RocksDB persistent storage backend | No |
| `python` | Python bindings via PyO3 | No |
| `tracing` | Observability via the `tracing` crate | No |

Python PyPI wheels ship `git`, `sql`, `rocksdb_storage`, `proximity`, and `proximity_text` enabled. Rust users opt in:

```toml
[dependencies.prollytree]
version = "0.4.1-beta"
features = ["git", "sql", "proximity", "proximity_text"]
```

## Documentation

- **[User Guide](https://zhangfengcdt.github.io/prollytree/)** — mkdocs site (architecture, CLI, Python API, examples, theory)
- **[Text Search Guide](https://zhangfengcdt.github.io/prollytree/text_search/)** — design, embedder identity, cascade, merge, externalisation
- **[Browser Demo](https://zhangfengcdt.github.io/prollytree/text_search_demo.html)** — interactive single-page deck of the text-search workflow
- **[Rust API Reference](https://docs.rs/prollytree)** — auto-generated from source
- **[Python Quickstart](python/README.md)** — Python-specific intro
- **[Runnable Examples](examples/)** — verifiable KV, versioning, namespaces, text search, SQL, multi-agent worktrees

## CLI

```bash
cargo install prollytree --features git
git-prolly --help
```

See the [user guide](https://zhangfengcdt.github.io/prollytree/cli/) for the full CLI walkthrough.

## Contributing

Contributions welcome — see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under the Apache License 2.0. See [LICENSE](LICENSE).
