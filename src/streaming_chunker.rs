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

//! Streaming chunker for building canonical prolly trees.
//!
//! This is a Rust port of Dolt's
//! `github.com/dolthub/dolt/go/store/prolly/tree/{chunker, node_splitter,
//! node_builder}.go`. The architecture replaces the in-place
//! `Balanced::balance` mutation loop, which cannot maintain history
//! independence, with a streaming pipeline:
//!
//! ```text
//!   sorted (key, value) stream
//!            |
//!            v
//!   Chunker (level 0) ---- splitter decides boundaries
//!            |
//!            v   on boundary: write node, send (lastKey, hash) up
//!   Chunker (level 1)
//!            ...
//! ```
//!
//! ## Why this fixes history independence
//!
//! Each chunker has a `Splitter` that resets at every chunk boundary.
//! The split decision for a chunk depends only on the items inside that
//! chunk - never on prior history. Feed the same sorted items into a
//! freshly-reset splitter and you get the same boundaries every time, so
//! the same chunks and the same root hash.
//!
//! ## Phase 1 (this file)
//!
//! No cursor support yet - callers must feed the entire final sorted
//! `(key, value)` sequence. This is still O(N) per `apply_mutations`
//! but is *correct by construction* and removes the post-hoc
//! `canonicalize` rebuild. Phase 2 adds `NodeCursor` + `advance_to` so
//! unchanged subtrees can be skipped.

use crate::config::TreeConfig;
use crate::node::ProllyNode;
use crate::storage::NodeStorage;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use twox_hash::XxHash64;

const HASH_SEED: u64 = 0;

// --------------------------------------------------------------------------
// Splitter
// --------------------------------------------------------------------------

/// Decides where the item stream gets split into chunks.
///
/// Implementations must satisfy: a chunk's split decision depends only on
/// items appended since the most recent `reset()`. This is what gives the
/// streaming chunker history independence.
pub trait Splitter {
    /// Add an item to the rolling state. Once an item triggers a
    /// boundary, `crossed_boundary` returns `true` until `reset` is called.
    fn append(&mut self, key: &[u8], value: &[u8]);

    /// `true` if the most recent `append` caused a chunk boundary.
    fn crossed_boundary(&self) -> bool;

    /// Clear all rolling state for the next chunk.
    fn reset(&mut self);
}

/// Rolling-hash splitter that matches the algorithm in
/// `ProllyNode::chunk_content` (`src/node.rs`).
///
/// State machine: keep the last `min_chunk_size` items in a sliding
/// window. After `min_chunk_size` items have been appended, compute a
/// polynomial rolling hash over the window and check it against the
/// pattern. Boundary on match. Sliding the window costs one polynomial
/// step per append.
pub struct RollingHashSplitter {
    base: u64,
    modulus: u64,
    min_chunk_size: usize,
    pattern: u64,
    /// `base^min_chunk_size mod modulus`, used to subtract the
    /// dropped-out item's contribution when the window slides.
    base_exp_min: u64,
    /// `(key_hash, value_hash)` of the items currently in the window.
    /// Capacity `min_chunk_size`; popped from the front when sliding.
    window: VecDeque<(u64, u64)>,
    hash: u64,
    count: usize,
    crossed: bool,
}

impl RollingHashSplitter {
    pub fn new<const N: usize>(config: &TreeConfig<N>) -> Self {
        let base_exp_min = mod_exp(config.base, config.min_chunk_size as u64, config.modulus);
        Self {
            base: config.base,
            modulus: config.modulus,
            min_chunk_size: config.min_chunk_size,
            pattern: config.pattern,
            base_exp_min,
            window: VecDeque::with_capacity(config.min_chunk_size),
            hash: 0,
            count: 0,
            crossed: false,
        }
    }
}

