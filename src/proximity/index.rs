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

//! Proximity (vector) index over the [`NodeStorage`] trait.
//!
//! # Architecture
//!
//! The index keeps the `(id, vector)` set as the **source of truth** in a
//! `BTreeMap` (sorted by id) and derives the content-addressed proximity tree
//! from that set on demand. Inserts and removes are O(log N) updates to the
//! map that mark the cached tree as dirty; the tree is rebuilt lazily on the
//! next call to [`ProximityIndex::knn`] or [`ProximityIndex::root_hash`].
//!
//! Each [`ProximityNode`] is serialised via [`crate::proximity::storage`] into
//! the existing [`NodeStorage<N>`] trait so every backend (in-memory, file,
//! git, rocksdb) absorbs the new node type with **zero trait changes**.
//!
//! Building from scratch on every flush is what guarantees **history
//! independence**: tree shape and root hash depend only on current data,
//! never on insertion order or update history.
//!
//! # Persistence
//!
//! [`ProximityIndex::persist`] saves the config + entries + root hash via
//! [`NodeStorage::save_config`] under a per-index key. [`ProximityIndex::load`]
//! restores them. The proximity tree itself lives in storage as content-
//! addressed `ProximityNode` blobs from the moment they are written; persist
//! only needs to write a small bookkeeping blob.
//!
//! `flush()` does **not** delete the previous root or its descendants. With a
//! shared backing store, that would corrupt other indexes that happen to
//! reference the same content-addressed subtrees. Old nodes become unreferenced
//! and are reclaimed by the backend's own GC (e.g. `git gc`).

use crate::digest::ValueDigest;
use crate::proximity::distance::{Distance, Metric};
use crate::proximity::level::vector_level;
use crate::proximity::node::ProximityNode;
use crate::proximity::storage::{unwrap_proximity_node, wrap_proximity_node};
use crate::storage::{InMemoryNodeStorage, NodeStorage, StorageError};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use thiserror::Error;

/// Tuning for a proximity index. Serialised alongside the index so future
/// processes that reopen the same index get the same `level_bits`, `metric`,
/// and dimensionality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProximityConfig {
    /// Vector dimensionality. Every inserted vector and every query must
    /// have exactly this length.
    pub dim: u16,
    /// Distance metric.
    pub metric: Metric,
    /// Bits of hashed input required to promote a vector one level. With
    /// `level_bits = 4` (the default) the expected per-level fanout is 16.
    pub level_bits: u8,
    /// Advisory cap on leaf size. Reserved for future PRs.
    pub max_bucket_size: u16,
}

impl Default for ProximityConfig {
    fn default() -> Self {
        Self {
            dim: 0,
            metric: Metric::Cosine,
            level_bits: 4,
            max_bucket_size: 64,
        }
    }
}

/// Errors from proximity-index operations.
#[derive(Debug, Error)]
pub enum ProximityError {
    /// Inserted or queried vector has the wrong dimensionality.
    #[error("dimension mismatch: index expects {expected}, got {got}")]
    DimensionMismatch { expected: u16, got: u16 },
    /// `dim` was zero in [`ProximityConfig`] — must be set before insert.
    #[error("ProximityConfig.dim must be > 0")]
    ZeroDim,
    /// Stored node could not be parsed as a [`ProximityNode`].
    #[error("corrupted proximity node: {0}")]
    Corrupted(String),
    /// Bincode serialisation failed.
    #[error("serialize error: {0}")]
    Serialize(String),
    /// Backing storage returned an error.
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    /// A referenced node hash was not present in storage.
    #[error("internal: node not found in storage (hash {0})")]
    MissingNode(String),
    /// `load` was called with a name that has never been persisted.
    #[error("no proximity index persisted under name {0:?}")]
    NotFound(String),
    /// `load` succeeded but the saved config blob could not be decoded.
    #[error("could not decode saved index state: {0}")]
    InvalidSavedState(String),
}

