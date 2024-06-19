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

use crate::node::ProllyNode;
use crate::storage::NodeStorage;

pub trait Visitor<'a, const N: usize, S: NodeStorage<N>> {
    /// Called before a call to [`Visitor::visit_node()`] with the same [`ProllyNode`].
    /// By default this is a no-op unless implemented.
    fn pre_visit_node(&mut self, node: &'a ProllyNode<N>, storage: &S) -> bool {
        let _ = node;
        let _ = storage;
        true
    }

    /// Visit the given [`ProllyNode`].
    fn visit_node(&mut self, node: &'a ProllyNode<N>, storage: &S) -> bool;

    /// Called after [`Visitor::visit_node()`] with the same [`ProllyNode`].
    /// By default this is a no-op unless implemented.
    fn post_visit_node(&mut self, node: &'a ProllyNode<N>, storage: &S) -> bool {
        let _ = node;
        let _ = storage;
        true
    }
}

struct BasicVisitor;

impl<'a, const N: usize, S: NodeStorage<N>> Visitor<'a, N, S> for BasicVisitor {
    fn visit_node(&mut self, node: &'a ProllyNode<N>, _storage: &S) -> bool {
        println!("Visiting node with keys: {:?}", node.keys);
        true
    }
}
