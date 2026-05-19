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

//! PR 3a — sub-index integration tests.
//!
//! These exercise `NamespaceHandle::proximity_index()` on the File-backed
//! `NamespacedKvStore` (the supported persistence path). The Git path is
//! covered separately by PR 3b once namespaced merge handles proximity
//! hash-mapping persistence.

#![cfg(all(feature = "proximity", feature = "git"))]

mod common;

use common::setup_repo_and_dataset;
use prollytree::git::versioned_store::FileNamespacedKvStore;
use prollytree::proximity::{Metric, ProximityConfig, ProximityError};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const N: usize = 32;

fn random_vectors(n: usize, dim: usize, seed: u64) -> Vec<(Vec<u8>, Vec<f32>)> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n)
        .map(|i| {
            let id = format!("id-{i:08}").into_bytes();
            let v: Vec<f32> = (0..dim).map(|_| rng.random_range(-1.0..1.0)).collect();
            (id, v)
        })
        .collect()
}

fn config(dim: u16, metric: Metric) -> ProximityConfig {
    ProximityConfig {
        dim,
        metric,
        level_bits: 4,
        max_bucket_size: 64,
    }
}

#[test]
fn proximity_index_inside_namespace_basic_insert_and_query() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    let data = random_vectors(50, 8, 0xa1);

    {
        let mut personal = store.namespace("personal");
        let mut docs = personal
            .proximity_index("docs", config(8, Metric::L2))
            .unwrap();
        for (id, v) in &data {
            docs.insert(id.clone(), v.clone()).unwrap();
        }

        // Query before commit — should work because the in-memory index is live.
        let hits = docs.knn(&vec![0.1f32; 8], 5, 32).unwrap();
        assert_eq!(hits.len(), 5);
    }

    store.commit("ingest proximity vectors").unwrap();
}

#[test]
fn proximity_index_survives_commit_and_reopen() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let data = random_vectors(80, 8, 0xb2);

    // Original hits captured against a known query — these must match after reopen.
    let query = vec![0.25f32; 8];
    let original_hits;

    {
        let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
        let mut personal = store.namespace("personal");
        let mut docs = personal
            .proximity_index("docs", config(8, Metric::L2))
            .unwrap();
        for (id, v) in &data {
            docs.insert(id.clone(), v.clone()).unwrap();
        }
        original_hits = docs.knn(&query, 10, 32).unwrap();
        // Hold the borrow until after capturing the hits, then drop before commit.
        drop(docs);
        drop(personal);
        store.commit("write index").unwrap();
    }

    // Fresh process — reopen everything from scratch.
    {
        let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
        let mut personal = store.namespace("personal");
        let mut docs = personal
            .proximity_index("docs", config(8, Metric::L2))
            .unwrap();
        assert_eq!(docs.len(), data.len());

        let reopened_hits = docs.knn(&query, 10, 32).unwrap();
        assert_eq!(reopened_hits, original_hits);
    }
}

#[test]
fn multiple_indexes_per_namespace() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let data = random_vectors(40, 4, 0xc3);

    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let mut personal = store.namespace("personal");
        {
            let mut by_title = personal
                .proximity_index("by_title", config(4, Metric::L2))
                .unwrap();
            for (id, v) in &data {
                by_title.insert(id.clone(), v.clone()).unwrap();
            }
        }
        {
            let mut by_summary = personal
                .proximity_index("by_summary", config(4, Metric::Cosine))
                .unwrap();
            for (id, v) in &data {
                by_summary.insert(id.clone(), v.clone()).unwrap();
            }
        }
    }
    store.commit("dual-index commit").unwrap();

    // Reopen and verify both indexes are queryable independently.
    drop(store);
    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let mut personal = store.namespace("personal");

    {
        let by_title = personal
            .proximity_index("by_title", config(4, Metric::L2))
            .unwrap();
        assert_eq!(by_title.len(), data.len());
        assert_eq!(by_title.config().metric, Metric::L2);
    }
    {
        let by_summary = personal
            .proximity_index("by_summary", config(4, Metric::Cosine))
            .unwrap();
        assert_eq!(by_summary.len(), data.len());
        assert_eq!(by_summary.config().metric, Metric::Cosine);
    }
}

