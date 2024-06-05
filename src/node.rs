use crate::value_digest::ValueDigest;
use crate::page::Page;
use rand::Rng;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Node<const N: usize, K> {
    key: K,
    value_hash: ValueDigest<N>,

    /// A pointer to a page with a strictly lower tree level, and containing
    /// strictly smaller/less-than keys when compared to "key".
    lt_pointer: Option<Box<Page<N, K>>>,

    /// Additional fields for probabilistic balancing
    level: usize,
}

impl<const N: usize, K: Ord + Clone> Node<N, K> {
    pub fn new(key: K, value_hash: ValueDigest<N>, level: usize) -> Self {
        Self {
            key,
            value_hash,
            lt_pointer: None,
            level,
        }
    }

    // Getter for key
    pub fn key(&self) -> &K {
        &self.key
    }

    // Getter for value_hash
    pub fn value_hash(&self) -> &ValueDigest<N> {
        &self.value_hash
    }

    // Getter for lt_pointer
    pub fn lt_pointer(&self) -> &Option<Box<Page<N, K>>> {
        &self.lt_pointer
    }

    // Setter for lt_pointer
    pub fn set_lt_pointer(&mut self, lt_pointer: Option<Box<Page<N, K>>>) {
        self.lt_pointer = lt_pointer;
    }

    // Getter for level
    pub fn level(&self) -> &usize {
        &self.level
    }

    // Insert, delete, and balancing functions
    pub fn insert(&mut self, key: K, value_hash: ValueDigest<N>) {
        if key < self.key {
            if let Some(ref mut lt_pointer) = self.lt_pointer {
                lt_pointer.insert(key, value_hash);
            } else {
                self.lt_pointer = Some(Box::new(Page::new()));
                self.lt_pointer.as_mut().unwrap().insert(key, value_hash);
            }
        } else {
            // Inserting in the current page (since it's a simple example)
            let mut page = Page::new();
            page.insert(key, value_hash);
            self.lt_pointer = Some(Box::new(page));
        }
        self.balance();
    }

    pub fn delete(&mut self, key: &K) -> bool {
        if key < &self.key {
            if let Some(ref mut lt_pointer) = self.lt_pointer {
                lt_pointer.delete(key)
            } else {
                false
            }
        } else if key == &self.key {
            // For simplicity, we are not handling the deletion of the root node here.
            false
        } else {
            if let Some(ref mut lt_pointer) = self.lt_pointer {
                lt_pointer.delete(key)
            } else {
                false
            }
        }
    }

    pub fn balance(&mut self) {
        // Implementing a simple probabilistic balancing using random insertion
        if let Some(ref mut lt_pointer) = self.lt_pointer {
            let mut rng = rand::thread_rng();
            if rng.gen_bool(0.5) {
                // Randomly decide to balance
                // Placeholder for actual balancing logic
                lt_pointer.nodes.sort_by(|a, b| a.key().cmp(&b.key()));
            }
        }
    }
}
