use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

use crate::digest::ValueDigest;
use crate::node2::NodeAlt;


// Define the NodeStorage trait
pub trait NodeStorage<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> NodeAlt<N, K>;
    fn insert_node(&mut self, hash: ValueDigest<N>, node: NodeAlt<N, K>);
    fn delete_node(&mut self, hash: &ValueDigest<N>);
}

// Implement the trait for HashMap storage
#[derive(Debug, Clone)]
pub struct HashMapNodeStorage<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> {
    map: HashMap<ValueDigest<N>, NodeAlt<N, K>>,
}

impl<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> HashMapNodeStorage<N, K> {
    pub fn new() -> Self {
        HashMapNodeStorage {
            map: HashMap::new(),
        }
    }
}

impl<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>> + 'static> NodeStorage<N, K> for HashMapNodeStorage<N, K> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> NodeAlt<N, K> {
        self.map.get(hash).cloned().unwrap_or_else(|| {
            // Create a default node if the hash is not found
            NodeAlt {
                key: Vec::new().into(),
                value_hash: ValueDigest::<N>([0; N]),
                children_hash: None,
                parent_hash: None,
                level: 0,
                is_leaf: true,
                subtree_counts: None,
                storage: Arc::new(Mutex::new(self.clone())),
            }
        })
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: NodeAlt<N, K>) {
        self.map.insert(hash, node);
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) {
        self.map.remove(hash);
    }
}

// Implement the trait for File System storage
#[derive(Serialize, Deserialize)]
struct SerializableNode<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> {
    key: K,
    value_hash: ValueDigest<N>,
    children_hash: Option<Vec<ValueDigest<N>>>,
    parent_hash: Option<ValueDigest<N>>,
    level: usize,
    is_leaf: bool,
    subtree_counts: Option<Vec<usize>>,
}