/// A version-controlled vector index backed by any [`NodeStorage<N>`].
#[derive(Debug)]
pub struct ProximityIndex<const N: usize, S: NodeStorage<N>> {
    entries: BTreeMap<Vec<u8>, Vec<f32>>,
    storage: S,
    root: Option<ValueDigest<N>>,
    dirty: bool,
    config: ProximityConfig,
}

/// Bookkeeping blob written by [`ProximityIndex::persist`] under
/// `proximity:<name>:state`. The proximity-node tree itself is referenced by
/// `root` and lives in [`NodeStorage`] alongside any other data.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedProximityState<const N: usize> {
    config: ProximityConfig,
    entries: BTreeMap<Vec<u8>, Vec<f32>>,
    root: Option<ValueDigest<N>>,
}

/// Registry entry for a proximity sub-index inside a namespace registry.
///
/// Holds just what's needed for the namespace registry: the persisted root
/// hash + config. The entries `BTreeMap` (the canonical source of truth) is
/// stored separately via [`NodeStorage::save_config`] under the same per-index
/// key as standalone [`ProximityIndex::persist`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProximityIndexEntry<const N: usize> {
    pub root_hash: Option<ValueDigest<N>>,
    pub config: ProximityConfig,
}

/// Deserialised view of a persisted `SavedProximityState` blob.
///
/// PR 3c uses this to read proximity-index state from arbitrary git commits
/// (via [`crate::git::metadata::MetadataBackend::read_file_at_commit`])
/// during namespaced merge — without instantiating a full
/// [`ProximityIndex`] for the historical state.
#[derive(Debug, Clone)]
pub struct PersistedProximityState<const N: usize> {
    pub config: ProximityConfig,
    pub entries: BTreeMap<Vec<u8>, Vec<f32>>,
    pub root_hash: Option<ValueDigest<N>>,
}

/// Deserialise a `SavedProximityState` bincode blob into a typed view.
///
/// Used by the namespaced merge integration in PR 3c.
pub fn deserialize_persisted_state<const N: usize>(
    bytes: &[u8],
) -> Result<PersistedProximityState<N>, ProximityError> {
    let state: SavedProximityState<N> = bincode::deserialize(bytes)
        .map_err(|e| ProximityError::InvalidSavedState(e.to_string()))?;
    Ok(PersistedProximityState {
        config: state.config,
        entries: state.entries,
        root_hash: state.root,
    })
}

impl<const N: usize, S: NodeStorage<N>> ProximityIndex<N, S> {
    /// Build a new, empty index backed by `storage`.
    pub fn new(storage: S, config: ProximityConfig) -> Self {
        Self {
            entries: BTreeMap::new(),
            storage,
            root: None,
            dirty: false,
            config,
        }
    }

    /// Read-only access to the configuration.
    pub fn config(&self) -> &ProximityConfig {
        &self.config
    }

    /// Borrow the backing storage. Useful when sharing one backend between
    /// multiple indexes.
    pub fn storage(&self) -> &S {
        &self.storage
    }

    /// Number of distinct ids currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no entries are stored.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total node count of the materialised proximity tree (walks the tree
    /// from `root`). Triggers a build if the tree is currently dirty.
    pub fn node_count(&mut self) -> Result<usize, ProximityError> {
        self.flush()?;
        let Some(root) = self.root.clone() else {
            return Ok(0);
        };
        let mut count = 0usize;
        let mut stack = vec![root];
        while let Some(h) = stack.pop() {
            count += 1;
            let node = self.load_node(&h)?;
            if !node.is_leaf() {
                stack.extend(node.child_hashes.iter().cloned());
            }
        }
        Ok(count)
    }

    /// Root hash of the materialised proximity tree. Triggers a build if dirty.
    pub fn root_hash(&mut self) -> Result<Option<&ValueDigest<N>>, ProximityError> {
        self.flush()?;
        Ok(self.root.as_ref())
    }

    /// Insert or update `(id, vector)`. Same id called twice replaces the
    /// stored vector. Tree build is deferred until the next query.
    pub fn insert(&mut self, id: Vec<u8>, vector: Vec<f32>) -> Result<(), ProximityError> {
        self.validate_dim_for_vector(&vector)?;
        self.entries.insert(id, vector);
        self.dirty = true;
        Ok(())
    }

