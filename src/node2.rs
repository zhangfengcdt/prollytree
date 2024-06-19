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

use crate::digest::ValueDigest;
use crate::storage2::NodeStorage2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Node2<const N: usize> {
    keys: Vec<u8>,
    values: Vec<u8>,
    is_leaf: bool,
    level: u8,
}

impl<const N: usize> Node2<N> {
    pub fn new(keys: Vec<u8>, values: Vec<u8>, is_leaf: bool, level: u8) -> Self {
        Node2 {
            keys,
            values,
            is_leaf,
            level,
        }
    }

    pub fn update_values<S: NodeStorage2<N>>(
        &mut self,
        new_values: Vec<u8>,
        storage: &mut S,
        hash: ValueDigest<N>,
    ) {
        self.values = new_values;
        self.save_to_storage(storage, hash);
    }

    pub fn update_keys<S: NodeStorage2<N>>(
        &mut self,
        new_keys: Vec<u8>,
        storage: &mut S,
        hash: ValueDigest<N>,
    ) {
        self.keys = new_keys;
        self.save_to_storage(storage, hash);
    }

    pub fn save_to_storage<S: NodeStorage2<N>>(&self, storage: &mut S, hash: ValueDigest<N>) {
        storage.insert_node(hash, self.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage2::HashMapNodeStorage2;

    #[test]
    fn test_update_values() {
        let mut storage = HashMapNodeStorage2::<32>::new();
        let mut node = Node2::new(vec![1, 2, 3], vec![4, 5, 6], true, 1);
        let hash = ValueDigest::new(b"test_hash");

        node.update_values(vec![7, 8, 9], &mut storage, hash.clone());

        let retrieved_node = storage.get_node_by_hash(&hash);
        assert_eq!(retrieved_node.values, vec![7, 8, 9]);
    }

    #[test]
    fn test_update_keys() {
        let mut storage = HashMapNodeStorage2::<32>::new();
        let mut node = Node2::new(vec![1, 2, 3], vec![4, 5, 6], true, 1);
        let hash = ValueDigest::new(b"test_hash");

        node.update_keys(vec![7, 8, 9], &mut storage, hash.clone());

        let retrieved_node = storage.get_node_by_hash(&hash);
        assert_eq!(retrieved_node.keys, vec![7, 8, 9]);
    }

    #[test]
    fn test_save_to_storage() {
        let mut storage = HashMapNodeStorage2::<32>::new();
        let node = Node2::new(vec![1, 2, 3], vec![4, 5, 6], true, 1);
        let hash = ValueDigest::new(b"test_hash");

        node.save_to_storage(&mut storage, hash.clone());

        let retrieved_node = storage.get_node_by_hash(&hash);
        assert_eq!(retrieved_node.keys, node.keys);
        assert_eq!(retrieved_node.values, node.values);
        assert_eq!(retrieved_node.is_leaf, node.is_leaf);
        assert_eq!(retrieved_node.level, node.level);
    }
}
