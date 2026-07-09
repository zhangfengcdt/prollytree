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

//! PR 0a — round-trip tests for [`NodeStorage::insert_blob`] /
//! [`get_blob`] / [`delete_blob`] across every backend that implements them.
//!
//! Each backend has the same expected behaviour:
//!
//! 1. `insert_blob` then `get_blob` round-trips bit-identical bytes.
//! 2. `get_blob` for a never-inserted hash returns `None`.
//! 3. `insert_blob` is idempotent — re-inserting the same hash is a no-op.
//! 4. `delete_blob` followed by `get_blob` returns `None`.
//! 5. `delete_blob` of a missing hash is success (idempotent).
//! 6. Blobs and nodes share the hash space but live in separate stores —
//!    inserting both at the same hash leaves both readable.
//! 7. A blob survives a fresh storage handle to the same backing data.

use prollytree::digest::ValueDigest;
use prollytree::node::ProllyNode;
use prollytree::storage::{FileNodeStorage, InMemoryNodeStorage, NodeStorage};
use tempfile::TempDir;

const N: usize = 32;

fn h(s: &[u8]) -> ValueDigest<N> {
    ValueDigest::<N>::new(s)
}

// ---------------------------------------------------------------------------
// File backend
// ---------------------------------------------------------------------------

#[test]
fn file_blob_round_trip() {
    let temp = TempDir::new().unwrap();
    let mut s = FileNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
    let hash = h(b"hello world");
    s.insert_blob(hash.clone(), b"hello world").unwrap();
    assert_eq!(s.get_blob(&hash).as_deref(), Some(b"hello world" as &[u8]));
}

#[test]
fn file_blob_get_missing_returns_none() {
    let temp = TempDir::new().unwrap();
    let s = FileNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
    assert!(s.get_blob(&h(b"never_written")).is_none());
}

#[test]
fn file_blob_insert_idempotent() {
    let temp = TempDir::new().unwrap();
    let mut s = FileNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
    let hash = h(b"x");
    s.insert_blob(hash.clone(), b"x").unwrap();
    s.insert_blob(hash.clone(), b"x").unwrap();
    assert_eq!(s.get_blob(&hash).as_deref(), Some(b"x" as &[u8]));
}

#[test]
fn file_blob_delete_then_get_is_none() {
    let temp = TempDir::new().unwrap();
    let mut s = FileNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
    let hash = h(b"to-be-removed");
    s.insert_blob(hash.clone(), b"to-be-removed").unwrap();
    s.delete_blob(&hash).unwrap();
    assert!(s.get_blob(&hash).is_none());
}

#[test]
fn file_blob_delete_missing_is_ok() {
    let temp = TempDir::new().unwrap();
    let mut s = FileNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
    assert!(s.delete_blob(&h(b"phantom")).is_ok());
}

#[test]
fn file_blobs_isolated_from_nodes() {
    let temp = TempDir::new().unwrap();
    let mut s = FileNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
    let hash = h(b"shared");
    s.insert_blob(hash.clone(), b"blob-bytes").unwrap();
    s.insert_node(hash.clone(), ProllyNode::<N>::default())
        .unwrap();
    // Both should be retrievable.
    assert!(s.get_node_by_hash(&hash).is_some());
    assert!(s.get_blob(&hash).is_some());
}

#[test]
fn file_blob_survives_fresh_handle() {
    // File backend persists to disk, so a brand-new storage handle pointing
    // at the same directory must see previously-written blobs.
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_path_buf();
    {
        let mut s = FileNodeStorage::<N>::new(path.clone()).unwrap();
        s.insert_blob(h(b"durable"), b"durable").unwrap();
    }
    let s = FileNodeStorage::<N>::new(path).unwrap();
    assert_eq!(
        s.get_blob(&h(b"durable")).as_deref(),
        Some(b"durable" as &[u8])
    );
}

#[test]
fn file_large_blob_round_trip() {
    // 1 MB payload — larger than any single node bincode blob in tests.
    // Exercises the temp-file + rename path for partial-write safety.
    let temp = TempDir::new().unwrap();
    let mut s = FileNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
    let payload: Vec<u8> = (0..1_048_576).map(|i| (i % 251) as u8).collect();
    let hash = h(&payload);
    s.insert_blob(hash.clone(), &payload).unwrap();
    let got = s.get_blob(&hash).expect("blob should exist");
    assert_eq!(got.len(), payload.len());
    assert_eq!(got, payload);
}

