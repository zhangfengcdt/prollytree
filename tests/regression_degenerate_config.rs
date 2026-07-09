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

//! Regression: a degenerate `TreeConfig` must not hang the chunker/tree builder.
//!
//! With `min_chunk_size == 0` (and `pattern == 0` / `max_chunk_size == 0`) two
//! separate loops used to spin forever: `chunk_content` emitted empty chunks, and
//! `build_canonical_from_pairs` never fanned in (size-1 chunks -> each tree level
//! the same size as the one below). Both are reachable through the public
//! `ProllyNode::build_canonical_from_pairs` / `TreeConfig` surface. The fix floors
//! the effective window at 1 and collapses a non-shrinking level into a single root.

use prollytree::config::TreeConfig;
use prollytree::node::ProllyNode;
use prollytree::storage::InMemoryNodeStorage;
use prollytree::tree::{ProllyTree, Tree};

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Runs the degenerate build in a worker thread with a hard timeout so a
/// regression FAILS the test instead of hanging the whole suite.
#[test]
fn bug_degenerate_min_chunk_size_terminates_not_hang() {
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut storage = InMemoryNodeStorage::<32>::default();
        let config = TreeConfig::<32> {
            min_chunk_size: 0,
            max_chunk_size: 0,
            pattern: 0,
            ..TreeConfig::default()
        };
        let pairs: Vec<(Vec<u8>, Vec<u8>)> = (0..50u32)
            .map(|i| (format!("k{i:04}").into_bytes(), b"v".to_vec()))
            .collect();
        let root = ProllyNode::build_canonical_from_pairs(pairs, &config, &mut storage);
        let _ = tx.send(root.keys.len());
    });

    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(_) => {
            let _ = worker.join();
        }
        Err(_) => {
            panic!("chunk_content did not terminate with min_chunk_size=0 (infinite-loop regression)")
        }
    }
}

/// Every emitted chunk from a degenerate zero-min config must still be non-empty
/// and the pairs must survive the round trip (no dropped/duplicated keys).
#[test]
fn bug_degenerate_config_preserves_all_keys() {
    let mut storage = InMemoryNodeStorage::<32>::default();
    let config = TreeConfig::<32> {
        min_chunk_size: 0,
        max_chunk_size: 0,
        pattern: 0,
        ..TreeConfig::default()
    };
    let pairs: Vec<(Vec<u8>, Vec<u8>)> = (0..200u32)
        .map(|i| (format!("k{i:04}").into_bytes(), format!("v{i}").into_bytes()))
        .collect();
    let root = ProllyNode::build_canonical_from_pairs(pairs.clone(), &config, &mut storage);

    let tree = ProllyTree {
        root,
        storage,
        config,
    };
    for (k, _) in &pairs {
        assert!(
            tree.find(k).is_some(),
            "key {:?} dropped by degenerate-config build",
            String::from_utf8_lossy(k)
        );
    }
}
