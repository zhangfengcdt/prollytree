# Adopting Dolt's streaming-chunker architecture

This document captures the findings from reading
`github.com/dolthub/dolt/go/store/prolly/tree/` and the plan to port
that design into ProllyTree.

## What Dolt does

### Separate the static tree from the pending edits

`MutableMap[K, V, O, M]` (`mutable_map.go`) wraps two things: the
static prolly tree (`Static M`) and a sorted skip-list of pending
edits (`Edits *skip.List`). Reads check `Edits` first and fall through
to `Static` on miss. Writes only ever touch `Edits` - the tree itself
is never mutated in place.

### Build a new tree from edits via a streaming chunker

When edits need to be materialized, `tree.ApplyMutations` (`mutator.go`)
takes a sorted iterator of `(key, Option<value>)` mutations and
produces a new root in a single streaming pass.

The structure is:

```
chunker (level 0)               <-- accumulates items, splits on boundary
   |
   v   (emits novelNode on chunk boundary)
chunker (level 1)               <-- accumulates (lastKey, addr) of level-0 chunks
   |
   v   (emits novelNode on chunk boundary)
chunker (level 2)
   ...
```

Each `chunker[S]` (`chunker.go`) holds:
  - `builder *nodeBuilder[S]`: accumulates keys/values for the
    currently in-progress chunk at this level
  - `splitter nodeSplitter`: decides where to emit a chunk boundary
  - `parent *chunker[S]`: created lazily; receives `(lastKey, hash)`
    pairs when this level emits a chunk
  - `cur *cursor`: tracks the read position in the **old** tree at
    this level

API: `AddPair`, `UpdatePair`, `DeletePair`, and `Done`. Internally:

  1. `AddPair(key, value)` -> `append(key, value, 1)` -> writes
     into `builder`, calls `splitter.Append(key, value)`, then
     `splitter.CrossedBoundary()` returns true -> emit a sealed
     chunk via `handleChunkBoundary`.
  2. `handleChunkBoundary` writes the in-progress chunk as a `Node`,
     gets its address, and recurses into the parent chunker with
     `(lastKey, addr)`. Then `splitter.Reset()`.
  3. `Done()` finalizes the in-progress chunk at every level and
     recursively asks the topmost parent for its single root.

### The splitter is deterministic and resets at every boundary

Two splitter implementations live in `node_splitter.go`. Both share
the critical property:

```go
func (sns *rollingHashSplitter) Reset() {
    sns.crossedBoundary = false
    sns.offset = 0
    sns.bz = buzhash.NewBuzHash(sns.window)
}
```

A chunk's split decision is purely a function of the bytes added
since the last `Reset()` - i.e. the items in that chunk only.
**This is the property that gives history independence**: feed the
same sorted items into a freshly-reset chunker and you get the same
boundaries every time.

The two splitters:

- `rollingHashSplitter`: buzhash over key+value bytes, with a
  dynamic pattern that gets easier to match as the chunk grows
  (`rollingHashPattern` shifts down by 1 every 1024 bytes). This
  shapes the chunk size distribution closer to binomial.
- `keySplitter` (the default): xxhash of the key only, compared
  against a Weibull-distribution threshold. Avoids the per-byte
  rolling-hash work for keys with non-trivial values.

### The cursor fast-forward optimization

`chunker.advanceTo(next *cursor)` is what makes Dolt's edits
incremental. The chunker holds a cursor `tc.cur` pointing at where
it's reading from in the *old* tree. `next` points at where the
*next* edit will land. The body of `advanceTo`:

1. Walk forward by re-feeding items from the old tree through
   the local splitter (so we re-create the same boundaries we had
   before the edit).
2. As soon as the new tree's chunk boundary aligns with the old
   tree's chunk boundary, we know the rest of this chunk is
   unchanged. Synchronize the cursors and *recurse to the parent
   chunker* - moving the parent's cursor one chunk forward without
   reading the leaves at all.
3. After parents are aligned, fast-forward this level's cursor to
   the edit point.

This is what brings the cost from O(tree size) per edit down to
O(log N + edit affected region).

## Why our current implementation is structurally different

`Balanced::balance` (`src/node.rs:298`) mutates the tree in place by
loading the modified node and *one* right sibling and re-chunking the
merged contents. Because there's no streaming model and no boundary
reset, the local rebalance doesn't recreate the canonical chunks a
fresh build would produce - existing internal-node structure drifts.

`ProllyTree::canonicalize` papers over this by walking all leaves and
rebuilding from scratch on every mutation. It's correct but O(N) per
op (see `docs/bench-canonicalize-impact.md`).

## Port plan

| Dolt | ProllyTree port |
|---|---|
| `tree.Chunker[S]` | `Chunker<'storage, S: NodeStorage<N>, SP: Splitter>` |
| `tree.NodeSplitter` | `Splitter` trait + `RollingHashSplitter` (reuses our current rolling-hash, just with reset semantics) |
| `tree.nodeBuilder[S]` | `NodeBuilder` (keys + values for one in-progress chunk) |
| `tree.cursor` | `NodeCursor` (lazy traversal, current key/value/child-hash) |
| `tree.ApplyMutations` | `ProllyTree::apply_mutations(sorted_edits)` |
| `tree.MutableMap.Edits` | `BTreeMap` in `apply_changes` (already there) |

