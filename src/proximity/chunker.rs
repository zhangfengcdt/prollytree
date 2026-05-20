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

//! Document-to-chunks splitting for [`crate::proximity::TextIndex`].
//!
//! Ships the [`Chunker`] trait plus two built-in implementations:
//!
//! - [`IdentityChunker`] — one chunk per document (default; matches the
//!   pre-multi-chunk behaviour, used when [`crate::proximity::TextIndexConfig`]
//!   doesn't explicitly opt into something else).
//! - [`LineChunker`] — one chunk per non-empty line. Useful for log-style
//!   ingestion where line granularity is what the consumer searches over.
//!
//! Multi-chunk plumbing is wired through [`crate::proximity::TextIndex`]:
//! a single document under a non-identity chunker produces multiple
//! `(doc_id, chunk_idx) → vector` entries, all sharing a prefix in the chunk
//! id so [`crate::proximity::TextIndex::delete`] can prefix-scan-remove every
//! chunk for a doc, and search dedups results back to the document at its
//! best chunk's distance.
//!
//! Future chunkers (`SentenceChunker`, `RecursiveChunker`, …) plug into the
//! same `Embedder`/`Chunker` slot on
//! [`crate::proximity::TextIndexConfig`] without an API break.

/// Splits a document into one or more chunks. Each chunk will receive its
/// own embedding when the consumer ([`crate::proximity::TextIndex`]) routes
/// inserts through this chunker.
///
/// Returning an empty `Vec` means "don't index this document" — useful when
/// a transformer determines the value isn't text (e.g. a binary blob in the
/// primary tree).
pub trait Chunker: Send + Sync {
    /// Stable identifier persisted alongside the index so a re-open under a
    /// different chunker can be detected. Examples: `"identity"`,
    /// `"sentence/256"`, `"recursive/markdown/512:128"`.
    fn id(&self) -> &str;

    /// Split `text` into chunks. Implementations must be deterministic —
    /// same input always yields the same chunks in the same order.
    fn split<'t>(&self, text: &'t str) -> Vec<&'t str>;
}

/// Trivial chunker: returns the whole document as a single chunk.
///
/// This is the v1 default. Search recall is best when documents are
/// reasonably short; longer documents benefit from sentence- or
/// paragraph-level chunking, which lands in a future PR.
#[derive(Debug, Clone, Default)]
pub struct IdentityChunker;

impl IdentityChunker {
    pub const ID: &'static str = "identity";
}

impl Chunker for IdentityChunker {
    fn id(&self) -> &str {
        Self::ID
    }

    fn split<'t>(&self, text: &'t str) -> Vec<&'t str> {
        vec![text]
    }
}

/// Line-based chunker: splits a document on `\n` boundaries, dropping empty
/// lines. Predictable enough to write deterministic tests against and useful
/// for line-oriented content (logs, code, structured notes).
#[derive(Debug, Clone, Default)]
pub struct LineChunker;

impl LineChunker {
    pub const ID: &'static str = "line";
}

impl Chunker for LineChunker {
    fn id(&self) -> &str {
        Self::ID
    }

    fn split<'t>(&self, text: &'t str) -> Vec<&'t str> {
        text.split('\n').filter(|l| !l.is_empty()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_returns_the_input() {
        let c = IdentityChunker;
        let chunks = c.split("hello world");
        assert_eq!(chunks, vec!["hello world"]);
    }

    #[test]
    fn identity_empty_input_still_returns_one_chunk() {
        // Differs from "no chunks" — the chunker doesn't decide
        // unindexability for empty strings; the consumer can if it wants.
        let chunks = IdentityChunker.split("");
        assert_eq!(chunks, vec![""]);
    }

    #[test]
    fn identity_id_is_stable() {
        assert_eq!(IdentityChunker.id(), "identity");
        assert_eq!(IdentityChunker::ID, "identity");
    }

    #[test]
    fn identity_is_deterministic() {
        let c = IdentityChunker;
        for s in ["foo", "", "multi\nline\ntext", "with unicode 日本語"] {
            assert_eq!(c.split(s), c.split(s));
        }
    }

    #[test]
    fn chunker_is_object_safe() {
        // Trait-object usage compiles — important for future
        // `Box<dyn Chunker>` storage if we ever need heterogeneous
        // chunkers in one container.
        let boxed: Box<dyn Chunker> = Box::new(IdentityChunker);
        assert_eq!(boxed.id(), "identity");
    }

    // ---- LineChunker ----------------------------------------------------

    #[test]
    fn line_chunker_splits_on_newlines() {
        let chunks = LineChunker.split("alpha\nbeta\ngamma");
        assert_eq!(chunks, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn line_chunker_drops_empty_lines() {
        let chunks = LineChunker.split("alpha\n\nbeta\n");
        assert_eq!(chunks, vec!["alpha", "beta"]);
    }

    #[test]
    fn line_chunker_single_line_returns_one_chunk() {
        let chunks = LineChunker.split("just one line");
        assert_eq!(chunks, vec!["just one line"]);
    }

    #[test]
    fn line_chunker_empty_input_returns_no_chunks() {
        assert_eq!(LineChunker.split("").len(), 0);
    }

    #[test]
    fn line_chunker_id_is_stable() {
        assert_eq!(LineChunker.id(), "line");
        assert_eq!(LineChunker::ID, "line");
    }
}
