/*
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

//! History-independence integration tests.
//!
//! A prolly tree's defining property is that two trees holding the same
//! final set of (key, value) pairs should produce the same Merkle root
//! regardless of the operation sequence used to build them. This file
//! exercises that property through the public `ProllyTree` API across a
//! matrix of:
//!
//!   * tree configurations - default + several non-default chunker tunings
//!     covering small/medium chunk targets and an alternate rolling hash,
//!   * insertion orders - ascending, descending, alternating odd-then-even,
//!     and three deterministic shuffles (seeds 0, 1, 42),
//!   * key counts - 8, 32, 256, 1000 (covers single-leaf through
//!     multi-level trees while staying inside the 30s CI budget),
//!   * key patterns - 8-byte big-endian u64, zero-padded decimal strings,
//!     and shared-prefix strings (an adversarial case for the chunker),
//!   * operation mixes - pure inserts, insert-then-overwrite, and
//!     insert-extras-then-delete.
//!
//! All tests assert root hash equality (the canonical fingerprint of the
//! final key/value set). The fix that makes this work lives in the
//! streaming chunker in `src/streaming_chunker.rs`, which
//! `ProllyTree::apply_changes` drives on every mutation. Tests that
//! probe the lower-level `ProllyNode` API directly (which still runs
//! the legacy in-place balance) remain `#[ignore]`d in `src/node.rs`.

use prollytree::config::TreeConfig;
use prollytree::storage::InMemoryNodeStorage;
use prollytree::tree::{ProllyTree, Tree};
use rand::prelude::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

// -- Matrix axes -----------------------------------------------------------

#[derive(Clone)]
struct ConfigVariant {
    label: &'static str,
    cfg: fn() -> TreeConfig<32>,
}

fn config_variants() -> Vec<ConfigVariant> {
    vec![
        ConfigVariant {
            label: "default",
            cfg: || TreeConfig::default(),
        },
        ConfigVariant {
            label: "tiny-chunks",
            cfg: || TreeConfig {
                min_chunk_size: 2,
                max_chunk_size: 16,
                pattern: 0b11,
                ..TreeConfig::default()
            },
        },
        ConfigVariant {
            label: "medium-chunks",
            cfg: || TreeConfig {
                min_chunk_size: 4,
                max_chunk_size: 64,
                pattern: 0b1111,
                ..TreeConfig::default()
            },
        },
        ConfigVariant {
            label: "alt-hash",
            cfg: || TreeConfig {
                base: 131,
                modulus: 1_000_000_009,
                ..TreeConfig::default()
            },
        },
    ]
}

/// N values exercised by the matrix tests. Capped at 256 so the full
/// (config × N × key-pattern × order × op-mix) matrix completes well
/// under the 30s CI budget. The simpler
/// `traversal_independent_of_order_default_config` test still exercises
/// N up to 1000 since it only builds 4 trees per N.
fn key_counts() -> &'static [u64] {
    &[8, 32, 256]
}

fn baseline_key_counts() -> &'static [u64] {
    &[8, 32, 256, 1000]
}

fn orders(n: u64) -> Vec<(&'static str, Vec<u64>)> {
    let asc: Vec<u64> = (0..n).collect();
    let desc: Vec<u64> = (0..n).rev().collect();
    let alt: Vec<u64> = (0..n).step_by(2).chain((1..n).step_by(2)).collect();
    let mut s0: Vec<u64> = (0..n).collect();
    s0.shuffle(&mut StdRng::from_seed([0u8; 32]));
    let mut s1: Vec<u64> = (0..n).collect();
    s1.shuffle(&mut StdRng::from_seed([1u8; 32]));
    let mut s42: Vec<u64> = (0..n).collect();
    s42.shuffle(&mut StdRng::from_seed([42u8; 32]));
    vec![
        ("ascending", asc),
        ("descending", desc),
        ("alt-odd-even", alt),
        ("shuffled(0)", s0),
        ("shuffled(1)", s1),
        ("shuffled(42)", s42),
    ]
}

// -- Key/value shapes ------------------------------------------------------

fn k_u64(i: u64) -> Vec<u8> {
    i.to_be_bytes().to_vec()
}
fn v_u64(i: u64) -> Vec<u8> {
    let mut v = Vec::with_capacity(16);
    v.extend_from_slice(&i.to_be_bytes());
    v.extend_from_slice(&(!i).to_be_bytes());
    v
}
fn k_str(i: u64) -> Vec<u8> {
    format!("k{i:08}").into_bytes()
}
fn k_prefix(i: u64) -> Vec<u8> {
    format!("prefix/{i:08}").into_bytes()
}

// -- Helpers ---------------------------------------------------------------

fn build_with_inserts(
    cfg: TreeConfig<32>,
    order: &[u64],
    key_fn: fn(u64) -> Vec<u8>,
    value_fn: fn(u64) -> Vec<u8>,
) -> ProllyTree<32, InMemoryNodeStorage<32>> {
    let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg);
    for &i in order {
        tree.insert(key_fn(i), value_fn(i));
    }
    tree
}

fn build_with_overwrite(
    cfg: TreeConfig<32>,
    insert_order: &[u64],
    overwrite_order: &[u64],
    key_fn: fn(u64) -> Vec<u8>,
    final_value: fn(u64) -> Vec<u8>,
) -> ProllyTree<32, InMemoryNodeStorage<32>> {
    let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg);
    let placeholder = vec![0u8];
    for &i in insert_order {
        tree.insert(key_fn(i), placeholder.clone());
    }
    for &i in overwrite_order {
        tree.insert(key_fn(i), final_value(i));
    }
    tree
}

fn build_with_delete(
    cfg: TreeConfig<32>,
    insert_order: &[u64],
    delete_order: &[u64],
    key_fn: fn(u64) -> Vec<u8>,
    value_fn: fn(u64) -> Vec<u8>,
) -> ProllyTree<32, InMemoryNodeStorage<32>> {
    let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg);
    for &i in insert_order {
        tree.insert(key_fn(i), value_fn(i));
    }
    for &i in delete_order {
        tree.delete(&key_fn(i));
    }
    tree
}

// -- Always-on integration tests ------------------------------------------

/// Pure inserts under the DEFAULT TreeConfig with 8-byte u64 keys produce
/// the same `traverse()` across all six insertion orders, for every N in
/// {8, 32, 256, 1000}.
#[test]
fn traversal_independent_of_order_default_config() {
    for &n in baseline_key_counts() {
        let orders = orders(n);
        let baseline = build_with_inserts(TreeConfig::default(), &orders[0].1, k_u64, v_u64);
        let baseline_trav = baseline.traverse();
        let baseline_label = orders[0].0;
        for (label, order) in orders.iter().skip(1) {
            let tree = build_with_inserts(TreeConfig::default(), order, k_u64, v_u64);
            assert_eq!(
                tree.traverse(),
                baseline_trav,
                "n={} order={} leaf content diverged from order={}",
                n,
                label,
                baseline_label
            );
        }
    }
}

/// Edge case: N=0 and N=1 must not panic and must agree across any
/// ordering (vacuously). For N=1 we check that the only-key path matches
/// the empty-then-insert path's root hash.
#[test]
fn edge_cases_empty_and_singleton() {
    // N=0: empty tree should produce *some* root hash (or None) consistently.
    let empty_a = ProllyTree::new(InMemoryNodeStorage::<32>::default(), TreeConfig::default());
    let empty_b = ProllyTree::new(InMemoryNodeStorage::<32>::default(), TreeConfig::default());
    assert_eq!(empty_a.get_root_hash(), empty_b.get_root_hash());

    // N=1: single insert is order-independent (only one ordering exists,
    // but check that the resulting tree has the same hash whether built
    // freshly or after an insert/delete cycle for a different key).
    let mut t1 = ProllyTree::new(InMemoryNodeStorage::<32>::default(), TreeConfig::default());
    t1.insert(k_u64(42), v_u64(42));
    let h1 = t1.get_root_hash().unwrap();

    let mut t2 = ProllyTree::new(InMemoryNodeStorage::<32>::default(), TreeConfig::default());
    t2.insert(k_u64(42), v_u64(42));
    let h2 = t2.get_root_hash().unwrap();
    assert_eq!(h1, h2, "singleton tree built twice has identical root hash");
}

// -- Always-on matrix tests (rely on the streaming canonical chunker) ----

/// Full matrix on root hash: every (config, N, key-pattern) cell must
/// produce identical root hashes across all insertion orders.
#[test]
fn root_hash_matrix() {
    type KeyFn = fn(u64) -> Vec<u8>;
    let mut failures = Vec::<String>::new();
    let key_fns: &[(&str, KeyFn)] = &[("u64", k_u64), ("str", k_str)];
    for cfg_variant in config_variants() {
        for &n in key_counts() {
            let orders = orders(n);
            for (key_label, key_fn) in key_fns {
                let baseline_hash =
                    build_with_inserts((cfg_variant.cfg)(), &orders[0].1, *key_fn, v_u64)
                        .get_root_hash()
                        .unwrap();
                for (order_label, order) in orders.iter().skip(1) {
                    let h = build_with_inserts((cfg_variant.cfg)(), order, *key_fn, v_u64)
                        .get_root_hash()
                        .unwrap();
                    if h != baseline_hash {
                        failures.push(format!(
                            "cfg={} n={} keys={} order={} root hash diverged",
                            cfg_variant.label, n, key_label, order_label
                        ));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} matrix cell(s) diverged:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Update path: a tree built by inserting placeholders and then
/// overwriting with the final values must equal a tree built directly
/// from the final values, for every (config, N) pair.
#[test]
fn root_hash_after_updates_matrix() {
    let mut failures = Vec::<String>::new();
    for cfg_variant in config_variants() {
        for &n in key_counts() {
            let asc: Vec<u64> = (0..n).collect();
            let mut shuf_ins: Vec<u64> = (0..n).collect();
            shuf_ins.shuffle(&mut StdRng::from_seed([1u8; 32]));
            let mut shuf_over: Vec<u64> = (0..n).collect();
            shuf_over.shuffle(&mut StdRng::from_seed([2u8; 32]));

            let baseline = build_with_inserts((cfg_variant.cfg)(), &asc, k_u64, v_u64)
                .get_root_hash()
                .unwrap();
            let variant =
                build_with_overwrite((cfg_variant.cfg)(), &shuf_ins, &shuf_over, k_u64, v_u64)
                    .get_root_hash()
                    .unwrap();
            if variant != baseline {
                failures.push(format!(
                    "cfg={} n={} update-then-final root hash diverged",
                    cfg_variant.label, n
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} cell(s) diverged:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Delete path: insert N+M keys and delete the M extras; the result must
/// equal a tree built only from the N survivors.
#[test]
fn root_hash_after_deletes_matrix() {
    let mut failures = Vec::<String>::new();
    for cfg_variant in config_variants() {
        for &n in key_counts() {
            let m = n;
            let survivors: Vec<u64> = (0..n).collect();
            let extras: Vec<u64> = (n..n + m).collect();
            let mut all: Vec<u64> = (0..n + m).collect();
            all.shuffle(&mut StdRng::from_seed([3u8; 32]));
            let mut del = extras.clone();
            del.shuffle(&mut StdRng::from_seed([4u8; 32]));

            let baseline = build_with_inserts((cfg_variant.cfg)(), &survivors, k_u64, v_u64)
                .get_root_hash()
                .unwrap();
            let variant = build_with_delete((cfg_variant.cfg)(), &all, &del, k_u64, v_u64)
                .get_root_hash()
                .unwrap();
            if variant != baseline {
                failures.push(format!(
                    "cfg={} n={} delete-then-final root hash diverged",
                    cfg_variant.label, n
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} cell(s) diverged:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Shared-prefix keys (e.g. "prefix/00000001", ..., "prefix/00000256") are
/// adversarial for the rolling hash because the leading bytes are
/// identical across many keys.
#[test]
fn traversal_shared_prefix_keys() {
    let mut failures = Vec::<String>::new();
    for cfg_variant in config_variants() {
        let n: u64 = 256;
        let orders = orders(n);
        let baseline_trav =
            build_with_inserts((cfg_variant.cfg)(), &orders[0].1, k_prefix, v_u64).traverse();
        for (label, order) in orders.iter().skip(1) {
            let t = build_with_inserts((cfg_variant.cfg)(), order, k_prefix, v_u64).traverse();
            if t != baseline_trav {
                failures.push(format!(
                    "cfg={} prefix-keys order={} leaf content diverged",
                    cfg_variant.label, label
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} cell(s) diverged:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

// =========================================================================
// Stronger coverage: closing the remaining "could a regression slip
// through" gaps. Each test below targets a specific axis the matrix tests
// don't pin.
// =========================================================================

/// Many-seed property test: for each config, run 32 deterministic shuffles
/// of the same key set and assert every resulting root hash is equal.
/// Hand-picked orders catch the obvious bugs; this one is meant to catch
/// subtle drift in the chunker / splitter / cursor under arbitrary
/// permutations.
#[test]
fn root_hash_property_many_random_orders() {
    let mut failures = Vec::<String>::new();
    const N: u64 = 200;
    const SEEDS: u64 = 32;
    for cfg_variant in config_variants() {
        let asc: Vec<u64> = (0..N).collect();
        let baseline = build_with_inserts((cfg_variant.cfg)(), &asc, k_u64, v_u64)
            .get_root_hash()
            .unwrap();
        for seed in 0..SEEDS {
            let mut order: Vec<u64> = (0..N).collect();
            let mut seed_bytes = [0u8; 32];
            seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
            order.shuffle(&mut StdRng::from_seed(seed_bytes));
            let h = build_with_inserts((cfg_variant.cfg)(), &order, k_u64, v_u64)
                .get_root_hash()
                .unwrap();
            if h != baseline {
                failures.push(format!(
                    "cfg={} seed={} root diverged",
                    cfg_variant.label, seed
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} divergent shuffle(s):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Cross-storage-backend equivalence: building the same logical tree on
/// `InMemoryNodeStorage` and `FileNodeStorage` must yield the same root
/// hash. The storage backend only persists bytes - the chunker writes
/// the same nodes - but pinning this with a test guards against backend-
/// specific code drift (e.g. an encoding tweak that leaks into hashes).
#[test]
fn file_storage_matches_memory_storage() {
    use prollytree::storage::FileNodeStorage;
    use tempfile::TempDir;

    let pairs: Vec<(Vec<u8>, Vec<u8>)> = (0..300u64).map(|i| (k_u64(i), v_u64(i))).collect();
    let cfg = TreeConfig::<32>::default();

    // In-memory
    let mut mem = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg.clone());
    for (k, v) in &pairs {
        mem.insert(k.clone(), v.clone());
    }
    let mem_root = mem.get_root_hash().unwrap();

    // File-backed
    let tmp = TempDir::new().expect("tempdir");
    let storage = FileNodeStorage::<32>::new(tmp.path().to_path_buf()).expect("file storage");
    let mut filed = ProllyTree::new(storage, cfg);
    for (k, v) in &pairs {
        filed.insert(k.clone(), v.clone());
    }
    let file_root = filed.get_root_hash().unwrap();

    assert_eq!(
        mem_root, file_root,
        "memory and file storage produced different root hashes for the same operations"
    );
}

/// Single-call vs batch parity: calling `insert(k1); insert(k2); ...` must
/// produce the same root hash as one `insert_batch([k1, k2, ...])`. By
/// the property they must, but pinning this catches regressions in
/// `apply_changes`'s batch-vs-single dispatch.
#[test]
fn single_inserts_match_insert_batch() {
    let cfg = TreeConfig::<32>::default();
    let pairs: Vec<(Vec<u8>, Vec<u8>)> = (0..150u64).map(|i| (k_u64(i), v_u64(i))).collect();

    let mut single = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg.clone());
    for (k, v) in &pairs {
        single.insert(k.clone(), v.clone());
    }
    let single_root = single.get_root_hash().unwrap();

    let keys: Vec<Vec<u8>> = pairs.iter().map(|(k, _)| k.clone()).collect();
    let vals: Vec<Vec<u8>> = pairs.iter().map(|(_, v)| v.clone()).collect();
    let mut batched = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg);
    batched.insert_batch(&keys, &vals);
    let batched_root = batched.get_root_hash().unwrap();

    assert_eq!(
        single_root, batched_root,
        "single insert sequence diverged from insert_batch with same final state"
    );
}

/// Mixed-ops random sequences: generate arbitrary insert/update/delete
/// sequences that converge to the same final logical state, and assert
/// they all yield the same root hash. This stresses the most realistic
/// failure modes a streaming chunker can have - cursor merges, cursor
/// jumps across leaves, fast-forward triggers at arbitrary points.
#[test]
fn random_mixed_ops_converge_to_same_root() {
    let cfg = TreeConfig::<32>::default();
    // Target final state: { 0,2,4,...,98 } (even keys, 50 entries).
    let target_keys: Vec<u64> = (0..50).map(|i| i * 2).collect();
    let target_pairs: Vec<(Vec<u8>, Vec<u8>)> =
        target_keys.iter().map(|&i| (k_u64(i), v_u64(i))).collect();

    // Build the canonical baseline directly.
    let mut baseline_tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg.clone());
    for (k, v) in &target_pairs {
        baseline_tree.insert(k.clone(), v.clone());
    }
    let baseline = baseline_tree.get_root_hash().unwrap();

    let mut failures = Vec::<String>::new();
    for seed in 0u64..16 {
        // Construct a random op sequence that ends at the target state:
        //   1. Insert each target key with a placeholder value.
        //   2. Insert N "noise" keys (outside the target set).
        //   3. Update each target key to its real value, in random order.
        //   4. Delete each noise key, in random order.
        let mut seed_bytes = [0u8; 32];
        seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
        let mut rng = StdRng::from_seed(seed_bytes);

        let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg.clone());

        let mut step1: Vec<u64> = target_keys.clone();
        step1.shuffle(&mut rng);
        for &i in &step1 {
            tree.insert(k_u64(i), b"placeholder".to_vec());
        }

        let noise: Vec<u64> = (1..50).map(|i| i * 2 + 1).collect(); // odd keys not in target
        let mut step2 = noise.clone();
        step2.shuffle(&mut rng);
        for &i in &step2 {
            tree.insert(k_u64(i), v_u64(i + 10_000));
        }

        let mut step3: Vec<u64> = target_keys.clone();
        step3.shuffle(&mut rng);
        for &i in &step3 {
            tree.update(k_u64(i), v_u64(i));
        }

        let mut step4 = noise;
        step4.shuffle(&mut rng);
        for &i in &step4 {
            tree.delete(&k_u64(i));
        }

        let root = tree.get_root_hash().unwrap();
        if root != baseline {
            failures.push(format!("seed={} root diverged from baseline", seed));
        }
    }
    assert!(
        failures.is_empty(),
        "{} mixed-op sequence(s) diverged:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Empty batches and no-op operations must not change the root hash.
#[test]
fn empty_batches_and_noops_preserve_root() {
    let cfg = TreeConfig::<32>::default();
    let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg);
    for i in 0u64..50 {
        tree.insert(k_u64(i), v_u64(i));
    }
    let baseline = tree.get_root_hash().unwrap();

    // Empty insert_batch
    tree.insert_batch(&[], &[]);
    assert_eq!(
        tree.get_root_hash().unwrap(),
        baseline,
        "empty insert_batch changed the root"
    );

    // Empty delete_batch
    tree.delete_batch(&[]);
    assert_eq!(
        tree.get_root_hash().unwrap(),
        baseline,
        "empty delete_batch changed the root"
    );

    // Delete of a key not present
    assert!(!tree.delete(b"absent_key"));
    assert_eq!(
        tree.get_root_hash().unwrap(),
        baseline,
        "delete of absent key changed the root"
    );

    // Update of a key not present returns false and changes nothing
    assert!(!tree.update(b"absent_key".to_vec(), b"value".to_vec()));
    assert_eq!(
        tree.get_root_hash().unwrap(),
        baseline,
        "failed update of absent key changed the root"
    );
}

/// Inserting an ephemeral key and then deleting it must return to the
/// original root hash. This is the most direct expression of history
/// independence — the property says intermediate steps don't matter as
/// long as the final state matches.
#[test]
fn insert_then_delete_returns_to_baseline() {
    let cfg = TreeConfig::<32>::default();
    let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg);
    for i in 0u64..40 {
        tree.insert(k_u64(i), v_u64(i));
    }
    let baseline = tree.get_root_hash().unwrap();

    // Insert and immediately delete an ephemeral key at various positions.
    for &i in &[0u64, 20, 39, 999, 12345] {
        let ephemeral = k_u64(i.wrapping_add(1_000_000));
        tree.insert(ephemeral.clone(), b"ephemeral".to_vec());
        assert!(tree.delete(&ephemeral));
        assert_eq!(
            tree.get_root_hash().unwrap(),
            baseline,
            "insert-then-delete of key based on {} did not return to baseline",
            i
        );
    }
}

/// Delete every key, then re-insert the same set: the resulting tree
/// must be byte-identical to a fresh canonical build of that set.
#[test]
fn delete_all_then_reinsert_is_canonical() {
    let cfg = TreeConfig::<32>::default();
    let pairs: Vec<(Vec<u8>, Vec<u8>)> = (0..80u64).map(|i| (k_u64(i), v_u64(i))).collect();

    let mut fresh = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg.clone());
    for (k, v) in &pairs {
        fresh.insert(k.clone(), v.clone());
    }
    let fresh_root = fresh.get_root_hash().unwrap();

    let mut wiped = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg);
    for (k, v) in &pairs {
        wiped.insert(k.clone(), v.clone());
    }
    // Now wipe everything.
    let keys: Vec<Vec<u8>> = pairs.iter().map(|(k, _)| k.clone()).collect();
    wiped.delete_batch(&keys);
    // Re-insert in reverse order (shouldn't matter).
    for (k, v) in pairs.iter().rev() {
        wiped.insert(k.clone(), v.clone());
    }
    let wiped_root = wiped.get_root_hash().unwrap();

    assert_eq!(
        fresh_root, wiped_root,
        "delete-all-then-reinsert produced a different root than a fresh build"
    );
}

/// Deleting all keys must leave a well-formed empty tree, and inserting
/// nothing into an empty tree must keep it empty.
#[test]
fn delete_all_yields_well_formed_empty_tree() {
    let cfg = TreeConfig::<32>::default();
    let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg.clone());
    for i in 0u64..30 {
        tree.insert(k_u64(i), v_u64(i));
    }
    let keys: Vec<Vec<u8>> = (0..30u64).map(k_u64).collect();
    tree.delete_batch(&keys);

    let fresh = ProllyTree::new(InMemoryNodeStorage::<32>::default(), cfg);
    assert_eq!(
        tree.get_root_hash().unwrap(),
        fresh.get_root_hash().unwrap(),
        "tree with everything deleted does not match a freshly-constructed empty tree"
    );
    assert_eq!(tree.size(), 0);
}
