# ProllyTree Python Bindings

[![Documentation](https://img.shields.io/badge/docs-github%20pages-blue)](https://zhangfengcdt.github.io/prollytree/)
[![PyPI](https://img.shields.io/pypi/v/prollytree)](https://pypi.org/project/prollytree/)

**Versioned, namespaced, semantically-searchable storage for AI agents** — Python bindings to the Rust ProllyTree crate.

Give each agent (or agent role) its own namespace inside one Git-versioned store; capture observations, plans, and tool results as key-value pairs; recall by meaning with a built-in vector index that lives alongside the data. Branch the store to try a speculative reasoning path, merge it back when it works, audit what changed between two timestamps — same primitives software developers already use, applied to agent memory.

## Quick Start

### Installation

```bash
pip install prollytree
```

PyPI wheels ship `git`, `sql`, `rocksdb_storage`, `proximity`, and `proximity_text` enabled by default — text search and the bundled MiniLM embedder are available out of the box.

### Agent-memory pattern in 10 lines

```python
from prollytree import NamespacedKvStore, MiniLmEmbedder

store = NamespacedKvStore("./agent_memory")
emb = MiniLmEmbedder()

store.text_index_open("agent:assistant", "by_body", emb)
store.set_cascade("agent:assistant", ["by_body"])    # primary writes auto-index

store.ns_insert("agent:assistant", b"obs:1", b"user prefers dark mode")
store.commit("session 42 memories")

hits = store.text_index_search("agent:assistant", "by_body",
                                "what does the user prefer?", k=3)
```

### Basic tree (when you just want the raw structure)

```python
from prollytree import ProllyTree

tree = ProllyTree()
tree.insert(b"hello", b"world")
tree.find(b"hello")              # b"world"
```

## Documentation

**[Complete Documentation](https://zhangfengcdt.github.io/prollytree/)**

The full documentation includes:
- [Quickstart Guide](https://zhangfengcdt.github.io/prollytree/quickstart/)
- [Python API Reference](https://zhangfengcdt.github.io/prollytree/api/python/)
- [Python Examples](https://zhangfengcdt.github.io/prollytree/examples/python/)
- [Architecture](https://zhangfengcdt.github.io/prollytree/architecture/)
- [Text Search](https://zhangfengcdt.github.io/prollytree/text_search/)

## Features

**For agents**
- **Per-agent namespaces** — many isolated key spaces in one Git repo, atomic across namespaces
- **Semantic recall** — vector / text index inside any namespace; bundled MiniLM, hash, and Python-callable embedders
- **Branchable scratch spaces** — branch the store for speculative reasoning, merge or discard
- **Auditable** — every memory mutation is a Git commit; diff, rewind, three-way merge

**Underneath**
- **Probabilistic B-tree with Merkle properties** — O(log n) ops, cryptographic inclusion proofs
- **Multiple storage backends** — In-memory, File, RocksDB, Git-backed
- **SQL interface** — query memory as relational tables via GlueSQL
- **Cascade + drift management** — atomic dual-write, audit + repair APIs
- **Large-value externalization** — values above a threshold land in content-addressed blobs

## Key Use Cases

### Versioned Storage
```python
from prollytree import VersionedKvStore, StorageBackend

# Default Git backend (recommended for full version control)
store = VersionedKvStore("./data")

# Or explicitly choose a storage backend
store = VersionedKvStore("./data", StorageBackend.Git)      # Full git versioning
store = VersionedKvStore("./data", StorageBackend.File)     # File-based storage
store = VersionedKvStore("./data", StorageBackend.InMemory) # In-memory (volatile)
store = VersionedKvStore("./data", StorageBackend.RocksDB)  # RocksDB (requires rocksdb_storage feature)

# Basic operations
store.insert(b"config", b"production_settings")
commit_id = store.commit("Add production config")

# Branch and experiment
store.create_branch("experiment")
store.insert(b"feature", b"experimental_data")
store.commit("Add experimental feature")

# Merge branches (Git backend only)
store.checkout("main")
store.merge("experiment")

# Diff between branches (Git backend only)
diffs = store.diff("main", "experiment")
for diff in diffs:
    print(f"Key: {diff.key}, Operation: {diff.operation}")

# Cryptographic verification on versioned data
proof = store.generate_proof(b"config")
is_valid = store.verify_proof(proof, b"config", b"production_settings")
```

### Namespaced Storage

`NamespacedKvStore` is the multi-tree counterpart of `VersionedKvStore`. Each
namespace owns its own prolly tree, but every namespace shares a single git
history — `commit`, `branch`, and `checkout` move every namespace together.

```python
from prollytree import NamespacedKvStore

store = NamespacedKvStore("./data")

# Per-namespace primary KV writes. Each namespace owns its own key space —
# the same key in two namespaces resolves independently.
store.ns_insert("users",    b"u:alice", b"Alice")
store.ns_insert("settings", b"theme",   b"dark")
store.commit("seed users + settings")        # one commit, both namespaces

store.branch("experiment")                   # create + switch
store.ns_insert("settings", b"theme", b"light")
store.commit("flip theme on experiment")

store.checkout("main")
store.ns_get("settings", b"theme")           # b"dark" again
store.list_namespaces()                      # ['users', 'settings', ...]
```

Migrating from `VersionedKvStore` is mostly mechanical — `store.insert(k, v)`
becomes `store.ns_insert(namespace, k, v)`. The branching API is `store.branch`
+ `store.checkout` (note: `current_branch` is a property, not a method). See
`python/examples/namespaced_example.py` for a complete walkthrough.

### Vector / Text Search

Any namespace can own zero or more text sub-indexes. A text index turns
documents into vectors via a configurable embedder and gives you top-k
similarity search that is versioned alongside the primary tree — branching and
merging cover both the primary tree and every sub-index atomically.

**The primary KV tree is the source of truth**; the text index stores only
`(id, vector)` pairs. Always write the document body into the primary tree too
— either explicitly or by enabling cascade — so you can resolve search hits
back to text and reindex if the embedder ever changes.

```python
from prollytree import NamespacedKvStore, MiniLmEmbedder

store = NamespacedKvStore("./data")
emb = MiniLmEmbedder()                       # bundled Candle + all-MiniLM-L6-v2

# text_index_open creates or re-opens the index. The embedder's id + version
# are persisted; opening with a mismatched embedder raises a clear error.
store.text_index_open("docs", "by_body", emb)

# Dual write: primary tree (source of truth) + text index (pointer).
docs = {
    b"doc:1": "the quick brown fox",
    b"doc:2": "lazy dog asleep on the mat",
}
for doc_id, text in docs.items():
    store.ns_insert("docs", doc_id, text.encode())
    store.text_index_insert("docs", "by_body", doc_id, text)
store.commit("seed corpus")

# Search returns (id_bytes, distance); resolve back to text via the primary.
for doc_id, score in store.text_index_search("docs", "by_body", "vulpine animal", k=5):
    body = store.ns_get("docs", doc_id).decode()
    print(f"{doc_id} (d={score:.3f}): {body}")
```

Three embedder options are bundled:

```python
from prollytree import HashEmbedder, MiniLmEmbedder, CallableEmbedder

HashEmbedder(dim=384, seed=0)                # deterministic, ML-free; tests / demos
MiniLmEmbedder()                             # bundled Candle + MiniLM-L6-v2 (semantic)
CallableEmbedder(                            # wrap any Python function
    id="openai:text-embedding-3-small",
    version="2024-01",
    dim=1536,
    embed_fn=my_openai_embed,
)
```

Cascade mode replaces the dual-write with a single `ns_insert` — the registered
text indexes auto-mirror every primary write (and primary delete):

```python
store.text_index_open("docs", "by_body", emb)
store.set_cascade("docs", ["by_body"])       # opt-in, per namespace

# One call now writes to both the primary tree AND the text index.
store.ns_insert("docs", b"doc:3", b"branching is a first-class operation")
store.commit("cascade-driven indexing")
```

Other knobs:
- `chunker="line"` splits each document on `\n` and indexes per-line; search
  dedups results back to the document id.
- `audit_text_index(ns, idx)` returns `{orphans_in_index, missing_from_index,
  is_in_sync}` to detect drift; `purge_text_index_orphans(ns, idx)` repairs it.
- `set_externalize_threshold(n)` + `gc_blobs()` push large values into a blob
  store and garbage-collect unreferenced blobs (File / RocksDB backends).

Feature-availability flags let callers fall back gracefully:

```python
import prollytree as p
if p.proximity_text_available:
    emb = p.MiniLmEmbedder()
elif p.proximity_available:
    emb = p.HashEmbedder(384, 0)
else:
    raise RuntimeError("wheel built without proximity features")
```

See `python/examples/text_index_example.py` for a runnable walkthrough covering
cascade, multi-chunk indexing, drift repair, and every embedder.

### SQL Queries
```python
from prollytree import ProllySQLStore

sql_store = ProllySQLStore("./database")
sql_store.execute("CREATE TABLE users (id INT, name TEXT)")
sql_store.execute("INSERT INTO users VALUES (1, 'Alice')")
results = sql_store.execute("SELECT * FROM users WHERE name = 'Alice'")
```

### Probabilistic Trees (raw building block)

When you need the verifiable B-tree without the versioning layer.

```python
from prollytree import ProllyTree

tree = ProllyTree()
tree.insert(b"user:123", b"Alice")
tree.insert(b"user:456", b"Bob")

# Cryptographic verification
proof = tree.generate_proof(b"user:123")
is_valid = tree.verify_proof(proof, b"user:123", b"Alice")
```

## Development

### Building from Source
```bash
git clone https://github.com/zhangfengcdt/prollytree
cd prollytree
./python/build_python.sh --all-features --install
```

### Running Tests
```bash
cd python/tests
python test_prollytree.py
```

## License

Licensed under the Apache License, Version 2.0
