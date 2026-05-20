# ProllyTree

[![Crates.io](https://img.shields.io/crates/v/prollytree.svg)](https://crates.io/crates/prollytree)
[![Documentation](https://docs.rs/prollytree/badge.svg)](https://docs.rs/prollytree)
[![License](https://img.shields.io/crates/l/prollytree.svg)](https://github.com/zhangfengcdt/prollytree/blob/main/LICENSE)
[![Downloads](https://img.shields.io/crates/d/prollytree.svg)](https://crates.io/crates/prollytree)

**Versioned, namespaced, semantically-searchable storage for AI agents** — built on a probabilistic B-tree with Merkle properties.

ProllyTree gives an agent a long-term memory it can branch, merge, and audit like source code: every write is committed to a real Git history, every memory is cryptographically verifiable, and the bundled text index lets the agent recall by meaning rather than exact key. Implemented in Rust with first-class Python bindings.

## Why ProllyTree for agent memory

- **Per-agent namespaces.** Run many isolated agents against one store — each gets its own prolly tree (key space, search index, history) inside the same Git repo. One commit covers every namespace atomically.
- **Semantic recall.** Vector / text indexes live inside any namespace. Search by meaning ("what did the user say about billing last week?") and resolve hits back to the original message via the primary tree.
- **Branchable scratch spaces.** Spin up an `experiment` branch for tool-call replays, A/B prompt strategies, or speculative reasoning. Discard or merge back. Real Git branches — `gh` / `git log` work.
- **Auditable.** Every memory mutation is a Git commit. Diff what an agent learned between two timestamps, rewind to a known-good state, or merge knowledge across agent instances with three-way conflict resolution.
- **Cryptographically verifiable.** Each value carries a Merkle inclusion proof. Useful when an agent's memory crosses trust boundaries (replicas, audit logs, ZK use cases).
- **No standalone vector database needed.** The text index and the primary store share one transaction. No syncing job, no eventual-consistency window between memory and embeddings.

## Features

| Capability | What it gives you |
|---|---|
| **Versioned KV store** | Git-backed branch / commit / diff / three-way merge on raw key-value state |
| **Namespaced KV store** | Many isolated prolly trees in one Git repo, atomic across namespaces |
| **Text / vector search** | Versioned ANN index inside any namespace; bundled MiniLM, hash, and callable embedders |
| **Multi-chunk indexing** | Split docs into chunks at index time, dedup on search by document |
| **Cascade mode** | One primary write auto-mirrors into every registered text index |
| **Drift detection + repair** | `audit_text_index` and `purge_text_index_orphans` |
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
prollytree = { version = "0.3.5-beta", features = ["git", "sql"] }
# Add `proximity` for the text-search surface, `proximity_text` for bundled MiniLM.
```

### Python

```bash
pip install prollytree   # ships with git, sql, proximity, proximity_text by default
```

## Examples

### Versioned + namespaced agent memory

Each agent gets its own namespace; one Git commit covers all of them.

```python
from prollytree import NamespacedKvStore

store = NamespacedKvStore("./agent_memory")

# Two agents, two isolated key spaces, one repo.
store.ns_insert("agent:planner",  b"task:current", b"draft Q3 roadmap")
store.ns_insert("agent:executor", b"task:current", b"deploy feature flag")
store.commit("seed agent state")

# Branch to experiment without disturbing the live store.
store.branch("experiment")
store.ns_insert("agent:planner", b"task:current", b"try alternative plan")
store.commit("alternative planning")

store.checkout("main")
store.ns_get("agent:planner", b"task:current")   # b"draft Q3 roadmap" — main is unchanged
```

### Semantic recall over an agent's memory

The text index lives inside the namespace. Search by meaning, resolve back to the original message.

```python
from prollytree import NamespacedKvStore, MiniLmEmbedder

store = NamespacedKvStore("./agent_memory")
emb = MiniLmEmbedder()                                   # bundled all-MiniLM-L6-v2

store.text_index_open("agent:assistant", "by_body", emb)
store.set_cascade("agent:assistant", ["by_body"])        # primary writes auto-index

# Capture every observation; index updates atomically with the primary tree.
store.ns_insert("agent:assistant", b"obs:1",
                b"user prefers dark mode interfaces")
store.ns_insert("agent:assistant", b"obs:2",
                b"user is learning ML with Python")
store.commit("memories from session 42")

# Recall by meaning, not exact phrasing.
for obs_id, distance in store.text_index_search(
        "agent:assistant", "by_body", "what's the user's interface preference?", k=3):
    body = store.ns_get("agent:assistant", obs_id).decode()
    print(f"{obs_id} (d={distance:.3f}): {body}")
```

### Verifiable raw KV (when you want proofs)

The raw `ProllyTree` ships a Merkle inclusion proof for every key.

```rust
use prollytree::tree::{ProllyTree, Tree};
use prollytree::storage::InMemoryNodeStorage;

let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::new(), Default::default());
tree.insert(b"user:alice".to_vec(), b"Alice".to_vec());

let proof = tree.generate_proof(b"user:alice");
assert!(tree.verify(proof, b"user:alice", Some(b"Alice")));
```

### Git-backed flat store (single key space)

```rust
use prollytree::git::versioned_store::StoreFactory;

let mut store = StoreFactory::git::<32, _>("data")?;
store.insert(b"config/api_key".to_vec(), b"v1".to_vec())?;
store.commit("Initial config")?;

store.create_branch("experimental")?;
store.insert(b"config/api_key".to_vec(), b"v2".to_vec())?;
store.commit("Try new key")?;
```

See [`examples/`](examples/) (Rust) and [`python/examples/`](python/examples/) (Python) for namespaces, text search, cascade, merge resolvers, and SQL.

## Embedders

The text-search surface ships three embedders. All three plug into `text_index_open(...)` the same way.

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
version = "0.3.5-beta"
features = ["git", "sql", "proximity", "proximity_text"]
```

## Performance

Benchmarks on Apple M3 Pro / 18 GB RAM:

- Insert: ~8–21 µs (scales O(log n))
- Lookup: ~1–3 µs (sub-linear due to caching)
- Memory: ~100 bytes per key-value pair
- Batch operations: ~25% faster than individual ops

Run `cargo bench` to reproduce. The vector index uses a lazy-rebuild pattern — mutations are amortised, the first search after a mutation pays the rebuild cost.

## Testing

```bash
# Rust tests
cargo test --features "git sql proximity"

# Python tests (build bindings first; --all-features includes proximity)
./python/build_python.sh --all-features --install
python -m pytest python/tests/
```

## Documentation

- **[User Guide](https://zhangfengcdt.github.io/prollytree/)** — mkdocs site (architecture, CLI, Python API, examples, theory)
- **[Text Search Guide](https://zhangfengcdt.github.io/prollytree/text_search/)** — design, embedder identity, cascade, merge, externalisation
- **[Browser Demo](https://zhangfengcdt.github.io/prollytree/text_search_demo.html)** — interactive single-page demo of the text-search workflow
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
