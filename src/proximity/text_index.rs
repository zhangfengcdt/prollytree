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
use crate::proximity::chunker::{Chunker, IdentityChunker};
use crate::proximity::embedder::{EmbedError, Embedder};
use crate::proximity::index::{ProximityConfig, ProximityError, ProximityIndex};
use crate::proximity::Metric;
use crate::storage::{NodeStorage, StorageError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Chunk-id encoding (PR — multi-chunk plumbing)
// ---------------------------------------------------------------------------
//
// A document-level id (what the user passes in) maps to one-or-more chunk
// ids in the underlying ProximityIndex via a length-prefixed envelope:
//
// ```text
// [ doc_id_len: 4 bytes LE ][ doc_id_bytes ][ chunk_idx: 4 bytes LE ]
// ```
//
// Properties: unambiguous parse for any byte content in doc_id, all chunks
// for a doc share the same `[doc_id_len][doc_id_bytes]` prefix (used by
// `delete_by_doc_id` to find every chunk in one scan).

/// Encode `(doc_id, chunk_idx)` into the underlying ProximityIndex's id space.
pub(crate) fn make_chunk_id(doc_id: &[u8], chunk_idx: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + doc_id.len() + 4);
    out.extend_from_slice(&(doc_id.len() as u32).to_le_bytes());
    out.extend_from_slice(doc_id);
    out.extend_from_slice(&chunk_idx.to_le_bytes());
    out
}

/// Inverse of [`make_chunk_id`]. Returns `None` for any bytes that don't
/// have the expected shape — primary-tree values that happen to be in the
/// underlying proximity index without going through the chunked-insert path
/// would parse as `None` here.
pub fn parse_chunk_id(bytes: &[u8]) -> Option<(Vec<u8>, u32)> {
    if bytes.len() < 8 {
        return None;
    }
    let len = u32::from_le_bytes(bytes[..4].try_into().ok()?) as usize;
    if bytes.len() != 4 + len + 4 {
        return None;
    }
    let doc_id = bytes[4..4 + len].to_vec();
    let chunk_idx = u32::from_le_bytes(bytes[4 + len..].try_into().ok()?);
    Some((doc_id, chunk_idx))
}

/// Prefix shared by every chunk-id for a given `doc_id` — used to find and
/// delete all chunks for a document in one prefix scan.
pub(crate) fn doc_id_prefix(doc_id: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + doc_id.len());
    out.extend_from_slice(&(doc_id.len() as u32).to_le_bytes());
    out.extend_from_slice(doc_id);
    out
}

/// Over-fetch multiplier for `search`: we ask the underlying KNN for
/// `k * MULTIPLIER` chunks and dedup by doc-id afterwards, so that documents
/// with many chunks don't crowd out distinct top-k results.
pub(crate) const OVERFETCH_MULTIPLIER: usize = 4;

/// Dedup a chunk-level hit list by doc-id, keeping each doc's best score, and
/// truncate to `k`. Used by both standalone [`TextIndex::search`] and the
/// namespaced sub-handle.
pub(crate) fn dedup_chunk_hits_by_doc(chunk_hits: Vec<(Vec<u8>, f32)>, k: usize) -> Vec<TextHit> {
    let mut best_per_doc: HashMap<Vec<u8>, f32> = HashMap::new();
    for (chunk_id, score) in chunk_hits {
        let doc_id = match parse_chunk_id(&chunk_id) {
            Some((d, _)) => d,
            None => chunk_id.clone(), // legacy non-chunked id; treat as its own doc
        };
        best_per_doc
            .entry(doc_id)
            .and_modify(|s| {
                if score < *s {
                    *s = score;
                }
            })
            .or_insert(score);
    }
    let mut docs: Vec<(Vec<u8>, f32)> = best_per_doc.into_iter().collect();
    docs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    docs.truncate(k);
    docs.into_iter()
        .map(|(id, score)| TextHit { id, score })
        .collect()
}

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
#[derive(Clone)]
pub struct TextIndexConfig<E: Embedder> {
    pub embedder: E,
    /// Chunker used to split documents on insert. Defaults to
    /// [`IdentityChunker`] (one chunk per document, behaviour identical to
    /// pre-multi-chunk).
    pub chunker: Arc<dyn Chunker>,
    pub metric: Metric,
    pub level_bits: u8,
    pub max_bucket_size: u16,
}

