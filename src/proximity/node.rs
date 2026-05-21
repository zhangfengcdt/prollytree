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

//! Node type for the proximity (vector) index.

use crate::digest::ValueDigest;
use serde::{Deserialize, Serialize};

/// One bucket in the proximity tree.
///
/// * Leaf nodes (`level == 0`) hold actual `(id, vector)` entries and have an
///   empty `child_hashes` vector.
/// * Internal nodes (`level > 0`) hold representative vectors and the content
///   hashes of their level-`(level - 1)` children. The invariant is that every
///   `(id, vector)` appearing at an internal node also appears in the child
///   that its representative descends to — so descent paths to any leaf are
///   always valid.
///
/// All three parallel vectors (`ids`, `vectors`, `child_hashes`) are kept in
/// lock-step. For a leaf node, `ids[i]` pairs with `vectors[i]`; for an
/// internal node, `ids[i]` / `vectors[i]` is the representative of the subtree
/// rooted at `child_hashes[i]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProximityNode<const N: usize> {
    /// Tree level (0 = leaf).
    pub level: u8,
    /// Entry / representative ids, sorted ascending by byte value for
    /// determinism — two indexes built from the same `(id, vector)` set must
    /// produce identical node serializations regardless of insertion order.
    pub ids: Vec<Vec<u8>>,
    /// Entry vectors (leaf) or representatives (internal). Same length as `ids`.
    pub vectors: Vec<Vec<f32>>,
    /// Content hashes of children, parallel to `ids`/`vectors`. Empty when
    /// this node is a leaf.
    pub child_hashes: Vec<ValueDigest<N>>,
    /// Vector dimensionality. Stored on every node as a sanity check so a
    /// dimension-mismatched query fails fast at the root.
    pub dim: u16,
    /// Distance-metric tag (see [`crate::proximity::Metric::from_tag`]).
    /// Stored so a query at any historical commit can be validated against
    /// the index's original metric.
    pub metric_tag: u8,
}

impl<const N: usize> ProximityNode<N> {
    /// Build a fresh node.
    pub fn new(
        level: u8,
        ids: Vec<Vec<u8>>,
        vectors: Vec<Vec<f32>>,
        child_hashes: Vec<ValueDigest<N>>,
        dim: u16,
        metric_tag: u8,
    ) -> Self {
        Self {
            level,
            ids,
            vectors,
            child_hashes,
            dim,
            metric_tag,
        }
    }

    /// True when this is a leaf (level 0) node.
    pub fn is_leaf(&self) -> bool {
        self.level == 0
    }

    /// Number of entries (or representatives, for internal nodes).
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// True when the node has no entries.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Content hash over the canonical bincode encoding of the node. The
    /// digest is what `NodeStorage` will index this node under in later PRs.
    pub fn get_hash(&self) -> ValueDigest<N> {
        let bytes = bincode::serialize(self).expect("ProximityNode bincode serialize");
        ValueDigest::new(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaf_predicate() {
        let n = ProximityNode::<32>::new(0, vec![b"a".to_vec()], vec![vec![1.0]], vec![], 1, 0);
        assert!(n.is_leaf());
        let n = ProximityNode::<32>::new(
            1,
            vec![b"a".to_vec()],
            vec![vec![1.0]],
            vec![ValueDigest::<32>::new(b"x")],
            1,
            0,
        );
        assert!(!n.is_leaf());
    }

    #[test]
    fn deterministic_hash_independent_of_clone() {
        let a = ProximityNode::<32>::new(
            0,
            vec![b"a".to_vec(), b"b".to_vec()],
            vec![vec![1.0, 2.0], vec![3.0, 4.0]],
            vec![],
            2,
            1,
        );
        let b = a.clone();
        assert_eq!(a.get_hash(), b.get_hash());
    }

    #[test]
    fn different_content_different_hash() {
        let a = ProximityNode::<32>::new(0, vec![b"a".to_vec()], vec![vec![1.0]], vec![], 1, 0);
        let b = ProximityNode::<32>::new(0, vec![b"a".to_vec()], vec![vec![2.0]], vec![], 1, 0);
        assert_ne!(a.get_hash(), b.get_hash());
    }

    #[test]
    fn bincode_roundtrip() {
        let original = ProximityNode::<32>::new(
            2,
            vec![b"x".to_vec(), b"y".to_vec()],
            vec![vec![0.1, 0.2], vec![0.3, 0.4]],
            vec![ValueDigest::<32>::new(b"c0"), ValueDigest::<32>::new(b"c1")],
            2,
            1,
        );
        let bytes = bincode::serialize(&original).unwrap();
        let restored: ProximityNode<32> = bincode::deserialize(&bytes).unwrap();
        assert_eq!(original.level, restored.level);
        assert_eq!(original.ids, restored.ids);
        assert_eq!(original.vectors, restored.vectors);
        assert_eq!(original.child_hashes, restored.child_hashes);
        assert_eq!(original.dim, restored.dim);
        assert_eq!(original.metric_tag, restored.metric_tag);
        assert_eq!(original.get_hash(), restored.get_hash());
    }
}
