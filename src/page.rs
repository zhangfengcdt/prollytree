use crate::value_digest::ValueDigest;
use crate::node::Node;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Page<const N: usize, K> {
    pub nodes: Vec<Node<N, K>>,
}

impl<const N: usize, K: Ord + Clone> Page<N, K> {
    pub fn new() -> Self {
        Page { nodes: Vec::new() }
    }

    pub fn insert(&mut self, key: K, value_hash: ValueDigest<N>) {
        let new_node = Node::new(key, value_hash, 0);
        self.nodes.push(new_node);
        self.nodes.sort_by(|a, b| a.key().cmp(&b.key()));
        self.balance();
    }

    pub fn delete(&mut self, key: &K) -> bool {
        if let Some(pos) = self.nodes.iter().position(|node| node.key() == key) {
            self.nodes.remove(pos);
            self.balance();
            true
        } else {
            false
        }
    }

    pub fn balance(&mut self) {
        const MAX_NODES: usize = 5; // Maximum number of nodes in a page before balancing
        if self.nodes.len() > MAX_NODES {
            // Split the page into two
            let mid = self.nodes.len() / 2;
            let right_nodes = self.nodes.split_off(mid);
            let right_page = Page { nodes: right_nodes };

            // Insert the middle key into the parent page
            let mid_node = self.nodes.pop().unwrap();
            let mut new_node = Node::new(mid_node.key().clone(), mid_node.value_hash().clone(), *mid_node.level() + 1);
            new_node.set_lt_pointer(Some(Box::new(right_page)));

            let left_page = Page { nodes: self.nodes.clone() };

            if let Some(ref mut last_node) = self.nodes.last_mut() {
                last_node.set_lt_pointer(Some(Box::new(left_page)));
            }
            self.nodes.push(new_node);
        }
    }

    pub fn find(&self, key: &K) -> Option<&Node<N, K>> {
        self.nodes.iter().find(|node| node.key() == key)
    }
}
