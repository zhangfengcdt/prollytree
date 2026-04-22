# Theory

This section explains the ideas that make ProllyTree work — not just how to call the API, but **why the data structure is shaped the way it is** and what invariants you can rely on.

If you've used a B-tree, a Merkle tree, or Git, each of the pieces below will already feel familiar. Prolly trees combine them in a specific way so that you get:

- **O(log n) operations** (from the B-tree side),
- **Content-addressed, verifiable structure** (from the Merkle side),
- **A shape that depends only on the data, not on insertion order** (the "prolly" / probabilistic balancing rule), and
- **Cheap three-way merges over real key-value sets** (from the Git-backed versioning layer).

## Read in this order

1. **[Prolly Trees](prolly_tree.md)** — the core data structure. What a prolly tree is, why it exists, and how it compares to classical B-trees and Merkle trees.
2. **[Probabilistic Balancing](rolling_hash.md)** — how node boundaries are chosen by a rolling-hash predicate, and why this gives you history-independent shape with O(log n) depth.
3. **[Merkle Properties & Proofs](merkle.md)** — how inclusion proofs are constructed, what the root hash tells you, and why two trees with the same root *are* the same tree.
4. **[Versioning & Merge](versioning.md)** — how the versioned KV store layers commits, branches, and three-way merges on top of the tree, and what conflict resolvers you can plug in.

## Why this matters in practice

| Design property | Where it shows up |
|---|---|
| Content-defined shape | Replicas converge. Two peers that independently inserted the same key/value set reach the same root hash without any coordination. |
| O(log n) operations | Point lookups, range scans, and proofs are all cheap on arbitrarily large stores. |
| Merkle addressing | Any subtree can be proven correct given only its hash. You can sync peers by exchanging differing subtrees (similar to a Merkle sync), not by replaying all writes. |
| Git-like versioning | Commits, branches, and `git log`-style history come for free on top of the tree. `git-prolly` uses real Git for the DAG. |
| KV-level three-way merge | Merges work on logical key-value changes, not on diffs of serialised bytes, so you never see spurious conflicts from reordered leaves. |

## Related reading

- The academic and engineering roots: Dolt's [blog post on prolly trees](https://www.dolthub.com/blog/2020-04-01-how-dolt-stores-table-data/) is a readable intro from a different implementation. The construction here is the same family with a few practical differences noted in [Probabilistic Balancing](rolling_hash.md).
- Merkle trees in general: the Wikipedia entry is the most concise reference.
- Prolly trees used as an AI-agent memory substrate: see [Memoir](https://github.com/zhangfengcdt/memoir), which builds semantic memory on top of this library.