#[test]
fn proximity_indexes_do_not_collide_across_namespaces() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let data_a = random_vectors(30, 4, 0xd4);
    let data_b = random_vectors(20, 4, 0xe5);

    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let mut ns_a = store.namespace("ns_a");
        let mut docs = ns_a.proximity_index("docs", config(4, Metric::L2)).unwrap();
        for (id, v) in &data_a {
            docs.insert(id.clone(), v.clone()).unwrap();
        }
    }
    {
        let mut ns_b = store.namespace("ns_b");
        let mut docs = ns_b.proximity_index("docs", config(4, Metric::L2)).unwrap();
        for (id, v) in &data_b {
            docs.insert(id.clone(), v.clone()).unwrap();
        }
    }
    store.commit("two namespaces").unwrap();

    drop(store);
    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let mut ns_a = store.namespace("ns_a");
    let docs_a = ns_a.proximity_index("docs", config(4, Metric::L2)).unwrap();
    assert_eq!(docs_a.len(), data_a.len());
    drop(docs_a);
    drop(ns_a);

    let mut ns_b = store.namespace("ns_b");
    let docs_b = ns_b.proximity_index("docs", config(4, Metric::L2)).unwrap();
    assert_eq!(docs_b.len(), data_b.len());
}

#[test]
fn proximity_index_load_dim_mismatch_returns_error() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let data = random_vectors(20, 8, 0xf6);

    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let mut personal = store.namespace("personal");
        let mut docs = personal
            .proximity_index("docs", config(8, Metric::L2))
            .unwrap();
        for (id, v) in &data {
            docs.insert(id.clone(), v.clone()).unwrap();
        }
    }
    store.commit("dim 8").unwrap();
    drop(store);

    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let err = personal
        .proximity_index("docs", config(16, Metric::L2))
        .unwrap_err();
    assert!(matches!(
        err,
        ProximityError::DimensionMismatch {
            expected: 8,
            got: 16
        }
    ));
}

#[test]
fn proximity_index_root_hash_deterministic_after_reopen() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let data = random_vectors(60, 4, 0x07);

    let original_root;
    {
        let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
        let mut personal = store.namespace("personal");
        let mut docs = personal
            .proximity_index("docs", config(4, Metric::L2))
            .unwrap();
        for (id, v) in &data {
            docs.insert(id.clone(), v.clone()).unwrap();
        }
        original_root = docs.root_hash().unwrap();
        drop(docs);
        drop(personal);
        store.commit("write").unwrap();
    }

    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let mut docs = personal
        .proximity_index("docs", config(4, Metric::L2))
        .unwrap();
    assert_eq!(docs.root_hash().unwrap(), original_root);
}

#[test]
fn drop_proximity_index_removes_from_in_memory_cache() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let mut personal = store.namespace("personal");

    {
        let _docs = personal
            .proximity_index("docs", config(4, Metric::L2))
            .unwrap();
    }
    assert!(personal.drop_proximity_index("docs"));
    assert!(!personal.drop_proximity_index("docs"));
}

// ---------------------------------------------------------------------------
// PR 3c — Git-backed merge integration
// ---------------------------------------------------------------------------
//
// `merge_with_proximity_resolver` is only on `GitNamespacedKvStore` (the
// merge API itself only exists for the Git backend). These tests exercise the
// full path: insert + commit on dest, branch + insert + commit on source,
// switch back, merge with a proximity resolver, verify the result.

mod git_merge {
    use super::*;
    use prollytree::diff::IgnoreConflictsResolver;
    use prollytree::git::versioned_store::GitNamespacedKvStore;
    use prollytree::proximity::{
        LatestVectorResolver, MeanVectorResolver, TakeSourceProximityResolver,
    };

    fn checkout(store: &mut GitNamespacedKvStore<N>, branch: &str) {
        store.checkout(branch).expect("checkout failed");
    }

    fn create_branch(store: &mut GitNamespacedKvStore<N>, branch: &str) {
        store.create_branch(branch).expect("create_branch failed");
    }

