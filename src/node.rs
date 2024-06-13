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
use crate::storage::NodeStorage;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::sync::{Arc, Mutex};

/// Represents a node in a prolly tree.
///
/// A prolly tree is a data structure used for efficient storage and retrieval of key-value pairs,
/// providing probabilistic balancing to ensure good performance characteristics.
///
/// # Type Parameters
///
/// * `N` - A constant parameter that determines the size of the `value_hash` array.
/// * `K` - The type of the key stored in the node. It must implement the `AsRef<[u8]>` trait.
///
#[derive(Clone)]
pub struct Node<const N: usize, K: AsRef<[u8]>> {
    /// The key associated with this node.
    pub(crate) key: K,

    /// A cryptographic hash of the value associated with this node.
    pub(crate) value_hash: ValueDigest<N>,

    /// An optional vector of cryptographic hashes representing the children nodes' contents.
    /// If the node is a leaf, this will be `None`.
    pub(crate) children_hash: Option<Vec<ValueDigest<N>>>,

    /// An optional cryptographic hash of the parent node's content.
    /// If the node is the root, this will be `None`.
    pub(crate) parent_hash: Option<ValueDigest<N>>,

    /// The level of the node in the tree. The root node starts at level 1.
    pub(crate) level: usize,

    /// A flag indicating whether the node is a leaf. Leaf nodes do not have children.
    pub(crate) is_leaf: bool,

    /// A vector of subtree counts. Each element represents the number of nodes in a corresponding
    /// subtree. If the node is a leaf, this will be `None`.
    pub(crate) subtree_counts: Option<Vec<usize>>,

    /// The storage instance used to retrieve nodes by their cryptographic hashes.
    /// This is a shared reference to a mutex-protected storage instance.
    pub(crate) storage: Arc<Mutex<dyn NodeStorage<N, K>>>,
}

// Manually implement Debug for NodeAlt
impl<const N: usize, K: AsRef<[u8]> + fmt::Debug> fmt::Debug for Node<N, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeAlt")
            .field("key", &self.key)
            .field("value_hash", &self.value_hash)
            .field("children_hash", &self.children_hash)
            .field("parent_hash", &self.parent_hash)
            .field("level", &self.level)
            .field("is_leaf", &self.is_leaf)
            .field("subtree_counts", &self.subtree_counts)
            .finish()
    }
}

