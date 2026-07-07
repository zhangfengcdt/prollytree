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

use prollytree::config::TreeConfig;
use prollytree::node::{Node, ProllyNode};
use prollytree::storage::InMemoryNodeStorage;
use prollytree::tree::{ProllyTree, Tree};

fn large_modulus_config() -> TreeConfig<32> {
    TreeConfig::<32> {
        base: 4_000_000_007,
        modulus: 5_000_000_000,
        min_chunk_size: 2,
        max_chunk_size: 8,
        pattern: 0b11,
        ..TreeConfig::default()
    }
}

#[test]
fn build_canonical_large_modulus_does_not_overflow_and_preserves_keys() {
    let config = large_modulus_config();
    let pairs: Vec<_> = (0..64u64)
        .map(|i| (i.to_be_bytes().to_vec(), format!("value-{i}").into_bytes()))
        .collect();
    let mut storage = InMemoryNodeStorage::<32>::default();

    let root = ProllyNode::<32>::build_canonical_from_pairs(pairs.clone(), &config, &mut storage);

    for (key, value) in pairs {
        let leaf = root
            .find(&key, &storage)
            .expect("key should be retrievable");
        let index = leaf
            .keys
            .iter()
            .position(|stored_key| stored_key == &key)
            .expect("leaf should contain key");
        assert_eq!(leaf.values[index], value);
    }
}

#[test]
fn prolly_tree_large_modulus_does_not_overflow_and_preserves_keys() {
    let config = large_modulus_config();
    let mut tree = ProllyTree::new(InMemoryNodeStorage::<32>::default(), config);

    for i in 0..64u64 {
        tree.insert(i.to_be_bytes().to_vec(), format!("value-{i}").into_bytes());
    }

    for i in 0..64u64 {
        let key = i.to_be_bytes().to_vec();
        let leaf = tree.find(&key).expect("key should be retrievable");
        let index = leaf
            .keys
            .iter()
            .position(|stored_key| stored_key == &key)
            .expect("leaf should contain key");
        assert_eq!(leaf.values[index], format!("value-{i}").into_bytes());
    }
}
