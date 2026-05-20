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

//! Version-controlled approximate-nearest-neighbour index.
//!
//! This module implements a content-addressed proximity map inspired by Dolt's
//! vector index (<https://www.dolthub.com/blog/2025-06-23-vector-index-deep-dive/>).
//! A vector's maximum tree level is derived deterministically from a hash of
//! `(id, vector)`, so the index shape depends only on the current data — not on
//! insertion order or history. The resulting Merkle tree is shared across
//! versions for free.
//!
//! # What ships here
//!
//! - [`ProximityNode`] — content-addressed vector-bucket node type, stored
//!   via [`crate::storage::NodeStorage`] alongside `ProllyNode`s.
//! - [`Distance`] / [`Metric`] — pluggable distance functions with three
//!   built-in metrics (L2, Cosine, InnerProduct).
//! - [`vector_level`] — the leading-zero-bits level-assignment hash that
//!   gives the tree its history-independent shape.
//! - [`ProximityIndex`] — raw vector index with insert / remove / knn over
//!   any `NodeStorage`-backed store, plus persist / load helpers.
//! - [`TextIndex`] — text-search wrapper that owns an [`Embedder`] and a
//!   [`Chunker`], with multi-chunk plumbing (chunk-id encoding, prefix-scan
//!   delete, search dedup back to documents).
//! - Embedders — deterministic [`HashEmbedder`] (ML-free, for tests) and,
//!   under the `proximity_text` feature, [`MiniLmEmbedder`] (Candle +
//!   all-MiniLM-L6-v2).
//! - Chunkers — [`IdentityChunker`] (default) and [`LineChunker`].
//! - Three-way merge — [`merge_proximity_index_sets`] plus built-in
//!   resolvers (`LatestVectorResolver`, `MeanVectorResolver`,
//!   take-source / take-destination).
//!
//! Namespace-level integration — multi-index lifecycle, cascade, audit, and
//! the Git-backed atomic commit path — lives in
//! [`crate::git::versioned_store::NamespacedKvStore`] and re-exports
//! [`ProximityNamespaceHandle`] back here so callers can stay in the
//! `prollytree::proximity::*` namespace.

mod chunker;
mod distance;
mod embedder;
mod index;
mod level;
pub mod merge;
#[cfg(feature = "proximity_text")]
mod minilm;
mod node;
mod storage;
pub(crate) mod text_index;

pub use chunker::{Chunker, IdentityChunker, LineChunker};
pub use distance::{Distance, Metric};
pub use embedder::{EmbedError, Embedder, HashEmbedder};
pub use index::{
    deserialize_persisted_state, PersistedProximityState, ProximityConfig, ProximityError,
    ProximityIndex, ProximityIndexEntry,
};
pub use level::vector_level;
pub use merge::{
    merge_proximity_index_sets, LatestVectorResolver, MeanVectorResolver, MeanVectorResolverError,
    MergeFailure, MergedProximitySet, ProximityConflict, ProximityConflictResolver,
    ProximityResolution, TakeDestinationProximityResolver, TakeSourceProximityResolver,
};
#[cfg(feature = "proximity_text")]
pub use minilm::{MiniLmEmbedder, DEFAULT_MODEL_ID, DEFAULT_REVISION, MINILM_DIM};
pub use node::ProximityNode;
pub use text_index::{TextHit, TextIndex, TextIndexConfig, TextIndexError};

// `ProximityNamespaceHandle` lives in the namespace machinery — namespaces are
// the layer that owns multi-index lifecycle. Re-exported from here so callers
// only need `use prollytree::proximity::*;`.
#[cfg(feature = "git")]
pub use crate::git::versioned_store::ProximityNamespaceHandle;
