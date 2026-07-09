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
use prollytree::digest::ValueDigest;
use prollytree::storage::{InMemoryNodeStorage, NodeStorage};
use prollytree::tree::{ProllyTree, Tree};
use sha2::{Digest, Sha256};

fn get<const N: usize, S: NodeStorage<N>>(tree: &ProllyTree<N, S>, key: &[u8]) -> Option<Vec<u8>> {
    tree.find(key).and_then(|node| {
        node.keys
            .iter()
            .zip(node.values.iter())
            .find(|(stored_key, _)| stored_key.as_slice() == key)
            .map(|(_, value)| value.clone())
    })
}

fn splitty_config<const N: usize>() -> TreeConfig<N> {
    TreeConfig {
        base: 257,
        modulus: 1_000_000_007,
        min_chunk_size: 2,
        max_chunk_size: 64,
        pattern: 0b1111,
        ..TreeConfig::<N>::default()
    }
}

fn roundtrip<const N: usize>() {
    let mut tree = ProllyTree::new(InMemoryNodeStorage::<N>::default(), splitty_config::<N>());

    for i in 0..256u32 {
        tree.insert(
            format!("key-{i:08}").into_bytes(),
            format!("value-{i}").into_bytes(),
        );
    }

    let root = tree.get_root_hash().expect("tree must have a root hash");
    assert_eq!(root.as_bytes().len(), N);

    for i in 0..256u32 {
        let key = format!("key-{i:08}").into_bytes();
        assert_eq!(get(&tree, &key), Some(format!("value-{i}").into_bytes()));
    }

    let update_key = b"key-00000042".to_vec();
    assert!(tree.update(update_key.clone(), b"UPDATED".to_vec()));
    assert_eq!(get(&tree, &update_key), Some(b"UPDATED".to_vec()));

    let delete_key = b"key-00000100".to_vec();
    assert!(tree.delete(&delete_key));
    assert_eq!(get(&tree, &delete_key), None);

    tree.persist_root();
    let reloaded = ProllyTree::load_from_storage(tree.storage.clone(), tree.config.clone())
        .expect("reload from storage must succeed");
    assert_eq!(get(&reloaded, &update_key), Some(b"UPDATED".to_vec()));
    assert_eq!(get(&reloaded, &delete_key), None);
}

fn history_independent<const N: usize>() {
    let ascending: Vec<u32> = (0..256).collect();
    let mut shuffled: Vec<u32> = (0..256).step_by(2).chain((1..256).step_by(2)).collect();
    shuffled.rotate_left(37);

    let build = |order: &[u32]| -> ValueDigest<N> {
        let mut tree = ProllyTree::new(InMemoryNodeStorage::<N>::default(), splitty_config::<N>());
        for &i in order {
            tree.insert(
                format!("key-{i:08}").into_bytes(),
                format!("value-{i}").into_bytes(),
            );
        }
        tree.get_root_hash().expect("root hash")
    };

    assert_eq!(build(&ascending), build(&shuffled));
}

#[test]
fn n32_digest_is_unchanged_sha256() {
    let data = b"prollytree canonical hash pin";
    let expected = Sha256::digest(data);

    assert_eq!(ValueDigest::<32>::new(data).as_bytes(), &expected[..]);
}

#[test]
fn wider_digest_extends_narrower_prefix() {
    let data = b"some content to hash";
    let d32 = ValueDigest::<32>::new(data);
    let d64 = ValueDigest::<64>::new(data);

    assert_eq!(&d64.as_bytes()[..32], d32.as_bytes());
}

#[test]
fn roundtrip_n16() {
    roundtrip::<16>();
}

#[test]
fn roundtrip_n32() {
    roundtrip::<32>();
}

#[test]
fn roundtrip_n48() {
    roundtrip::<48>();
}

#[test]
fn roundtrip_n64() {
    roundtrip::<64>();
}

#[test]
fn history_independence_n64() {
    history_independent::<64>();
}
