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

//! Text-search wrapper over [`crate::proximity::ProximityIndex`] (PR 4a).
//!
//! [`TextIndex<E, S>`] hides the `Vec<f32>` indirection behind an embedder.
//! Callers `insert(id, text)` and `search(query, k)` — the embedder produces
//! vectors internally. The embedder's identity (`id` + `version`) is
//! persisted with the index so re-opening under a different embedder fails
//! with [`TextIndexError::EmbedderMismatch`] rather than silently mixing
//! incompatible vectors.
//!
//! PR 4a ships the standalone API. The namespaced sub-handle
//! (`NamespaceHandle::text_index`) layers on top in the same PR.

use crate::digest::ValueDigest;
use crate::proximity::embedder::{EmbedError, Embedder};
use crate::proximity::index::{ProximityConfig, ProximityError, ProximityIndex};
use crate::proximity::Metric;
use crate::storage::{NodeStorage, StorageError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from [`TextIndex`] operations.
#[derive(Debug, Error)]
pub enum TextIndexError {
    /// The stored embedder identity / version does not match the embedder
    /// supplied on `open`. Re-opening would silently mix incompatible
    /// vectors. Resolve by either (a) opening with the original embedder,
    /// or (b) calling [`TextIndex::reindex`] under the new embedder to
    /// re-embed every document.
    #[error("embedder mismatch: stored {stored_id}@{stored_version}, supplied {provided_id}@{provided_version}")]
    EmbedderMismatch {
        stored_id: String,
        stored_version: String,
        provided_id: String,
        provided_version: String,
    },

    /// The stored embedder's dim doesn't match what the supplied embedder
    /// produces. Separate from EmbedderMismatch so callers can distinguish
    /// "wrong embedder" from "wrong dim setup".
    #[error("dimension mismatch: stored index uses dim {stored}, embedder produces dim {got}")]
    DimensionMismatch { stored: u16, got: u16 },

    /// The underlying [`ProximityIndex`] returned an error.
    #[error("proximity error: {0}")]
    Proximity(#[from] ProximityError),

    /// The embedder failed.
    #[error("embedder error: {0}")]
    Embed(#[from] EmbedError),

    /// `load()` was called with a name that has never been persisted.
    #[error("no text index persisted under name {0:?}")]
    NotFound(String),

    /// Persisted state blob could not be decoded.
    #[error("could not decode saved text index state: {0}")]
    InvalidSavedState(String),

    /// Bincode serialise failed.
    #[error("serialize error: {0}")]
    Serialize(String),

    /// Backing storage returned an error.
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
}

/// Search hit returned from [`TextIndex::search`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextHit {
    pub id: Vec<u8>,
    /// Distance from the query (smaller is better; semantics depend on the
    /// configured [`Metric`]).
    pub score: f32,
}

/// Construction-time configuration for [`TextIndex`].
#[derive(Debug, Clone)]
pub struct TextIndexConfig<E: Embedder> {
    pub embedder: E,
    pub metric: Metric,
    pub level_bits: u8,
    pub max_bucket_size: u16,
}

impl<E: Embedder> TextIndexConfig<E> {
    /// Build with sensible defaults: `Cosine` metric, `level_bits = 4`,
    /// `max_bucket_size = 64`.
    pub fn new(embedder: E) -> Self {
        Self {
            embedder,
            metric: Metric::Cosine,
            level_bits: 4,
            max_bucket_size: 64,
        }
    }
}

/// Bookkeeping blob written under `text:<name>:state`. Exposed at
/// `pub(crate)` so the namespaced text-index handle in
/// `crate::git::versioned_store::namespaced` can share the same
/// identity-validation logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedTextIndexState {
    pub(crate) embedder_id: String,
    pub(crate) embedder_version: String,
    pub(crate) dim: u16,
    pub(crate) metric: Metric,
    pub(crate) level_bits: u8,
    pub(crate) max_bucket_size: u16,
}

/// Storage key under which a text index's identity blob is persisted.
pub(crate) fn text_state_key(name: &str) -> String {
    format!("text:{name}:state")
}

/// Storage / save-config name used for the **inner** proximity index that
/// backs a named text index. Namespaced text indexes reuse this naming so
/// the underlying proximity tree never collides with user-named proximity
/// indexes (assuming users don't pick names starting with `__text__`).
pub(crate) fn text_inner_proximity_name(name: &str) -> String {
    format!("__text__{name}")
}

/// Validate the stored text-index identity blob, or write a fresh one if
/// none exists yet. Returns `Ok(())` when the supplied embedder matches the
/// stored identity (or none was stored yet).
pub(crate) fn validate_or_write_text_identity<const N: usize, E, S>(
    storage: &S,
    state_key: &str,
    embedder: &E,
    metric: Metric,
    level_bits: u8,
    max_bucket_size: u16,
) -> Result<(), TextIndexError>
where
    E: Embedder,
    S: NodeStorage<N>,
{
    if let Some(bytes) = storage.get_config(state_key) {
        let state: SavedTextIndexState = bincode::deserialize(&bytes)
            .map_err(|e| TextIndexError::InvalidSavedState(e.to_string()))?;
        if state.embedder_id != embedder.id() || state.embedder_version != embedder.version() {
            return Err(TextIndexError::EmbedderMismatch {
                stored_id: state.embedder_id,
                stored_version: state.embedder_version,
                provided_id: embedder.id().to_string(),
                provided_version: embedder.version().to_string(),
            });
        }
        if state.dim != embedder.dim() {
            return Err(TextIndexError::DimensionMismatch {
                stored: state.dim,
                got: embedder.dim(),
            });
        }
        Ok(())
    } else {
        let state = SavedTextIndexState {
            embedder_id: embedder.id().to_string(),
            embedder_version: embedder.version().to_string(),
            dim: embedder.dim(),
            metric,
            level_bits,
            max_bucket_size,
        };
        let bytes =
            bincode::serialize(&state).map_err(|e| TextIndexError::Serialize(e.to_string()))?;
        storage.save_config(state_key, &bytes);
        Ok(())
    }
}

/// Text-search index built on top of [`ProximityIndex`].
///
/// `E` is the embedder; `S` is the backing [`NodeStorage`] (same backends as
/// `ProximityIndex` — `InMemory`, `File`, `RocksDB`).
pub struct TextIndex<const N: usize, E: Embedder, S: NodeStorage<N>> {
    inner: ProximityIndex<N, S>,
    embedder: E,
}

impl<const N: usize, E: Embedder, S: NodeStorage<N>> std::fmt::Debug for TextIndex<N, E, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextIndex")
            .field("embedder_id", &self.embedder.id())
            .field("embedder_version", &self.embedder.version())
            .field("dim", &self.embedder.dim())
            .field("len", &self.inner.len())
            .finish()
    }
}

