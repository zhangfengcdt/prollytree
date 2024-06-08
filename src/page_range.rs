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

impl<'a, K: AsRef<[u8]>, H: Default + Digest + AsRef<[u8]> + Clone + FixedOutputReset>
    PageRange<'a, K, H>
{
    /// Construct a [`PageRange`] for the given key interval and [`PageDigest`].
    ///
    /// # Panics
    ///
    /// If `start` is greater than `end`, this method panics.
    pub fn new(start: &'a K, end: &'a K) -> Self
    where
        K: PartialOrd,
    {
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
    pub(crate) fn is_superset_of(&self, other: &Self) -> bool
    where
        K: PartialOrd,
    {
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
