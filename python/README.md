# ProllyTree Python Bindings

[![Documentation](https://img.shields.io/badge/docs-read%20the%20docs-blue)](https://prollytree.readthedocs.io/en/latest/)
[![PyPI](https://img.shields.io/pypi/v/prollytree)](https://pypi.org/project/prollytree/)

Python bindings for ProllyTree - a probabilistic tree data structure that combines B-trees and Merkle trees for efficient, verifiable data storage.

## Quick Start

### Installation

```bash
pip install prollytree
```

### Basic Usage

```python
from prollytree import ProllyTree

# Create a tree and insert data
tree = ProllyTree()
tree.insert(b"hello", b"world")
value = tree.find(b"hello")  # Returns b"world"
```

## Documentation

**[Complete Documentation](https://prollytree.readthedocs.io/en/latest/)**

The full documentation includes:
- [Quickstart Guide](https://prollytree.readthedocs.io/en/latest/quickstart.html)
- [API Reference](https://prollytree.readthedocs.io/en/latest/api.html)
- [Examples](https://prollytree.readthedocs.io/en/latest/examples.html)
- [Advanced Usage](https://prollytree.readthedocs.io/en/latest/advanced.html)

## Features

- **Probabilistic Trees** - High-performance data storage with automatic balancing
- **Versioned Storage** - Git-like version control for key-value data
- **Multiple Storage Backends** - Choose from Git, File, InMemory, or RocksDB storage
- **Cryptographic Verification** - Merkle proofs for data integrity across trees and versioned storage
- **SQL Queries** - Query your data using SQL syntax
- **Namespaced Storage** - Multiple isolated KV trees in one versioned store
- **Vector / Text Search** - Versioned ANN index with optional bundled MiniLM embedder

## Key Use Cases

### Probabilistic Trees
```python
from prollytree import ProllyTree

tree = ProllyTree()
tree.insert(b"user:123", b"Alice")
tree.insert(b"user:456", b"Bob")

# Cryptographic verification
proof = tree.generate_proof(b"user:123")
is_valid = tree.verify_proof(proof, b"user:123", b"Alice")
```


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