impl<const N: usize, E: Embedder, S: NodeStorage<N>> TextIndex<N, E, S> {
    /// Build a fresh, empty text index.
    ///
    /// The supplied `config.embedder` becomes the index's permanent embedder —
    /// the index can only be re-opened later with an embedder whose `id` and
    /// `version` match.
    pub fn new(storage: S, config: TextIndexConfig<E>) -> Self {
        let inner = ProximityIndex::new(
            storage,
            ProximityConfig {
                dim: config.embedder.dim(),
                metric: config.metric,
                level_bits: config.level_bits,
                max_bucket_size: config.max_bucket_size,
            },
        );
        Self {
            inner,
            embedder: config.embedder,
        }
    }

    /// Load a previously persisted text index.
    ///
    /// The supplied `embedder` must match the stored identity. Mismatch
    /// returns [`TextIndexError::EmbedderMismatch`]; dim mismatch returns
    /// [`TextIndexError::DimensionMismatch`].
    pub fn load(storage: S, name: &str, embedder: E) -> Result<Self, TextIndexError> {
        let state_key = text_state_key(name);
        // Must exist for a load() call (use `new` for create-or-load semantics).
        if storage.get_config(&state_key).is_none() {
            return Err(TextIndexError::NotFound(name.to_string()));
        }
        // Shared validation logic ensures consistency with the namespaced path.
        // For load() we expect the blob to exist, but the helper handles both
        // cases — that's harmless here.
        validate_or_write_text_identity::<N, _, _>(
            &storage,
            &state_key,
            &embedder,
            Metric::Cosine,
            4,
            64,
        )?;

        // Delegate to ProximityIndex::load for the entries + tree state. The
        // saved ProximityConfig (including the actual metric / level_bits /
        // max_bucket_size that were used at create time) lives inside the
        // proximity state blob, so the values we pass to
        // `validate_or_write_text_identity` above only apply when the blob is
        // missing — i.e. they don't override the real state.
        let proximity_name = text_inner_proximity_name(name);
        let inner = ProximityIndex::load(storage, &proximity_name)?;
        Ok(Self { inner, embedder })
    }