    /// Remove an id from the index. Returns `true` when an entry was removed.
    pub fn remove(&mut self, id: &[u8]) -> bool {
        let removed = self.entries.remove(id).is_some();
        if removed {
            self.dirty = true;
        }
        removed
    }

    /// k-nearest-neighbour search.
    ///
    /// * `query` — query vector (must match index dimensionality).
    /// * `k` — number of results.
    /// * `ef` — beam width carried at each internal level. Larger trades
    ///   latency for recall. `max(k * 4, 32)` is a reasonable starting point.
    pub fn knn(
        &mut self,
        query: &[f32],
        k: usize,
        ef: usize,
    ) -> Result<Vec<(Vec<u8>, f32)>, ProximityError> {
        self.validate_dim_for_vector(query)?;
        self.flush()?;

        let Some(root_hash) = self.root.as_ref().cloned() else {
            return Ok(Vec::new());
        };

        let metric = self.config.metric;
        let ef = ef.max(k.max(1));
        let mut beam: Vec<(f32, ValueDigest<N>)> = vec![(0.0, root_hash)];

        loop {
            let head_is_leaf = self.load_node(&beam[0].1)?.is_leaf();
            if head_is_leaf {
                break;
            }
            let mut next: Vec<(f32, ValueDigest<N>)> = Vec::new();
            for (_, h) in beam.iter() {
                let n = self.load_node(h)?;
                debug_assert!(!n.is_leaf());
                for i in 0..n.child_hashes.len() {
                    let d = metric.distance(query, &n.vectors[i]);
                    next.push((d, n.child_hashes[i].clone()));
                }
            }
            dedup_keep_min(&mut next);
            next.sort_by(|a, b| f32_total_cmp(a.0, b.0));
            next.truncate(ef);
            beam = next;
        }

        let mut seen: HashSet<Vec<u8>> = HashSet::new();
        let mut results: Vec<(f32, Vec<u8>)> = Vec::new();
        for (_, h) in beam.iter() {
            let leaf = self.load_node(h)?;
            debug_assert!(leaf.is_leaf());
            for (id, v) in leaf.ids.iter().zip(leaf.vectors.iter()) {
                if seen.insert(id.clone()) {
                    let d = metric.distance(query, v);
                    results.push((d, id.clone()));
                }
            }
        }
        results.sort_by(|a, b| f32_total_cmp(a.0, b.0));
        results.truncate(k);
        Ok(results.into_iter().map(|(d, id)| (id, d)).collect())
    }

    /// Persist the index under `name` via [`NodeStorage::save_config`]. The
    /// proximity-node tree itself is already in [`NodeStorage`] from when
    /// `flush()` wrote it. After this call the index can be re-opened via
    /// [`ProximityIndex::load`] with the same `name`.
    ///
    /// Calls [`NodeStorage::sync`] at the end so backends with deferred
    /// bookkeeping (e.g. `GitNodeStorage`'s hash-mapping snapshot) write
    /// their state to disk in time for a fresh handle to read it.
    pub fn persist(&mut self, name: &str) -> Result<Option<ValueDigest<N>>, ProximityError> {
        self.flush()?;
        let state = SavedProximityState {
            config: self.config.clone(),
            entries: self.entries.clone(),
            root: self.root.clone(),
        };
        let bytes =
            bincode::serialize(&state).map_err(|e| ProximityError::Serialize(e.to_string()))?;
        self.storage.save_config(&Self::state_key(name), &bytes);
        self.storage.sync()?;
        Ok(self.root.clone())
    }

    /// Load a previously persisted index. Returns [`ProximityError::NotFound`]
    /// if no index has been persisted under `name`.
    pub fn load(storage: S, name: &str) -> Result<Self, ProximityError> {
        let bytes = storage
            .get_config(&Self::state_key(name))
            .ok_or_else(|| ProximityError::NotFound(name.to_string()))?;
        let state: SavedProximityState<N> = bincode::deserialize(&bytes)
            .map_err(|e| ProximityError::InvalidSavedState(e.to_string()))?;
        Ok(Self {
            entries: state.entries,
            storage,
            root: state.root,
            dirty: false,
            config: state.config,
        })
    }

