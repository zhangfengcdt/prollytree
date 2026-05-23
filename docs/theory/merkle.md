# Merkle Properties & Proofs

Every node in a prolly tree is addressed by the hash of its serialised contents. That includes internal nodes, which reference their children by hash. The **root hash** therefore authenticates the entire tree.

This page explains what the root hash gives you, how inclusion proofs are constructed, and why "two trees with the same root hash" is a hard claim.

## The root hash

Let `H` be a collision-resistant hash function (ProllyTree uses SHA-256 by default, so `H : {0,1}* → {0,1}^256`).

For any node `N`:

$$
\text{hash}(N) = H(\text{serialize}(N))
$$

and for an internal node whose children are `C_1, C_2, …, C_k`:

$$
\text{serialize}(N) = \bigl[\,(\text{separator}_1, \text{hash}(C_1)),\,\ldots,\,(\text{separator}_k, \text{hash}(C_k))\,\bigr]
$$

The root hash `root = hash(Root)` transitively covers every node and every entry. Changing any byte anywhere in the tree changes at least one hash at each level from the edit up to the root, so **the root hash is a fingerprint of the entire KV set**.

Combined with the [history-independent shape](rolling_hash.md#history-independence-restated), this means:

> If `root(tree_A) == root(tree_B)`, then `tree_A` and `tree_B` hold the same set of key/value pairs — no matter how they were built.

## Inclusion proofs

An **inclusion proof** for a key `k` is the sequence of node serialisations along the path from the leaf containing `k` up to the root, with enough information at each level to recompute that level's hash.

```rust
let proof = tree.generate_proof(b"user:alice");
let ok = tree.verify(proof, b"user:alice", Some(b"Alice Johnson"));
assert!(ok);
```

### What's in the proof

At each level from leaf to root:

1. The **siblings** of the node on the path, in order. Each sibling is represented by its hash only — you don't need the siblings' contents.
2. Enough of the **node on the path** to recompute its hash once you've confirmed the entry at the leaf.

The verifier recomputes hashes level-by-level and checks the final hash equals the root hash the verifier already has.

### What you need to verify

- The **root hash** (trusted out-of-band — typically from a Git commit or a signed publication).
- The **key** and optionally the **value** you claim is in the tree.
- The **proof** returned by `generate_proof`.

You do **not** need the rest of the tree, the storage backend, or any network access.

### Cost

- Proof size: **O(log n)** hashes.
- Verification cost: **O(log n)** hash evaluations.

Both scale with tree height, not with dataset size.

## Non-inclusion (absence) proofs

A prolly tree can also prove that a key is **not** present: return the two adjacent keys in the leaf where `k` would have landed, plus the usual path-to-root hashes. The verifier checks that (a) the two adjacent keys bracket `k` lexicographically, and (b) the leaf's hash matches the expected value. Absence is proven without revealing the rest of the tree.

## Two trees with the same root hash are the same tree

This is the key invariant that makes prolly trees useful for replication.

**Claim.** If `root(A) = root(B)` then `A` and `B` represent the same KV set.

**Sketch of why it holds.**

1. The root hash is computed from the serialisation of the root node, which includes the hashes of all of its children.
2. By collision resistance of `H`, equal root hashes ⇒ equal root serialisations ⇒ equal child hash sets at level 1.
3. Apply inductively: equal hashes at every level ⇒ equal serialisations at every level ⇒ equal leaves.
4. Equal leaves ⇒ equal KV sets.

This argument works only because prolly trees are **history-independent** — two trees with the same KV set actually are byte-equal. In a classical Merkle tree the argument fails because two trees with the same data can have different shapes (and therefore different root hashes).

## Using the root hash in practice

### As a data fingerprint

Expose the root hash at every commit. `VersionedKvStore` does this automatically — each commit stores its tree's root hash, and diffing commits becomes a tree-level diff.

### As a replication primitive

Two peers comparing datasets:

```
peer A: root = 0xabcd…
peer B: root = 0xabcd…  →  same data, done
```

If the roots differ:

```
1. Fetch each side's root node (one node, O(1)).
2. Compare children pairwise — skip the ones whose hashes already match.
3. Recurse into the children that differ.
```

You exchange only the *differing* subtrees. This is the pattern behind systems like [Dolt](https://github.com/dolthub/dolt) and our own `VersionedKvStore` diff.

### As an audit trail

The root hash at each commit is a tamper-evident witness. Publish a signed root hash, and any auditor can verify claims about any key's value at that commit by running `verify(proof, key, value)` — no need to trust the replica serving the proof.

## Worked example

```rust
use prollytree::tree::{ProllyTree, Tree};
use prollytree::storage::InMemoryNodeStorage;

let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::new(), Default::default());
tree.insert(b"user:alice".to_vec(), b"Alice".to_vec());
tree.insert(b"user:bob".to_vec(),   b"Bob".to_vec());

let root = tree.root_hash();            // publish this
let proof = tree.generate_proof(b"user:alice");

// Verifier side: given only `root` and `proof`, check alice's value.
assert!(tree.verify(proof, b"user:alice", Some(b"Alice")));
```

A verifier can be given just `root`, `"user:alice"`, `"Alice"`, and the proof — and confirm the claim without trusting whoever served the data.

## Next

- **[Versioning & Merge](versioning.md)** — how commits, branches, and three-way merges stack on top of the root-hash invariant.
- Return to the **[Theory overview](index.md)**.