    /// Persist the index under `name`. After this call the index can be
    /// re-opened via [`TextIndex::load`] with the same `name`.
    pub fn persist(&mut self, name: &str) -> Result<Option<ValueDigest<N>>, TextIndexError> {
        let cfg = self.inner.config().clone();
        let state = SavedTextIndexState {
            embedder_id: self.embedder.id().to_string(),
            embedder_version: self.embedder.version().to_string(),
            dim: cfg.dim,
            metric: cfg.metric,
            level_bits: cfg.level_bits,
            max_bucket_size: cfg.max_bucket_size,
        };
        let bytes =
            bincode::serialize(&state).map_err(|e| TextIndexError::Serialize(e.to_string()))?;
        self.inner
            .storage()
            .save_config(&text_state_key(name), &bytes);

        // Persist the inner proximity index under a paired name.
        let proximity_name = text_inner_proximity_name(name);
        let root = self.inner.persist(&proximity_name)?;
        Ok(root)
    }

    /// Insert or update a document — embeds `text` and stores the resulting
    /// vector under `id`.
    pub fn insert(&mut self, id: &[u8], text: &str) -> Result<(), TextIndexError> {
        let vec = self.embedder.embed(text)?;
        if vec.len() as u16 != self.embedder.dim() {
            return Err(TextIndexError::Embed(EmbedError::DimensionMismatch {
                expected: self.embedder.dim(),
                got: vec.len() as u16,
            }));
        }
        self.inner.insert(id.to_vec(), vec)?;
        Ok(())
    }

    /// Remove a document. Returns whether anything was removed.
    pub fn delete(&mut self, id: &[u8]) -> bool {
        self.inner.remove(id)
    }

    /// k-nearest-neighbour search. Embeds `query` and runs the KNN under
    /// the configured metric.
    pub fn search(&mut self, query: &str, k: usize) -> Result<Vec<TextHit>, TextIndexError> {
        let q = self.embedder.embed(query)?;
        let ef = (k * 4).max(32);
        let hits = self.inner.knn(&q, k, ef)?;
        Ok(hits
            .into_iter()
            .map(|(id, score)| TextHit { id, score })
            .collect())
    }

    /// Re-embed every entry currently in the index using the current
    /// embedder. Use this after deliberately switching embedders, to bring
    /// the index up to the new identity.
    ///
    /// Walks `(id, text)` pairs from the **provided `texts` map**, which the
    /// caller assembles from their source-of-truth document store. PR 4a
    /// keeps text storage out of the index (the index only sees vectors); a
    /// later PR adds primary-tree integration for automatic
    /// reindex-from-primary.
    pub fn reindex_from_texts<I>(&mut self, texts: I) -> Result<(), TextIndexError>
    where
        I: IntoIterator<Item = (Vec<u8>, String)>,
    {
        // Drop all existing entries first.
        let ids: Vec<Vec<u8>> = self.inner.entries_snapshot().keys().cloned().collect();
        for id in ids {
            self.inner.remove(&id);
        }
        for (id, text) in texts {
            let vec = self.embedder.embed(&text)?;
            self.inner.insert(id, vec)?;
        }
        Ok(())
    }

    /// Number of stored documents.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// True if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Root hash of the materialised proximity tree.
    pub fn root_hash(&mut self) -> Result<Option<ValueDigest<N>>, TextIndexError> {
        Ok(self.inner.root_hash()?.cloned())
    }

    /// Read-only access to the wrapped embedder.
    pub fn embedder(&self) -> &E {
        &self.embedder
    }

    /// Read-only access to the underlying proximity configuration.
    pub fn proximity_config(&self) -> &ProximityConfig {
        self.inner.config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proximity::embedder::HashEmbedder;
    use crate::storage::InMemoryNodeStorage;

    fn config(dim: u16) -> TextIndexConfig<HashEmbedder> {
        TextIndexConfig::new(HashEmbedder::new(dim, 0))
    }

    #[test]
    fn insert_and_search_finds_exact_match() {
        let storage = InMemoryNodeStorage::<32>::new();
        let mut idx = TextIndex::new(storage, config(32));
        idx.insert(b"doc:1", "the quick brown fox").unwrap();
        idx.insert(b"doc:2", "another piece of text").unwrap();
        idx.insert(b"doc:3", "yet more content").unwrap();

        let hits = idx.search("the quick brown fox", 1).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, b"doc:1".to_vec());
        // Exact match → near-zero cosine distance.
        assert!(
            hits[0].score < 1e-4,
            "expected near-zero, got {}",
            hits[0].score
        );
    }

    #[test]
    fn delete_removes_document_from_search() {
        let storage = InMemoryNodeStorage::<32>::new();
        let mut idx = TextIndex::new(storage, config(8));
        idx.insert(b"a", "hello world").unwrap();
        idx.insert(b"b", "goodbye world").unwrap();
        assert!(idx.delete(b"a"));
        assert!(!idx.delete(b"a"));
        let hits = idx.search("hello world", 2).unwrap();
        assert!(!hits.iter().any(|h| h.id == b"a".to_vec()));
        assert!(hits.iter().any(|h| h.id == b"b".to_vec()));
    }