    /// Snapshot of the current `(id, vector)` entries. The returned `BTreeMap`
    /// is a deep clone of the canonical source-of-truth set, useful for
    /// passing into [`crate::proximity::merge_proximity_index_sets`].
    pub fn entries_snapshot(&self) -> BTreeMap<Vec<u8>, Vec<f32>> {
        self.entries.clone()
    }

    /// Replace the entry set wholesale and mark the materialised tree dirty.
    ///
    /// Used by the namespaced merge logic: after three-way merging the entry
    /// sets across base/source/dest, the result is installed back via this
    /// method, then `commit_impl` flushes it through `persist()`.
    ///
    /// Returns `Err(DimensionMismatch)` if any vector's length differs from
    /// `self.config.dim`.
    pub fn replace_entries(
        &mut self,
        entries: BTreeMap<Vec<u8>, Vec<f32>>,
    ) -> Result<(), ProximityError> {
        for v in entries.values() {
            if v.len() as u16 != self.config.dim {
                return Err(ProximityError::DimensionMismatch {
                    expected: self.config.dim,
                    got: v.len() as u16,
                });
            }
        }
        self.entries = entries;
        self.dirty = true;
        Ok(())
    }

    /// Force a rebuild of the materialised tree if it is dirty. Idempotent
    /// when the tree is already current.
    pub fn flush(&mut self) -> Result<(), ProximityError> {
        if !self.dirty {
            return Ok(());
        }
        if self.config.dim == 0 {
            return Err(ProximityError::ZeroDim);
        }

        if self.entries.is_empty() {
            self.root = None;
            self.dirty = false;
            return Ok(());
        }

        let entries: Vec<Entry> = self
            .entries
            .iter()
            .map(|(id, v)| Entry {
                id: id.clone(),
                vector: v.clone(),
                level: vector_level(id, v, self.config.level_bits),
            })
            .collect();

        let top_level = entries.iter().map(|e| e.level).max().unwrap();
        let root_hash = self.build_subtree(&entries, top_level)?;
        self.root = Some(root_hash);
        self.dirty = false;
        Ok(())
    }

    fn build_subtree(
        &mut self,
        entries: &[Entry],
        level: u8,
    ) -> Result<ValueDigest<N>, ProximityError> {
        let dim = self.config.dim;
        let metric_tag = self.config.metric.metric_tag();

        if level == 0 {
            return self.build_leaf(entries);
        }

        let boundary_indices: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.level >= level)
            .map(|(i, _)| i)
            .collect();
        debug_assert!(
            !boundary_indices.is_empty(),
            "build_subtree at level {level} found no boundaries"
        );

        let boundaries: Vec<(Vec<u8>, Vec<f32>)> = boundary_indices
            .iter()
            .map(|&i| (entries[i].id.clone(), entries[i].vector.clone()))
            .collect();

        let metric = self.config.metric;
        let mut buckets: Vec<Vec<Entry>> = vec![Vec::new(); boundaries.len()];
        for entry in entries {
            let bi = nearest_boundary(&boundaries, &entry.vector, &metric);
            buckets[bi].push(entry.clone());
        }

        let mut child_hashes = Vec::with_capacity(boundaries.len());
        for bucket in &buckets {
            let h = self.build_subtree(bucket, level - 1)?;
            child_hashes.push(h);
        }

