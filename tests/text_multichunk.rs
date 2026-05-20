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

//! Multi-chunk plumbing tests.
//!
//! Verifies that under a non-identity chunker (`LineChunker`):
//! - inserts produce N chunks per document
//! - `len()` returns the doc count (deduplicated), `chunk_count()` returns
//!   the underlying chunk count
//! - `delete(doc_id)` removes EVERY chunk for that document via prefix scan
//! - `search()` returns top-k documents (dedup-by-doc internally)
//! - the cascade path uses the registered chunker too

#![cfg(all(feature = "proximity", feature = "git"))]

mod common;

use common::setup_repo_and_dataset;
use prollytree::git::versioned_store::FileNamespacedKvStore;
use prollytree::proximity::{HashEmbedder, LineChunker, TextIndexConfig};

const N: usize = 32;

fn line_cfg(dim: u16, seed: u64) -> TextIndexConfig<HashEmbedder> {
    TextIndexConfig::new(HashEmbedder::new(dim, seed)).with_chunker(LineChunker)
}

#[test]
fn line_chunker_produces_one_chunk_per_line() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    let mut personal = store.namespace("personal");
    let mut docs = personal.text_index("docs", line_cfg(8, 0)).unwrap();
    docs.insert(b"doc:1", "first line\nsecond line\nthird line")
        .unwrap();

    assert_eq!(docs.len(), 1, "one document inserted");
    assert_eq!(docs.chunk_count(), 3, "three chunks (one per line)");
}

#[test]
fn delete_removes_all_chunks_for_a_doc() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let mut docs = personal.text_index("docs", line_cfg(8, 0)).unwrap();

    docs.insert(b"doc:1", "alpha\nbeta\ngamma").unwrap();
    docs.insert(b"doc:2", "delta\nepsilon").unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(docs.chunk_count(), 5);

    assert!(docs.delete(b"doc:1"));
    assert_eq!(docs.len(), 1);
    assert_eq!(docs.chunk_count(), 2, "only doc:2's chunks remain");
}

#[test]
fn re_insert_replaces_all_old_chunks() {
    // Inserting the same doc_id with FEWER chunks must not leave stale
    // chunks from the previous insert.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let mut docs = personal.text_index("docs", line_cfg(8, 0)).unwrap();

    docs.insert(b"doc:1", "one\ntwo\nthree\nfour").unwrap();
    assert_eq!(docs.chunk_count(), 4);

    docs.insert(b"doc:1", "shorter\ntext").unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs.chunk_count(), 2, "old 4 chunks replaced by 2 new");
}

#[test]
fn search_returns_top_k_documents_dedupped() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let mut docs = personal.text_index("docs", line_cfg(16, 0)).unwrap();

    // Each doc has several lines; one of them is a known query match.
    docs.insert(
        b"doc:hit",
        "unrelated noise\nthe exact target line\nmore filler",
    )
    .unwrap();
    docs.insert(
        b"doc:other",
        "completely different content\nnothing matching here",
    )
    .unwrap();

    let hits = docs.search("the exact target line", 1).unwrap();
    assert_eq!(hits.len(), 1, "k=1 returns 1 document");
    assert_eq!(
        hits[0].id, b"doc:hit",
        "matching chunk's doc_id is the result"
    );
    assert!(
        hits[0].score < 1e-4,
        "exact match should yield near-zero distance"
    );
}

#[test]
fn search_dedups_when_many_chunks_match() {
    // A doc with multiple chunks closely matching the query must appear
    // only ONCE in the results, scored at its best chunk's distance.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let mut docs = personal.text_index("docs", line_cfg(8, 0)).unwrap();

    docs.insert(
        b"doc:hit",
        "near match here\nthe exact phrase\nyet more text", // 3 chunks; one exact
    )
    .unwrap();
    docs.insert(b"doc:other", "completely different\nstill different")
        .unwrap();

    let hits = docs.search("the exact phrase", 5).unwrap();
    let hit_ids: Vec<_> = hits.iter().map(|h| h.id.clone()).collect();
    let doc_hit_count = hit_ids
        .iter()
        .filter(|id| id == &&b"doc:hit".to_vec())
        .count();
    assert_eq!(
        doc_hit_count, 1,
        "doc:hit should appear once even though several chunks match"
    );
}

#[test]
fn cascade_with_chunker_inserts_per_line() {
    // Cascade respects the registered chunker — the primary insert mirrors
    // into the text index as multiple chunks, not a single embedding.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let _ = store
            .namespace("personal")
            .text_index("docs", line_cfg(8, 0))
            .unwrap();
    }
    store.set_cascade("personal", vec!["docs".to_string()]);

    {
        let mut personal = store.namespace("personal");
        personal
            .insert(b"doc:1".to_vec(), b"alpha\nbeta\ngamma".to_vec())
            .unwrap();
    }
    store.commit("cascade chunked").unwrap();

    let mut personal = store.namespace("personal");
    let docs = personal.text_index("docs", line_cfg(8, 0)).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs.chunk_count(), 3, "cascade chunked the primary value");
}

#[test]
fn cascade_delete_removes_all_chunks() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let _ = store
            .namespace("personal")
            .text_index("docs", line_cfg(8, 0))
            .unwrap();
    }
    store.set_cascade("personal", vec!["docs".to_string()]);

    {
        let mut personal = store.namespace("personal");
        personal
            .insert(b"doc:1".to_vec(), b"one\ntwo\nthree".to_vec())
            .unwrap();
    }
    store.commit("insert").unwrap();
    {
        let mut personal = store.namespace("personal");
        assert!(personal.delete(b"doc:1").unwrap());
    }
    store.commit("delete").unwrap();

    let mut personal = store.namespace("personal");
    let docs = personal.text_index("docs", line_cfg(8, 0)).unwrap();
    assert_eq!(docs.chunk_count(), 0, "cascade delete removed all chunks");
}

#[test]
fn empty_document_under_line_chunker_indexes_nothing() {
    // An empty doc under LineChunker produces 0 chunks — treated as opt-out
    // (matches the cascade transformer's `None` semantics).
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let mut docs = personal.text_index("docs", line_cfg(8, 0)).unwrap();

    docs.insert(b"doc:1", "").unwrap();
    assert_eq!(docs.chunk_count(), 0);
    assert_eq!(docs.len(), 0);
}

#[test]
fn identity_chunker_remains_default_back_compat() {
    // Without `.with_chunker(...)`, the default is IdentityChunker — one
    // chunk per document, exactly like pre-multi-chunk behaviour.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let mut docs = personal
        .text_index("docs", TextIndexConfig::new(HashEmbedder::new(8, 0)))
        .unwrap();

    docs.insert(b"doc:1", "first line\nsecond line\nthird line")
        .unwrap();

    assert_eq!(docs.len(), 1);
    assert_eq!(
        docs.chunk_count(),
        1,
        "IdentityChunker should produce 1 chunk regardless of newlines"
    );
}
