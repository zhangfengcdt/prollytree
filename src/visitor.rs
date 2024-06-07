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

use crate::{node::Node, page::Page};

/// An observer of [`Page`] and the [`Node`] instances within them during tree
/// traversal.
pub trait Visitor<'a, const N: usize, K: AsRef<[u8]>> {
    /// Called before a a call to [`Visitor::visit_node()`] with the same
    /// [`Node`].
    ///
    /// By default this is a no-op unless implemented.
    fn pre_visit_node(&mut self, node: &'a Node<N, K>) -> bool {
        let _ = node;
        true
    }

    /// Visit the given [`Node`].
    fn visit_node(&mut self, node: &'a Node<N, K>) -> bool;

    /// Called after [`Visitor::visit_node()`] with the same [`Node`].
    ///
    /// By default this is a no-op unless implemented.
    fn post_visit_node(&mut self, node: &'a Node<N, K>) -> bool {
        let _ = node;
        true
    }

    /// Visit the given [`Page`], which was referenced via a high-page link if
    /// `high_page` is true.
    ///
    /// By default this is a no-op unless implemented.
    fn visit_page(&mut self, page: &'a Page<N, K>, high_page: bool) -> bool {
        let _ = page;
        let _ = high_page;
        true
    }

    /// Called after [`Visitor::visit_page()`] with the same [`Page`].
    ///
    /// By default this is a no-op unless implemented.
    fn post_visit_page(&mut self, page: &'a Page<N, K>) -> bool {
        let _ = page;
        true
    }
}
