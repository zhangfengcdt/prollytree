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
//! ## Phases
//!
//! - **Phase 1**: `Chunker` + `Splitter` + `NodeBuilder` +
//!   `build_tree_from_sorted_pairs`. Streams a full sorted sequence
//!   through the chunker to produce a canonical tree. Used as the
//!   backend for `ProllyTree::canonicalize`.
//! - **Phase 2**: `NodeCursor` + `apply_mutations`. Walks an existing
//!   tree's leaves with a cursor and merges in a sorted batch of
//!   mutations through the same chunker. Replaces the legacy
//!   `ProllyNode::insert`/`delete` path in `ProllyTree`'s public API.
//! - **Phase 3**: `try_pure_append` (skip reading existing leaves when
//!   every mutation is an insert past the tree's max key) and
//!   `fast_forward_to_end` (skip remaining unchanged leaves once the
//!   chunker re-syncs with the old tree). Brings per-op cost down for
//!   the common cases of append-heavy workloads and small mid-tree
//!   edits without sacrificing canonicality.

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
    /// Hash of the most recently emitted chunk at this level. Set by
    /// `handle_boundary`, cleared whenever a new item is appended (so
    /// callers see `Some(h)` only immediately after a boundary). Used
    /// by `apply_mutations`'s fast-forward to detect when the new
    /// tree's chunk hash matches an old chunk's hash - i.e. when the
    /// chunker has re-synchronized with the old tree.
    last_emit_hash: Option<crate::digest::ValueDigest<N>>,
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
            last_emit_hash: None,
            _marker: std::marker::PhantomData,
        }
    }

    /// Add a `(key, value)` pair at the leaf level.
    pub fn add_pair(&mut self, storage: &mut S, key: Vec<u8>, value: Vec<u8>) {
        self.append(storage, key, value);
    }

    /// The hash of the most recently emitted chunk at this level, if
    /// nothing has been appended since. Consumed by the caller to test
    /// alignment with an old tree's chunk.
    pub fn take_last_emit_hash(&mut self) -> Option<crate::digest::ValueDigest<N>> {
        self.last_emit_hash.take()
    }

    /// True if the chunker's in-progress chunk is empty (i.e. the next
    /// append starts a fresh chunk). After a boundary emit this is
    /// true; while we're filling a chunk it is false.
    pub fn is_at_boundary(&self) -> bool {
        self.builder.is_empty()
    }

    /// Append a precomputed `(firstKey, child_hash_bytes)` entry to a
    /// chunker that is currently at a boundary. Used by the fast-forward
    /// path: when alignment with the old tree is detected, the next
    /// unchanged old leaf can be propagated to this level's parent
    /// chunker without re-feeding its leaf items.
    ///
    /// The caller MUST ensure `self.is_at_boundary()` is true; otherwise
    /// the in-progress chunk would absorb this entry mid-way, which is
    /// not what fast-forward expects.
    pub fn append_subtree_at_parent_level(
        &mut self,
        storage: &mut S,
        first_key: Vec<u8>,
        child_hash_bytes: Vec<u8>,
    ) {
        debug_assert!(
            self.is_at_boundary(),
            "fast-forward append requires a freshly emitted boundary at this level"
        );
        if self.parent.is_none() {
            self.parent = Some(Box::new(Chunker::new(&self.config, self.level + 1)));
        }
        let parent = self.parent.as_mut().expect("parent created");
        parent.append(storage, first_key, child_hash_bytes);
    }

    fn append(&mut self, storage: &mut S, key: Vec<u8>, value: Vec<u8>) {
        // A new append invalidates the last-emit signal.
        self.last_emit_hash = None;
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
        // Record this emit's hash so the caller can test for old-tree
        // alignment.
        self.last_emit_hash = Some(hash);
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
// NodeCursor: lazy traversal of an existing tree
// --------------------------------------------------------------------------

/// A stateful position into an existing tree.
///
/// Mirrors Dolt's `tree.cursor` (`node_cursor.go`): a linked list of
/// cursors, one per tree level, each pointing at a `(node, idx)` pair.
/// `advance` moves the leaf cursor forward, recursively bumping parents
/// when a leaf is exhausted; `seek` re-positions for a target key by
/// walking up to the lowest covering ancestor and then back down.
///
/// The cursor holds nodes by value (cloned from storage on read) - this
/// matches our existing `NodeStorage::get_node_by_hash` returning
/// `Arc<ProllyNode>`. Cloning the inner node is acceptable here because
/// the cursor only holds one node per level at a time.
#[derive(Clone)]
pub struct NodeCursor<const N: usize> {
    pub nd: ProllyNode<N>,
    /// Current index within `nd.keys` / `nd.values`. May be `-1` (before
    /// start) or `nd.keys.len() as i32` (past end) when the cursor is
    /// being advanced/retreated across node boundaries.
    pub idx: i32,
    pub parent: Option<Box<NodeCursor<N>>>,
}

impl<const N: usize> NodeCursor<N> {
    /// Construct a cursor positioned at the first key in the leftmost
    /// leaf of `root`'s subtree.
    pub fn at_start<S: NodeStorage<N>>(root: ProllyNode<N>, storage: &S) -> Self {
        let mut cur = Self {
            nd: root,
            idx: 0,
            parent: None,
        };
        while !cur.nd.is_leaf {
            let child_hash_bytes = cur.nd.values[cur.idx as usize].clone();
            let child = storage
                .get_node_by_hash(&crate::digest::ValueDigest::raw_hash(&child_hash_bytes))
                .map(|arc| (*arc).clone())
                .expect("child reachable from cursor");
            let parent = std::mem::replace(
                &mut cur,
                Self {
                    nd: child,
                    idx: 0,
                    parent: None,
                },
            );
            cur.parent = Some(Box::new(parent));
        }
        cur
    }

    /// Construct a cursor positioned at the first key >= `target_key`.
    /// If `target_key` is greater than every key, the cursor is invalid
    /// (idx == count).
    pub fn at_key<S: NodeStorage<N>>(root: ProllyNode<N>, target_key: &[u8], storage: &S) -> Self {
        // Walk down from the root following the pivot that "contains" the key.
        // At each internal level, find the rightmost pivot <= target_key
        // (matching the existing `ProllyTree::find` walk in src/node.rs).
        let mut cur = Self {
            nd: root,
            idx: 0,
            parent: None,
        };
        loop {
            if cur.nd.is_leaf {
                // Leaf: binary search for >= target_key.
                let pos = match cur
                    .nd
                    .keys
                    .binary_search_by(|k| k.as_slice().cmp(target_key))
                {
                    Ok(i) => i as i32,
                    Err(i) => i as i32,
                };
                cur.idx = pos;
                return cur;
            }
            // Internal: descend into the child whose pivot covers target_key.
            let i = cur
                .nd
                .keys
                .iter()
                .rposition(|k| target_key >= k.as_slice())
                .unwrap_or(0);
            cur.idx = i as i32;
            let child_hash_bytes = cur.nd.values[i].clone();
            let child = storage
                .get_node_by_hash(&crate::digest::ValueDigest::raw_hash(&child_hash_bytes))
                .map(|arc| (*arc).clone())
                .expect("child reachable from cursor");
            let parent = std::mem::replace(
                &mut cur,
                Self {
                    nd: child,
                    idx: 0,
                    parent: None,
                },
            );
            cur.parent = Some(Box::new(parent));
        }
    }

    pub fn valid(&self) -> bool {
        !self.nd.keys.is_empty() && self.idx >= 0 && (self.idx as usize) < self.nd.keys.len()
    }

    pub fn current_key(&self) -> &[u8] {
        &self.nd.keys[self.idx as usize]
    }

    pub fn current_value(&self) -> &[u8] {
        &self.nd.values[self.idx as usize]
    }

    pub fn at_node_end(&self) -> bool {
        (self.idx as usize) + 1 >= self.nd.keys.len()
    }

    fn has_next_in_node(&self) -> bool {
        (self.idx as usize) + 1 < self.nd.keys.len()
    }

    fn invalidate_at_end(&mut self) {
        self.idx = self.nd.keys.len() as i32;
    }

    fn out_of_bounds(&self) -> bool {
        self.idx < 0 || (self.idx as usize) >= self.nd.keys.len()
    }

    fn skip_to_node_start(&mut self) {
        self.idx = 0;
    }

    /// Advance to the next leaf-level item. Recursively bumps parents
    /// when a leaf is exhausted, then re-descends into the new leaf.
    pub fn advance<S: NodeStorage<N>>(&mut self, storage: &S) {
        if self.has_next_in_node() {
            self.idx += 1;
            return;
        }
        if self.parent.is_none() {
            self.invalidate_at_end();
            return;
        }
        // Recurse: bump the parent.
        let parent = self.parent.as_mut().expect("parent present");
        parent.advance(storage);
        if parent.out_of_bounds() {
            self.invalidate_at_end();
            return;
        }
        // The parent now points at a new child. Fetch it.
        let child_hash_bytes = parent.nd.values[parent.idx as usize].clone();
        let child = storage
            .get_node_by_hash(&crate::digest::ValueDigest::raw_hash(&child_hash_bytes))
            .map(|arc| (*arc).clone())
            .expect("child reachable from cursor");
        // The new child may itself be an internal node if our parent is
        // more than one level up - but for a single-level advance,
        // parent's child is always at our level.
        self.nd = child;
        self.skip_to_node_start();
    }
}

// --------------------------------------------------------------------------
// apply_mutations: cursor-aware streaming over an existing tree
// --------------------------------------------------------------------------

/// Apply a sorted batch of mutations to `root`, producing a new tree.
///
/// `mutations` is an iterator of `(key, Option<value>)` in ascending key
/// order. `None` means delete. The function walks the old tree with a
/// cursor and feeds unchanged items + mutated items through a fresh
/// chunker. The chunker's deterministic splitter ensures the output is
/// canonical regardless of the old tree's shape or the mutation order
/// (within the sorted batch).
///
/// Includes the Phase 3 alignment-aware fast-forward: after every chunk
/// emit at level 0, if the emit happens at the end of an *old* leaf
/// AND the emitted chunk's hash equals that old leaf's hash, the
/// chunker is back in sync with the old tree. If no more mutations
/// remain, we can copy the remaining old leaves directly into the
/// level-1 chunker without re-feeding their items.
pub fn apply_mutations<const N: usize, S, I>(
    root: ProllyNode<N>,
    mutations: I,
    config: &TreeConfig<N>,
    storage: &mut S,
) -> ProllyNode<N>
where
    S: NodeStorage<N>,
    I: IntoIterator<Item = (Vec<u8>, Option<Vec<u8>>)>,
{
    // If the root is an empty leaf, the old tree contributes nothing -
    // just stream the mutations through a fresh chunker.
    if root.is_leaf && root.keys.is_empty() {
        let mut chunker = Chunker::<N, S>::new(config, 0);
        for (k, opt_v) in mutations {
            if let Some(v) = opt_v {
                chunker.add_pair(storage, k, v);
            }
            // deletes against empty tree are no-ops
        }
        return chunker.done(storage);
    }

    // Pure-append fast path: if every mutation is an insert past the
    // tree's max key, all existing leaves are unchanged. Feed them
    // directly to the level-1 chunker without reading each leaf's
    // items, then process the new keys via a fresh level-0 chunker.
    //
    // For a tree with K existing leaves and one new key past the max,
    // this turns the inner loop's O(tree size) walk into O(K) parent
    // traversals - the dominant speedup for append-heavy workloads.
    let mut muts_vec: Vec<(Vec<u8>, Option<Vec<u8>>)> = mutations.into_iter().collect();
    if let Some(result) = try_pure_append(&root, &mut muts_vec, config, storage) {
        return result;
    }

    let mut cur = NodeCursor::at_start(root, storage);
    let mut chunker = Chunker::<N, S>::new(config, 0);

    let mut muts = muts_vec.into_iter().peekable();

    while cur.valid() {
        // Are there mutations that apply at or before the cursor's
        // current key?
        let cur_key = cur.current_key().to_vec();
        let mut consumed_cur = false;
        while let Some((mk, _)) = muts.peek() {
            match mk.as_slice().cmp(&cur_key) {
                std::cmp::Ordering::Less => {
                    // Insert this new key before cur_key.
                    let (mk, mv) = muts.next().expect("peeked");
                    if let Some(v) = mv {
                        chunker.add_pair(storage, mk, v);
                    }
                    // Delete of a non-existent key: no-op.
                }
                std::cmp::Ordering::Equal => {
                    // Update or delete of cur_key.
                    let (_, mv) = muts.next().expect("peeked");
                    match mv {
                        Some(v) => chunker.add_pair(storage, cur_key.clone(), v),
                        None => { /* delete: skip cur_key */ }
                    }
                    consumed_cur = true;
                    break;
                }
                std::cmp::Ordering::Greater => break,
            }
        }

        // Alignment / fast-forward can only kick in once there are no
        // more pending mutations AND the cursor is about to cross a
        // leaf boundary. Compute the leaf hash lazily so the
        // SHA-256-over-leaf cost is paid only when it could pay off.
        let about_to_leave_leaf = cur.at_node_end() && muts.peek().is_none();
        let leaf_hash_being_left = if about_to_leave_leaf {
            Some(cur.nd.get_hash())
        } else {
            None
        };

        if !consumed_cur {
            // Pass the existing cur_key/value through unchanged.
            let v = cur.current_value().to_vec();
            chunker.add_pair(storage, cur_key, v);
        }
        cur.advance(storage);

        // ----- Alignment / fast-forward -----
        //
        // If we just left an old leaf AND the chunker's most recent
        // emit happened at this exact transition AND the emitted hash
        // equals the old leaf's hash, then the chunker is back in
        // sync with the old tree. Fast-forward by copying the rest of
        // the old leaves directly into the level-1 chunker.
        if let Some(old_hash) = leaf_hash_being_left {
            if let Some(emit_hash) = chunker.take_last_emit_hash() {
                if emit_hash == old_hash && chunker.is_at_boundary() {
                    fast_forward_to_end(&mut chunker, &mut cur, storage);
                    break;
                }
            }
        }
    }

    // Drain any remaining mutations that fall past the end of the old tree.
    for (k, opt_v) in muts {
        if let Some(v) = opt_v {
            chunker.add_pair(storage, k, v);
        }
    }

    chunker.done(storage)
}

/// Pure-append optimization: when every mutation is an insert with a
/// key strictly greater than the tree's max key, the entire existing
/// tree is unchanged. Iterate the leaves left-to-right and feed each
/// leaf's `(firstKey, hash)` directly to the level-1 chunker, then
/// stream the new keys through a fresh level-0 chunker whose emissions
/// feed into that same level-1 parent.
///
/// Returns `Some(root)` when applied, `None` if the precondition isn't
/// met (e.g. any delete, or any mutation key <= tree max).
fn try_pure_append<const N: usize, S: NodeStorage<N>>(
    root: &ProllyNode<N>,
    muts: &mut Vec<(Vec<u8>, Option<Vec<u8>>)>,
    config: &TreeConfig<N>,
    storage: &mut S,
) -> Option<ProllyNode<N>> {
    if muts.is_empty() {
        return None;
    }
    // Any delete disqualifies the fast path.
    if muts.iter().any(|(_, v)| v.is_none()) {
        return None;
    }
    let max_key = tree_max_key(root, storage)?;
    // The mutations come from a BTreeMap upstream so they're already
    // sorted; the smallest key is muts[0].
    if muts[0].0.as_slice() <= max_key.as_slice() {
        return None;
    }

    // Collect existing leaves. We can't just feed ALL of them as-is to
    // the level-1 chunker: the LAST leaf might be incomplete (its
    // canonical chunker boundary fires only when full), so the new
    // keys need to be merged with it via a level-0 chunker. The
    // earlier leaves are guaranteed canonical boundaries (the splitter
    // pattern matched at their boundaries) so they can be propagated
    // directly.
    let mut leaves: Vec<(Vec<u8>, Vec<u8>, ProllyNode<N>)> = Vec::new();
    iter_leaves(root, storage, |leaf| {
        if !leaf.keys.is_empty() {
            leaves.push((
                leaf.keys[0].clone(),
                leaf.get_hash().as_bytes().to_vec(),
                leaf.clone(),
            ));
        }
    });
    if leaves.is_empty() {
        return None;
    }

    let mut chunker = Chunker::<N, S>::new(config, 0);

    // Feed all-but-the-last leaf to the level-1 parent chunker as
    // precomputed `(firstKey, hash)` entries. These leaves had canonical
    // boundaries when they were originally chunked.
    let last_leaf = leaves.pop().expect("non-empty");
    for (fk, h_bytes, _) in leaves {
        chunker.append_subtree_at_parent_level(storage, fk, h_bytes);
    }

    // Re-process the last existing leaf's items + the new keys through
    // the level-0 chunker. Its emits feed the level-1 parent we just
    // populated, preserving the canonical level-1 sequence.
    for (k, v) in last_leaf.2.keys.iter().zip(last_leaf.2.values.iter()) {
        chunker.add_pair(storage, k.clone(), v.clone());
    }
    for (k, opt_v) in muts.drain(..) {
        let v = opt_v.expect("pre-checked: no deletes in pure-append batch");
        chunker.add_pair(storage, k, v);
    }

    Some(chunker.done(storage))
}

/// Walk all leaves of `root` in key order, invoking `visit` on each.
fn iter_leaves<const N: usize, S: NodeStorage<N>>(
    root: &ProllyNode<N>,
    storage: &S,
    mut visit: impl FnMut(&ProllyNode<N>),
) {
    fn recurse<const N: usize, S: NodeStorage<N>>(
        node: &ProllyNode<N>,
        storage: &S,
        visit: &mut dyn FnMut(&ProllyNode<N>),
    ) {
        if node.is_leaf {
            visit(node);
            return;
        }
        for child_hash_bytes in &node.values {
            let child = storage
                .get_node_by_hash(&crate::digest::ValueDigest::raw_hash(child_hash_bytes))
                .map(|arc| (*arc).clone())
                .expect("child reachable");
            recurse(&child, storage, visit);
        }
    }
    recurse(root, storage, &mut visit);
}

/// Largest key in the tree, by walking the rightmost spine.
fn tree_max_key<const N: usize, S: NodeStorage<N>>(
    root: &ProllyNode<N>,
    storage: &S,
) -> Option<Vec<u8>> {
    if root.keys.is_empty() {
        return None;
    }
    if root.is_leaf {
        return root.keys.last().cloned();
    }
    let last_child_hash_bytes = root.values.last()?.clone();
    let last_child = storage
        .get_node_by_hash(&crate::digest::ValueDigest::raw_hash(
            &last_child_hash_bytes,
        ))
        .map(|arc| (*arc).clone())?;
    tree_max_key(&last_child, storage)
}

/// Fast-forward over the remaining old leaves once the chunker is in
/// sync with the old tree. Each subsequent leaf is propagated to the
/// level-1 chunker as a single `(firstKey, leaf_hash)` entry, without
/// reading the leaf's items. The cursor is consumed to its end.
fn fast_forward_to_end<const N: usize, S: NodeStorage<N>>(
    chunker: &mut Chunker<'_, N, S>,
    cur: &mut NodeCursor<N>,
    storage: &mut S,
) {
    debug_assert_eq!(
        chunker.level, 0,
        "fast_forward_to_end currently supports leaf-level chunkers only"
    );
    debug_assert!(
        chunker.is_at_boundary(),
        "fast_forward_to_end requires the chunker to be at a boundary"
    );

    // The cursor has just been advanced past the boundary leaf via
    // `advance`. If the advance crossed the parent's last child, the
    // cursor is now invalid - nothing more to fast-forward.
    while cur.valid() {
        // The current leaf is wholly unchanged (caller guaranteed no
        // pending mutations). Promote it to the parent chunker.
        let first_key = match cur.nd.keys.first() {
            Some(k) => k.clone(),
            None => return,
        };
        let leaf_hash = cur.nd.get_hash();
        chunker.append_subtree_at_parent_level(storage, first_key, leaf_hash.as_bytes().to_vec());

        // Jump to the start of the *next* leaf. `advance` would walk
        // item-by-item; we want to skip the whole leaf. Manually bump
        // the parent cursor by one (since the level-0 chunker has now
        // absorbed this leaf) and refetch.
        if cur.parent.is_none() {
            // The "leaf" is actually the root of a single-leaf tree.
            // There's nothing past it.
            cur.invalidate_at_end();
            return;
        }
        let parent = cur.parent.as_mut().expect("parent present");
        // Move parent.idx to the next child slot. `parent.idx` is
        // currently pointing at the just-emitted child.
        let next_idx = parent.idx + 1;
        if (next_idx as usize) >= parent.nd.values.len() {
            // No next child at this level. Try advancing the parent
            // to find a further sibling chunk further up the tree.
            parent.advance(storage);
            if parent.out_of_bounds() {
                cur.invalidate_at_end();
                return;
            }
            // Parent moved to a new internal node; fetch its first child.
            let child_hash_bytes = parent.nd.values[parent.idx as usize].clone();
            let child = storage
                .get_node_by_hash(&crate::digest::ValueDigest::raw_hash(&child_hash_bytes))
                .map(|arc| (*arc).clone())
                .expect("child reachable");
            cur.nd = child;
            cur.idx = 0;
            continue;
        }
        parent.idx = next_idx;
        let child_hash_bytes = parent.nd.values[parent.idx as usize].clone();
        let child = storage
            .get_node_by_hash(&crate::digest::ValueDigest::raw_hash(&child_hash_bytes))
            .map(|arc| (*arc).clone())
            .expect("child reachable");
        cur.nd = child;
        cur.idx = 0;
    }
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

    // -- apply_mutations (cursor-driven) tests --

    #[test]
    fn apply_mutations_against_empty_tree() {
        let cfg = TreeConfig::<32>::default();
        let mut storage = InMemoryNodeStorage::<32>::default();
        let empty = NodeBuilder::<32>::new(&cfg, 0).build();

        let muts: Vec<(Vec<u8>, Option<Vec<u8>>)> =
            (0..200u64).map(|i| (key(i), Some(val(i)))).collect();
        let root = apply_mutations(empty, muts, &cfg, &mut storage);

        let mut s2 = InMemoryNodeStorage::<32>::default();
        let pairs: Vec<_> = (0..200u64).map(|i| (key(i), val(i))).collect();
        let expected = build_tree_from_sorted_pairs::<32, _>(pairs, &cfg, &mut s2);

        assert_eq!(root.get_hash(), expected.get_hash());
    }

    #[test]
    fn apply_mutations_passes_through_unchanged_tree() {
        let cfg = TreeConfig::<32>::default();
        let mut storage = InMemoryNodeStorage::<32>::default();
        let pairs: Vec<_> = (0..500u64).map(|i| (key(i), val(i))).collect();
        let original = build_tree_from_sorted_pairs::<32, _>(pairs, &cfg, &mut storage);
        let original_hash = original.get_hash();

        let no_muts: Vec<(Vec<u8>, Option<Vec<u8>>)> = vec![];
        let result = apply_mutations(original, no_muts, &cfg, &mut storage);
        assert_eq!(result.get_hash(), original_hash);
    }

    #[test]
    fn apply_mutations_insert_into_middle() {
        let cfg = TreeConfig::<32>::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        let evens: Vec<_> = (0..100u64).map(|i| (key(2 * i), val(2 * i))).collect();
        let original = build_tree_from_sorted_pairs::<32, _>(evens, &cfg, &mut storage);

        let odds: Vec<(Vec<u8>, Option<Vec<u8>>)> = (0..100u64)
            .map(|i| (key(2 * i + 1), Some(val(2 * i + 1))))
            .collect();
        let result = apply_mutations(original, odds, &cfg, &mut storage);

        let mut s2 = InMemoryNodeStorage::<32>::default();
        let pairs: Vec<_> = (0..200u64).map(|i| (key(i), val(i))).collect();
        let expected = build_tree_from_sorted_pairs::<32, _>(pairs, &cfg, &mut s2);

        assert_eq!(result.get_hash(), expected.get_hash());
    }

    #[test]
    fn apply_mutations_delete_some_keys() {
        let cfg = TreeConfig::<32>::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        let all: Vec<_> = (0..200u64).map(|i| (key(i), val(i))).collect();
        let original = build_tree_from_sorted_pairs::<32, _>(all, &cfg, &mut storage);

        let dels: Vec<(Vec<u8>, Option<Vec<u8>>)> =
            (0..100u64).map(|i| (key(2 * i + 1), None)).collect();
        let result = apply_mutations(original, dels, &cfg, &mut storage);

        let mut s2 = InMemoryNodeStorage::<32>::default();
        let evens: Vec<_> = (0..100u64).map(|i| (key(2 * i), val(2 * i))).collect();
        let expected = build_tree_from_sorted_pairs::<32, _>(evens, &cfg, &mut s2);

        assert_eq!(result.get_hash(), expected.get_hash());
    }

    #[test]
    fn apply_mutations_update_some_values() {
        let cfg = TreeConfig::<32>::default();
        let mut storage = InMemoryNodeStorage::<32>::default();

        let pairs: Vec<_> = (0..200u64).map(|i| (key(i), val(i))).collect();
        let original = build_tree_from_sorted_pairs::<32, _>(pairs, &cfg, &mut storage);

        let updates: Vec<(Vec<u8>, Option<Vec<u8>>)> = (0..200u64)
            .map(|i| (key(i), Some(val(i + 1_000_000))))
            .collect();
        let result = apply_mutations(original, updates, &cfg, &mut storage);

        let mut s2 = InMemoryNodeStorage::<32>::default();
        let expected_pairs: Vec<_> = (0..200u64).map(|i| (key(i), val(i + 1_000_000))).collect();
        let expected = build_tree_from_sorted_pairs::<32, _>(expected_pairs, &cfg, &mut s2);

        assert_eq!(result.get_hash(), expected.get_hash());
    }
}