impl<
        const N: usize,
        K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>> + Serialize + for<'de> Deserialize<'de>,
    > Node<N, K>
{
    // impl<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> Node<N, K> {
    /// Creates a new `NodeAlt` instance.
    ///
    /// # Arguments
    ///
    /// * `key` - The key associated with the node.
    /// * `value_hash` - The cryptographic hash of the value associated with the node.
    /// * `is_leaf` - A flag indicating whether the node is a leaf.
    ///
    /// # Returns
    ///
    /// A new `NodeAlt` instance.
    pub fn new_with_hash(
        key: K,
        value_hash: ValueDigest<N>,
        is_leaf: bool,
        storage: Arc<Mutex<dyn NodeStorage<N, K>>>,
    ) -> Self {
        Node {
            key,
            value_hash,
            children_hash: if is_leaf { None } else { Some(vec![]) },
            parent_hash: None,
            level: 1,
            is_leaf,
            subtree_counts: if is_leaf { None } else { Some(vec![]) },
            storage,
        }
    }

    /// Creates a new `NodeAlt` instance with a value.
    ///
    /// # Arguments
    ///
    /// * `key` - The key associated with the node.
    /// * `value` - The value associated with the node.
    /// * `is_leaf` - A flag indicating whether the node is a leaf.
    /// * `storage` - The storage backend for the node.
    ///
    /// # Returns
    ///
    /// A new `NodeAlt` instance.
    pub fn new<V>(
        key: K,
        value: V,
        is_leaf: bool,
        storage: Arc<Mutex<dyn NodeStorage<N, K>>>,
    ) -> Self
    where
        V: AsRef<[u8]>,
    {
        let value_hash = ValueDigest::new(value.as_ref());
        Node {
            key,
            value_hash,
            children_hash: if is_leaf { None } else { Some(vec![]) },
            parent_hash: None,
            level: 1,
            is_leaf,
            subtree_counts: if is_leaf { None } else { Some(vec![]) },
            storage,
        }
    }

    /// Inserts a new key-value pair into the tree, updating the necessary content addresses.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to insert.
    /// * `value_hash` - The cryptographic hash of the value to insert.
    pub fn insert(&mut self, key: K, value_hash: ValueDigest<N>) {
        if self.is_leaf {
            if self.children_hash.is_none() {
                self.children_hash = Some(vec![]);
            }

            let mut new_node =
                Node::new_with_hash(key.clone(), value_hash.clone(), true, self.storage.clone());
            new_node.parent_hash = Some(self.calculate_hash());
            let new_node_hash = new_node.calculate_hash();

            // Insert the new node into the storage
            self.insert_node(new_node);

            self.children_hash.as_mut().unwrap().push(new_node_hash);

            let mut child_hashes_with_keys: Vec<(ValueDigest<N>, K)> = self
                .children_hash
                .as_ref()
                .unwrap()
                .iter()
                .map(|child_hash| {
                    let child_node = self.get_node_by_hash(child_hash);
                    (child_hash.clone(), child_node.key.clone())
                })
                .collect();

            child_hashes_with_keys.sort_by(|a, b| a.1.as_ref().cmp(b.1.as_ref()));

            let sorted_child_hashes: Vec<ValueDigest<N>> = child_hashes_with_keys
                .into_iter()
                .map(|(child_hash, _)| child_hash)
                .collect();
            self.children_hash = Some(sorted_child_hashes);

            const MAX_CHILDREN: usize = 3;
            if self.children_hash.as_ref().unwrap().len() > MAX_CHILDREN {
                self.split();
            }
        } else {
            let mut updated_child_hashes = vec![];

            if let Some(children_hash) = self.children_hash.take() {
                for child_hash in children_hash {
                    let child_key = self.get_key_from_hash(&child_hash);
                    if key.as_ref() < child_key.as_ref() {
                        let mut child_node = self.get_node_by_hash(&child_hash);
                        child_node.insert(key.clone(), value_hash.clone());
                        updated_child_hashes.push(child_node.calculate_hash());
                        updated_child_hashes
                            .extend_from_slice(&self.children_hash.take().unwrap_or_default());
                        self.children_hash = Some(updated_child_hashes);
                        return;
                    } else {
                        updated_child_hashes.push(child_hash);
                    }
                }

                if let Some(last_child_hash) = updated_child_hashes.last_mut() {
                    let mut last_child_node = self.get_node_by_hash(last_child_hash);
                    last_child_node.insert(key.clone(), value_hash.clone());
                    *last_child_hash = last_child_node.calculate_hash();
                }

                self.children_hash = Some(updated_child_hashes);
            }
        }
    }

    /// Inserts a new key-value pair into the tree.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to insert.
    /// * `value` - The value to insert.
    pub fn insert_with_value<V>(&mut self, key: K, value: V)
    where
        V: AsRef<[u8]>,
    {
        let value_hash = ValueDigest::new(value.as_ref());
        self.insert(key, value_hash);
    }

    /// Updates the value associated with a key in the tree.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to update.
    /// * `value_hash` - The new cryptographic hash of the value to update.
    pub fn update(&mut self, key: K, value_hash: ValueDigest<N>) {
        if self.is_leaf {
            if self.key == key {
                self.value_hash = value_hash;
                self.insert_node(self.clone());
            }
        } else if let Some(children_hash) = self.children_hash.take() {
            let mut updated_child_hashes = vec![];

            for child_hash in children_hash.iter() {
                let mut child_node = self.get_node_by_hash(child_hash);
                if child_node.key == key {
                    child_node.update(key.clone(), value_hash.clone());
                    let updated_child_hash = child_node.calculate_hash();
                    self.insert_node(child_node);
                    updated_child_hashes.push(updated_child_hash);
                } else {
                    updated_child_hashes.push(child_hash.clone());
                }
            }

            self.children_hash = Some(updated_child_hashes);
        }
    }

    /// Updates the value associated with a key in the tree.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to update.
    /// * `value` - The new value to update.
    pub fn update_with_value<V>(&mut self, key: K, value: V)
    where
        V: AsRef<[u8]>,
    {
        let value_hash = ValueDigest::new(value.as_ref());
        self.update(key, value_hash);
    }

    /// Deletes a key-value pair from the tree.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to delete.
    pub fn delete(&mut self, key: &K) {
        if self.is_leaf {
            if self.key == *key {
                // Special case: if this is the root node and it matches the key, clear its key and value.
                self.key = "".as_bytes().to_vec().into();
                self.value_hash = ValueDigest::<N>::default();
            }
            if let Some(mut children_hash) = self.children_hash.take() {
                children_hash.retain(|child_hash| {
                    let retain = &self.get_key_from_hash(child_hash) != key;
                    if !retain {
                        self.delete_node(child_hash);
                    }
                    retain
                });
                self.children_hash = Some(children_hash);
            }
        } else if let Some(children_hash) = self.children_hash.take() {
            let mut updated_child_hashes = vec![];

            for child_hash in children_hash.iter() {
                if &self.get_key_from_hash(child_hash) == key {
                    self.delete_node(child_hash);
                } else {
                    let mut child_node = self.get_node_by_hash(child_hash);
                    child_node.delete(key);
                    updated_child_hashes.push(child_node.calculate_hash());
                }
            }

            self.children_hash = Some(updated_child_hashes);
        }
    }

    /// Searches for a key in the tree and returns the corresponding node if found.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to search for.
    ///
    /// # Returns
    ///
    /// An `Option` containing the node if found, or `None` if not found.
    pub fn search(&self, key: &K) -> Option<Node<N, K>> {
        if self.key == *key {
            return Some(self.clone());
        }
        if let Some(children_hash) = &self.children_hash {
            for child_hash in children_hash {
                let child_node = self.get_node_by_hash(child_hash);
                if let Some(result) = child_node.search(key) {
                    return Some(result);
                }
            }
        }
        None
    }

    /// Searches for a key in the tree and returns the corresponding node if found.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to search for.
    ///
    /// # Returns
    ///
    /// An `Option` containing the node if found, or `None` if not found.
    pub fn find(&self, key: &K) -> Option<Node<N, K>> {
        self.search(key)
    }

    /// Searches for a key in the tree and returns the corresponding node if found.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to search for.
    ///
    /// # Returns
    ///
    /// An `Option` containing the node if found, or `None` if not found.
    pub fn find_with_value<V>(&self, key: &K) -> Option<Node<N, K>>
    where
        V: AsRef<[u8]>,
    {
        self.find(key)
    }

    /// Splits the node if it has too many children.
    fn split(&mut self) {
        if let Some(mut children_hash) = self.children_hash.take() {
            let mid = children_hash.len() / 2;
            let right_children_hash = children_hash.split_off(mid);

            let promoted_key = self.get_key_from_hash(&right_children_hash[0]);
            let promoted_value_hash = self
                .get_node_by_hash(&right_children_hash[0])
                .value_hash
                .clone();

            let new_node = Node {
                key: promoted_key.clone(),
                value_hash: promoted_value_hash.clone(),
                children_hash: Some(right_children_hash.clone()),
                parent_hash: None,
                level: self.level,
                is_leaf: self.is_leaf,
                subtree_counts: self.subtree_counts.clone(),
                storage: self.storage.clone(),
            };

            for child_hash in right_children_hash.iter() {
                let mut child_node = self.get_node_by_hash(child_hash);
                child_node.parent_hash = Some(new_node.calculate_hash());
            }

            self.children_hash = Some(children_hash.clone());

            if let Some(parent_hash) = &self.parent_hash {
                let mut parent_node = self.get_node_by_hash(parent_hash);
                parent_node.insert_internal(new_node);
            } else {
                let mut new_root = Node::new_with_hash(
                    promoted_key,
                    promoted_value_hash,
                    false,
                    self.storage.clone(),
                );
                new_root.children_hash =
                    Some(vec![self.calculate_hash(), new_node.calculate_hash()]);
                new_root.level = self.level + 1;
                self.parent_hash = Some(new_root.calculate_hash());
            }
        }
    }

    /// Retrieves the parent node if it exists.
    ///
    /// # Returns
    ///
    /// An `Option` containing the parent node if it exists, or `None` if it does not.
    // fn get_parent(&self) -> Option<NodeAlt<N, K>> {
    //     self.parent_hash
    //         .as_ref()
    //         .map(|hash| self.get_node_by_hash(hash))
    // }

    /// Inserts an internal node into the current node.
    ///
    /// # Arguments
    ///
    /// * `new_node` - The new node to insert.
    fn insert_internal(&mut self, new_node: Node<N, K>) {
        if let Some(mut children_hash) = self.children_hash.take() {
            children_hash.push(new_node.calculate_hash());

            let mut child_hashes_with_keys: Vec<(ValueDigest<N>, K)> = children_hash
                .iter()
                .map(|child_hash| {
                    let child_node = self.get_node_by_hash(child_hash);
                    (child_hash.clone(), child_node.key.clone())
                })
                .collect();

            child_hashes_with_keys.sort_by(|a, b| a.1.as_ref().cmp(b.1.as_ref()));

            let sorted_child_hashes: Vec<ValueDigest<N>> = child_hashes_with_keys
                .into_iter()
                .map(|(child_hash, _)| child_hash)
                .collect();
            self.children_hash = Some(sorted_child_hashes);
        }
    }

    /// A placeholder for the method that calculates the hash of the node.
    ///
    /// # Returns
    ///
    /// The cryptographic hash of the node's content.
    pub(crate) fn calculate_hash(&self) -> ValueDigest<N> {
        let mut combined_data = Vec::new();

        // Append the key
        combined_data.extend_from_slice(self.key.as_ref());

        // Append the value hash
        combined_data.extend_from_slice(self.value_hash.as_bytes());

        // Append the children's hashes if they exist
        if let Some(children_hash) = &self.children_hash {
            for child_hash in children_hash {
                combined_data.extend_from_slice(child_hash.as_bytes());
            }
        }

        // Create a ValueDigest from the combined data
        ValueDigest::new(&combined_data)
    }

    pub fn get_key_from_hash(&self, hash: &ValueDigest<N>) -> K {
        let node = self.get_node_by_hash(hash);
        node.key.clone()
    }

    pub fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Node<N, K> {
        let storage = self.storage.lock().unwrap();
        storage.get_node_by_hash(hash)
    }

    pub fn insert_node(&self, node: Node<N, K>) {
        let hash = node.calculate_hash();
        let mut storage = self.storage.lock().unwrap();
        storage.insert_node(hash, node);
    }

    pub fn delete_node(&self, hash: &ValueDigest<N>) {
        let mut storage = self.storage.lock().unwrap();
        storage.delete_node(hash);
    }
}