impl Splitter for RollingHashSplitter {
    fn append(&mut self, key: &[u8], value: &[u8]) {
        if self.crossed {
            // A subsequent append after a boundary is the caller's
            // problem - they should have called reset. Be defensive.
            return;
        }

        let kh = hash_item(key, self.modulus);
        let vh = hash_item(value, self.modulus);
        self.count += 1;

        if self.window.len() < self.min_chunk_size {
            // Building the initial window. This mirrors
            // `initialize_rolling_hash`: hash = hash * base + kh + vh.
            self.hash = (self.hash.wrapping_mul(self.base) + kh + vh) % self.modulus;
            self.window.push_back((kh, vh));
        } else {
            // Slide: drop the front of the window, add the new item.
            // Matches `update_rolling_hash`.
            let (old_kh, old_vh) = self.window.pop_front().unwrap();
            let mut h = (self.hash.wrapping_mul(self.base) + kh + vh) % self.modulus;
            h = (h + self.modulus - (old_kh.wrapping_mul(self.base_exp_min)) % self.modulus)
                % self.modulus;
            h = (h + self.modulus - (old_vh.wrapping_mul(self.base_exp_min)) % self.modulus)
                % self.modulus;
            self.hash = h;
            self.window.push_back((kh, vh));
        }

        // Match `chunk_content`: only check the pattern once the window
        // has reached `min_chunk_size` items.
        if self.count >= self.min_chunk_size && (self.hash & self.pattern) == self.pattern {
            self.crossed = true;
        }
    }

    fn crossed_boundary(&self) -> bool {
        self.crossed
    }

    fn reset(&mut self) {
        self.window.clear();
        self.hash = 0;
        self.count = 0;
        self.crossed = false;
    }
}

fn hash_item(item: &[u8], modulus: u64) -> u64 {
    let mut hasher = XxHash64::with_seed(HASH_SEED);
    item.hash(&mut hasher);
    hasher.finish() % modulus
}

fn mod_exp(mut base: u64, mut exp: u64, modulus: u64) -> u64 {
    let mut result: u64 = 1;
    base %= modulus;
    while exp > 0 {
        if exp & 1 == 1 {
            result = (result.wrapping_mul(base)) % modulus;
        }
        exp >>= 1;
        base = (base.wrapping_mul(base)) % modulus;
    }
    result
}

// --------------------------------------------------------------------------
// NodeBuilder
// --------------------------------------------------------------------------

/// Accumulates `(key, value)` pairs for one in-progress chunk. When the
/// chunker emits a boundary, the builder is consumed into a `ProllyNode`
/// at the configured level and a fresh builder is started.
pub struct NodeBuilder<const N: usize> {
    pub keys: Vec<Vec<u8>>,
    pub values: Vec<Vec<u8>>,
    level: u8,
    is_leaf: bool,
    config: TreeConfig<N>,
}

impl<const N: usize> NodeBuilder<N> {
    pub fn new(config: &TreeConfig<N>, level: u8) -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
            level,
            is_leaf: level == 0,
            config: config.clone(),
        }
    }

    pub fn add(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.keys.push(key);
        self.values.push(value);
    }

    pub fn count(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Consume the builder into a `ProllyNode`. Leaves the builder
    /// empty so it can be reused for the next chunk.
    pub fn build(&mut self) -> ProllyNode<N> {
        ProllyNode {
            keys: std::mem::take(&mut self.keys),
            key_schema: self.config.key_schema.clone(),
            values: std::mem::take(&mut self.values),
            value_schema: self.config.value_schema.clone(),
            is_leaf: self.is_leaf,
            level: self.level,
            base: self.config.base,
            modulus: self.config.modulus,
            min_chunk_size: self.config.min_chunk_size,
            max_chunk_size: self.config.max_chunk_size,
            pattern: self.config.pattern,
            split: false,
            merged: false,
            encode_types: Vec::new(),
            encode_values: Vec::new(),
        }
    }
}

// --------------------------------------------------------------------------
// Chunker
// --------------------------------------------------------------------------

/// Streaming chunker. Accepts `(key, value)` pairs in sorted order,
/// emits chunks at boundaries chosen by the splitter, and recursively
/// builds parent chunkers for higher tree levels.
///
/// Lifetime: `'s` is the storage borrow lifetime. The chunker borrows
/// storage mutably so it can persist emitted chunks as it goes.
pub struct Chunker<'s, const N: usize, S: NodeStorage<N>> {
    builder: NodeBuilder<N>,
    splitter: RollingHashSplitter,
    parent: Option<Box<Chunker<'s, N, S>>>,
    config: TreeConfig<N>,
    level: u8,
    /// Reference to storage. Each chunker level needs to write its
    /// chunks. The shared mutable borrow is passed down explicitly
    /// rather than held in a field, because parent chunkers also need
    /// to write - see `handle_boundary`.
    _marker: std::marker::PhantomData<&'s mut S>,
}

