# Performance: streaming chunker vs canonicalize stop-gap

Measurement of `cargo bench --bench tree -- insert_single` (criterion,
release build) at each step of the history-independence fix. The
benchmark inserts N keys one at a time into a fresh `ProllyTree` with
`TreeConfig::default()` and `InMemoryNodeStorage`.

| N      | Baseline (02e1cfa) | canonicalize | Phase 1 chunker | Phase 2 cursor | Phase 3 fast-forward |
|--------|---------------------|--------------|------------------|----------------|----------------------|
| 100    | 1.36 ms             | 4.20 ms (3.1x) | -              | -              | -                    |
| 1 000  | 67.1 ms             | 363 ms (5.4x)  | 178 ms (2.6x)  | 165 ms (2.5x)  | 153 ms (2.3x)        |
| 10 000 | 1.14 s              | 30.1 s (26.4x) | 13.3 s (11.7x) | 16.6 s (14.5x) | 11.2 s (9.8x)        |

Phases 1+2 are still O(N²) per insert sequence - each insert triggers
an O(N) walk + chunker pass. The reduction from 26x to ~12-15x comes
from:

  - dropping the wasted in-place `Balanced::balance` work that
    canonicalize used to discard;
  - using a streaming chunker (Phase 1) instead of a batch
    collect-then-rebuild;
  - bypassing `BTreeMap` materialization via cursor walk (Phase 2).

Phase 3 adds two fast paths matching Dolt's chunker design:

  - **Pure-append**: when every mutation is an insert past the tree's
    max key, walk the existing internal spine once to collect
    `(firstKey, leaf_hash)` for each leaf, feed all but the last leaf
    directly into the level-1 chunker, then re-process the last leaf's
    items plus new keys through level-0. This skips reading the items
    of N-1 leaves. Dominates the win for append-heavy workloads like
    the `insert_single` benchmark.

  - **Alignment-aware fast-forward**: after each chunk emit, if the
    cursor was at the end of an old leaf and the emitted chunk's hash
    equals that leaf's hash, the chunker has re-synced with the old
    tree. If no more mutations are pending, `fast_forward_to_end`
    copies the remaining old leaves to the level-1 chunker without
    reading their items. Helps mid-tree mutations where the tail of
    the tree is unchanged.

At N=10 000 inserts in ascending order (the `insert_single` benchmark),
Phase 3 is **2.7x faster than Phase 2** and **9.8x slower than the
pre-fix baseline** (which had the order-dependence bug). The remaining
gap to baseline comes from: per-leaf hash-mapping lookups in storage,
the inherent O(N²) nature of one-mutation-per-call (each call still
processes the last leaf), and the cost of writing the new tree's nodes
to storage on every commit.

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
