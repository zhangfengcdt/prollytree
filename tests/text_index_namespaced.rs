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

//! PR 4a — namespaced text-index integration tests.
//!
//! Exercise `NamespaceHandle::text_index()` on the File-backed
//! `NamespacedKvStore` (the supported persistence path for v1). Uses the
//! built-in `HashEmbedder` so the tests have no network/ML dependency. The
//! Candle MiniLM embedder lands in PR 4b.

#![cfg(all(feature = "proximity", feature = "git"))]

mod common;

use common::setup_repo_and_dataset;
use prollytree::git::versioned_store::FileNamespacedKvStore;
use prollytree::proximity::{HashEmbedder, TextIndexConfig, TextIndexError};

const N: usize = 32;

fn cfg(dim: u16, seed: u64) -> TextIndexConfig<HashEmbedder> {
    TextIndexConfig::new(HashEmbedder::new(dim, seed))
}

#[test]
fn text_index_insert_search_basic() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let mut docs = personal.text_index("docs", cfg(32, 0)).unwrap();

    docs.insert(b"doc:1", "the quick brown fox").unwrap();
    docs.insert(b"doc:2", "lazy dog naps in sun").unwrap();
    docs.insert(b"doc:3", "a third unrelated thing").unwrap();

    // Exact match for an existing text → that document tops the result list.
    let hits = docs.search("the quick brown fox", 1).unwrap();
    assert_eq!(hits[0].id, b"doc:1".to_vec());
    assert!(hits[0].score < 1e-4);
}

#[test]
fn text_index_search_saturates_overfetch_for_huge_k() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let mut docs = personal.text_index("docs", cfg(8, 0)).unwrap();
    docs.insert(b"doc:1", "the quick brown fox").unwrap();

    let hits = docs.search("the quick brown fox", usize::MAX).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, b"doc:1".to_vec());
}

#[test]
fn text_index_survives_commit_and_reopen() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let query = "lazy dog naps in sun";

    let original_top;
    {
        let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
        let mut personal = store.namespace("personal");
        let mut docs = personal.text_index("docs", cfg(32, 0)).unwrap();
        docs.insert(b"doc:1", "the quick brown fox").unwrap();
        docs.insert(b"doc:2", "lazy dog naps in sun").unwrap();
        docs.insert(b"doc:3", "a third unrelated thing").unwrap();
        original_top = docs.search(query, 1).unwrap();
        drop(docs);
        drop(personal);
        store.commit("ingest").unwrap();
    }

    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let mut docs = personal.text_index("docs", cfg(32, 0)).unwrap();
    assert_eq!(docs.len(), 3);
    assert_eq!(docs.search(query, 1).unwrap(), original_top);
}

#[test]
fn reopen_with_different_embedder_id_returns_mismatch() {
    // Different embedder family at re-open time → EmbedderMismatch.
    use prollytree::proximity::{EmbedError, Embedder};

    struct DifferentFamily(HashEmbedder);
    impl Embedder for DifferentFamily {
        fn id(&self) -> &str {
            "another:family/v1"
        }
        fn version(&self) -> &str {
            self.0.version()
        }
        fn dim(&self) -> u16 {
            self.0.dim()
        }
        fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
            self.0.embed(text)
        }
    }

    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let mut personal = store.namespace("personal");
        let mut docs = personal.text_index("docs", cfg(16, 0)).unwrap();
        docs.insert(b"a", "x").unwrap();
    }
    store.commit("ingest").unwrap();
    drop(store);

    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let err = personal
        .text_index(
            "docs",
            TextIndexConfig::new(DifferentFamily(HashEmbedder::new(16, 0))),
        )
        .unwrap_err();
    assert!(
        matches!(err, TextIndexError::EmbedderMismatch { .. }),
        "expected EmbedderMismatch, got {err:?}"
    );
}

#[test]
fn reopen_with_different_embedder_version_returns_mismatch() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let mut personal = store.namespace("personal");
        let mut docs = personal.text_index("docs", cfg(16, 0)).unwrap();
        docs.insert(b"a", "x").unwrap();
    }
    store.commit("ingest").unwrap();
    drop(store);

    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    // Different seed → different `version()` string.
    let err = personal.text_index("docs", cfg(16, 1)).unwrap_err();
    assert!(matches!(err, TextIndexError::EmbedderMismatch { .. }));
}