impl<'s, const N: usize, S: NodeStorage<N>> Chunker<'s, N, S> {
    pub fn new(config: &TreeConfig<N>, level: u8) -> Self {
        Self {
            builder: NodeBuilder::new(config, level),
            splitter: RollingHashSplitter::new(config),
            parent: None,
            config: config.clone(),
            level,
            _marker: std::marker::PhantomData,
        }
    }

    /// Add a `(key, value)` pair at the leaf level.
    pub fn add_pair(&mut self, storage: &mut S, key: Vec<u8>, value: Vec<u8>) {
        self.append(storage, key, value);
    }

    fn append(&mut self, storage: &mut S, key: Vec<u8>, value: Vec<u8>) {
        // Update splitter state on a borrow of the bytes before they're
        // moved into the builder.
        self.splitter.append(&key, &value);
        self.builder.add(key, value);

        if self.splitter.crossed_boundary() {
            self.handle_boundary(storage);
        }
    }

    /// Emit the current in-progress chunk as a sealed node and feed
    /// `(firstKey, hash)` to the parent chunker (creating one if needed).
    ///
    /// Note: Dolt uses *last-key* as the parent pivot in its port. We
    /// use *first-key* to match the existing ProllyTree convention -
    /// `Balanced::balance` and `ProllyTree::find` both treat internal
    /// node keys as "first key of this child's subtree." This affects
    /// the rolling-hash content fed into the parent chunker but not
    /// history-independence: the same final state always yields the
    /// same first-keys and hashes.
    fn handle_boundary(&mut self, storage: &mut S) {
        debug_assert!(
            self.builder.count() > 0,
            "in-progress chunk must be non-empty at a boundary"
        );

        // Snapshot firstKey for the parent before we consume the builder.
        let first_key = self
            .builder
            .keys
            .first()
            .cloned()
            .expect("non-empty builder");

        let node = self.builder.build();
        let hash = node.get_hash();
        let _ = storage.insert_node(hash.clone(), node);

        // Ensure parent exists, then forward (firstKey, hash_bytes).
        if self.parent.is_none() {
            self.parent = Some(Box::new(Chunker::new(&self.config, self.level + 1)));
        }
        let parent = self.parent.as_mut().expect("parent created");
        parent.append(storage, first_key, hash.as_bytes().to_vec());

        self.splitter.reset();
        // `build` already left the builder empty.
    }

    /// Finalize the tree: flush any pending in-progress chunk at each
    /// level, then walk down the spine to find the canonical root.
    ///
    /// Returns the root `ProllyNode` AND persists it to storage so the
    /// caller can look it up by hash.
    pub fn done(mut self, storage: &mut S) -> ProllyNode<N> {
        // Walk up the spine. At each level, if the parent has anything
        // pending OR this level has more than one entry's worth of data,
        // emit a final chunk and recurse.
        //
        // The "canonical root" rule (matching Dolt): if at this level
        // we have exactly one chunk *and* there is no parent yet, that
        // single chunk IS the root. If we have multiple chunks, we still
        // need to seal the trailing chunk and the parent assembles them.
        if self.builder.count() == 0 {
            // Possible at any level except level 0 fresh start: we hit a
            // boundary exactly at the end of the input, so nothing is
            // pending. Pop up.
            if let Some(parent) = self.parent.take() {
                return parent.done(storage);
            }
            // Level 0 with no items: empty tree. Return an empty leaf.
            return NodeBuilder::<N>::new(&self.config, 0).build();
        }

        // We have items at this level. If there's a parent (we've already
        // emitted at least one chunk at this level), seal the trailing
        // chunk and let the parent finalize.
        if self.parent.is_some() {
            let first_key = self.builder.keys.first().cloned().expect("non-empty");
            let node = self.builder.build();
            let hash = node.get_hash();
            let _ = storage.insert_node(hash.clone(), node);
            let mut parent = self.parent.take().expect("parent present");
            parent.append(storage, first_key, hash.as_bytes().to_vec());
            return parent.done(storage);
        }

        // No parent at this level, single in-progress chunk. This IS the
        // canonical root (or the only leaf in a tiny tree).
        let root = self.builder.build();
        let root_hash = root.get_hash();
        let _ = storage.insert_node(root_hash, root.clone());
        root
    }
}

// --------------------------------------------------------------------------
// apply_mutations: stream a sorted edit iterator through a fresh chunker.
// --------------------------------------------------------------------------

