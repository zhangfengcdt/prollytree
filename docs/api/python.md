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

- [Examples → Python bindings](../examples/python.md) — worked examples for versioning, SQL, LangMem.
- [FAQ](../faq.md#python-bindings) — common Python-specific questions.