    #[test]
    fn git_proximity_insert_commit_reopen() {
        // Sanity check — verifies PR 3a's flush + PR 3c's hash-mapping
        // consolidation work end-to-end on Git.
        let (_temp, dataset) = setup_repo_and_dataset();
        let data = random_vectors(40, 4, 0xa1);

        let original_root;
        {
            let mut store = GitNamespacedKvStore::<N>::init(&dataset).unwrap();
            let mut personal = store.namespace("personal");
            let mut docs = personal
                .proximity_index("docs", config(4, Metric::L2))
                .unwrap();
            for (id, v) in &data {
                docs.insert(id.clone(), v.clone()).unwrap();
            }
            original_root = docs.root_hash().unwrap();
            drop(docs);
            drop(personal);
            store.commit("ingest").unwrap();
        }

        let mut store = GitNamespacedKvStore::<N>::open(&dataset).unwrap();
        let mut personal = store.namespace("personal");
        let mut docs = personal
            .proximity_index("docs", config(4, Metric::L2))
            .unwrap();
        assert_eq!(docs.len(), data.len());
        assert_eq!(docs.root_hash().unwrap(), original_root);
        // Query something — exercises the full read path through the
        // consolidated `prolly_hash_mappings`.
        let _hits = docs.knn(&vec![0.1f32; 4], 5, 32).unwrap();
    }

    #[test]
    fn merge_with_proximity_disjoint_inserts() {
        // Two branches insert disjoint id ranges; merge should produce a
        // union with no conflicts regardless of which proximity resolver
        // is used (no overlapping ids).
        let (_temp, dataset) = setup_repo_and_dataset();
        let dim = 4u16;

        let main_data = random_vectors(20, dim as usize, 0xb1);
        let feature_data = random_vectors(20, dim as usize, 0xb2);
        // Rename feature_data ids so they don't clash with main_data.
        let feature_data: Vec<_> = feature_data
            .into_iter()
            .map(|(id, v)| {
                let mut k = b"f-".to_vec();
                k.extend(id);
                (k, v)
            })
            .collect();

        let mut store = GitNamespacedKvStore::<N>::init(&dataset).unwrap();
        // Initial commit with main_data.
        {
            let mut personal = store.namespace("personal");
            let mut docs = personal
                .proximity_index("docs", config(dim, Metric::L2))
                .unwrap();
            for (id, v) in &main_data {
                docs.insert(id.clone(), v.clone()).unwrap();
            }
        }
        store.commit("main: ingest").unwrap();

        // Create feature branch and add feature_data.
        create_branch(&mut store, "feature");
        {
            let mut personal = store.namespace("personal");
            let mut docs = personal
                .proximity_index("docs", config(dim, Metric::L2))
                .unwrap();
            for (id, v) in &feature_data {
                docs.insert(id.clone(), v.clone()).unwrap();
            }
        }
        store.commit("feature: ingest").unwrap();

        // Switch back to main, merge feature.
        checkout(&mut store, "main");
        // Re-open the index so it's loaded into memory for merge to consider.
        {
            let mut personal = store.namespace("personal");
            let _ = personal
                .proximity_index("docs", config(dim, Metric::L2))
                .unwrap();
        }

        store
            .merge_with_proximity_resolver(
                "feature",
                &IgnoreConflictsResolver,
                &TakeSourceProximityResolver,
            )
            .expect("merge with proximity should succeed");

        // After merge, the index should contain both sets of ids.
        let mut personal = store.namespace("personal");
        let mut docs = personal
            .proximity_index("docs", config(dim, Metric::L2))
            .unwrap();
        assert_eq!(docs.len(), main_data.len() + feature_data.len());

        // Spot-check: each id is queryable.
        let q = vec![0.0f32; dim as usize];
        let hits = docs
            .knn(&q, main_data.len() + feature_data.len(), 64)
            .unwrap();
        assert_eq!(hits.len(), main_data.len() + feature_data.len());
    }