#[test]
fn multiple_text_indexes_per_namespace_are_independent() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let mut personal = store.namespace("personal");
        {
            let mut titles = personal.text_index("titles", cfg(16, 1)).unwrap();
            titles.insert(b"doc:1", "Annual report 2025").unwrap();
            titles.insert(b"doc:2", "Tomato soup recipe").unwrap();
        }
        {
            let mut summaries = personal.text_index("summaries", cfg(16, 2)).unwrap();
            summaries
                .insert(b"doc:1", "Company performance was strong.")
                .unwrap();
            summaries
                .insert(b"doc:2", "Simple soup made with fresh tomatoes.")
                .unwrap();
        }
    }
    store.commit("two indexes").unwrap();

    drop(store);
    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    {
        let titles = personal.text_index("titles", cfg(16, 1)).unwrap();
        assert_eq!(titles.len(), 2);
    }
    {
        let summaries = personal.text_index("summaries", cfg(16, 2)).unwrap();
        assert_eq!(summaries.len(), 2);
    }
}

#[test]
fn proximity_and_text_index_with_same_name_dont_collide() {
    // A user-named proximity index "docs" and a text index "docs" should
    // coexist in the same namespace because the text-index internally lives
    // under "__text__docs".
    use prollytree::proximity::{Metric, ProximityConfig};

    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let mut personal = store.namespace("personal");
        let prox_cfg = ProximityConfig {
            dim: 4,
            metric: Metric::L2,
            level_bits: 4,
            max_bucket_size: 64,
        };
        let mut prox = personal.proximity_index("docs", prox_cfg).unwrap();
        prox.insert(b"v:1".to_vec(), vec![1.0, 0.0, 0.0, 0.0])
            .unwrap();
    }
    {
        let mut personal = store.namespace("personal");
        let mut text = personal.text_index("docs", cfg(8, 0)).unwrap();
        text.insert(b"t:1", "hello").unwrap();
    }
    store.commit("both").unwrap();

    drop(store);
    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let prox = personal
        .proximity_index(
            "docs",
            prollytree::proximity::ProximityConfig {
                dim: 4,
                metric: prollytree::proximity::Metric::L2,
                level_bits: 4,
                max_bucket_size: 64,
            },
        )
        .unwrap();
    assert_eq!(prox.len(), 1);
    drop(prox);
    let text = personal.text_index("docs", cfg(8, 0)).unwrap();
    assert_eq!(text.len(), 1);
}

#[test]
fn drop_text_index_removes_in_memory_cache() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let mut personal = store.namespace("personal");

    {
        let _docs = personal.text_index("docs", cfg(8, 0)).unwrap();
    }
    assert!(personal.drop_text_index("docs"));
    assert!(!personal.drop_text_index("docs"));
}

// ---------------------------------------------------------------------------
// PR 4c — Drift management (audit + purge_orphans)
// ---------------------------------------------------------------------------

#[test]
fn audit_in_sync_index_returns_empty_report() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    {
        let mut personal = store.namespace("personal");
        personal
            .insert(b"a".to_vec(), b"content a".to_vec())
            .unwrap();
        personal
            .insert(b"b".to_vec(), b"content b".to_vec())
            .unwrap();
        let mut docs = personal.text_index("docs", cfg(8, 0)).unwrap();
        docs.insert(b"a", "content a").unwrap();
        docs.insert(b"b", "content b").unwrap();
    }

    let report = store.audit_text_index("personal", "docs").unwrap();
    assert!(report.is_in_sync(), "expected in-sync, got {report:?}");
}