        let ids: Vec<Vec<u8>> = boundaries.iter().map(|(id, _)| id.clone()).collect();
        let vecs: Vec<Vec<f32>> = boundaries.iter().map(|(_, v)| v.clone()).collect();
        let node = ProximityNode::new(level, ids, vecs, child_hashes, dim, metric_tag);
        self.persist_node(node)
    }

    fn build_leaf(&mut self, entries: &[Entry]) -> Result<ValueDigest<N>, ProximityError> {
        let ids: Vec<Vec<u8>> = entries.iter().map(|e| e.id.clone()).collect();
        let vecs: Vec<Vec<f32>> = entries.iter().map(|e| e.vector.clone()).collect();
        let node = ProximityNode::new(
            0,
            ids,
            vecs,
            vec![],
            self.config.dim,
            self.config.metric.metric_tag(),
        );
        self.persist_node(node)
    }

    fn persist_node(&mut self, node: ProximityNode<N>) -> Result<ValueDigest<N>, ProximityError> {
        let hash = node.get_hash();
        let wrapper = wrap_proximity_node(&node)?;
        self.storage.insert_node(hash.clone(), wrapper)?;
        Ok(hash)
    }

    fn load_node(&self, hash: &ValueDigest<N>) -> Result<ProximityNode<N>, ProximityError> {
        let wrapper = self
            .storage
            .get_node_by_hash(hash)
            .ok_or_else(|| ProximityError::MissingNode(format!("{hash}")))?;
        unwrap_proximity_node(&wrapper)
    }

    fn validate_dim_for_vector(&self, vector: &[f32]) -> Result<(), ProximityError> {
        if self.config.dim == 0 {
            return Err(ProximityError::ZeroDim);
        }
        if vector.len() as u16 != self.config.dim {
            return Err(ProximityError::DimensionMismatch {
                expected: self.config.dim,
                got: vector.len() as u16,
            });
        }
        Ok(())
    }

    fn state_key(name: &str) -> String {
        format!("proximity:{name}:state")
    }
}

/// In-memory convenience: build an index backed by [`InMemoryNodeStorage`].
impl<const N: usize> ProximityIndex<N, InMemoryNodeStorage<N>> {
    pub fn new_in_memory(config: ProximityConfig) -> Self {
        Self::new(InMemoryNodeStorage::new(), config)
    }
}

#[derive(Clone)]
struct Entry {
    id: Vec<u8>,
    vector: Vec<f32>,
    level: u8,
}

fn nearest_boundary<D: Distance>(
    boundaries: &[(Vec<u8>, Vec<f32>)],
    query: &[f32],
    metric: &D,
) -> usize {
    debug_assert!(!boundaries.is_empty());
    let mut best_bi = 0usize;
    let mut best_d = metric.distance(query, &boundaries[0].1);
    for bi in 1..boundaries.len() {
        let d = metric.distance(query, &boundaries[bi].1);
        let cmp = match f32_total_cmp(d, best_d) {
            Ordering::Equal => boundaries[bi].0.cmp(&boundaries[best_bi].0),
            o => o,
        };
        if cmp == Ordering::Less {
            best_bi = bi;
            best_d = d;
        }
    }
    best_bi
}

