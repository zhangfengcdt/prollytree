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

//! PR 0b — End-to-end externalisation tests on `FileNamespacedKvStore`.
//!
//! Strategy: large values written through `NamespaceHandle::insert` should
//! land as blobs in the underlying [`NodeStorage`]'s blob store (testable
//! via `inner_storage().get_blob(hash)`), while small values stay inline.
//! In all cases the public `get()` API returns the user's original bytes.

#![cfg(feature = "git")]

mod common;

use common::setup_repo_and_dataset;
use prollytree::digest::ValueDigest;
use prollytree::git::versioned_store::FileNamespacedKvStore;
use prollytree::storage::NodeStorage;
use std::path::Path;
use std::process::Command;

const N: usize = 32;

fn run_git(repo_path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn insert_orphan_blob(store: &FileNamespacedKvStore<N>, payload: &[u8]) -> ValueDigest<N> {
    let hash = ValueDigest::<N>::new(payload);
    let mut storage = store.inner_storage().clone();
    storage.insert_blob(hash.clone(), payload).unwrap();
    hash
}

#[test]
fn small_value_stays_inline_when_threshold_set() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(1024));

    let payload = b"short value".to_vec();
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"small".to_vec(), payload.clone()).unwrap();
    }
    store.commit("small").unwrap();

    // get() returns the original.
    let personal = store.namespace("personal");
    assert_eq!(personal.get(b"small"), Some(payload.clone()));

    // The blob store should NOT have an entry at the payload's hash.
    let hash = ValueDigest::<N>::new(&payload);
    assert!(
        store.inner_storage().get_blob(&hash).is_none(),
        "small value should not be in blob store"
    );
}

#[test]
fn large_value_lands_in_blob_store_but_get_returns_original() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));

    // Payload > 64 bytes should be externalised.
    let payload: Vec<u8> = (0..2048).map(|i| (i % 251) as u8).collect();
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"big".to_vec(), payload.clone()).unwrap();
    }
    store.commit("big").unwrap();

    // get() transparently returns the original bytes.
    let personal = store.namespace("personal");
    assert_eq!(personal.get(b"big"), Some(payload.clone()));

    // The blob store now holds the payload, keyed by its content hash.
    let hash = ValueDigest::<N>::new(&payload);
    let blob = store
        .inner_storage()
        .get_blob(&hash)
        .expect("blob should be present after externalisation");
    assert_eq!(blob, payload);
}

#[test]
fn boundary_at_threshold_stays_inline() {
    // Predicate is `value.len() > threshold` — a value exactly at the
    // threshold stays inline.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(100));

    let payload: Vec<u8> = vec![0xAB; 100];
    {
        let mut personal = store.namespace("personal");
        personal
            .insert(b"borderline".to_vec(), payload.clone())
            .unwrap();
    }
    store.commit("border").unwrap();

    let hash = ValueDigest::<N>::new(&payload);
    assert!(
        store.inner_storage().get_blob(&hash).is_none(),
        "value at threshold should stay inline"
    );

    let personal = store.namespace("personal");
    assert_eq!(personal.get(b"borderline"), Some(payload));
}

#[test]
fn one_byte_above_threshold_externalises() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(100));

    let payload: Vec<u8> = vec![0xCD; 101];
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), payload.clone()).unwrap();
    }
    store.commit("over").unwrap();

    let hash = ValueDigest::<N>::new(&payload);
    assert!(
        store.inner_storage().get_blob(&hash).is_some(),
        "one byte above threshold should externalise"
    );

    let personal = store.namespace("personal");
    assert_eq!(personal.get(b"k"), Some(payload));
}

