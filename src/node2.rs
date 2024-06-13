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
use std::cell::RefCell;
use std::rc::{Rc, Weak};

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
#[derive(Debug, Clone)]
pub struct NodeAlt<const N: usize, K: AsRef<[u8]>> {
    /// The key associated with this node.
    key: K,

    /// A cryptographic hash of the value associated with this node.
    value_hash: ValueDigest<N>,

    /// A vector of child nodes. If the node is a leaf, this will be `None`.
    children: Option<Vec<Rc<RefCell<NodeAlt<N, K>>>>>,

    /// A weak reference to the parent node. This allows for traversal back up the tree without creating
    /// reference cycles. If the node is the root, this will be `None`.
    parent: Option<Weak<RefCell<NodeAlt<N, K>>>>,

    /// The level of the node in the tree. The root node starts at level 1.
    level: usize,

    /// A flag indicating whether the node is a leaf. Leaf nodes do not have children.
    is_leaf: bool,

    /// A vector of subtree counts. Each element represents the number of nodes in a corresponding
    /// subtree. If the node is a leaf, this will be `None`.
    subtree_counts: Option<Vec<usize>>,
}

impl<const N: usize, K: AsRef<[u8]> + Clone + PartialEq + From<Vec<u8>>> NodeAlt<N, K> {
    pub fn new(key: K, value_hash: ValueDigest<N>, is_leaf: bool) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(NodeAlt {
            key,
            value_hash,
            children: if is_leaf { None } else { Some(vec![]) },
            parent: None,
            level: 1,
            is_leaf,
            subtree_counts: if is_leaf { None } else { Some(vec![]) },
        }))
    }

    pub fn insert(&mut self, key: K, value_hash: ValueDigest<N>) {
        if self.is_leaf {
            if self.children.is_none() {
                self.children = Some(vec![]);
            }

            let new_node = NodeAlt::new(key, value_hash, true);
            let parent = Rc::downgrade(&Rc::new(RefCell::new(self.clone())));
            new_node.borrow_mut().parent = Some(parent);
            self.children.as_mut().unwrap().push(new_node);

            self.children
                .as_mut()
                .unwrap()
                .sort_by(|a, b| a.borrow().key.as_ref().cmp(b.borrow().key.as_ref()));

            const MAX_CHILDREN: usize = 3;
            if self.children.as_ref().unwrap().len() > MAX_CHILDREN {
                self.split();
            }
        } else if let Some(children) = &mut self.children {
            for child in children.iter_mut() {
                if key.as_ref() < child.borrow().key.as_ref() {
                    child.borrow_mut().insert(key, value_hash);
                    return;
                }
            }

            if let Some(last_child) = children.last_mut() {
                last_child.borrow_mut().insert(key, value_hash);
            }
        }
    }

    pub fn update(&mut self, key: K, value_hash: ValueDigest<N>) {
        if self.is_leaf {
            if self.key == key {
                self.value_hash = value_hash;
            }
        } else if let Some(children) = &mut self.children {
            for child in children.iter_mut() {
                child.borrow_mut().update(key.clone(), value_hash.clone());
            }
        }
    }

    pub fn delete(&mut self, key: &K) {
        if self.is_leaf {
            if self.key == *key {
                // Special case: if this is the root node and it matches the key, clear its key and value.
                self.key = "".as_bytes().to_vec().into();
                self.value_hash = ValueDigest::<N>::default();
            }
            if let Some(children) = &mut self.children {
                children.retain(|child| &child.borrow().key != key);
            }
        } else if let Some(children) = &mut self.children {
            for child in children.iter_mut() {
                if &child.borrow().key == key {
                    children.retain(|c| &c.borrow().key != key);
                    return;
                }
                child.borrow_mut().delete(key);
            }
        }
    }

    pub fn search(&self, key: &K) -> Option<Rc<RefCell<NodeAlt<N, K>>>> {
        if self.key == *key {
            return Some(Rc::new(RefCell::new(self.clone())));
        }
        if let Some(children) = &self.children {
            for child in children {
                if let Some(result) = child.borrow().search(key) {
                    return Some(result);
                }
            }
        }
        None
    }

    fn split(&mut self) {
        if let Some(children) = &mut self.children {
            let mid = children.len() / 2;
            let right_children = children.split_off(mid);

            let promoted_key = right_children[0].borrow().key.clone();
            let promoted_value_hash = right_children[0].borrow().value_hash.clone();

            let new_node = Rc::new(RefCell::new(NodeAlt {
                key: promoted_key.clone(),
                value_hash: promoted_value_hash.clone(),
                children: Some(right_children),
                parent: None,
                level: self.level,
                is_leaf: self.is_leaf,
                subtree_counts: self.subtree_counts.clone(),
            }));

            for child in new_node.borrow().children.as_ref().unwrap().iter() {
                child.borrow_mut().parent = Some(Rc::downgrade(&new_node));
            }

            self.children = Some(children.clone());

            if let Some(parent) = self.get_parent() {
                parent.borrow_mut().insert_internal(new_node);
            } else {
                let new_root = NodeAlt::new(promoted_key, promoted_value_hash, false);
                new_root.borrow_mut().children =
                    Some(vec![Rc::new(RefCell::new(self.clone())), new_node.clone()]);
                new_root.borrow_mut().level = self.level + 1;
                self.parent = Some(Rc::downgrade(&new_root));
            }
        }
    }

    fn get_parent(&self) -> Option<Rc<RefCell<NodeAlt<N, K>>>> {
        self.parent.as_ref()?.upgrade()
    }

    fn insert_internal(&mut self, new_node: Rc<RefCell<NodeAlt<N, K>>>) {
        if let Some(children) = &mut self.children {
            children.push(new_node);
            children.sort_by(|a, b| a.borrow().key.as_ref().cmp(b.borrow().key.as_ref()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::ValueDigest;

    #[test]
    fn test_insert() {
        let key = "example_key".as_bytes().to_vec();
        let value = b"test data 1";
        let value_hash = ValueDigest::<32>::new(value);
        let root = NodeAlt::new(key.clone(), value_hash.clone(), true);

        let new_key = "new_key".as_bytes().to_vec();
        let new_value = b"test data 2";
        let new_value_hash = ValueDigest::<32>::new(new_value);
        root.borrow_mut()
            .insert(new_key.clone(), new_value_hash.clone());

        assert!(root.borrow().search(&new_key).is_some());
    }

    #[test]
    fn test_update() {
        let key = "example_key".as_bytes().to_vec();
        let value = b"test data 1";
        let value_hash = ValueDigest::<32>::new(value);
        let root = NodeAlt::new(key.clone(), value_hash.clone(), true);

        let new_value = b"updated data";
        let new_value_hash = ValueDigest::<32>::new(new_value);
        root.borrow_mut()
            .update(key.clone(), new_value_hash.clone());

        let result = root.borrow().search(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap().borrow().value_hash, new_value_hash);
    }

    #[test]
    fn test_delete() {
        let key = "example_key".as_bytes().to_vec();
        let value = b"test data 1";
        let value_hash = ValueDigest::<32>::new(value);
        let root = NodeAlt::new(key.clone(), value_hash.clone(), true);

        root.borrow_mut().delete(&key);

        assert!(root.borrow().search(&key).is_none());
    }

    #[test]
    fn test_search() {
        let key = "example_key".as_bytes().to_vec();
        let value = b"test data 1";
        let value_hash = ValueDigest::<32>::new(value);
        let root = NodeAlt::new(key.clone(), value_hash.clone(), true);

        let result = root.borrow().search(&key);

        assert!(result.is_some());
        assert_eq!(result.unwrap().borrow().key, key);
    }
}