### Phase 1 - chunker only (no cursor)

- `NodeBuilder<N>`: holds `keys: Vec<Vec<u8>>`, `values: Vec<Vec<u8>>`,
  `level: u8`, `config: TreeConfig<N>`. Method: `add(key, value)`,
  `build() -> ProllyNode<N>`, `count()`, `is_empty()`.
- `Splitter` trait: `fn append(&mut self, key: &[u8], value: &[u8])`,
  `fn crossed_boundary(&self) -> bool`, `fn reset(&mut self)`.
- `RollingHashSplitter`: keeps a rolling hash, byte-by-byte, with
  the same constants we have today. `reset` zeros state.
- `Chunker<'s, S>`: streaming builder with `parent: Option<Box<Chunker>>`,
  `builder: NodeBuilder`, `splitter: RollingHashSplitter`, `level: u8`,
  `storage: &'s mut S`. Methods: `add_pair`, `update_pair`,
  `delete_pair`, `done`. Internal: `handle_boundary`, `append_to_parent`.

Wire into `ProllyTree::apply_changes`: build a `Chunker` at level 0,
feed sorted `(key, value)` pairs (skipping deletes whose key isn't
in the snapshot), call `done`. Drop the `canonicalize` rebuild.

This phase keeps the O(N) cost of a full rebuild but replaces the
"rebuild from leaves" with "streaming chunker over leaves" - the
correctness path is now identical to Dolt's.

### Phase 2 - NodeCursor + advanceTo

Implement `NodeCursor` with `seek(key)`, `advance()`, `current_key()`,
`current_value()`, `at_node_end()`, plus the linked-list `parent`
pointer up to the root.

Implement `Chunker::advance_to(&mut self, next: &Cursor)` matching
Dolt's algorithm: walk forward feeding through the local splitter
until the in-progress chunk's boundary aligns with the old tree's
boundary, then recurse into the parent.

`apply_mutations` becomes: open a cursor at the first edit, open a
chunker rooted at that cursor, for each edit `cur.seek(edit.key)` ->
`chunker.advance_to(&cur)` -> apply the edit.

## Status

Phase 1 (streaming chunker as `canonicalize` backend) and Phase 2
(cursor-driven `apply_mutations` replacing the legacy `insert` /
`delete` path) are both landed. The `#[ignore]`d node-level tests at
the *primitive* `ProllyNode` API still fail because they exercise
`node.insert` / `node.delete` directly, which still runs the legacy
in-place `Balanced::balance`. The public API at every consumer layer
(`ProllyTree`, `GitVersionedKvStore`, `GitNamespacedKvStore`) has
history-independence guaranteed by construction now.

Bench at N=10 000 inserts (`cargo bench --bench tree -- insert_single`):

  - Pre-fix baseline (02e1cfa): 1.14 s
  - canonicalize stop-gap (1f88dd1): 30.1 s (26.4x)
  - Phase 1 streaming chunker (5c09a86): 13.3 s (11.7x)
  - Phase 2 cursor-driven (28ec971): 16.6 s (14.5x)

Phase 2 regressed slightly because the cursor adds per-leaf overhead
without yet enabling the chunk-boundary fast-forward that gives Dolt's
algorithm its `O(log N + edits)` cost per op. The fast-forward is the
remaining work; see "Phase 3" below.

### Phase 3 - cursor fast-forward (remaining work)

Implement Dolt's `chunker.advanceTo(next *cursor)` recursive
algorithm:

1. `NodeCursor::compare(other) -> i32` matching Dolt's
   `compareCursors`.
2. `Chunker::append` extended to accept items at any level (so a
   parent chunker can absorb `(firstKey, child_hash)` pairs
   wholesale without re-descending into leaves).
3. `Chunker::advance_to(cur, target)` that walks forward, feeding
   items from `cur` into the local splitter; when both the new
   chunker boundary and `cur` land on a chunk boundary at the same
   level, recurse into the parent chunker to fast-forward over
   unchanged subtrees.
4. `apply_mutations` switches from item-by-item walking to:
   `cur.seek(mut.key)` -> `chunker.advance_to(cur)` -> apply the
   mutation. Between mutations, the chunker skips entire unchanged
   subtrees via the parent recursion.

Expected outcome: bench at N=10 000 returns to within ~2x of baseline
(roughly Dolt's own per-op cost for small batches), keeping all
canonical-tree guarantees.

### Phase 4 - cleanup (after Phase 3 lands)

- Remove or refactor the 6 `#[ignore]`d tests in `src/node.rs`. They
  test the *primitive* layer; the property is now provided at a
  higher abstraction. Either delete them with a note pointing to the
  streaming-chunker tests, or rewrite them against the chunker
  directly.
- Optionally: remove `ProllyTree::canonicalize` entirely and inline
  its caller, since `apply_changes` is now the only mutation path.
- Refresh `docs/bench-canonicalize-impact.md` with the final numbers.

## Verification

1. Integration matrix in `tests/history_independence.rs`: 6 tests
   pass (was 4 with stop-gap).
2. ProllyTree-level history-independence tests in `src/tree.rs`: 4 of
   4 pass (was 1 of 4 with stop-gap).
3. VersionedKvStore + NamespacedKvStore tests: 6 of 6 pass.
4. Streaming-chunker module tests: 10 of 10 pass.
5. Full suite green under all three CI feature flag variants.