#[test]
fn externalised_value_survives_commit_and_reopen() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let payload: Vec<u8> = (0..1_048_576).map(|i| (i % 251) as u8).collect(); // 1 MB
    let hash = ValueDigest::<N>::new(&payload);

    {
        let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
        store.set_externalize_threshold(Some(64 * 1024));
        let mut personal = store.namespace("personal");
        personal
            .insert(b"document".to_vec(), payload.clone())
            .unwrap();
        drop(personal);
        store.commit("ingest").unwrap();
    }

    // Fresh process. Read-side doesn't require setting the threshold.
    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let personal = store.namespace("personal");
    let got = personal.get(b"document").expect("should be present");
    assert_eq!(got.len(), payload.len());
    assert_eq!(got, payload);
    // The blob is still on disk under the same hash.
    assert!(store.inner_storage().get_blob(&hash).is_some());
}

#[test]
fn threshold_disabled_keeps_old_behaviour() {
    // No threshold ⇒ everything stays inline. This is the back-compat
    // default — existing users see no behaviour change at all.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    assert!(store.externalize_threshold().is_none());

    let payload: Vec<u8> = vec![0x42; 5_000];
    let hash = ValueDigest::<N>::new(&payload);
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), payload.clone()).unwrap();
    }
    store.commit("inline").unwrap();

    // Blob store is empty.
    assert!(store.inner_storage().get_blob(&hash).is_none());

    let personal = store.namespace("personal");
    assert_eq!(personal.get(b"k"), Some(payload));
}

#[test]
fn staged_large_value_is_visible_before_commit() {
    // Pre-commit, the value lives in staging as inline user bytes (not yet
    // externalised). The user-facing `get` should still return original.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));

    let payload: Vec<u8> = vec![0xCC; 200];
    let mut personal = store.namespace("personal");
    personal
        .insert(b"staged".to_vec(), payload.clone())
        .unwrap();
    assert_eq!(personal.get(b"staged"), Some(payload));
}

#[test]
fn delete_then_get_returns_none_even_when_externalised() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));

    let payload = vec![0xDD; 500];
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), payload).unwrap();
    }
    store.commit("write").unwrap();

    {
        let mut personal = store.namespace("personal");
        assert!(personal.delete(b"k").unwrap());
    }
    store.commit("delete").unwrap();

    let personal = store.namespace("personal");
    assert!(personal.get(b"k").is_none());
}

#[test]
fn upsert_changes_externalised_value() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));

    let v1 = vec![0xAA; 300];
    let v2 = vec![0xBB; 400];
    let h1 = ValueDigest::<N>::new(&v1);
    let h2 = ValueDigest::<N>::new(&v2);

    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), v1).unwrap();
    }
    store.commit("v1").unwrap();

    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), v2.clone()).unwrap();
    }
    store.commit("v2").unwrap();

    // Both blobs are present (v1 isn't eagerly deleted — that's PR 0c GC).
    assert!(store.inner_storage().get_blob(&h1).is_some());
    assert!(store.inner_storage().get_blob(&h2).is_some());

    // User sees the latest.
    let personal = store.namespace("personal");
    assert_eq!(personal.get(b"k"), Some(v2));
}

// ---------------------------------------------------------------------------
// PR 0c — gc_blobs
// ---------------------------------------------------------------------------

#[test]
fn gc_blobs_empty_store_is_noop() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    let report = store.gc_blobs().unwrap();
    assert_eq!(report.total, 0);
    assert_eq!(report.referenced, 0);
    assert_eq!(report.removed, 0);
    assert!(report.errors.is_empty());
}

#[test]
fn gc_blobs_keeps_referenced_blobs() {
    // Insert two externalised values, both referenced. GC should keep them.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));

    let p1: Vec<u8> = vec![0xAA; 200];
    let p2: Vec<u8> = vec![0xBB; 300];
    let h1 = ValueDigest::<N>::new(&p1);
    let h2 = ValueDigest::<N>::new(&p2);

    {
        let mut personal = store.namespace("personal");
        personal.insert(b"a".to_vec(), p1).unwrap();
        personal.insert(b"b".to_vec(), p2).unwrap();
    }
    store.commit("two").unwrap();

    let report = store.gc_blobs().unwrap();
    assert_eq!(report.total, 2);
    assert_eq!(report.referenced, 2);
    assert_eq!(report.removed, 0);

    // Both blobs are still readable.
    assert!(store.inner_storage().get_blob(&h1).is_some());
    assert!(store.inner_storage().get_blob(&h2).is_some());
}

