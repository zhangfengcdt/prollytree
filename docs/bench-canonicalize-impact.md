# Performance impact of the canonicalize stop-gap

Measurement of `cargo bench --bench tree -- insert_single` (criterion,
release build) before and after the canonicalize fix added in commit
`befcf7f`. The benchmark inserts N keys one at a time into a fresh
`ProllyTree` with `TreeConfig::default()` and `InMemoryNodeStorage`.

| N      | Baseline (02e1cfa) | With canonicalize (1f88dd1) | Slowdown |
|--------|---------------------|------------------------------|----------|
| 100    | 1.36 ms             | 4.20 ms                      | 3.1x     |
| 1 000  | 67.1 ms             | 363 ms                       | 5.4x     |
| 10 000 | 1.14 s              | 30.1 s                       | 26.4x    |

The scaling matches the expected O(N²) per insert sequence (each insert
triggers an O(N) rebuild from leaves). The baseline is roughly O(N log N)
per sequence.

### Reading

The stop-gap is correct (closes the order-dependence bug at the public
API) and tolerable at N ≤ 1000, but dominates wall-clock time for
larger trees. The 26x regression at N=10 000 makes it unsuitable for
production workloads of more than a few thousand keys per commit.

The incremental-rebalance work (Step 3) lifts the per-insert cost back
to O(log N + affected-chunk) while preserving the guarantee. The
`#[ignore]`d node-level tests in `src/node.rs` are the acceptance
suite that gates re-enabling that path.
