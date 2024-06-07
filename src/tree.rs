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

#![allow(dead_code)]
use sha2::digest::{FixedOutputReset, HashMarker};
use sha2::Sha256;
use std::marker::PhantomData;

use crate::digest::ValueDigest;
use crate::page::Page;

/// Represents a prolly tree with probabilistic balancing.
/// The tree is designed to be efficient and support operations like insertion,
/// deletion, and balancing, which maintain the probabilistic properties of the tree.
#[derive(Debug, Clone)]
pub struct ProllyTree<const N: usize, K: AsRef<[u8]>, V, H = Sha256> {
    root: Page<N, K>,
    root_hash: Option<Vec<u8>>,
    _value_type: PhantomData<V>,
    hasher: H,
}

impl<const N: usize, K, V, H> Default for ProllyTree<N, K, V, H>
where
    K: Ord + Clone + AsRef<[u8]>,
    V: Clone + AsRef<[u8]>,
    H: Default + FixedOutputReset + HashMarker,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, K, V, H> ProllyTree<N, K, V, H>
where
    K: Ord + Clone + AsRef<[u8]>,
    V: Clone + AsRef<[u8]>,
    H: Default + FixedOutputReset + HashMarker,
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
            self.root_hash = Some(self.root.calculate_hash(&mut self.hasher));
        }
        &self.root_hash
    }
}