// ---------------------------------------------------------------------------
// In-memory backend
// ---------------------------------------------------------------------------

#[test]
fn memory_blob_insert_existing_mismatch_errors() {
    let mut s = InMemoryNodeStorage::<N>::new();
    let hash = h(b"correct");
    s.insert_blob(hash.clone(), b"correct").unwrap();

    let err = s
        .insert_blob(hash.clone(), b"wrong")
        .expect_err("same hash with different bytes must not report success");
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );
    assert_eq!(s.get_blob(&hash).as_deref(), Some(b"correct" as &[u8]));
}

// ---------------------------------------------------------------------------
// RocksDB backend
// ---------------------------------------------------------------------------

#[cfg(feature = "rocksdb_storage")]
mod rocksdb_backend {
    use super::*;
    use prollytree::storage::RocksDBNodeStorage;

    #[test]
    fn rocksdb_blob_round_trip() {
        let temp = TempDir::new().unwrap();
        let mut s = RocksDBNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
        let hash = h(b"hello rocksdb");
        s.insert_blob(hash.clone(), b"hello rocksdb").unwrap();
        assert_eq!(
            s.get_blob(&hash).as_deref(),
            Some(b"hello rocksdb" as &[u8])
        );
    }

    #[test]
    fn rocksdb_blob_get_missing_returns_none() {
        let temp = TempDir::new().unwrap();
        let s = RocksDBNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
        assert!(s.get_blob(&h(b"never_written")).is_none());
    }

    #[test]
    fn rocksdb_blob_idempotent() {
        let temp = TempDir::new().unwrap();
        let mut s = RocksDBNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
        let hash = h(b"v");
        s.insert_blob(hash.clone(), b"v").unwrap();
        s.insert_blob(hash.clone(), b"v").unwrap();
        assert_eq!(s.get_blob(&hash).as_deref(), Some(b"v" as &[u8]));
    }

    #[test]
    fn rocksdb_blob_insert_existing_mismatch_errors() {
        let temp = TempDir::new().unwrap();
        let mut s = RocksDBNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
        let hash = h(b"correct");
        s.insert_blob(hash.clone(), b"correct").unwrap();

        let err = s
            .insert_blob(hash.clone(), b"wrong")
            .expect_err("same hash with different bytes must not report success");
        assert!(
            err.to_string().contains("already exists"),
            "unexpected error: {err}"
        );
        assert_eq!(s.get_blob(&hash).as_deref(), Some(b"correct" as &[u8]));
    }

    #[test]
    fn rocksdb_blob_delete() {
        let temp = TempDir::new().unwrap();
        let mut s = RocksDBNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
        let hash = h(b"transient");
        s.insert_blob(hash.clone(), b"transient").unwrap();
        s.delete_blob(&hash).unwrap();
        assert!(s.get_blob(&hash).is_none());
        // Deleting again is still ok.
        assert!(s.delete_blob(&hash).is_ok());
    }

    #[test]
    fn rocksdb_blobs_isolated_from_nodes() {
        let temp = TempDir::new().unwrap();
        let mut s = RocksDBNodeStorage::<N>::new(temp.path().to_path_buf()).unwrap();
        let hash = h(b"shared");
        s.insert_blob(hash.clone(), b"blob-bytes").unwrap();
        s.insert_node(hash.clone(), ProllyNode::<N>::default())
            .unwrap();
        assert!(s.get_node_by_hash(&hash).is_some());
        assert!(s.get_blob(&hash).is_some());
    }

    #[test]
    fn rocksdb_blob_survives_reopen() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().to_path_buf();
        {
            let mut s = RocksDBNodeStorage::<N>::new(path.clone()).unwrap();
            s.insert_blob(h(b"durable"), b"durable").unwrap();
        }
        // RocksDB requires exclusive access — original handle is dropped above.
        let s = RocksDBNodeStorage::<N>::new(path).unwrap();
        assert_eq!(
            s.get_blob(&h(b"durable")).as_deref(),
            Some(b"durable" as &[u8])
        );
    }
}
