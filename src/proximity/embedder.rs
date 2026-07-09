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

//! Text-to-vector embedders for [`crate::proximity::TextIndex`] (PR 4a).
//!
//! # API
//!
//! Implementors define `embed(text) -> Vec<f32>`, plus an `id()` and
//! `version()` pair that are persisted alongside the [`TextIndex`] so a
//! re-open under a different embedder fails fast (`EmbedderMismatch`)
//! rather than silently mixing incompatible vectors.
//!
//! # PR 4a — built-in embedder
//!
//! [`HashEmbedder`] is a deterministic, pure-Rust embedder built from a
//! seeded SHA-256-derived projection. It produces well-formed vectors of
//! configurable dimension and is useful for tests and end-to-end demos. It
//! has no semantic meaning — semantic embeddings ship in PR 4b via
//! Candle + all-MiniLM-L6-v2.

use thiserror::Error;

/// Errors an [`Embedder`] may return from `embed()`.
#[derive(Debug, Error)]
pub enum EmbedError {
    /// Tokenization / preprocessing failed.
    #[error("embed failed: {0}")]
    Failure(String),
    /// The embedder produced a vector of the wrong dimension. This is a bug
    /// in the embedder; surface it so the index can refuse the value.
    #[error("embed produced wrong dim: expected {expected}, got {got}")]
    DimensionMismatch { expected: u16, got: usize },
}

/// Text-to-vector embedder.
///
/// Implementations must:
///
/// 1. Return a vector of length `dim()`.
/// 2. Produce **deterministic** output for the same input (so re-indexing the
///    same data yields the same proximity tree).
/// 3. Persist their identity via `id()` + `version()`. These are stored on
///    disk in the text-index registry entry and compared on re-open. The
///    identity should change whenever the embedding distribution changes —
///    for example, bumping the model weights or switching pooling strategies.
pub trait Embedder: Send + Sync {
    /// Stable identifier for this embedder family (e.g.
    /// `"candle:minilm-l6-v2"`).
    fn id(&self) -> &str;

    /// Stable version tag within the family. For ML embedders, the content
    /// hash of the weights file. For deterministic embedders like
    /// [`HashEmbedder`], a config-derived fingerprint.
    fn version(&self) -> &str;

    /// Vector dimensionality this embedder produces.
    fn dim(&self) -> u16;

    /// Embed `text` into a `dim()`-length vector.
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError>;
}

/// Deterministic, hash-based embedder.
///
/// Maps text to a fixed-dimension vector by SHA-256-ing chunks of the input.
/// Useful for tests, examples, and validating the index API end-to-end
/// without any ML dependency. Recall against natural-language semantic
/// queries is essentially zero — this is **not a semantic embedder**.
///
/// The vector is L2-normalised, so cosine similarity behaves correctly.
#[derive(Debug, Clone)]
pub struct HashEmbedder {
    dim: u16,
    seed: u64,
    id: String,
    version: String,
}

impl HashEmbedder {
    /// Build a new hash embedder of the given dimensionality.
    ///
    /// `seed` lets you create multiple non-overlapping deterministic
    /// embedders (e.g. for A/B comparisons in tests).
    pub fn new(dim: u16, seed: u64) -> Self {
        let id = "prollytree:hash-embedder/v1".to_string();
        let version = format!("dim={dim};seed={seed}");
        Self {
            dim,
            seed,
            id,
            version,
        }
    }
}

impl Embedder for HashEmbedder {
    fn id(&self) -> &str {
        &self.id
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn dim(&self) -> u16 {
        self.dim
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        use crate::digest::ValueDigest;

        let mut out = vec![0.0f32; self.dim as usize];
        // Fill the vector by computing a SHA-256 of `seed || dim_index || text`
        // for each dimension. Map digest bytes to [-1, 1] via i8 normalisation.
        for (i, slot) in out.iter_mut().enumerate() {
            let mut buf = Vec::with_capacity(8 + 4 + text.len());
            buf.extend_from_slice(&self.seed.to_le_bytes());
            buf.extend_from_slice(&(i as u32).to_le_bytes());
            buf.extend_from_slice(text.as_bytes());
            let h = ValueDigest::<32>::new(&buf);
            // Take the first byte of the hash, map to f32 in [-1, 1].
            let b = h.as_bytes()[0] as i8;
            *slot = (b as f32) / 128.0;
        }
        // L2-normalise the vector. This makes cosine distance behave as the
        // angle and bounds the values, so downstream level-assignment hashing
        // doesn't depend on raw magnitude.
        let norm: f32 = out.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in out.iter_mut() {
                *x /= norm;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_embedder_basic_shape() {
        let e = HashEmbedder::new(32, 0);
        let v = e.embed("hello").unwrap();
        assert_eq!(v.len(), 32);
        // L2-normalised: length should be ~1.
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4, "expected unit norm, got {norm}");
    }

    #[test]
    fn hash_embedder_deterministic() {
        let e = HashEmbedder::new(8, 42);
        assert_eq!(e.embed("foo").unwrap(), e.embed("foo").unwrap());
        assert_ne!(e.embed("foo").unwrap(), e.embed("bar").unwrap());
    }

    #[test]
    fn different_seeds_produce_different_vectors() {
        let e1 = HashEmbedder::new(16, 0);
        let e2 = HashEmbedder::new(16, 1);
        assert_ne!(e1.embed("abc").unwrap(), e2.embed("abc").unwrap());
    }

    #[test]
    fn version_changes_with_dim_and_seed() {
        let v1 = HashEmbedder::new(8, 0).version().to_string();
        let v2 = HashEmbedder::new(16, 0).version().to_string();
        let v3 = HashEmbedder::new(8, 1).version().to_string();
        assert_ne!(v1, v2);
        assert_ne!(v1, v3);
    }

    #[test]
    fn id_is_stable_across_instances() {
        let a = HashEmbedder::new(8, 0);
        let b = HashEmbedder::new(16, 99);
        assert_eq!(a.id(), b.id()); // family id is fixed
    }

    #[test]
    fn empty_text_is_embeddable() {
        let e = HashEmbedder::new(8, 0);
        let v = e.embed("").unwrap();
        assert_eq!(v.len(), 8);
    }
}
