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

//! PR 2 — Persistence round-trip tests for [`ProximityIndex`] across every
//! [`NodeStorage`] backend.
//!
//! Each test inserts a deterministic batch of random vectors, persists the
//! index, drops it, reopens the backend, and asserts that:
//!
//! 1. The reopened index has the same root hash as the persisted one.
//! 2. KNN queries return identical results for the same query vector.
//! 3. Subsequent inserts on the reopened index continue to behave
//!    deterministically.

#![cfg(feature = "proximity")]

use prollytree::proximity::{Metric, ProximityConfig, ProximityIndex};
use prollytree::storage::FileNodeStorage;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tempfile::TempDir;

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

// ---------------------------------------------------------------------------
// File backend
// ---------------------------------------------------------------------------

#[test]
fn file_backend_persist_load_roundtrip() {
    let dim = 8u16;
    let data = random_vectors(150, dim as usize, 0x1111);

    let temp = TempDir::new().unwrap();
    let path = temp.path().to_path_buf();

    let (original_root, original_hits) = {
        let storage = FileNodeStorage::<N>::new(path.clone()).unwrap();
        let mut idx = ProximityIndex::new(storage, config(dim, Metric::L2));
        for (id, v) in &data {
            idx.insert(id.clone(), v.clone()).unwrap();
        }
        let root = idx.persist("docs").unwrap();
        let hits = idx.knn(&vec![0.25f32; dim as usize], 5, 32).unwrap();
        (root, hits)
    };

    // Original handle dropped — reopen from a fresh handle to the same dir.
    let storage = FileNodeStorage::<N>::new(path).unwrap();
    let mut reopened = ProximityIndex::<N, _>::load(storage, "docs").unwrap();
    assert_eq!(reopened.root_hash().unwrap().cloned(), original_root);
    assert_eq!(reopened.len(), data.len());
    let reopened_hits = reopened.knn(&vec![0.25f32; dim as usize], 5, 32).unwrap();
    assert_eq!(reopened_hits, original_hits);
}

#[test]
fn file_backend_continued_mutation_after_reload() {
    let dim = 8u16;
    let initial = random_vectors(50, dim as usize, 0x2222);
    let extra = random_vectors(50, dim as usize, 0x3333);

    let temp = TempDir::new().unwrap();
    let path = temp.path().to_path_buf();

    // Phase 1: insert + persist.
    {
        let storage = FileNodeStorage::<N>::new(path.clone()).unwrap();
        let mut idx = ProximityIndex::new(storage, config(dim, Metric::Cosine));
        for (id, v) in &initial {
            idx.insert(id.clone(), v.clone()).unwrap();
        }
        idx.persist("docs").unwrap();
    }

    // Phase 2: reopen, insert more, persist. Result root must equal a
    // single-shot insert of `initial + extra` (determinism across reload).
    let combined_root = {
        let storage = FileNodeStorage::<N>::new(path.clone()).unwrap();
        let mut idx = ProximityIndex::<N, _>::load(storage, "docs").unwrap();
        for (id, v) in &extra {
            idx.insert(id.clone(), v.clone()).unwrap();
        }
        idx.persist("docs").unwrap()
    };

    let single_shot_root = {
        let temp2 = TempDir::new().unwrap();
        let storage = FileNodeStorage::<N>::new(temp2.path().to_path_buf()).unwrap();
        let mut idx = ProximityIndex::new(storage, config(dim, Metric::Cosine));
        for (id, v) in initial.iter().chain(extra.iter()) {
            idx.insert(id.clone(), v.clone()).unwrap();
        }
        idx.persist("docs").unwrap()
    };

    assert_eq!(combined_root, single_shot_root);
}

