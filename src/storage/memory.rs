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
use crate::node::ProllyNode;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use super::NodeStorage;

/// An implementation of `NodeStorage` that stores nodes in a HashMap.
///
/// # Type Parameters
///
/// - `N`: The size of the value digest.
#[derive(Debug)]
pub struct InMemoryNodeStorage<const N: usize> {
    map: HashMap<ValueDigest<N>, Arc<ProllyNode<N>>>,
    configs: RwLock<HashMap<String, Vec<u8>>>,
}

impl<const N: usize> Clone for InMemoryNodeStorage<N> {
    fn clone(&self) -> Self {
        InMemoryNodeStorage {
            map: self.map.clone(),
            configs: RwLock::new(self.configs.read().clone()),
        }
    }
}

impl<const N: usize> Default for InMemoryNodeStorage<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> InMemoryNodeStorage<N> {
    pub fn new() -> Self {
        InMemoryNodeStorage {
            map: HashMap::new(),
            configs: RwLock::new(HashMap::new()),
        }
    }
}

impl<const N: usize> NodeStorage<N> for InMemoryNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<Arc<ProllyNode<N>>> {
        self.map.get(hash).cloned()
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, mut node: ProllyNode<N>) -> Option<()> {
        // Clear transient flags before storing so reads never see stale state.
        node.split = false;
        node.merged = false;
        self.map.insert(hash, Arc::new(node));
        Some(())
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Option<()> {
        self.map.remove(hash);
        Some(())
    }

    fn save_config(&self, key: &str, config: &[u8]) {
        self.configs
            .write()
            .insert(key.to_string(), config.to_vec());
    }

    fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        self.configs.read().get(key).cloned()
    }
}