impl<E: Embedder + std::fmt::Debug> std::fmt::Debug for TextIndexConfig<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextIndexConfig")
            .field("embedder", &self.embedder)
            .field("chunker_id", &self.chunker.id())
            .field("metric", &self.metric)
            .field("level_bits", &self.level_bits)
            .field("max_bucket_size", &self.max_bucket_size)
            .finish()
    }
}

impl<E: Embedder> TextIndexConfig<E> {
    /// Build with sensible defaults: [`IdentityChunker`], `Cosine` metric,
    /// `level_bits = 4`, `max_bucket_size = 64`.
    pub fn new(embedder: E) -> Self {
        Self {
            embedder,
            chunker: Arc::new(IdentityChunker),
            metric: Metric::Cosine,
            level_bits: 4,
            max_bucket_size: 64,
        }
    }

    /// Builder: swap the chunker. Use this for multi-chunk indexing — for
    /// example `.with_chunker(LineChunker)`.
    pub fn with_chunker<C: Chunker + 'static>(mut self, chunker: C) -> Self {
        self.chunker = Arc::new(chunker);
        self
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
        let state: SavedTextIndexState = crate::serde_bincode::deserialize(&bytes)
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
        let bytes = crate::serde_bincode::serialize(&state)
            .map_err(|e| TextIndexError::Serialize(e.to_string()))?;
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
    chunker: Arc<dyn Chunker>,
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
            chunker: config.chunker,
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
        //
        // The chunker isn't persisted (bincode limitation — see PR notes).
        // Reopening uses the default `IdentityChunker`; users with a custom
        // chunker must call `with_chunker(...)` on the returned index before
        // any chunker-dependent operation.
        let proximity_name = text_inner_proximity_name(name);
        let inner = ProximityIndex::load(storage, &proximity_name)?;
        Ok(Self {
            inner,
            embedder,
            chunker: Arc::new(IdentityChunker),
        })
    }

    /// Swap the chunker after the fact. Useful for `load(...)` callers whose
    /// index was originally created with a non-identity chunker (since
    /// chunker identity isn't persisted in the saved state blob).
    pub fn set_chunker<C: Chunker + 'static>(&mut self, chunker: C) {
        self.chunker = Arc::new(chunker);
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
        let bytes = crate::serde_bincode::serialize(&state)
            .map_err(|e| TextIndexError::Serialize(e.to_string()))?;
        self.inner
            .storage()
            .save_config(&text_state_key(name), &bytes);

        // Persist the inner proximity index under a paired name.
        let proximity_name = text_inner_proximity_name(name);
        let root = self.inner.persist(&proximity_name)?;
        Ok(root)
    }

    /// Insert or update a document. The text is split via the configured
    /// `Chunker` into one or more chunks; each chunk is embedded and stored
    /// under its own chunk-id.
    ///
    /// **Re-insert semantics:** replacement chunks are embedded first, then
    /// existing chunks for this `id` are removed, so a failed embed leaves the
    /// previous document searchable. A successful empty chunk set removes the
    /// document from the index.
    pub fn insert(&mut self, id: &[u8], text: &str) -> Result<(), TextIndexError> {
        let chunks = self.chunker.split(text);
        if chunks.is_empty() {
            // The chunker explicitly opted out (e.g. an empty document under
            // LineChunker). Treat as "don't index" rather than as an error —
            // matches the cascade transformer's `None` semantics.
            self.delete_chunks_for_doc(id);
            return Ok(());
        }
        if self.embedder.dim() == 0 {
            return Err(TextIndexError::Proximity(ProximityError::ZeroDim));
        }
        let mut replacements = Vec::with_capacity(chunks.len());
        for (idx, chunk_text) in chunks.iter().enumerate() {
            let vec = self.embedder.embed(chunk_text)?;
            if vec.len() as u16 != self.embedder.dim() {
                return Err(TextIndexError::Embed(EmbedError::DimensionMismatch {
                    expected: self.embedder.dim(),
                    got: vec.len() as u16,
                }));
            }
            let chunk_id = make_chunk_id(id, idx as u32);
            replacements.push((chunk_id, vec));
        }

        self.delete_chunks_for_doc(id);
        for (chunk_id, vec) in replacements {
            self.inner.insert(chunk_id, vec)?;
        }
        Ok(())
    }

    /// Remove a document — removes every chunk associated with `id`.
    /// Returns whether at least one chunk was removed.
    pub fn delete(&mut self, id: &[u8]) -> bool {
        self.delete_chunks_for_doc(id)
    }

    /// k-nearest-neighbour search. Embeds `query`, over-fetches chunks from
    /// the underlying KNN, then dedups by doc-id (keeping each doc's best
    /// chunk score). Returns top-k **documents**.
    pub fn search(&mut self, query: &str, k: usize) -> Result<Vec<TextHit>, TextIndexError> {
        if k == 0 {
            return Ok(Vec::new());
        }
        let q = self.embedder.embed(query)?;
        // Over-fetch chunks so the dedup-by-doc step still yields `k` docs
        // even when there are several chunks per doc. The 4× multiplier is
        // a reasonable middle ground.
        let raw_k = (k * OVERFETCH_MULTIPLIER).max(k);
        let ef = (raw_k * 4).max(32);
        let chunk_hits = self.inner.knn(&q, raw_k, ef)?;
        Ok(dedup_chunk_hits_by_doc(chunk_hits, k))
    }

    /// Internal: remove every chunk for `doc_id`. Returns whether any were
    /// removed. Uses an in-memory scan of the `ProximityIndex`'s entry set;
    /// a future optimisation can switch to a range-based BTreeMap query.
    fn delete_chunks_for_doc(&mut self, doc_id: &[u8]) -> bool {
        let prefix = doc_id_prefix(doc_id);
        let to_remove: Vec<Vec<u8>> = self
            .inner
            .entries_snapshot()
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        let mut any = false;
        for cid in to_remove {
            if self.inner.remove(&cid) {
                any = true;
            }
        }
        any
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
        // Drop every chunk in the index. Cheaper than per-doc deletion since
        // we're rebuilding from scratch anyway.
        let ids: Vec<Vec<u8>> = self.inner.entries_snapshot().keys().cloned().collect();
        for id in ids {
            self.inner.remove(&id);
        }
        // Re-insert each doc through the chunker.
        for (id, text) in texts {
            self.insert(&id, &text)?;
        }
        Ok(())
    }

    /// Number of stored **documents** (deduplicated across chunks).
    ///
    /// Under [`IdentityChunker`] (the default) chunks and docs are 1:1, so
    /// this is the same as `chunk_count`. Under a multi-chunk chunker,
    /// docs ≤ chunks.
    pub fn len(&self) -> usize {
        use std::collections::HashSet;
        let mut docs: HashSet<Vec<u8>> = HashSet::new();
        for k in self.inner.entries_snapshot().keys() {
            match parse_chunk_id(k) {
                Some((doc, _)) => {
                    docs.insert(doc);
                }
                None => {
                    docs.insert(k.clone());
                }
            }
        }
        docs.len()
    }

    /// Total chunk count in the underlying proximity index. Diagnostic
    /// counterpart to `len`.
    pub fn chunk_count(&self) -> usize {
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

    #[derive(Debug, Clone)]
    struct FailsOnNeedleEmbedder {
        inner: HashEmbedder,
        needle: &'static str,
    }

    impl FailsOnNeedleEmbedder {
        fn new(dim: u16, needle: &'static str) -> Self {
            Self {
                inner: HashEmbedder::new(dim, 0),
                needle,
            }
        }
    }

    impl Embedder for FailsOnNeedleEmbedder {
        fn id(&self) -> &str {
            self.inner.id()
        }

        fn version(&self) -> &str {
            self.inner.version()
        }

        fn dim(&self) -> u16 {
            self.inner.dim()
        }

        fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
            if text.contains(self.needle) {
                return Err(EmbedError::Failure("forced insert failure".to_string()));
            }
            self.inner.embed(text)
        }
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
    fn failed_reinsert_preserves_existing_document() {
        let storage = InMemoryNodeStorage::<32>::new();
        let mut idx = TextIndex::new(
            storage,
            TextIndexConfig::new(FailsOnNeedleEmbedder::new(8, "fail-insert")),
        );
        idx.insert(b"doc:1", "stable text").unwrap();

        let err = idx.insert(b"doc:1", "please fail-insert").unwrap_err();
        assert!(err.to_string().contains("forced insert failure"));

        let hits = idx.search("stable text", 1).unwrap();
        assert_eq!(hits[0].id, b"doc:1".to_vec());
        assert!(hits[0].score < 1e-4);
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