// Manual implementation of Serialize
impl<const N: usize, K: AsRef<[u8]> + Serialize> Serialize for Node<N, K> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Node", 8)?;
        state.serialize_field("key", &self.key)?;
        state.serialize_field("value_hash", &self.value_hash)?;
        state.serialize_field("children_hash", &self.children_hash)?;
        state.serialize_field("parent_hash", &self.parent_hash)?;
        state.serialize_field("level", &self.level)?;
        state.serialize_field("is_leaf", &self.is_leaf)?;
        state.serialize_field("subtree_counts", &self.subtree_counts)?;
        state.end()
    }
}

// Manual implementation of Deserialize
impl<
        'de,
        const N: usize,
        K: AsRef<[u8]> + Deserialize<'de> + Clone + From<Vec<u8>> + PartialEq + 'static,
    > Deserialize<'de> for Node<N, K>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct NodeData<const N: usize, K> {
            key: K,
            value_hash: ValueDigest<N>,
            children_hash: Option<Vec<ValueDigest<N>>>,
            parent_hash: Option<ValueDigest<N>>,
            level: usize,
            is_leaf: bool,
            subtree_counts: Option<Vec<usize>>,
        }

        let data = NodeData::deserialize(deserializer)?;
        Ok(Node {
            key: data.key,
            value_hash: data.value_hash,
            children_hash: data.children_hash,
            parent_hash: data.parent_hash,
            level: data.level,
            is_leaf: data.is_leaf,
            subtree_counts: data.subtree_counts,
            storage: Arc::new(Mutex::new(crate::storage::HashMapNodeStorage::new())), // Placeholder for actual storage
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::HashMapNodeStorage;
    type KeyType = Vec<u8>;

    #[test]
    fn test_insert() {
        let storage = Arc::new(Mutex::new(HashMapNodeStorage::<32, Vec<u8>>::new()));
        let key: KeyType = "example_key".as_bytes().to_vec();
        let value = b"test data 1";
        let value_hash = ValueDigest::<32>::new(value);
        let mut root = Node::new_with_hash(key.clone(), value_hash.clone(), true, storage.clone());

        let new_key: KeyType = "new_key".as_bytes().to_vec();
        let new_value = b"test data 2";
        let new_value_hash = ValueDigest::<32>::new(new_value);
        root.insert(new_key.clone(), new_value_hash.clone());

        assert!(root.search(&new_key).is_some());
    }

    #[test]
    fn test_update() {
        let storage = Arc::new(Mutex::new(HashMapNodeStorage::<32, Vec<u8>>::new()));
        let key: KeyType = "example_key".as_bytes().to_vec();
        let value = b"test data 1";
        let value_hash = ValueDigest::<32>::new(value);
        let mut root = Node::new_with_hash(key.clone(), value_hash.clone(), true, storage.clone());

        let new_value = b"updated data";
        let new_value_hash = ValueDigest::<32>::new(new_value);
        root.update(key.clone(), new_value_hash.clone());

        let result = root.search(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value_hash, new_value_hash);
    }

    #[test]
    fn test_delete() {
        let storage = Arc::new(Mutex::new(HashMapNodeStorage::<32, Vec<u8>>::new()));
        let key: KeyType = "example_key".as_bytes().to_vec();
        let value = b"test data 1";
        let value_hash = ValueDigest::<32>::new(value);
        let mut root = Node::new_with_hash(key.clone(), value_hash.clone(), true, storage.clone());

        root.delete(&key);

        assert!(root.search(&key).is_none());
    }

    #[test]
    fn test_search() {
        let storage = Arc::new(Mutex::new(HashMapNodeStorage::<32, Vec<u8>>::new()));
        let key: KeyType = "example_key".as_bytes().to_vec();
        let value = b"test data 1";
        let value_hash = ValueDigest::<32>::new(value);
        let root = Node::new_with_hash(key.clone(), value_hash.clone(), true, storage.clone());

        let result = root.search(&key);

        assert!(result.is_some());
        assert_eq!(result.unwrap().key, key);
    }
}
