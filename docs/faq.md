# FAQ

## General

### What is a prolly tree, in one sentence?

A B-tree whose node boundaries are chosen by a content-defined hash predicate, which makes the tree shape a function of the key set alone — so the root hash becomes a stable fingerprint. See [Prolly Trees](theory/prolly_tree.md) for the full story.

### How is this different from a Merkle tree?

A plain Merkle tree is content-addressed, but two Merkle trees with the same data can have different shapes (and root hashes). A prolly tree is **history-independent**: identical KV sets yield identical trees, byte for byte.

### How is this different from a B-tree?

A classical B-tree rebalances on *count*, so shape depends on insertion order. A prolly tree rebalances on a content-defined predicate — same content means same shape regardless of order. See [Probabilistic Balancing](theory/rolling_hash.md).

### How is this different from Git?

Git versions files; it merges with text-level diffs. `git-prolly` versions structured key-value data and merges at the KV level using a three-way algorithm over Merkle subtrees. You can use Git on top of ProllyTree (the default `git` backend is a real Git repo) for remotes, tags, etc.

### When should I *not* use ProllyTree?

- When a single SQLite file would do.
- When you need a full relational database (joins across millions of rows, transactions spanning many tables with isolation guarantees). GlueSQL is embedded but not a replacement for Postgres.
- When you need strong multi-writer concurrency on a single branch. ProllyTree concurrency is per-branch; concurrent writers to the *same* branch should serialise.

## Usage

### How big can a store get before performance degrades?

RocksDB-backed stores have been tested to the low millions of keys without tuning. File-backed stores should be kept to tens of thousands of keys — one file per node doesn't scale. See [Storage Backends](storage.md) for guidance.

### Are commits cheap?

Yes. A commit records the current root hash plus the usual Git commit metadata. Tree nodes that didn't change aren't re-serialised — the new root hash just references the same child hashes as the previous commit.

### Do I need to use Git?

Not necessarily. The tree works on top of `InMemoryNodeStorage`, `FileNodeStorage`, or `RocksDBNodeStorage` without Git. But if you want commits, branches, and merges, the `VersionedKvStore` needs the `git` feature.

### Can I use a custom hash function?

The tree is parameterised by digest length (commonly `32` for SHA-256) but the hash function itself is currently fixed. If you have a specific need, open an issue.

### Can two processes write to the same Git-backed store?

Use `StoreFactory::git_threadsafe` (or the worktree manager) for multi-threaded access. For multi-*process* writers on the same branch, serialise via an external lock — Git's own locking is per-repository and not sufficient for arbitrary concurrent writers.

## Merging

### What happens when two branches change the same key?

The merge engine detects the conflict and delegates to a **conflict resolver**. Built-in options: `IgnoreAll` (keep destination), `TakeSource`, `TakeDestination`. You can also plug in a custom resolver. See [Theory → Versioning & Merge](theory/versioning.md).

### Can I probe for conflicts without merging?

Yes:

```python
ok, conflicts = store.try_merge("feature")
```

`try_merge` walks the diff and reports conflicts without mutating state.

### Why did my merge succeed silently when I expected a conflict?

You're almost certainly using `ConflictResolution.IgnoreAll` (the default in some flows). That's not technically a silent merge — it's the documented "keep destination" behaviour — but it can be surprising. Use `try_merge` first if you want to know before committing.

## Python bindings

### Does `pip install prollytree` include SQL and Git support?

Yes. The published wheel is built with the `python` + `sql` features and the `git` feature is on by default.

### Where are the Python docs?

Here, in the [Python API](api/python.md) section, and with examples in [Examples → Python bindings](examples/python.md). The old Sphinx-based docs on Read the Docs may still be reachable but are being replaced by this site.

### Can I use ProllyTree as a LangGraph / LangMem backend?

Yes — ProllyTree is designed with AI agent memory in mind. See the [LangMem example](examples/python.md#langmem-integration-for-ai-agent-memory) and the [Memoir project](https://github.com/zhangfengcdt/memoir), which builds a full semantic memory system on top of ProllyTree.

## SQL

### Which SQL engine does ProllyTree use?

[GlueSQL](https://github.com/gluesql/gluesql). It's embedded (no external process) and speaks a useful subset of SQL. See [SQL Interface](sql.md) for the supported features and known limitations.

### Can I run SQL on a historical commit?

Yes, read-only:

```bash
git-prolly sql -b v1.0 "SELECT COUNT(*) FROM users"
```

Write queries are rejected when `-b` is set to keep historical commits immutable.

### Why `SELECT * FROM users` returns `I64(1)` instead of `1`?

GlueSQL's default table output includes the wire type. Use `-o json` or `-o csv` for a cleaner shape.

## Storage backends

### Which backend should I use in production?

Most likely **RocksDB**. It's the only backend tuned for write-heavy, large-dataset workloads. `File` is fine for small datasets and debugging; `InMemory` is for tests. The **Git backend** is production-capable *as long as you commit* — experimental raw Git object storage (without commits) is explicitly unsafe because of `git gc`. See [Storage Backends](storage.md).

## Contributing / support

### Where do I report bugs?

[github.com/zhangfengcdt/prollytree/issues](https://github.com/zhangfengcdt/prollytree/issues).

### Where do I see the Rust API?

[docs.rs/prollytree](https://docs.rs/prollytree) — auto-generated from the source. The [Rust API](api/rust.md) page here is a pointer.

### Is there a Discord / Slack?

Not currently. Open an issue or a discussion on GitHub.
