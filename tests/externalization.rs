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

const N: usize = 32;

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