#[test]
fn gc_blobs_keeps_upsert_history_and_removes_true_orphan() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));

    let v1 = vec![0xAA; 200];
    let v2 = vec![0xBB; 300];
    let h1 = ValueDigest::<N>::new(&v1);
    let h2 = ValueDigest::<N>::new(&v2);

    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), v1).unwrap();
    }
    store.commit("v1").unwrap();
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), v2.clone()).unwrap();
    }
    store.commit("v2").unwrap();

    // Pre-GC: both blobs are still around.
    assert!(store.inner_storage().get_blob(&h1).is_some());
    assert!(store.inner_storage().get_blob(&h2).is_some());
    let orphan = vec![0xCC; 350];
    let orphan_hash = insert_orphan_blob(&store, &orphan);
    assert!(store.inner_storage().get_blob(&orphan_hash).is_some());

    let report = store.gc_blobs().unwrap();
    assert_eq!(report.total, 3);
    assert_eq!(report.referenced, 2);
    assert_eq!(report.removed, 1);
    assert_eq!(report.remaining(), 2);

    // Both committed blobs survive; only the never-referenced blob is gone.
    assert!(store.inner_storage().get_blob(&h1).is_some());
    assert!(store.inner_storage().get_blob(&h2).is_some());
    assert!(store.inner_storage().get_blob(&orphan_hash).is_none());

    // The user still reads the latest value normally.
    let personal = store.namespace("personal");
    assert_eq!(personal.get(b"k"), Some(v2));
}

#[test]
fn gc_blobs_keeps_blobs_referenced_by_historical_commits() {
    let (temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));

    let old_value = vec![0xA1; 256];
    let new_value = vec![0xB2; 300];
    let old_hash = ValueDigest::<N>::new(&old_value);
    let new_hash = ValueDigest::<N>::new(&new_value);

    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), old_value.clone()).unwrap();
    }
    store.commit("old externalized value").unwrap();
    store.create_branch("old-value").unwrap();
    drop(store);
    run_git(temp.path(), &["checkout", "main"]);

    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), new_value.clone()).unwrap();
    }
    store.commit("new externalized value").unwrap();

    assert!(store.inner_storage().get_blob(&old_hash).is_some());
    assert!(store.inner_storage().get_blob(&new_hash).is_some());

    let report = store.gc_blobs().unwrap();
    assert_eq!(report.total, 2);
    assert_eq!(report.referenced, 2);
    assert_eq!(report.removed, 0);
    assert!(store.inner_storage().get_blob(&old_hash).is_some());
    assert!(store.inner_storage().get_blob(&new_hash).is_some());

    drop(store);
    run_git(temp.path(), &["checkout", "old-value"]);
    let mut old_store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();
    let personal = old_store.namespace("personal");
    assert_eq!(personal.get(b"k"), Some(old_value));
}

#[test]
fn gc_blobs_keeps_deleted_history_and_removes_true_orphan() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));

    let payload = vec![0xCC; 500];
    let hash = ValueDigest::<N>::new(&payload);
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), payload).unwrap();
    }
    store.commit("insert").unwrap();
    assert!(store.inner_storage().get_blob(&hash).is_some());

    {
        let mut personal = store.namespace("personal");
        assert!(personal.delete(b"k").unwrap());
    }
    store.commit("delete").unwrap();

    // Blob still around after delete commit and remains referenced by history.
    assert!(store.inner_storage().get_blob(&hash).is_some());
    let orphan = vec![0xDD; 350];
    let orphan_hash = insert_orphan_blob(&store, &orphan);

    let report = store.gc_blobs().unwrap();
    assert_eq!(report.total, 2);
    assert_eq!(report.referenced, 1);
    assert_eq!(report.removed, 1);

    assert!(store.inner_storage().get_blob(&hash).is_some());
    assert!(store.inner_storage().get_blob(&orphan_hash).is_none());
}

