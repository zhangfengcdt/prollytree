# Incremental rebalance — roadmap

This document describes the work needed to remove the `canonicalize`
stop-gap (see `docs/bench-canonicalize-impact.md`) and lift the
history-independence guarantee from "rebuild on every mutation" to a
proper O(log N + affected-chunk) incremental algorithm. The
`#[ignore]`d tests in `src/node.rs` form the acceptance suite for this
work.

## Why a small fix won't do

I explored a targeted fix: a `removed: bool` flag on `ProllyNode` set
by `balance` when a delete empties a node with no right sibling to
absorb it, with the parent dropping the empty child from its children
list. This is the simplest of the documented divergences (the
"empty trailing leaves after delete" bug).

The change compiled cleanly but broke `node::tests::test_delete`. The
new flag interacted badly with the existing `merged` / `split`
signaling: when a delete is followed by a merge-with-next-sibling that
*also* doesn't produce a stable chunk boundary, the result depends on
which flag the parent acts on first. Patching this case by case
quickly turns into the full rewrite.

The lesson: the existing `merged` / `split` two-bit signal is not
expressive enough for a correct incremental algorithm. Any focused
fix has to share state with the rest of the protocol, so the only
maintainable shape is to redesign the protocol.

## Required redesign

A correct incremental rebalance needs to do, after each leaf
modification:

1. **Identify the affected sibling range.** The rolling-hash chunker
   places boundaries based on a window of `min_chunk_size` keys. A
   mutation at key K can shift boundaries within `[K - min, K + min]`
   keys on either side. So the rebalance must load the affected leaf
   plus enough left and right neighbors that all shifted boundaries
   fall within the loaded range.

2. **Merge into one flat buffer and re-chunk.** Run `chunk_content`
   over the merged buffer. The leftmost and rightmost emitted chunks'
   outer boundaries must match what a global rechunk would produce; if
   not, extend the loaded range and repeat.

3. **Signal a range replacement to the parent.** Instead of the
   current `split: bool` / `merged: bool` pair, return a richer
   structure:

   ```rust
   enum BalanceOutcome<const N: usize> {
       Unchanged,
       ReplaceRange {
           remove_left_count: usize,
           remove_right_count: usize,
           new_children: Vec<(Vec<u8>, ValueDigest<N>)>,
       },
   }
   ```

4. **Parent applies the range replacement and recurses.** The parent
   removes the affected slice from its `keys` / `values` and inserts
   the new pivots / child hashes. Then *the parent itself* is
   subject to the same rebalance, because its pivots changed.

5. **Stop at the root.** When recursion reaches the root, if the
   root has multiple chunks it splits into a new root one level up,
   matching the current "is_root_node ⇒ promote" logic.

## Where the existing code falls short

- `Balanced::balance` (`src/node.rs:298`) only loads the *right*
  sibling. The left sibling is never considered. Boundary shifts that
  push leftward are lost.
- `chunk_content` (`src/node.rs:504`) is called on a single node's
  content, so the leftmost chunk's left boundary is fixed at the
  buffer start - which is rarely where a global rechunk would put it.
- `Node::insert` / `Node::delete` (`src/node.rs:648` / `src/node.rs:769`)
  pattern-match on `split` and `merged` to patch a single index in
  the parent. They can't express "remove indices [i-1, i, i+1] and
  insert these 5 new children at position i-1".

## Cross-cutting work

- **Diff and merge paths** (`src/diff.rs`, `src/tree.rs::merge`) walk
  the tree comparing nodes by hash. Once the chunker is canonical
  these become more efficient (more shared subtrees) but the
  algorithms themselves don't need to change.
- **Existing tests** that pin specific traversal strings (e.g.
  `node::tests::test_insert_in_order`) may need to be updated if
  canonical chunking puts boundaries differently than the current
  ad-hoc rebalance. Update the expected strings, don't disable the
  tests.
- **`ProllyTree::canonicalize`** can be removed once the new path
  passes the `#[ignore]`d acceptance suite in `src/node.rs`. Do this
  as a separate commit so the test suite gates the change.

## Out-of-scope for this work

- Tuning the chunker parameters (`min_chunk_size`, `max_chunk_size`,
  `pattern`) for better leaf size distribution.
- Replacing the `entry-count`-based `max_chunk_size` with a byte cap
  (filed elsewhere as a separate issue).
- Refactoring the merge protocol to support more conflict resolution
  policies.

## Verification plan

1. Each `#[ignore]`d test in `src/node.rs` (the
   `test_history_independence_*_root_hash_*` and
   `test_history_independence_*_traversal_*` families) goes from
   `#[ignore]` to passing.
2. The integration matrix in `tests/history_independence.rs` keeps
   passing without `ProllyTree::canonicalize` being called.
3. `cargo bench --bench tree -- insert_single` recovers to within 2x
   of the pre-fix baseline at N=10 000 (today: 26x slower).
4. Full suite (lib + integration + bench compile) stays green under
   all three CI feature flag variants.
