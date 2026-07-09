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

//! Regression: `generate_proof` / `verify` must not panic on an underfull
//! internal node.
//!
//! Both indexed `node.values[i]` on an internal node without the empty-check +
//! `i.min(len-1)` clamp that `ProllyNode::find` already applies, so an internal
//! node left underfull by a delete pattern (fewer values than keys, or none)
//! panicked with an out-of-bounds index. The fix mirrors `find`'s guard.

use prollytree::config::TreeConfig;
use prollytree::storage::InMemoryNodeStorage;
use prollytree::tree::{ProllyTree, Tree};

fn splitty_config() -> TreeConfig<32> {
    TreeConfig {
        min_chunk_size: 2,
        max_chunk_size: 64,
        pattern: 0b1111,
        ..TreeConfig::<32>::default()
    }
}

/// Empty-values internal node: pre-fix `generate_proof` panicked on `values[0]`.
#[test]
fn bug_proof_on_empty_internal_node_does_not_panic() {
    let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), splitty_config());
    for i in 0..500u32 {
        tree.insert(format!("key-{i:08}").into_bytes(), format!("v{i}").into_bytes());
    }
    assert!(!tree.root.is_leaf, "test needs a multi-level tree (internal root)");

    // Simulate the transient state a delete can leave.
    tree.root.values.clear();

    let proof = tree.generate_proof(b"key-00000042");
    let _ = tree.verify(proof, b"key-00000042", None);
}

/// Underfull (keys.len() > values.len()): `rposition` can return an index that
/// is out of bounds for the shortened `values` vec.
#[test]
fn bug_proof_on_underfull_internal_node_does_not_panic() {
    let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), splitty_config());
    for i in 0..500u32 {
        tree.insert(format!("key-{i:08}").into_bytes(), format!("v{i}").into_bytes());
    }
    assert!(!tree.root.is_leaf, "test needs a multi-level tree (internal root)");

    tree.root.values.pop();

    let proof = tree.generate_proof(b"key-99999999");
    let _ = tree.verify(proof, b"key-99999999", None);
}

/// Positive guard: the fix must not break proofs on an intact multi-level tree.
#[test]
fn proof_roundtrip_multilevel() {
    let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), splitty_config());
    for i in 0..500u32 {
        tree.insert(format!("key-{i:08}").into_bytes(), format!("v{i}").into_bytes());
    }
    let key = b"key-00000042";
    let proof = tree.generate_proof(key);
    assert!(tree.verify(proof, key, None), "valid proof for a present key must verify");

    let absent = b"key-does-not-exist";
    let proof = tree.generate_proof(absent);
    assert!(!tree.verify(proof, absent, None), "absent key must not verify");
}
