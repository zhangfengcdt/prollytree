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
//! # Current scope (PR 1)
//!
//! This first slice ships:
//!
//! - [`ProximityNode`] — the node type stored under content-addressed hashes.
//! - [`Distance`] / [`Metric`] — pluggable distance with three built-in metrics.
//! - [`vector_level`] — the level-assignment hash function.
//! - [`ProximityIndex`] — `insert` / `knn` against an in-memory store.
//!
//! Persistence across `NodeStorage` backends, sub-index integration with
//! namespaces, version-controlled merge, and the text-search wrapper are
//! follow-on PRs.

mod distance;
mod index;
mod level;
mod node;
mod storage;

pub use distance::{Distance, Metric};
pub use index::{ProximityConfig, ProximityError, ProximityIndex};
pub use level::vector_level;
pub use node::ProximityNode;
