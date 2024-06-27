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

use crate::node::{Node, ProllyNode};
use crate::storage::NodeStorage;

pub trait ProllyTreeTrait<const N: usize, S: NodeStorage<N>> {
    fn new(root: ProllyNode<N>, storage: S) -> Self;
    fn insert(&mut self, key: Vec<u8>, value: Vec<u8>);
    fn delete(&mut self, key: &[u8]) -> bool;
    fn find(&self, key: &[u8]) -> Option<ProllyNode<N>>;
    fn traverse(&self) -> String;
    fn formatted_traverse<F>(&self, formatter: F) -> String
    where
        F: Fn(&ProllyNode<N>) -> String;
}

pub struct ProllyTree<const N: usize, S: NodeStorage<N>> {
    root: ProllyNode<N>,
    storage: S,
}

impl<const N: usize, S: NodeStorage<N>> ProllyTreeTrait<N, S> for ProllyTree<N, S> {
    fn new(root: ProllyNode<N>, storage: S) -> Self {
        ProllyTree { root, storage }
    }

    fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.root.insert(key, value, &mut self.storage, None);
    }

    fn delete(&mut self, key: &[u8]) -> bool {
        self.root.delete(key, &mut self.storage, None)
    }

    fn find(&self, key: &[u8]) -> Option<ProllyNode<N>> {
        self.root.find(key, &self.storage)
    }

    fn traverse(&self) -> String {
        self.root.traverse(&self.storage)
    }

    fn formatted_traverse<F>(&self, formatter: F) -> String
    where
        F: Fn(&ProllyNode<N>) -> String,
    {
        self.root.formatted_traverse(&self.storage, formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::HashMapNodeStorage;

    #[test]
    fn test_insert_and_find() {
        let storage = HashMapNodeStorage::<32>::new();

        let root = ProllyNode::default();
        let mut tree = ProllyTree::new(root, storage);

        tree.insert(b"key1".to_vec(), b"value1".to_vec());
        tree.insert(b"key2".to_vec(), b"value2".to_vec());

        assert!(tree.find(b"key1").is_some());
        assert!(tree.find(b"key2").is_some());
        assert!(tree.find(b"key3").is_none());
    }

    #[test]
    fn test_delete() {
        let storage = HashMapNodeStorage::<32>::new();
        let root = ProllyNode::default();
        let mut tree = ProllyTree::new(root, storage);

        tree.insert(b"key1".to_vec(), b"value1".to_vec());
        tree.insert(b"key2".to_vec(), b"value2".to_vec());

        assert!(tree.delete(b"key1"));
        assert!(tree.find(b"key1").is_none());
        assert!(tree.find(b"key2").is_some());
    }

    #[test]
    fn test_traverse() {
        let storage = HashMapNodeStorage::<32>::new();
        let root = ProllyNode::default();
        let mut tree = ProllyTree::new(root, storage);

        let key1 = b"key1".to_vec();
        let key2 = b"key2".to_vec();

        tree.insert(key1.clone(), b"value1".to_vec());
        tree.insert(key2.clone(), b"value2".to_vec());

        let traversal = tree.traverse();

        // Convert byte arrays to their binary representation strings for comparison
        let expected_key1 = format!("{:?}", key1);
        let expected_key2 = format!("{:?}", key2);

        // Check if the traversal contains the expected keys
        assert!(traversal.contains(&expected_key1.to_string()));
        assert!(traversal.contains(&expected_key2.to_string()));
    }
}
