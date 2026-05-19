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