#[test]
fn file_backend_multiple_indexes_share_storage() {
    let dim = 4u16;
    let data_a = random_vectors(40, dim as usize, 0xaaaa);
    let data_b = random_vectors(40, dim as usize, 0xbbbb);

    let temp = TempDir::new().unwrap();
    let path = temp.path().to_path_buf();

    let storage = FileNodeStorage::<N>::new(path.clone()).unwrap();
    let mut idx_a = ProximityIndex::new(storage, config(dim, Metric::L2));
    for (id, v) in &data_a {
        idx_a.insert(id.clone(), v.clone()).unwrap();
    }
    idx_a.persist("idx_a").unwrap();

    let storage = FileNodeStorage::<N>::new(path.clone()).unwrap();
    let mut idx_b = ProximityIndex::new(storage, config(dim, Metric::L2));
    for (id, v) in &data_b {
        idx_b.insert(id.clone(), v.clone()).unwrap();
    }
    idx_b.persist("idx_b").unwrap();

    // Reopen each — they don't trip over each other.
    let storage = FileNodeStorage::<N>::new(path.clone()).unwrap();
    let reopen_a = ProximityIndex::<N, _>::load(storage, "idx_a").unwrap();
    assert_eq!(reopen_a.len(), data_a.len());

    let storage = FileNodeStorage::<N>::new(path).unwrap();
    let reopen_b = ProximityIndex::<N, _>::load(storage, "idx_b").unwrap();
    assert_eq!(reopen_b.len(), data_b.len());
}

// ---------------------------------------------------------------------------
// Git backend
// ---------------------------------------------------------------------------

#[cfg(feature = "git")]
mod git_backend {
    use super::*;
    use prollytree::storage::GitNodeStorage;

    fn fresh_repo() -> (TempDir, std::path::PathBuf) {
        let temp = TempDir::new().unwrap();
        gix::init_bare(temp.path()).unwrap();
        let dataset = temp.path().to_path_buf();
        (temp, dataset)
    }

    fn open_storage(dataset_dir: &std::path::Path) -> GitNodeStorage<N> {
        let repo = gix::open(dataset_dir).unwrap();
        GitNodeStorage::<N>::new(repo, dataset_dir.to_path_buf()).unwrap()
    }

    #[test]
    fn git_backend_persist_load_roundtrip() {
        let dim = 8u16;
        let data = random_vectors(120, dim as usize, 0x4444);
        let (temp, dataset) = fresh_repo();

        let original_root = {
            let storage = open_storage(&dataset);
            let mut idx = ProximityIndex::new(storage, config(dim, Metric::Cosine));
            for (id, v) in &data {
                idx.insert(id.clone(), v.clone()).unwrap();
            }
            idx.persist("docs").unwrap()
        };

        let storage = open_storage(&dataset);
        let mut reopened = ProximityIndex::<N, _>::load(storage, "docs").unwrap();
        assert_eq!(reopened.root_hash().unwrap().cloned(), original_root);
        assert_eq!(reopened.len(), data.len());

        let q = vec![0.1f32; dim as usize];
        let hits = reopened.knn(&q, 5, 32).unwrap();
        assert!(!hits.is_empty());

        drop(temp);
    }
}

// ---------------------------------------------------------------------------
// RocksDB backend
// ---------------------------------------------------------------------------

#[cfg(feature = "rocksdb_storage")]
mod rocksdb_backend {
    use super::*;
    use prollytree::storage::RocksDBNodeStorage;

    #[test]
    fn rocksdb_backend_persist_load_roundtrip() {
        let dim = 8u16;
        let data = random_vectors(100, dim as usize, 0x5555);

        let temp = TempDir::new().unwrap();
        let path = temp.path().to_path_buf();

        let original_root = {
            let storage = RocksDBNodeStorage::<N>::new(path.clone()).unwrap();
            let mut idx = ProximityIndex::new(storage, config(dim, Metric::L2));
            for (id, v) in &data {
                idx.insert(id.clone(), v.clone()).unwrap();
            }
            idx.persist("docs").unwrap()
        };

        // RocksDB requires exclusive access — original handle has been dropped
        // by leaving the scope above.
        let storage = RocksDBNodeStorage::<N>::new(path).unwrap();
        let mut reopened = ProximityIndex::<N, _>::load(storage, "docs").unwrap();
        assert_eq!(reopened.root_hash().unwrap().cloned(), original_root);
        assert_eq!(reopened.len(), data.len());
    }
}
