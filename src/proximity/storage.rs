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

//! Bridge from [`ProximityNode`] to the existing [`NodeStorage<N>`] trait.
//!
//! The trait stores [`ProllyNode<N>`] values keyed by content hash. To reuse
//! every backend (in-memory, file, git, rocksdb) without modifying the trait,
//! each [`ProximityNode`] is wrapped in a single-entry [`ProllyNode`] whose
//! value is the bincode encoding of the proximity node. The storage key is
//! the **proximity node's** own content hash — backends trust caller-provided
//! hashes, so the wrapper's "natural" hash never matters.

use crate::node::ProllyNode;
use crate::proximity::index::ProximityError;
use crate::proximity::node::ProximityNode;

/// Sentinel key recognised by [`unwrap_proximity_node`]. A `0x01` prefix keeps
/// it out of typical user key ranges; the trailing `prox` is human-readable
/// when inspecting raw storage.
const PROX_SENTINEL: &[u8] = b"\x01prox";

/// Wrap a [`ProximityNode`] in a [`ProllyNode`] suitable for storage via
/// [`crate::storage::NodeStorage`].
pub(crate) fn wrap_proximity_node<const N: usize>(
    node: &ProximityNode<N>,
) -> Result<ProllyNode<N>, ProximityError> {
    let payload = bincode::serialize(node).map_err(|e| ProximityError::Serialize(e.to_string()))?;
    // Start from `Default` to inherit the chunking-config defaults (base,
    // modulus, pattern), then overwrite the fields we care about. Both
    // chunk-size bounds at usize::MAX make the rolling-hash chunker treat the
    // wrapper as un-splittable.
    Ok(ProllyNode {
        keys: vec![PROX_SENTINEL.to_vec()],
        values: vec![payload],
        is_leaf: true,
        level: 0,
        min_chunk_size: usize::MAX,
        max_chunk_size: usize::MAX,
        ..ProllyNode::default()
    })
}

/// Inverse of [`wrap_proximity_node`]. Returns `ProximityError::Corrupted`
/// when the wrapper doesn't have the expected shape.
pub(crate) fn unwrap_proximity_node<const N: usize>(
    wrapper: &ProllyNode<N>,
) -> Result<ProximityNode<N>, ProximityError> {
    if wrapper.values.len() != 1
        || wrapper.keys.first().map(|k| k.as_slice()) != Some(PROX_SENTINEL)
    {
        return Err(ProximityError::Corrupted(
            "wrapper is not a proximity node".into(),
        ));
    }
    bincode::deserialize::<ProximityNode<N>>(&wrapper.values[0])
        .map_err(|e| ProximityError::Corrupted(format!("bincode: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::ValueDigest;
    use crate::proximity::node::ProximityNode;

    #[test]
    fn wrap_unwrap_roundtrip() {
        let original = ProximityNode::<32>::new(
            1,
            vec![b"a".to_vec(), b"b".to_vec()],
            vec![vec![1.0, 2.0], vec![3.0, 4.0]],
            vec![ValueDigest::<32>::new(b"x"), ValueDigest::<32>::new(b"y")],
            2,
            0,
        );
        let wrapper = wrap_proximity_node(&original).unwrap();
        let restored = unwrap_proximity_node(&wrapper).unwrap();
        assert_eq!(original.level, restored.level);
        assert_eq!(original.ids, restored.ids);
        assert_eq!(original.vectors, restored.vectors);
        assert_eq!(original.child_hashes, restored.child_hashes);
        assert_eq!(original.dim, restored.dim);
        assert_eq!(original.metric_tag, restored.metric_tag);
        assert_eq!(original.get_hash(), restored.get_hash());
    }

    #[test]
    fn unwrap_rejects_non_wrapper() {
        let plain: ProllyNode<32> = ProllyNode::default();
        assert!(unwrap_proximity_node(&plain).is_err());
    }

    #[test]
    fn unwrap_rejects_wrong_sentinel() {
        let mut bad: ProllyNode<32> = ProllyNode::default();
        bad.keys = vec![b"wrong".to_vec()];
        bad.values = vec![vec![0, 1, 2]];
        assert!(unwrap_proximity_node::<32>(&bad).is_err());
    }
}