#[test]
fn gc_blobs_keeps_blobs_across_namespaces() {
    // Blobs referenced from one namespace must not be deleted just because
    // another namespace also references them — but more importantly, blobs
    // unique to namespace B must not be deleted when GC runs after the user
    // only touched namespace A.
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));

    let pa = vec![0xAA; 200];
    let pb = vec![0xBB; 300];
    let ha = ValueDigest::<N>::new(&pa);
    let hb = ValueDigest::<N>::new(&pb);

    {
        let mut a = store.namespace("ns_a");
        a.insert(b"x".to_vec(), pa).unwrap();
    }
    {
        let mut b = store.namespace("ns_b");
        b.insert(b"y".to_vec(), pb).unwrap();
    }
    store.commit("both").unwrap();

    // Drop and reopen so neither namespace is in the in-memory cache
    // initially. gc_blobs MUST load all namespaces from the registry,
    // not just operate on the in-memory ones, or it'd delete `hb`.
    drop(store);
    let mut store = FileNamespacedKvStore::<N>::open(&dataset).unwrap();

    let report = store.gc_blobs().unwrap();
    assert_eq!(report.total, 2);
    assert_eq!(
        report.referenced, 2,
        "gc must load all namespaces from registry"
    );
    assert_eq!(report.removed, 0);

    assert!(store.inner_storage().get_blob(&ha).is_some());
    assert!(store.inner_storage().get_blob(&hb).is_some());
}

#[test]
fn gc_blobs_idempotent() {
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));

    let v1 = vec![0xAA; 200];
    let v2 = vec![0xBB; 300];
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), v1).unwrap();
    }
    store.commit("v1").unwrap();
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"k".to_vec(), v2).unwrap();
    }
    store.commit("v2").unwrap();

    let orphan = vec![0xCC; 350];
    let orphan_hash = insert_orphan_blob(&store, &orphan);
    assert!(store.inner_storage().get_blob(&orphan_hash).is_some());

    let first = store.gc_blobs().unwrap();
    assert_eq!(first.removed, 1);

    // Second GC has nothing to do.
    let second = store.gc_blobs().unwrap();
    assert_eq!(second.total, 2);
    assert_eq!(second.referenced, 2);
    assert_eq!(second.removed, 0);
}

#[test]
fn same_large_value_under_two_keys_writes_blob_once() {
    // Content-addressed storage means two keys pointing at identical payload
    // share one blob. We verify by inserting twice and confirming get_blob
    // still returns the expected payload (idempotent dedup at insert_blob
    // is the real driver — both leaves contain identical envelopes).
    let (_temp, dataset) = setup_repo_and_dataset();
    let mut store = FileNamespacedKvStore::<N>::init(&dataset).unwrap();
    store.set_externalize_threshold(Some(64));

    let payload: Vec<u8> = (0..500).map(|i| i as u8).collect();
    let hash = ValueDigest::<N>::new(&payload);
    {
        let mut personal = store.namespace("personal");
        personal.insert(b"a".to_vec(), payload.clone()).unwrap();
        personal.insert(b"b".to_vec(), payload.clone()).unwrap();
    }
    store.commit("dedup").unwrap();

    let blob = store.inner_storage().get_blob(&hash).expect("blob exists");
    assert_eq!(blob, payload);

    let personal = store.namespace("personal");
    assert_eq!(personal.get(b"a"), Some(payload.clone()));
    assert_eq!(personal.get(b"b"), Some(payload));
}