/// Build a canonical prolly tree from a sorted `(key, value)` sequence
/// by streaming the items through a fresh `Chunker`. The resulting root
/// is persisted to storage and returned.
///
/// This is the Phase-1 entry point (no cursor optimization). It's
/// equivalent to `ProllyNode::build_canonical_from_pairs` but expressed
/// in terms of the streaming chunker pipeline, which is what makes the
/// algorithm history-independent by construction.
pub fn build_tree_from_sorted_pairs<const N: usize, S: NodeStorage<N>>(
    pairs: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
    config: &TreeConfig<N>,
    storage: &mut S,
) -> ProllyNode<N> {
    let mut chunker = Chunker::<N, S>::new(config, 0);
    for (k, v) in pairs {
        chunker.add_pair(storage, k, v);
    }
    chunker.done(storage)
}

// --------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryNodeStorage;

    fn key(i: u64) -> Vec<u8> {
        i.to_be_bytes().to_vec()
    }
    fn val(i: u64) -> Vec<u8> {
        let mut v = Vec::with_capacity(16);
        v.extend_from_slice(&i.to_be_bytes());
        v.extend_from_slice(&(!i).to_be_bytes());
        v
    }

    #[test]
    fn empty_input_produces_empty_leaf() {
        let cfg = TreeConfig::<32>::default();
        let mut storage = InMemoryNodeStorage::<32>::default();
        let root = build_tree_from_sorted_pairs::<32, _>(std::iter::empty(), &cfg, &mut storage);
        assert!(root.is_leaf);
        assert!(root.keys.is_empty());
        assert_eq!(root.level, 0);
    }

    #[test]
    fn single_item_is_a_single_leaf() {
        let cfg = TreeConfig::<32>::default();
        let mut storage = InMemoryNodeStorage::<32>::default();
        let root = build_tree_from_sorted_pairs::<32, _>(
            std::iter::once((key(0), val(0))),
            &cfg,
            &mut storage,
        );
        assert!(root.is_leaf);
        assert_eq!(root.keys.len(), 1);
    }

    #[test]
    fn many_items_below_min_stay_in_one_leaf() {
        // Default min_chunk_size = 8; with 4 items we're below the
        // window and the splitter never sees a full window, so no
        // boundary fires.
        let cfg = TreeConfig::<32>::default();
        let mut storage = InMemoryNodeStorage::<32>::default();
        let pairs: Vec<_> = (0..4u64).map(|i| (key(i), val(i))).collect();
        let root = build_tree_from_sorted_pairs::<32, _>(pairs.clone(), &cfg, &mut storage);
        assert!(root.is_leaf);
        assert_eq!(root.keys.len(), pairs.len());
    }

    #[test]
    fn root_hash_independent_of_iteration_order() {
        // Sanity check: building from the same sorted sequence produces
        // the same hash, regardless of who hands the items to us.
        let cfg = TreeConfig::<32>::default();
        let mut s1 = InMemoryNodeStorage::<32>::default();
        let mut s2 = InMemoryNodeStorage::<32>::default();
        let pairs: Vec<_> = (0..256u64).map(|i| (key(i), val(i))).collect();
        let r1 = build_tree_from_sorted_pairs::<32, _>(pairs.clone(), &cfg, &mut s1);
        let r2 = build_tree_from_sorted_pairs::<32, _>(pairs.iter().cloned(), &cfg, &mut s2);
        assert_eq!(r1.get_hash(), r2.get_hash());
    }

    #[test]
    fn matches_node_build_canonical_from_pairs() {
        // The streaming chunker should produce the same root hash as
        // ProllyNode::build_canonical_from_pairs for a non-trivial input.
        // This is the key correctness check before wiring into ProllyTree.
        let cfg = TreeConfig::<32>::default();
        let pairs: Vec<_> = (0..1024u64).map(|i| (key(i), val(i))).collect();

        let mut s_stream = InMemoryNodeStorage::<32>::default();
        let root_stream = build_tree_from_sorted_pairs::<32, _>(pairs.clone(), &cfg, &mut s_stream);

        let mut s_batch = InMemoryNodeStorage::<32>::default();
        let root_batch = ProllyNode::<32>::build_canonical_from_pairs(pairs, &cfg, &mut s_batch);

        assert_eq!(
            root_stream.get_hash(),
            root_batch.get_hash(),
            "streaming chunker produced a different canonical root than ProllyNode::build_canonical_from_pairs"
        );
    }
}
