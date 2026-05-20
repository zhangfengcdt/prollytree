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

//! PR 4d — Auto-cascade tests.
//!
//! When `set_cascade(ns, [...])` is configured, `NamespaceHandle::insert` /
//! `::delete` automatically embed + upsert / remove from each listed text
//! sub-index that is currently loaded. Targets not yet opened in this process
//! and values that aren't valid UTF-8 are silently skipped — drift can be
//! detected after the fact via `audit_text_index`.

#![cfg(all(feature = "proximity", feature = "git"))]

mod common;

use common::setup_repo_and_dataset;
use prollytree::git::versioned_store::FileNamespacedKvStore;
use prollytree::proximity::{HashEmbedder, TextIndexConfig};

const N: usize = 32;

fn cfg(dim: u16, seed: u64) -> TextIndexConfig<HashEmbedder> {
    TextIndexConfig::new(HashEmbedder::new(dim, seed))
}

#[test]
fn no_cascade_by_default() {
    // Default behaviour: primary insert does NOT touch text indexes.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    // Open a text index so it exists in the in-memory cache.
    {
        let _ = store
            .namespace("personal")
            .text_index("docs", cfg(8, 0))
            .unwrap();
    }
    {
        let mut personal = store.namespace("personal");
        personal
            .insert(b"doc:1".to_vec(), b"hello world".to_vec())
            .unwrap();
    }
    store.commit("primary").unwrap();

    // The text index should NOT have been touched.
    let mut personal = store.namespace("personal");
    let docs = personal.text_index("docs", cfg(8, 0)).unwrap();
    assert_eq!(docs.len(), 0);
}

#[test]
fn cascade_mirrors_primary_inserts_to_one_text_index() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    // Open the text index FIRST so the cascade target is loaded.
    {
        let _ = store
            .namespace("personal")
            .text_index("docs", cfg(16, 0))
            .unwrap();
    }
    store.set_cascade("personal", vec!["docs".to_string()]);

    {
        let mut personal = store.namespace("personal");
        personal
            .insert(b"doc:1".to_vec(), b"the quick brown fox".to_vec())
            .unwrap();
        personal
            .insert(b"doc:2".to_vec(), b"lazy dog naps".to_vec())
            .unwrap();
    }
    store.commit("cascade").unwrap();

    // Both writes should now be searchable through the text index.
    let mut personal = store.namespace("personal");
    let mut docs = personal.text_index("docs", cfg(16, 0)).unwrap();
    assert_eq!(docs.len(), 2);
    let hits = docs.search("the quick brown fox", 1).unwrap();
    assert_eq!(hits[0].id, b"doc:1".to_vec());
}

#[test]
fn cascade_mirrors_to_multiple_text_indexes() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let mut personal = store.namespace("personal");
        let _ = personal.text_index("by_title", cfg(8, 1)).unwrap();
        let _ = personal.text_index("by_summary", cfg(8, 2)).unwrap();
    }
    store.set_cascade(
        "personal",
        vec!["by_title".to_string(), "by_summary".to_string()],
    );

    {
        let mut personal = store.namespace("personal");
        personal
            .insert(b"doc:1".to_vec(), b"a third unrelated thing".to_vec())
            .unwrap();
    }
    store.commit("two-cascade").unwrap();

    let mut personal = store.namespace("personal");
    {
        let by_title = personal.text_index("by_title", cfg(8, 1)).unwrap();
        assert_eq!(by_title.len(), 1);
    }
    {
        let by_summary = personal.text_index("by_summary", cfg(8, 2)).unwrap();
        assert_eq!(by_summary.len(), 1);
    }
}

#[test]
fn cascade_delete_removes_from_text_indexes() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let _ = store
            .namespace("personal")
            .text_index("docs", cfg(8, 0))
            .unwrap();
    }
    store.set_cascade("personal", vec!["docs".to_string()]);

    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), b"content".to_vec()).unwrap();
    }
    store.commit("write").unwrap();
    {
        let mut personal = store.namespace("personal");
        assert!(personal.delete(b"k").unwrap());
    }
    store.commit("delete").unwrap();

    let mut personal = store.namespace("personal");
    let docs = personal.text_index("docs", cfg(8, 0)).unwrap();
    assert_eq!(docs.len(), 0);
}