    #[test]
    fn empty_index_search_returns_empty() {
        let storage = InMemoryNodeStorage::<32>::new();
        let mut idx = TextIndex::<32, _, _>::new(storage, config(8));
        let hits = idx.search("anything", 5).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn persist_then_load_matches_original() {
        let storage = InMemoryNodeStorage::<32>::new();
        let mut idx = TextIndex::new(storage, config(16));
        idx.insert(b"a", "first document").unwrap();
        idx.insert(b"b", "second document").unwrap();
        idx.insert(b"c", "third document").unwrap();
        let original_hits = idx.search("second document", 3).unwrap();
        idx.persist("docs").unwrap();
        let storage_after = idx.inner.storage().clone();

        let mut reopened: TextIndex<32, _, _> =
            TextIndex::load(storage_after, "docs", HashEmbedder::new(16, 0)).unwrap();
        assert_eq!(reopened.len(), 3);
        let reopened_hits = reopened.search("second document", 3).unwrap();
        assert_eq!(reopened_hits, original_hits);
    }

    #[test]
    fn load_with_different_embedder_id_returns_mismatch() {
        // We don't have a second embedder family yet, so simulate this by
        // wrapping HashEmbedder in a thin shim that reports a different id.
        struct DifferentIdEmbedder(HashEmbedder);
        impl Embedder for DifferentIdEmbedder {
            fn id(&self) -> &str {
                "other:family/v1"
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

        let storage = InMemoryNodeStorage::<32>::new();
        let mut idx = TextIndex::new(storage, config(8));
        idx.insert(b"a", "x").unwrap();
        idx.persist("docs").unwrap();
        let storage_after = idx.inner.storage().clone();

        let err = TextIndex::<32, _, _>::load(
            storage_after,
            "docs",
            DifferentIdEmbedder(HashEmbedder::new(8, 0)),
        )
        .unwrap_err();
        assert!(
            matches!(err, TextIndexError::EmbedderMismatch { .. }),
            "expected EmbedderMismatch, got {err:?}"
        );
    }

    #[test]
    fn load_with_different_embedder_version_returns_mismatch() {
        let storage = InMemoryNodeStorage::<32>::new();
        let mut idx = TextIndex::new(storage, TextIndexConfig::new(HashEmbedder::new(8, 0)));
        idx.insert(b"a", "x").unwrap();
        idx.persist("docs").unwrap();
        let storage_after = idx.inner.storage().clone();

        // Different seed → different version string.
        let err = TextIndex::<32, _, _>::load(storage_after, "docs", HashEmbedder::new(8, 1))
            .unwrap_err();
        assert!(matches!(err, TextIndexError::EmbedderMismatch { .. }));
    }

    #[test]
    fn load_unknown_name_returns_not_found() {
        let storage = InMemoryNodeStorage::<32>::new();
        let err =
            TextIndex::<32, _, _>::load(storage, "missing", HashEmbedder::new(8, 0)).unwrap_err();
        assert!(matches!(err, TextIndexError::NotFound(name) if name == "missing"));
    }

    #[test]
    fn reindex_from_texts_clears_then_re_embeds() {
        let storage = InMemoryNodeStorage::<32>::new();
        let mut idx = TextIndex::new(storage, config(8));
        idx.insert(b"a", "stale").unwrap();
        idx.insert(b"b", "stale").unwrap();
        assert_eq!(idx.len(), 2);

        idx.reindex_from_texts(vec![
            (b"a".to_vec(), "fresh a".to_string()),
            (b"c".to_vec(), "fresh c".to_string()),
        ])
        .unwrap();

        assert_eq!(idx.len(), 2);
        // "a" was kept but re-embedded; "b" was dropped; "c" was added.
        let hits = idx.search("fresh c", 1).unwrap();
        assert_eq!(hits[0].id, b"c".to_vec());
    }

    #[test]
    fn search_returns_at_most_k() {
        let storage = InMemoryNodeStorage::<32>::new();
        let mut idx = TextIndex::new(storage, config(16));
        for i in 0..10 {
            let id = format!("id-{i}").into_bytes();
            let text = format!("text-{i}");
            idx.insert(&id, &text).unwrap();
        }
        assert_eq!(idx.search("text-3", 3).unwrap().len(), 3);
        assert_eq!(idx.search("text-3", 100).unwrap().len(), 10);
    }
}
