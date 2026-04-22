# Probabilistic Balancing

The previous page, [Prolly Trees](prolly_tree.md), described the data structure at a high level. This page drops one level lower: **how does the tree actually decide where one node ends and the next begins?**

## The balancing predicate

Classical B-trees balance on *count*: a node splits when it has more than some number of entries. That policy makes the tree's shape depend on insertion order.

A prolly tree balances on *content*: it applies a deterministic, local predicate to the keys (and sometimes adjacent hashes) to decide whether the current position is a node boundary. Because the predicate is a pure function of the content, **the same key set produces the same tree**.

There are two common predicate families, and ProllyTree ships with both:

### 1. Max-nodes rule (default)

Enabled via the `prolly_balance_max_nodes` feature (on by default).

A boundary falls wherever the local key hash is "small enough" relative to a threshold that encodes the target average node size. Conceptually: treat the hash as a uniform 32-byte integer, and split when `hash(k) mod M == 0` for some modulus `M`. With hash size `H` and modulus `M`:

- **Expected node size** ≈ `M` keys.
- **Node size distribution** is approximately geometric with parameter `1/M`, concentrated around `M`.
- **Expected tree height** for `n` keys is approximately `log_M(n)`.

This is the rule you want for most workloads. It's fast, stateless, and doesn't need to look at neighbours.

### 2. Rolling-hash rule

Enabled via the `prolly_balance_rolling_hash` feature.

Instead of hashing a single key, the predicate is computed over a sliding window of recent keys, similar to the [rolling hash](https://en.wikipedia.org/wiki/Rolling_hash) construction used by [content-defined chunking](https://en.wikipedia.org/wiki/Content-defined_chunking) in tools like `rsync` and `restic`.

The win over the max-nodes rule is that the boundary also depends on a short neighbourhood of keys, which makes the boundary more robust to single-key edits — you get slightly *smaller* diffs across edits at the cost of a bit more work per insertion.

For most applications the default max-nodes rule is the right choice. Enable the rolling-hash variant only if you have measured that diff size matters more than insertion latency.

## Why this gives O(log n) depth

Treat each potential boundary as an independent Bernoulli trial with success probability `p = 1/M`:

- **Node sizes** are geometrically distributed with mean `M`.
- **The number of nodes at level `ℓ`** is roughly `n / M^ℓ`, because every level collapses the count by a factor of `M` (nodes at the lower level become entries at the upper level).
- **The tree has a single root** when `n / M^ℓ ≈ 1`, i.e. `ℓ ≈ log_M(n)`.

So the expected height is logarithmic with base `M`, and `M` is your tuning knob: larger `M` gives shallower trees but heavier leaves. The default configuration targets leaves that fit comfortably in a few KB.

## Tight control via `TreeConfig`

If you need to override the defaults — for instance when benchmarking or when your key distribution is pathological — use `TreeConfig`:

```rust
use prollytree::config::TreeConfig;

let config = TreeConfig::<32> {
    base: 4,      // influences branching fanout
    modulus: 64,  // target node size ≈ this many entries
    ..Default::default()
};
```

From Python:

```python
from prollytree import ProllyTree, TreeConfig

config = TreeConfig(base=4, modulus=64)
tree = ProllyTree(config=config)
```

A few things to know:

- **`modulus` is your dominant lever.** Higher `modulus` → fewer, wider leaves → fewer node loads per lookup, but each leaf is bigger and re-insertion touches more data.
- **`base` affects internal-node branching.** Higher base → shallower trees but wider internal nodes.
- **Keys should have well-distributed hashes.** If they don't (for example, all keys share a long common prefix whose hash collapses to the same value), the balancing rule degenerates and you can end up with very skewed node sizes. The fix is to use a good hash — which ProllyTree does by default.

## History independence, restated

The point of a content-defined predicate is that **the set of boundaries is a function of the key set, not the order of operations**. That yields three properties that are hard to get any other way:

1. **Build order doesn't matter.** Insert `{a, b, c, d, e}` in any permutation — you get the same tree and the same root hash.
2. **Bulk load = incremental load.** There's no separate "build a tree from a sorted list" codepath that produces a different result; the incremental path converges to the same shape.
3. **Replicas converge without coordination.** Two nodes that receive the same writes — possibly in different orders — have bit-identical stores and can verify agreement by comparing 32 bytes.

## What about concurrent writes?

Within a single tree instance, writes are serialised — see [Architecture](../architecture.md#concurrency). Under the versioning layer, concurrent *branches* are completely independent; they converge at merge time via [three-way merge](versioning.md).

## Next

- **[Merkle Properties & Proofs](merkle.md)** — how the shape invariant turns into a cryptographic guarantee.
- **[Versioning & Merge](versioning.md)** — what happens when you put commits, branches, and merges on top.
