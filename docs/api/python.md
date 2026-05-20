# Python API

The Python package `prollytree` exposes the full Rust surface via PyO3. This page is a hand-written reference — for cross-linked source-level docs, see the `.pyi` stubs shipped with the wheel (they also power IDE autocomplete).

Install with `pip install prollytree`. See [Installation → Python bindings](../installation.md#python-bindings) for build-from-source details.

## `ProllyTree`

The low-level tree. Stores `bytes → bytes` with Merkle properties.

```python
from prollytree import ProllyTree, TreeConfig

tree = ProllyTree()                              # in-memory, default config
tree = ProllyTree(config=TreeConfig(modulus=64)) # tuned config
tree = ProllyTree(storage_type="file",
                  path="/path/to/data")          # persistent
```

### Core operations

| Method | Notes |
|---|---|
| `insert(key: bytes, value: bytes)` | Create or overwrite. |
| `update(key: bytes, value: bytes)` | Requires key to exist. |
| `delete(key: bytes) -> bool` | True if a key was removed. |
| `find(key: bytes) -> bytes \| None` | Point lookup. |
| `insert_batch(items: list[tuple[bytes, bytes]])` | Amortised rebalancing. |
| `root_hash() -> bytes` | Stable fingerprint of the KV set. |
| `generate_proof(key: bytes)` | Returns a proof object. |
| `verify(proof, key: bytes, value: bytes \| None) -> bool` | Validate inclusion / absence. |

See [Theory → Merkle Properties & Proofs](../theory/merkle.md) for what proofs contain.

## `TreeConfig`

```python
from prollytree import TreeConfig

cfg = TreeConfig(base=4, modulus=64)
```

- `base` — internal-node fanout hint.
- `modulus` — target average leaf size. Larger ⇒ shallower trees, bigger leaves.

See [Probabilistic Balancing](../theory/rolling_hash.md) for tuning guidance.

## `VersionedKvStore`

The Git-backed versioned key-value store. Exposes commits, branches, diffs, and merges.

```python
from prollytree import VersionedKvStore, ConflictResolution

store = VersionedKvStore("/path/to/store")

store.insert(b"user:alice", b"Alice")
store.commit("seed")

store.create_branch("feature")
store.update(b"user:alice", b"Alice Smith")
store.commit("rename")

store.checkout("main")
store.merge("feature", ConflictResolution.TakeSource)
```

### Core operations

| Method | Notes |
|---|---|
| `insert(key, value)` / `update(key, value)` / `delete(key)` | Staged on the current branch. |
| `get(key) -> bytes \| None` | Read the current branch. |
| `commit(message: str) -> str` | Returns the commit hash. |
| `log() -> list[dict]` | Commit history for the current branch. |
| `status()` | Staged changes not yet committed. |
| `create_branch(name)` / `checkout(name)` | Branch management. |
| `merge(branch, resolver=ConflictResolution.IgnoreAll) -> str` | Three-way merge; returns merge-commit hash. |
| `try_merge(branch) -> (bool, list[MergeConflict])` | Probe without applying. |
| `diff(from_ref, to_ref)` | List of `(key, op, old, new)` tuples. |
| `history(key)` | Every commit that touched `key`. |
| `keys_at(ref)` | Keys that existed at a given commit/branch. |

### `ConflictResolution`

Strategy enum for `merge()`.

| Value | Meaning |
|---|---|
| `ConflictResolution.IgnoreAll` | Keep destination value. |
| `ConflictResolution.TakeSource` | Prefer incoming value. |
| `ConflictResolution.TakeDestination` | Prefer current value. |

### `MergeConflict`

Value class returned by `try_merge`. Fields: `key`, `base_value`, `source_value`, `destination_value`.

See [Theory → Versioning & Merge](../theory/versioning.md) for the algorithm.

## `NamespacedKvStore`

A multi-tree counterpart of `VersionedKvStore`. Each namespace owns its own prolly tree (and optionally one or more text sub-indexes); all namespaces share one git history, so `commit`, `branch`, `checkout`, `merge` move every namespace atomically.

```python
from prollytree import NamespacedKvStore

store = NamespacedKvStore("/path/to/store")     # init or open

store.ns_insert("users",    b"u:alice", b"Alice")
store.ns_insert("settings", b"theme",   b"dark")
store.commit("seed users + settings")

store.branch("experiment")                      # create + switch
store.ns_insert("settings", b"theme", b"light")
store.commit("flip theme")
store.checkout("main")                          # both namespaces snap back
```

### Core operations

| Method | Notes |
|---|---|
| `ns_insert(ns, key, value)` / `ns_get(ns, key)` / `ns_delete(ns, key)` | Per-namespace primary KV. |
| `ns_list_keys(ns) -> list[bytes]` | All keys in a namespace. |
| `list_namespaces() -> list[str]` | Every namespace known to the store. |
| `delete_namespace(prefix) -> bool` | Drop a namespace wholesale. |
| `get_namespace_root_hash(prefix)` | Per-namespace fingerprint for change detection. |
| `commit(message: str) -> str` | One commit covering every dirty namespace + sub-index atomically. |
| `branch(name)` / `checkout(name)` | Create-and-switch / switch existing branch. |
| `merge(source_branch, ...) -> str` | Per-namespace 3-way merge. |
| `current_branch` (property) | Current branch name (not a method). |

### Text indexing

Each namespace can host text sub-indexes. The primary KV tree is the source of truth; the index stores `(id, vector)` pairs only. See [Text Search](../text_search.md) for the full design.

| Method | Notes |
|---|---|
| `text_index_open(ns, idx, embedder, chunker=None)` | Create or re-open. Persists the embedder identity on first open and validates it on every reopen. `chunker` is `"identity"` (default) or `"line"`. |
| `text_index_insert(ns, idx, id: bytes, text: str)` | Embed + chunk + insert. Same `id` upserts. |
| `text_index_delete(ns, idx, id: bytes) -> bool` | Prefix-scans + removes every chunk for the doc. |
| `text_index_search(ns, idx, query: str, k: int) -> list[tuple[bytes, float]]` | Top-k documents (deduped across chunks) ordered by ascending distance. |
| `text_index_len(ns, idx)` / `text_index_chunk_count(ns, idx)` | Distinct documents vs raw chunks. |
| `text_index_drop(ns, idx) -> bool` | Drop in-memory cache + Python-side embedder/chunker registration. |

### Cascade

`ns_insert` and `ns_delete` can auto-mirror into registered text indexes — no dual-write needed.

| Method | Notes |
|---|---|
| `set_cascade(ns, [idx_name, ...])` | Opt in. Runtime-only (not persisted). |
| `clear_cascade(ns)` | Opt out. |
| `cascade_for_namespace(ns) -> list[str] \| None` | Inspect current cascade list. |

### Drift management

| Method | Notes |
|---|---|
| `audit_text_index(ns, idx) -> dict` | `{"orphans_in_index", "missing_from_index", "is_in_sync"}`. |
| `purge_text_index_orphans(ns, idx) -> int` | Remove index entries that have no primary row. |

### Externalisation + blob GC

| Method | Notes |
|---|---|
| `set_externalize_threshold(bytes: int \| None)` | Values larger than `bytes` are stored as content-addressed blobs (only a 44-byte envelope inline). `None` disables. |
| `externalize_threshold() -> int \| None` | Current threshold. |
| `gc_blobs() -> dict` | `{"total", "referenced", "removed", "errors"}`. File / RocksDB backends only. |

## Embedders

Three embedder classes are exposed. All three plug into `text_index_open(...)` identically.

### `HashEmbedder`

Deterministic SHA-256-based, no extra deps. Not semantic — useful for tests and exact-match lookup.

```python
from prollytree import HashEmbedder
emb = HashEmbedder(dim=384, seed=0)
emb.id          # 'prollytree:hash-embedder/v1'
emb.dim         # 384
emb.embed("text")
```

### `MiniLmEmbedder`

Bundled Candle + `sentence-transformers/all-MiniLM-L6-v2` (384-d). Real semantic search. First call downloads ~90 MB of weights into `$PROLLYTREE_EMBEDDER_CACHE`. Requires a wheel built with the `proximity_text` feature (default on PyPI).

```python
from prollytree import MiniLmEmbedder
emb = MiniLmEmbedder()                                       # defaults
emb = MiniLmEmbedder(model_id="...", revision="main")        # override either field
```

### `CallableEmbedder`

Wrap any Python embedding function — OpenAI, Cohere, sentence-transformers, your own pipeline.

```python
from prollytree import CallableEmbedder
emb = CallableEmbedder(
    id="openai:text-embedding-3-small",       # persisted with the index
    version="2024-01",                        # change when distribution changes
    dim=1536,
    embed_fn=lambda text: ...,                # returns list[float] of length `dim`
)
```

The wrapped callable runs under the GIL. Dim mismatches surface as a clear `ValueError`.

## Feature-availability flags

The package exposes booleans that mirror the wheel's compiled features. Useful for fallback in libraries that want to remain importable on slim wheels:

```python
import prollytree as p
p.sql_available              # ProllySQLStore present
p.git_available              # WorktreeManager / WorktreeVersionedKvStore present
p.namespaced_available       # NamespacedKvStore present
p.proximity_available        # HashEmbedder / CallableEmbedder + text-index methods present
p.proximity_text_available   # MiniLmEmbedder present
```

## `ProllySQLStore`

GlueSQL adapter — treat the store as relational tables.

```python
from prollytree import ProllySQLStore

sql = ProllySQLStore("/path/to/store")
sql.execute("CREATE TABLE users (id INTEGER, name TEXT)")
sql.execute("INSERT INTO users VALUES (1, 'Alice')")
rows = sql.execute("SELECT * FROM users WHERE id = 1")
```

`.execute(query: str, params: tuple | None = None)` returns a list of rows for `SELECT` and an affected-row count for DML. See the [SQL Interface](../sql.md) for supported SQL features.

## Exceptions

The bindings raise a `ProllyTreeError` hierarchy:

- `ProllyTreeError` — base class.
- `StorageError` — I/O / backend problems.
- `MergeError` — merge failures (when you refuse to plug in a resolver).
- `SqlError` — GlueSQL failures.

Catch selectively where it matters:

```python
from prollytree import ProllyTreeError, StorageError

try:
    store = VersionedKvStore("/some/path")
    store.insert(b"k", b"v")
except StorageError as e:
    print("storage failed:", e)
except ProllyTreeError as e:
    print("generic tree failure:", e)
```

## Pointers

- [Examples → Python bindings](../examples/python.md) — worked examples for versioning, SQL, namespaces, and text indexing.
- [FAQ](../faq.md#python-bindings) — common Python-specific questions.
