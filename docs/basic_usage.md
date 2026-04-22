# Basic Usage

This page walks through ProllyTree's core APIs at a slower pace than the [quickstart](quickstart.md). Each section includes a worked example you can paste into a file and run.

## Creating a tree

A ProllyTree is parameterised by the hash size (usually `32` for SHA-256) and a `NodeStorage` backend. For throwaway code, in-memory storage is fine:

```rust
use prollytree::tree::{ProllyTree, Tree};
use prollytree::storage::InMemoryNodeStorage;
use prollytree::config::TreeConfig;

let storage = InMemoryNodeStorage::<32>::new();
let config = TreeConfig::<32>::default();
let mut tree = ProllyTree::new(storage, config);
```

For persistence, swap the backend — `FileNodeStorage`, `RocksDBNodeStorage`, or `GitNodeStorage`. The tree logic is identical. See [Storage Backends](storage.md).

## Reading and writing

```rust
// Insert / update / delete — keys and values are Vec<u8>.
tree.insert(b"alpha".to_vec(), b"1".to_vec());
tree.insert(b"beta".to_vec(),  b"2".to_vec());
tree.update(b"alpha".to_vec(), b"1-updated".to_vec());
tree.delete(b"beta");

// Point lookup. `find` returns the leaf node containing the key; you then
// locate the entry inside it.
if let Some(node) = tree.find(b"alpha") {
    for (i, k) in node.keys.iter().enumerate() {
        if k == b"alpha" {
            assert_eq!(&node.values[i], b"1-updated");
        }
    }
}

// Batch insert — amortises the rebalancing cost.
tree.insert_batch(&[
    (b"k1".to_vec(), b"v1".to_vec()),
    (b"k2".to_vec(), b"v2".to_vec()),
]);
```

Python mirrors the API:

```python
from prollytree import ProllyTree

tree = ProllyTree()
tree.insert(b"alpha", b"1")
tree.update(b"alpha", b"1-updated")
tree.insert_batch([(b"k1", b"v1"), (b"k2", b"v2")])

value = tree.find(b"alpha")  # -> b"1-updated"
```

## Proving data with the Merkle root

Because a prolly tree is content-addressed, equal data produces the equal root hash regardless of insertion order. You can use this for **inclusion proofs** — proving a key exists with a particular value given only the root hash:

```rust
let proof = tree.generate_proof(b"alpha");
let ok = tree.verify(proof, b"alpha", Some(b"1-updated"));
assert!(ok);
```

See [Theory → Merkle Properties & Proofs](theory/merkle.md) for what's happening underneath.

## Versioned key-value store

The `VersionedKvStore` layer turns a ProllyTree into a Git-like KV store. Every commit snapshots the tree's root hash; branching, merging, and diffing all work at the key-value level.

```rust
use prollytree::git::versioned_store::StoreFactory;

let mut store = StoreFactory::git::<32, _>("data")?;  // path must be inside a git repo

store.insert(b"config/theme".to_vec(), b"light".to_vec())?;
let c1 = store.commit("initial config")?;

// Branching.
store.create_branch("dark-mode")?;
store.update(b"config/theme".to_vec(), b"dark".to_vec())?;
store.commit("try dark")?;

// Back to main, merge.
store.checkout("main")?;
store.merge("dark-mode")?;   // three-way merge on the KV level
```

Python:

```python
from prollytree import VersionedKvStore, ConflictResolution

store = VersionedKvStore("./store")
store.insert(b"config:theme", b"light")
store.commit("init")

store.create_branch("dark-mode")
store.update(b"config:theme", b"dark")
store.commit("try dark")

store.checkout("main")
merge = store.merge("dark-mode", ConflictResolution.TakeSource)
```

See [Theory → Versioning & Merge](theory/versioning.md) for the three-way-merge semantics and conflict-resolver choices.

## Conflict resolution

When two branches change the same key, you choose how to resolve the conflict:

| Resolver | Behavior |
|---|---|
| `ConflictResolution.IgnoreAll` | Keep the destination (current branch) value. |
| `ConflictResolution.TakeSource` | Take the incoming branch value. |
| `ConflictResolution.TakeDestination` | Keep the destination value (alias for IgnoreAll in many flows). |

You can also probe for conflicts without applying a merge:

```python
ok, conflicts = store.try_merge("dark-mode")
if not ok:
    for c in conflicts:
        print(c.key, c.source_value, c.destination_value)
```

## SQL queries

With the `sql` feature you can treat the tree as a relational store via GlueSQL.

```python
from prollytree import ProllySQLStore

sql = ProllySQLStore("./sql_store")
sql.execute("CREATE TABLE users (id INTEGER, name TEXT)")
sql.execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob')")
rows = sql.execute("SELECT * FROM users WHERE id = 1")
```

See the [SQL Interface](sql.md) page for the full `git-prolly sql …` surface, including branch-scoped read-only queries.

## Patterns to know

- **Batch your writes.** `insert_batch` / `update_batch` amortise rebalancing and give ~25% speedups over one-at-a-time calls.
- **Pick your storage backend by workload.** Fast tests → `InMemory`. Local dev → `File`. Production → `RocksDB`. See [Storage Backends](storage.md).
- **Commit on logical boundaries.** With `VersionedKvStore`, every `commit(msg)` is a real Git commit — keep them meaningful.
- **Probe before merging.** `try_merge` surfaces conflicts without mutating state, which is especially useful from scripts.

## Next steps

- [Architecture](architecture.md) — the layered design.
- [Theory](theory/index.md) — why the tree is shaped this way.
- [Examples](examples/index.md) — longer end-to-end scenarios.
- [FAQ](faq.md).