fn f32_total_cmp(a: f32, b: f32) -> Ordering {
    a.partial_cmp(&b).unwrap_or_else(|| {
        let an = a.is_nan();
        let bn = b.is_nan();
        if an && bn {
            Ordering::Equal
        } else if an {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    })
}

fn dedup_keep_min<const N: usize>(pairs: &mut Vec<(f32, ValueDigest<N>)>) {
    let mut best: HashMap<ValueDigest<N>, f32> = HashMap::with_capacity(pairs.len());
    for (d, h) in pairs.drain(..) {
        best.entry(h)
            .and_modify(|cur| {
                if f32_total_cmp(d, *cur) == Ordering::Less {
                    *cur = d;
                }
            })
            .or_insert(d);
    }
    pairs.extend(best.into_iter().map(|(h, d)| (d, h)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

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

    fn brute_force_topk(
        data: &[(Vec<u8>, Vec<f32>)],
        query: &[f32],
        k: usize,
        metric: Metric,
    ) -> Vec<Vec<u8>> {
        let mut scored: Vec<(f32, Vec<u8>)> = data
            .iter()
            .map(|(id, v)| (metric.distance(query, v), id.clone()))
            .collect();
        scored.sort_by(|a, b| f32_total_cmp(a.0, b.0));
        scored.into_iter().take(k).map(|(_, id)| id).collect()
    }

    fn config(dim: u16, metric: Metric) -> ProximityConfig {
        ProximityConfig {
            dim,
            metric,
            level_bits: 4,
            max_bucket_size: 64,
        }
    }

    // Test 1 — recall vs brute force (L2).
    #[test]
    fn recall_vs_bruteforce_l2() {
        let dim = 16;
        let n = 1_000;
        let k = 10;
        let ef = 64;
        let data = random_vectors(n, dim, 0x1234_5678);

        let mut idx = ProximityIndex::<32, _>::new_in_memory(config(dim as u16, Metric::L2));
        for (id, v) in &data {
            idx.insert(id.clone(), v.clone()).unwrap();
        }

        let queries = random_vectors(20, dim, 0x9999_aaaa);
        let mut total_recall = 0.0f32;
        for (_, q) in &queries {
            let truth: HashSet<Vec<u8>> = brute_force_topk(&data, q, k, Metric::L2)
                .into_iter()
                .collect();
            let got: HashSet<Vec<u8>> = idx
                .knn(q, k, ef)
                .unwrap()
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            let hit = truth.intersection(&got).count() as f32;
            total_recall += hit / (k as f32);
        }
        let avg = total_recall / (queries.len() as f32);
        assert!(
            avg >= 0.85,
            "L2 average recall@{k} = {avg:.3}, expected >= 0.85"
        );
    }

    #[test]
    fn recall_vs_bruteforce_cosine() {
        let dim = 32;
        let n = 1_000;
        let k = 10;
        let ef = 64;
        let data = random_vectors(n, dim, 0xc0ffee);

        let mut idx = ProximityIndex::<32, _>::new_in_memory(config(dim as u16, Metric::Cosine));
        for (id, v) in &data {
            idx.insert(id.clone(), v.clone()).unwrap();
        }

        let queries = random_vectors(20, dim, 0xdead_beef);
        let mut total_recall = 0.0f32;
        for (_, q) in &queries {
            let truth: HashSet<Vec<u8>> = brute_force_topk(&data, q, k, Metric::Cosine)
                .into_iter()
                .collect();
            let got: HashSet<Vec<u8>> = idx
                .knn(q, k, ef)
                .unwrap()
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            let hit = truth.intersection(&got).count() as f32;
            total_recall += hit / (k as f32);
        }
        let avg = total_recall / (queries.len() as f32);
        assert!(avg >= 0.85, "cosine avg recall@{k} = {avg:.3}");
    }

    // Test 2 — determinism.
    #[test]
    fn determinism_order_independent() {
        let dim = 8;
        let data = random_vectors(200, dim, 42);

        let mut idx_a = ProximityIndex::<32, _>::new_in_memory(config(dim as u16, Metric::L2));
        for (id, v) in &data {
            idx_a.insert(id.clone(), v.clone()).unwrap();
        }

        let mut data_rev = data.clone();
        data_rev.reverse();
        let mut idx_b = ProximityIndex::<32, _>::new_in_memory(config(dim as u16, Metric::L2));
        for (id, v) in &data_rev {
            idx_b.insert(id.clone(), v.clone()).unwrap();
        }

        let mut data_shuf = data.clone();
        let mut rng = StdRng::seed_from_u64(7);
        for i in (1..data_shuf.len()).rev() {
            let j = rng.random_range(0..=i);
            data_shuf.swap(i, j);
        }
        let mut idx_c = ProximityIndex::<32, _>::new_in_memory(config(dim as u16, Metric::L2));
        for (id, v) in &data_shuf {
            idx_c.insert(id.clone(), v.clone()).unwrap();
        }

        let h_a = idx_a.root_hash().unwrap().cloned();
        let h_b = idx_b.root_hash().unwrap().cloned();
        let h_c = idx_c.root_hash().unwrap().cloned();
        assert_eq!(h_a, h_b);
        assert_eq!(h_a, h_c);
    }

    #[test]
    fn determinism_under_intermediate_updates() {
        let dim = 4;
        let final_data = random_vectors(50, dim, 5);

        let mut idx_a = ProximityIndex::<32, _>::new_in_memory(config(dim as u16, Metric::L2));
        for (id, v) in &final_data {
            idx_a.insert(id.clone(), v.clone()).unwrap();
        }

        let mut idx_b = ProximityIndex::<32, _>::new_in_memory(config(dim as u16, Metric::L2));
        for (id, _) in &final_data {
            idx_b.insert(id.clone(), vec![999.0; dim]).unwrap();
        }
        for (id, v) in &final_data {
            idx_b.insert(id.clone(), v.clone()).unwrap();
        }

        assert_eq!(
            idx_a.root_hash().unwrap().cloned(),
            idx_b.root_hash().unwrap().cloned()
        );
    }

    #[test]
    fn determinism_under_remove() {
        let dim = 4;
        let data = random_vectors(50, dim, 8);

        let surviving: Vec<_> = data
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 3 != 0)
            .map(|(_, e)| e.clone())
            .collect();
        let mut idx_a = ProximityIndex::<32, _>::new_in_memory(config(dim as u16, Metric::L2));
        for (id, v) in &surviving {
            idx_a.insert(id.clone(), v.clone()).unwrap();
        }

        let mut idx_b = ProximityIndex::<32, _>::new_in_memory(config(dim as u16, Metric::L2));
        for (id, v) in &data {
            idx_b.insert(id.clone(), v.clone()).unwrap();
        }
        for (i, (id, _)) in data.iter().enumerate() {
            if i % 3 == 0 {
                assert!(idx_b.remove(id));
            }
        }

        assert_eq!(
            idx_a.root_hash().unwrap().cloned(),
            idx_b.root_hash().unwrap().cloned()
        );
    }

    #[test]
    fn metric_variants_disagree_on_some_query() {
        let dim = 4;
        let data = random_vectors(200, dim, 13);

        let mut idx_l2 = ProximityIndex::<32, _>::new_in_memory(config(dim as u16, Metric::L2));
        let mut idx_cos =
            ProximityIndex::<32, _>::new_in_memory(config(dim as u16, Metric::Cosine));
        for (id, v) in &data {
            idx_l2.insert(id.clone(), v.clone()).unwrap();
            idx_cos.insert(id.clone(), v.clone()).unwrap();
        }

        let queries = random_vectors(50, dim, 99);
        let mut disagreement = 0usize;
        for (_, q) in &queries {
            let top_l2 = idx_l2.knn(q, 5, 64).unwrap();
            let top_cos = idx_cos.knn(q, 5, 64).unwrap();
            if top_l2 != top_cos {
                disagreement += 1;
            }
        }
        assert!(disagreement > 0);
    }

    // Boundary conditions.

    #[test]
    fn empty_index_knn_returns_empty() {
        let mut idx = ProximityIndex::<32, _>::new_in_memory(config(4, Metric::L2));
        assert!(idx.knn(&[0.0; 4], 5, 32).unwrap().is_empty());
    }

    #[test]
    fn empty_index_root_is_none() {
        let mut idx = ProximityIndex::<32, _>::new_in_memory(config(4, Metric::L2));
        assert!(idx.root_hash().unwrap().is_none());
    }

    #[test]
    fn dim_zero_rejected() {
        let mut idx = ProximityIndex::<32, _>::new_in_memory(ProximityConfig::default());
        assert!(matches!(
            idx.insert(b"x".to_vec(), vec![1.0]),
            Err(ProximityError::ZeroDim)
        ));
    }

    #[test]
    fn dim_mismatch_on_insert() {
        let mut idx = ProximityIndex::<32, _>::new_in_memory(config(4, Metric::L2));
        let err = idx.insert(b"x".to_vec(), vec![1.0, 2.0]).unwrap_err();
        assert!(matches!(
            err,
            ProximityError::DimensionMismatch {
                expected: 4,
                got: 2
            }
        ));
    }

    #[test]
    fn dim_mismatch_on_query() {
        let mut idx = ProximityIndex::<32, _>::new_in_memory(config(4, Metric::L2));
        idx.insert(b"x".to_vec(), vec![1.0; 4]).unwrap();
        let err = idx.knn(&[0.0, 0.0], 5, 32).unwrap_err();
        assert!(matches!(err, ProximityError::DimensionMismatch { .. }));
    }

    #[test]
    fn upsert_replaces_vector() {
        let mut idx = ProximityIndex::<32, _>::new_in_memory(config(2, Metric::L2));
        idx.insert(b"k".to_vec(), vec![1.0, 0.0]).unwrap();
        idx.insert(b"k".to_vec(), vec![0.0, 1.0]).unwrap();
        let hits = idx.knn(&[0.0, 1.0], 1, 8).unwrap();
        assert_eq!(hits[0].0, b"k".to_vec());
        assert!(hits[0].1 < 1e-5);
    }

    #[test]
    fn single_entry_roundtrip() {
        let mut idx = ProximityIndex::<32, _>::new_in_memory(config(3, Metric::L2));
        idx.insert(b"only".to_vec(), vec![1.0, 2.0, 3.0]).unwrap();
        let hits = idx.knn(&[1.0, 2.0, 3.0], 5, 8).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, b"only".to_vec());
    }

    #[test]
    fn remove_missing_returns_false() {
        let mut idx = ProximityIndex::<32, _>::new_in_memory(config(2, Metric::L2));
        assert!(!idx.remove(b"nope"));
    }

    #[test]
    fn remove_existing_returns_true() {
        let mut idx = ProximityIndex::<32, _>::new_in_memory(config(2, Metric::L2));
        idx.insert(b"k".to_vec(), vec![1.0, 0.0]).unwrap();
        assert!(idx.remove(b"k"));
        assert!(idx.knn(&[1.0, 0.0], 1, 8).unwrap().is_empty());
    }

    // Persistence tests against InMemoryNodeStorage (other backends covered
    // by tests/proximity_persistence.rs).

    #[test]
    fn persist_load_roundtrip_in_memory() {
        let dim = 8u16;
        let data = random_vectors(100, dim as usize, 0xabcd);

        let storage = InMemoryNodeStorage::<32>::new();
        let mut idx = ProximityIndex::new(storage.clone(), config(dim, Metric::L2));
        for (id, v) in &data {
            idx.insert(id.clone(), v.clone()).unwrap();
        }
        let original_root = idx.persist("docs").unwrap();
        // Storage handle has been mutated in place by inserts — reuse the
        // mutated copy by cloning the index's `storage` field.
        let storage_after = idx.storage().clone();

        let mut reopened = ProximityIndex::<32, _>::load(storage_after, "docs").unwrap();
        assert_eq!(reopened.root_hash().unwrap().cloned(), original_root);
        assert_eq!(reopened.len(), data.len());

        // Query results should be identical between original and reopened.
        let q = vec![0.1f32; dim as usize];
        let from_original = idx.knn(&q, 5, 32).unwrap();
        let from_reopened = reopened.knn(&q, 5, 32).unwrap();
        assert_eq!(from_original, from_reopened);
    }

    #[test]
    fn load_unknown_name_returns_not_found() {
        let storage = InMemoryNodeStorage::<32>::new();
        let err = ProximityIndex::<32, _>::load(storage, "missing").unwrap_err();
        assert!(matches!(err, ProximityError::NotFound(name) if name == "missing"));
    }

    #[test]
    fn persist_empty_index_then_load() {
        let storage = InMemoryNodeStorage::<32>::new();
        let mut idx = ProximityIndex::new(storage, config(4, Metric::L2));
        idx.persist("empty").unwrap();
        let storage_after = idx.storage().clone();
        let mut reopened = ProximityIndex::<32, _>::load(storage_after, "empty").unwrap();
        assert!(reopened.is_empty());
        assert!(reopened.root_hash().unwrap().is_none());
    }

    #[test]
    fn node_count_walks_tree() {
        let dim = 4u16;
        let data = random_vectors(100, dim as usize, 0xfeed);
        let mut idx = ProximityIndex::<32, _>::new_in_memory(config(dim, Metric::L2));
        for (id, v) in &data {
            idx.insert(id.clone(), v.clone()).unwrap();
        }
        let count = idx.node_count().unwrap();
        // For 100 entries with level_bits=4 we expect a handful of levels.
        // Just sanity-check that there's at least one node.
        assert!(count > 0);
    }
}