    #[test]
    fn merge_with_proximity_conflicting_update_uses_mean() {
        // Both branches modify the same id's vector differently. With the
        // MeanVectorResolver the merged vector is the element-wise average.
        let (_temp, dataset) = setup_repo_and_dataset();
        let dim = 2u16;

        let mut store = GitNamespacedKvStore::<N>::init(&dataset).unwrap();
        {
            let mut personal = store.namespace("personal");
            let mut docs = personal
                .proximity_index("docs", config(dim, Metric::L2))
                .unwrap();
            docs.insert(b"k".to_vec(), vec![0.0, 0.0]).unwrap();
        }
        store.commit("base").unwrap();

        create_branch(&mut store, "feature");
        {
            let mut personal = store.namespace("personal");
            let mut docs = personal
                .proximity_index("docs", config(dim, Metric::L2))
                .unwrap();
            docs.insert(b"k".to_vec(), vec![4.0, 8.0]).unwrap();
        }
        store.commit("feature: update k").unwrap();

        checkout(&mut store, "main");
        {
            let mut personal = store.namespace("personal");
            let mut docs = personal
                .proximity_index("docs", config(dim, Metric::L2))
                .unwrap();
            docs.insert(b"k".to_vec(), vec![2.0, 4.0]).unwrap();
        }
        store.commit("main: update k").unwrap();

        // Reopen the index on main so merge considers it.
        {
            let mut personal = store.namespace("personal");
            let _ = personal
                .proximity_index("docs", config(dim, Metric::L2))
                .unwrap();
        }

        let mean = MeanVectorResolver::new(Metric::L2).unwrap();
        store
            .merge_with_proximity_resolver("feature", &IgnoreConflictsResolver, &mean)
            .expect("merge should succeed with MeanVectorResolver");

        // Mean of [4, 8] (source) and [2, 4] (dest) = [3, 6].
        let mut personal = store.namespace("personal");
        let mut docs = personal
            .proximity_index("docs", config(dim, Metric::L2))
            .unwrap();
        let hits = docs.knn(&[3.0, 6.0], 1, 8).unwrap();
        assert_eq!(hits[0].0, b"k".to_vec());
        // Distance from [3,6] to merged value [3,6] should be ~0.
        assert!(
            hits[0].1 < 1e-3,
            "expected mean-merged vector, got distance {}",
            hits[0].1
        );
    }

    #[test]
    fn merge_with_proximity_latest_picks_higher_timestamp() {
        // Encode a timestamp into the vector's last element. Source has a
        // higher timestamp on the conflicting id, so LatestVectorResolver
        // should pick source's vector.
        let (_temp, dataset) = setup_repo_and_dataset();
        let dim = 3u16;

        let mut store = GitNamespacedKvStore::<N>::init(&dataset).unwrap();
        {
            let mut personal = store.namespace("personal");
            let mut docs = personal
                .proximity_index("docs", config(dim, Metric::L2))
                .unwrap();
            // ts = 100
            docs.insert(b"k".to_vec(), vec![1.0, 1.0, 100.0]).unwrap();
        }
        store.commit("base").unwrap();

        create_branch(&mut store, "feature");
        {
            let mut personal = store.namespace("personal");
            let mut docs = personal
                .proximity_index("docs", config(dim, Metric::L2))
                .unwrap();
            // ts = 500 (newer)
            docs.insert(b"k".to_vec(), vec![5.0, 5.0, 500.0]).unwrap();
        }
        store.commit("feature: bump k").unwrap();

        checkout(&mut store, "main");
        {
            let mut personal = store.namespace("personal");
            let mut docs = personal
                .proximity_index("docs", config(dim, Metric::L2))
                .unwrap();
            // ts = 300 (newer than base but older than feature's update)
            docs.insert(b"k".to_vec(), vec![3.0, 3.0, 300.0]).unwrap();
        }
        store.commit("main: nudge k").unwrap();

        {
            let mut personal = store.namespace("personal");
            let _ = personal
                .proximity_index("docs", config(dim, Metric::L2))
                .unwrap();
        }

        let latest = LatestVectorResolver::new(|_id, v| v[2] as u64);
        store
            .merge_with_proximity_resolver("feature", &IgnoreConflictsResolver, &latest)
            .expect("merge should succeed");

        // Source wins (ts 500 > 300): merged value is [5, 5, 500].
        let mut personal = store.namespace("personal");
        let mut docs = personal
            .proximity_index("docs", config(dim, Metric::L2))
            .unwrap();
        let hits = docs.knn(&[5.0, 5.0, 500.0], 1, 8).unwrap();
        assert_eq!(hits[0].0, b"k".to_vec());
        assert!(hits[0].1 < 1e-3, "expected source vector to win");
    }
}
