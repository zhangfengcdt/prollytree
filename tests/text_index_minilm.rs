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

//! PR 4b — integration tests that plug `MiniLmEmbedder` into the namespaced
//! `TextIndex` API.
//!
//! Every test that actually calls `.embed()` is `#[ignore]`d so default
//! `cargo test` runs don't try to download ~90 MB of model weights. Run
//! manually with `--include-ignored` to validate end-to-end.

#![cfg(all(feature = "proximity_text", feature = "git"))]

mod common;

use common::setup_repo_and_dataset;
use prollytree::git::versioned_store::FileNamespacedKvStore;
use prollytree::proximity::{Embedder, MiniLmEmbedder, TextIndexConfig, MINILM_DIM};

const N: usize = 32;

#[test]
fn minilm_embedder_plugs_into_text_index_config_generic() {
    // Compile-time check that `TextIndexConfig<MiniLmEmbedder>` is buildable
    // and that the embedder advertises the expected metadata. No network.
    let cfg = TextIndexConfig::new(MiniLmEmbedder::default());
    assert_eq!(cfg.embedder.dim(), MINILM_DIM);
    let _ = cfg.metric; // type-level check
}

#[test]
fn minilm_embedder_constructs_namespaced_handle_lazily() {
    // Opening the text index lazily writes the identity blob without
    // touching the model: the embedder is only consulted for its
    // `id()`/`version()`/`dim()` here, none of which load weights.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let docs = personal
        .text_index("docs", TextIndexConfig::new(MiniLmEmbedder::default()))
        .expect("opening with MiniLM should not require model load");
    assert!(docs.is_empty());
    assert_eq!(docs.embedder().dim(), MINILM_DIM);
}

#[test]
#[ignore = "downloads model from HuggingFace; run with --include-ignored"]
fn minilm_embed_search_round_trip_through_namespaced_text_index() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    // Pre-warm the embedder so we surface load failures clearly rather than
    // as a generic insert error.
    let embedder = MiniLmEmbedder::default();
    embedder.warm_up().expect("MiniLM warm-up failed");

    {
        let mut personal = store.namespace("personal");
        let mut docs = personal
            .text_index("docs", TextIndexConfig::new(embedder))
            .unwrap();

        docs.insert(b"cat", "the cat sat on the mat").unwrap();
        docs.insert(b"cat-2", "a cat is sitting on a mat").unwrap();
        docs.insert(b"physics", "quantum field theory and renormalization")
            .unwrap();
        docs.insert(b"food", "tomato soup with basil and cream")
            .unwrap();

        // Semantic search: the cat query should rank both cat documents
        // above the physics one. We don't pin specific top-1 — model
        // determinism + tokenizer choices could vary that — we just check
        // the ordering relative to the unrelated entry.
        let hits = docs.search("a feline on a rug", 4).unwrap();
        let mut cat_positions = Vec::new();
        let mut physics_position = None;
        for (i, h) in hits.iter().enumerate() {
            if h.id == b"cat" || h.id == b"cat-2" {
                cat_positions.push(i);
            }
            if h.id == b"physics" {
                physics_position = Some(i);
            }
        }
        let max_cat = *cat_positions.iter().max().unwrap_or(&usize::MAX);
        let phys = physics_position.unwrap_or(usize::MAX);
        assert!(
            max_cat < phys,
            "expected both cat documents to rank above 'physics'; got hits {:?}",
            hits.iter()
                .map(|h| String::from_utf8_lossy(&h.id))
                .collect::<Vec<_>>()
        );
    }

    store.commit("ingest cat documents").unwrap();
}

#[test]
#[ignore = "downloads model from HuggingFace; run with --include-ignored"]
fn minilm_text_index_survives_commit_and_reopen() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let query = "feline on a rug";
    let original;

    {
        let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
        let embedder = MiniLmEmbedder::default();
        embedder.warm_up().expect("MiniLM warm-up failed");

        let mut personal = store.namespace("personal");
        let mut docs = personal
            .text_index("docs", TextIndexConfig::new(embedder))
            .unwrap();
        docs.insert(b"cat", "the cat sat on the mat").unwrap();
        docs.insert(b"dog", "the dog ran in the park").unwrap();
        original = docs.search(query, 2).unwrap();
        drop(docs);
        drop(personal);
        store.commit("ingest").unwrap();
    }

    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let mut personal = store.namespace("personal");
    let mut docs = personal
        .text_index("docs", TextIndexConfig::new(MiniLmEmbedder::default()))
        .unwrap();
    assert_eq!(docs.len(), 2);
    let reopened = docs.search(query, 2).unwrap();
    assert_eq!(reopened, original);
}