#[test]
fn audit_detects_orphan_ids_in_index() {
    // Index has more ids than the primary tree → those extras are orphans.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    {
        let mut personal = store.namespace("personal");
        personal.insert(b"a".to_vec(), b"x".to_vec()).unwrap();
        // Index contains 'a' AND 'orphan' — 'orphan' isn't in primary.
        let mut docs = personal.text_index("docs", cfg(8, 0)).unwrap();
        docs.insert(b"a", "x").unwrap();
        docs.insert(b"orphan", "ghost").unwrap();
    }

    let report = store.audit_text_index("personal", "docs").unwrap();
    assert_eq!(report.orphans_in_index, vec![b"orphan".to_vec()]);
    assert!(report.missing_from_index.is_empty());
    assert!(!report.is_in_sync());
}

#[test]
fn audit_detects_missing_ids_in_index() {
    // Primary tree has more ids than the index → those are missing.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    {
        let mut personal = store.namespace("personal");
        personal.insert(b"a".to_vec(), b"x".to_vec()).unwrap();
        personal.insert(b"b".to_vec(), b"y".to_vec()).unwrap();
        let mut docs = personal.text_index("docs", cfg(8, 0)).unwrap();
        docs.insert(b"a", "x").unwrap();
        // 'b' not added to the index.
    }

    let report = store.audit_text_index("personal", "docs").unwrap();
    assert!(report.orphans_in_index.is_empty());
    assert_eq!(report.missing_from_index, vec![b"b".to_vec()]);
}

#[test]
fn audit_returns_not_found_for_unknown_index() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        // Touch the namespace so it exists; no index opened.
        let _ = store.namespace("personal");
    }
    let err = store
        .audit_text_index("personal", "never_opened")
        .unwrap_err();
    assert!(matches!(err, TextIndexError::NotFound(_)));
}

#[test]
fn purge_orphans_removes_them_from_index() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    {
        let mut personal = store.namespace("personal");
        personal.insert(b"a".to_vec(), b"x".to_vec()).unwrap();
        personal.insert(b"b".to_vec(), b"y".to_vec()).unwrap();
        let mut docs = personal.text_index("docs", cfg(8, 0)).unwrap();
        docs.insert(b"a", "x").unwrap();
        docs.insert(b"b", "y").unwrap();
        docs.insert(b"orphan-1", "ghost").unwrap();
        docs.insert(b"orphan-2", "another ghost").unwrap();
    }

    let purged = store.purge_text_index_orphans("personal", "docs").unwrap();
    assert_eq!(purged, 2);

    // Audit should now be in sync.
    let report = store.audit_text_index("personal", "docs").unwrap();
    assert!(report.is_in_sync(), "still not in sync: {report:?}");

    // Confirm by counting via the handle.
    let mut personal = store.namespace("personal");
    let docs = personal.text_index("docs", cfg(8, 0)).unwrap();
    assert_eq!(docs.len(), 2);
}

#[test]
fn purge_orphans_is_zero_on_synced_index() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"a".to_vec(), b"x".to_vec()).unwrap();
        let mut docs = personal.text_index("docs", cfg(8, 0)).unwrap();
        docs.insert(b"a", "x").unwrap();
    }
    let purged = store.purge_text_index_orphans("personal", "docs").unwrap();
    assert_eq!(purged, 0);
}

// ---------------------------------------------------------------------------
// PR 4a — namespace isolation (existing test, kept here)
// ---------------------------------------------------------------------------

#[test]
fn text_index_isolated_across_namespaces() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let mut ns_a = store.namespace("ns_a");
        let mut docs = ns_a.text_index("docs", cfg(8, 0)).unwrap();
        docs.insert(b"a", "from a").unwrap();
    }
    {
        let mut ns_b = store.namespace("ns_b");
        let mut docs = ns_b.text_index("docs", cfg(8, 0)).unwrap();
        docs.insert(b"b1", "first from b").unwrap();
        docs.insert(b"b2", "second from b").unwrap();
    }
    store.commit("two namespaces").unwrap();

    drop(store);
    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    {
        let mut ns_a = store.namespace("ns_a");
        let docs = ns_a.text_index("docs", cfg(8, 0)).unwrap();
        assert_eq!(docs.len(), 1);
    }
    {
        let mut ns_b = store.namespace("ns_b");
        let docs = ns_b.text_index("docs", cfg(8, 0)).unwrap();
        assert_eq!(docs.len(), 2);
    }
}
