# Performance: streaming chunker vs canonicalize stop-gap

Measurement of `cargo bench --bench tree -- insert_single` (criterion,
release build) at each step of the history-independence fix. The
benchmark inserts N keys one at a time into a fresh `ProllyTree` with
`TreeConfig::default()` and `InMemoryNodeStorage`.

| N      | Baseline (02e1cfa) | canonicalize (1f88dd1) | Phase 1 chunker (5c09a86) | Phase 2 cursor (28ec971) |
|--------|---------------------|------------------------|---------------------------|--------------------------|
| 100    | 1.36 ms             | 4.20 ms (3.1x)         | -                         | -                        |
| 1 000  | 67.1 ms             | 363 ms (5.4x)          | 178 ms (2.6x)             | 165 ms (2.5x)            |
| 10 000 | 1.14 s              | 30.1 s (26.4x)         | 13.3 s (11.7x)            | 16.6 s (14.5x)           |

All three "with fix" columns are still O(N²) per insert sequence -
each insert triggers an O(N) walk + chunker pass. The reduction from
26x to ~12-15x comes from:

  - dropping the wasted in-place `Balanced::balance` work that
    canonicalize used to discard;
  - using a streaming chunker (Phase 1) instead of a batch
    collect-then-rebuild;
  - bypassing `BTreeMap` materialization via cursor walk (Phase 2).

Phase 2 is slightly slower than Phase 1 because the cursor adds
per-leaf overhead (refetching child nodes from storage) without yet
enabling the chunk-boundary fast-forward that lets the chunker skip
unchanged subtrees.

### Phase 3 (remaining)

The cursor fast-forward, `chunker.advanceTo(next)`, is what brings the
per-op cost from O(N) to `O(log N + edits)`. See
`docs/dolt-streaming-chunker.md` for the design. With the fast-forward
landed, the expected bench is ~2x of baseline for incremental
insert sequences.

### Correctness status (independent of perf)

The history-independence guarantee is now structural at every
consumed layer:

  - `ProllyTree` API: 4 root-hash-equality tests pass
    (`src/tree.rs::history_independence_tests`).
  - Integration matrix: 6 tests pass
    (`tests/history_independence.rs`).
  - `GitVersionedKvStore`: 3 tests pass
    (`src/git/versioned_store/tests.rs::history_independence_tests`).
  - `GitNamespacedKvStore`: 3 tests pass
    (`src/git/versioned_store/namespaced_tests.rs`).
  - Tree-merge canonicality: 3 tests pass
    (`src/tree.rs::merge_canonicality_tests`).
  - Streaming chunker module: 10 unit tests pass
    (`src/streaming_chunker.rs::tests`).
