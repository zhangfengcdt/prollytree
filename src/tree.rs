#![allow(dead_code)]
use sha2::Digest;
use sha2::Sha256;
use std::marker::PhantomData;

use crate::page::Page;
use crate::value_digest::ValueDigest;

/// Represents the default hash function used in the ProllyTree.
#[derive(Default)]
pub struct DefaultHasher;

impl DefaultHasher {
    /// Hash a given value and return the resulting digest.
    pub fn hash<T: AsRef<[u8]>>(&self, value: T) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(value.as_ref());
        let result = hasher.finalize();

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result[..32]);
        hash
    }
}

/// Represents a prolly tree with probabilistic balancing.
#[derive(Debug, Clone)]
pub struct ProllyTree<const N: usize, K, V, H = DefaultHasher> {
    root: Page<N, K>,
    root_hash: Option<Vec<u8>>,
    _value_type: PhantomData<V>,
    hasher: H,
}

impl<const N: usize, K, V, H> ProllyTree<N, K, V, H>
where
    K: Ord + Clone,
    V: Clone + AsRef<[u8]>,
    H: Default,
{
    pub fn new() -> Self {
        ProllyTree {
            root: Page::new(0),
            root_hash: None,
            _value_type: PhantomData,
            hasher: H::default(),
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        let value_hash = ValueDigest::new(value.as_ref());
        self.root.insert(key, value_hash);
        self.root_hash = None; // Invalidate the cached root hash
    }

    pub fn delete(&mut self, key: &K) -> bool {
        let result = self.root.delete(key);
        if result {
            self.root_hash = None; // Invalidate the cached root hash
        }
        result
    }

    pub fn root_hash(&mut self) -> &Option<Vec<u8>> {
        if self.root_hash.is_none() {
            self.root_hash = Some(self.root.calculate_hash());
        }
        &self.root_hash
    }
}
