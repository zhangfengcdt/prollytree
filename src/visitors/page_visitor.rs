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
// #![allow(dead_code)]

use crate::node::Node;
use crate::page::Page;
use crate::page_range::PageRange;
use crate::visitor::Visitor;

/// Record the page range & hashes for the visited pages.
#[derive(Debug)]
pub(crate) struct PageVisitor<'a, K> {
    out: Vec<PageRange<'a, K>>,
}

impl<'a, K> Default for PageVisitor<'a, K> {
    fn default() -> Self {
        Self {
            out: Default::default(),
        }
    }
}

impl<'a, const N: usize, K> Visitor<'a, N, K> for PageVisitor<'a, K>
where
    K: Ord + PartialOrd + AsRef<[u8]> + Clone,
{
    fn visit_node(&mut self, _node: &'a Node<N, K>) -> bool {
        true
    }

    fn visit_page(&mut self, page: &'a Page<N, K>, _high_page: bool) -> bool {
        self.out.push(PageRange::from(page));
        true
    }
}