#[test]
fn cascade_silently_skips_unopened_text_index() {
    // Listed cascade target that has never been opened → silently skipped.
    // The primary insert still goes through; drift is observable via audit.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    store.set_cascade("personal", vec!["docs".to_string()]); // never opened

    {
        let mut personal = store.namespace("personal");
        personal
            .insert(b"k".to_vec(), b"text content".to_vec())
            .unwrap();
    }
    store.commit("orphaned cascade target").unwrap();

    // Now open the text index — it's empty, because the cascade silently
    // skipped (target wasn't loaded at insert time). User can detect this
    // via audit + reindex if they care.
    let mut personal = store.namespace("personal");
    let docs = personal.text_index("docs", cfg(8, 0)).unwrap();
    assert_eq!(docs.len(), 0);
}

#[test]
fn cascade_silently_skips_non_utf8_value() {
    // PR 4d minimum: cascade requires UTF-8 input. Non-UTF-8 values are
    // skipped silently. A future value_transformer hook will let users
    // override this for structured-binary payloads.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    {
        let _ = store
            .namespace("personal")
            .text_index("docs", cfg(8, 0))
            .unwrap();
    }
    store.set_cascade("personal", vec!["docs".to_string()]);

    // Bytes that are not valid UTF-8.
    let bad_utf8 = vec![0xC3, 0x28];
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), bad_utf8.clone()).unwrap();
    }
    store.commit("binary").unwrap();

    // Primary tree has the bytes; text index was skipped.
    let mut personal = store.namespace("personal");
    assert_eq!(personal.get(b"k"), Some(bad_utf8));
    let docs = personal.text_index("docs", cfg(8, 0)).unwrap();
    assert_eq!(docs.len(), 0);
}

#[test]
fn cascade_unconfigured_namespace_unaffected() {
    // Cascade is per-namespace; configuring it for "personal" must NOT
    // affect "work" inserts.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();

    {
        let _ = store
            .namespace("personal")
            .text_index("docs", cfg(8, 0))
            .unwrap();
        let _ = store
            .namespace("work")
            .text_index("docs", cfg(8, 0))
            .unwrap();
    }
    store.set_cascade("personal", vec!["docs".to_string()]);

    {
        let mut personal = store.namespace("personal");
        personal.insert(b"a".to_vec(), b"text a".to_vec()).unwrap();
    }
    {
        let mut work = store.namespace("work");
        work.insert(b"b".to_vec(), b"text b".to_vec()).unwrap();
    }
    store.commit("both").unwrap();

    let mut personal = store.namespace("personal");
    let docs_p = personal.text_index("docs", cfg(8, 0)).unwrap();
    assert_eq!(docs_p.len(), 1, "personal/docs should have cascaded entry");
    drop(docs_p);
    drop(personal);

    let mut work = store.namespace("work");
    let docs_w = work.text_index("docs", cfg(8, 0)).unwrap();
    assert_eq!(docs_w.len(), 0, "work/docs has no cascade configured");
}

#[test]
fn clear_cascade_disables_further_mirroring() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    {
        let _ = store
            .namespace("personal")
            .text_index("docs", cfg(8, 0))
            .unwrap();
    }
    store.set_cascade("personal", vec!["docs".to_string()]);
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"a".to_vec(), b"first".to_vec()).unwrap();
    }
    store.commit("first").unwrap();

    store.clear_cascade("personal");
    assert!(store.cascade_for_namespace("personal").is_none());

    {
        let mut personal = store.namespace("personal");
        personal.insert(b"b".to_vec(), b"second".to_vec()).unwrap();
    }
    store.commit("second").unwrap();

    let mut personal = store.namespace("personal");
    let docs = personal.text_index("docs", cfg(8, 0)).unwrap();
    assert_eq!(
        docs.len(),
        1,
        "only the first insert (before clear_cascade) should be in the index"
    );
}

#[test]
fn cascade_lookup_accessor_reflects_config() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    assert!(store.cascade_for_namespace("personal").is_none());

    store.set_cascade(
        "personal",
        vec!["docs".to_string(), "summaries".to_string()],
    );
    let list = store.cascade_for_namespace("personal").unwrap();
    assert_eq!(list, &["docs".to_string(), "summaries".to_string()]);
}
