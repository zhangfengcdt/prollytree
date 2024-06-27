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
use crate::node::{Node, ProllyNode};
use crate::storage::NodeStorage;

pub trait Tree<const N: usize, S: NodeStorage<N>> {
    fn new(root: ProllyNode<N>, storage: S) -> Self;
    fn insert(&mut self, key: Vec<u8>, value: Vec<u8>);
    fn update(&mut self, key: Vec<u8>, value: Vec<u8>) -> bool;
    fn delete(&mut self, key: &[u8]) -> bool;
    fn find(&self, key: &[u8]) -> Option<ProllyNode<N>>;
    fn traverse(&self) -> String;
    fn formatted_traverse<F>(&self, formatter: F) -> String
    where
        F: Fn(&ProllyNode<N>) -> String;
    fn get_root_hash(&self) -> Option<ValueDigest<N>>;
    fn size(&self) -> usize;
    fn depth(&self) -> usize;
    fn summary(&self) -> String;
    fn stats(&self) -> TreeStats;
}

pub struct TreeStats {
    pub num_nodes: usize,
    pub num_leaves: usize,
    pub num_internal_nodes: usize,
    pub max_depth: usize,
    pub avg_node_size: f64,
    pub std_node_size: f64,
    pub min_node_size: f64,
    pub max_node_size: f64,
}

impl TreeStats {
    pub fn new() -> Self {
        TreeStats {
            num_nodes: 0,
            num_leaves: 0,
            num_internal_nodes: 0,
            max_depth: 0,
            avg_node_size: 0.0,
            std_node_size: 0.0,
            min_node_size: 0.0,
            max_node_size: 0.0,
        }
    }
}

impl Default for TreeStats {
    fn default() -> Self {
        TreeStats::new()
    }
}

pub struct ProllyTree<const N: usize, S: NodeStorage<N>> {
    root: ProllyNode<N>,
    storage: S,
}

impl<const N: usize, S: NodeStorage<N>> Tree<N, S> for ProllyTree<N, S> {
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

    fn get_root_hash(&self) -> Option<ValueDigest<N>> {
        todo!()
    }

    fn update(&mut self, key: Vec<u8>, value: Vec<u8>) -> bool {
        if self.find(&key).is_some() {
            self.insert(key, value);
            true
        } else {
            false
        }
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn depth(&self) -> usize {
        todo!()
    }

    fn summary(&self) -> String {
        todo!()
    }

    fn stats(&self) -> TreeStats {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryNodeStorage;

    #[test]
    fn test_insert_and_find() {
        let storage = InMemoryNodeStorage::<32>::new();

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
        let storage = InMemoryNodeStorage::<32>::new();
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
        let storage = InMemoryNodeStorage::<32>::new();
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
