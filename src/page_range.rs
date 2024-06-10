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

use crate::page::Page;
use sha2::digest::{FixedOutputReset, HashMarker};
use sha2::{Digest, Sha256};
use std::fmt::Display;

#[derive(Debug, PartialEq)]
pub struct PageRange<'a, K, H = Sha256> {
    /// The inclusive start & end key bounds of this range.
    start: &'a K,
    end: &'a K,
    hasher: H,
}

impl<'a, K, H> Display for PageRange<'a, K, H>
where
    K: Display,
    H: Default + FixedOutputReset + HashMarker,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("({}, {})", self.start, self.end))
    }
}

impl<'a, K, H: Clone> Clone for PageRange<'a, K, H> {
    fn clone(&self) -> Self {
        Self {
            start: self.start,
            end: self.end,
            hasher: self.hasher.clone(),
        }
    }
}

impl<'a, const N: usize, K: Ord + Clone + AsRef<[u8]>, H: Default + Clone> From<&'a Page<N, K>>
    for PageRange<'a, K, H>
{
    #[inline(always)]
    fn from(page: &'a Page<N, K>) -> Self {
        PageRange {
            start: page.min_subtree_key(),
            end: page.max_subtree_key(),
            hasher: H::default(),
        }
    }
}

impl<'a, K, H> PageRange<'a, K, H>
where
    K: AsRef<[u8]> + PartialOrd,
    H: Default + Clone + FixedOutputReset + HashMarker,
{
    /// Construct a [`PageRange`] for the given key interval and [`PageDigest`].
    ///
    /// # Panics
    ///
    /// If `start` is greater than `end`, this method panics.
    pub fn new(start: &'a K, end: &'a K) -> Self {
        assert!(start <= end);
        Self {
            start,
            end,
            hasher: H::default(),
        }
    }

    /// Returns the inclusive start of this [`PageRange`].
    pub fn start(&self) -> &'a K {
        self.start
    }

    /// Returns the inclusive end of this [`PageRange`]
    pub fn end(&self) -> &'a K {
        self.end
    }

    /// Returns true if `self` is a superset of `other` (not a strict superset -
    /// equal ranges are treated as supersets of each other).
    pub(crate) fn is_superset_of(&self, other: &Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    pub fn calculate_hash(&self) -> Vec<u8> {
        let mut hasher = self.hasher.clone();
        Digest::update(&mut hasher, self.start.as_ref());
        Digest::update(&mut hasher, self.end.as_ref());
        let result = hasher.finalize_reset();
        result.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Sha256;

    #[test]
    fn test_page_range_new() {
        let start = "a";
        let end = "z";
        let range: PageRange<&str, Sha256> = PageRange::new(&start, &end);
        assert_eq!(range.start(), &start);
        assert_eq!(range.end(), &end);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn test_page_range_new_invalid() {
        let start = "z";
        let end = "a";
        let _range: PageRange<&str, Sha256> = PageRange::new(&start, &end);
    }

    #[test]
    fn test_page_range_is_superset_of() {
        let range1: PageRange<&str, Sha256> = PageRange::new(&"a", &"z");
        let range2: PageRange<&str, Sha256> = PageRange::new(&"b", &"y");
        assert!(range1.is_superset_of(&range2));
    }

    #[test]
    fn test_page_range_calculate_hash() {
        let range: PageRange<&str, Sha256> = PageRange::new(&"a", &"z");
        let hash1: Vec<u8> = range.calculate_hash();
        let hash2: Vec<u8> = range.calculate_hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_page_range_clone() {
        let range1: PageRange<&str, Sha256> = PageRange::new(&"a", &"z");
        let range2: PageRange<&str, Sha256> = range1.clone();
        assert_eq!(range1.calculate_hash(), range2.calculate_hash());
    }
}
