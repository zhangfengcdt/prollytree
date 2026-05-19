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

//! History-independent level assignment for proximity-map entries.
//!
//! A vector's maximum level in the tree is derived from a hash of
//! `(id, vector)`. Because the function is pure, the index shape depends
//! solely on the current data — never on insertion order or update history.

use crate::digest::ValueDigest;

/// Hard cap on the level a vector can reach. With `level_bits = 4` and a
/// uniformly distributed hash, the probability of exceeding level 31 is
/// `2^(-128)` — astronomically small, but the cap keeps tree-growth loops
/// finite under adversarial input.
pub const MAX_LEVEL: u8 = 31;

/// Compute the tree level a vector reaches.
///
/// The function hashes `id || little_endian(vector)` with SHA-256 and counts
/// the leading zero bits of the digest. Every `level_bits` zero bits promote
/// the vector one level. With the default `level_bits = 4` the expected
/// per-level fanout is 16 (one in 16 vectors reaches level 1, one in 256
/// reaches level 2, and so on).
///
/// # Arguments
/// * `id` — the entry's identifier bytes (whatever the caller chose as key).
/// * `vector` — the vector contents.
/// * `level_bits` — number of leading zero bits required to promote one level.
///   Must be >= 1; values of 3 or 4 are typical.
///
/// # Returns
/// A level in `[0, MAX_LEVEL]`. 0 means the vector lives only in a leaf node.
pub fn vector_level(id: &[u8], vector: &[f32], level_bits: u8) -> u8 {
    let level_bits = level_bits.max(1);
    let mut buf = Vec::with_capacity(id.len() + vector.len() * 4);
    buf.extend_from_slice(id);
    for f in vector {
        buf.extend_from_slice(&f.to_le_bytes());
    }
    let hash = ValueDigest::<32>::new(&buf);
    let lz = leading_zero_bits(hash.as_bytes());
    let level = lz / u32::from(level_bits);
    level.min(u32::from(MAX_LEVEL)) as u8
}

/// Count leading zero bits across a byte slice.
fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut count = 0u32;
    for &b in bytes {
        if b == 0 {
            count += 8;
        } else {
            return count + b.leading_zeros();
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_zeros_basic() {
        assert_eq!(leading_zero_bits(&[]), 0);
        assert_eq!(leading_zero_bits(&[0xFF]), 0);
        assert_eq!(leading_zero_bits(&[0x80]), 0);
        assert_eq!(leading_zero_bits(&[0x40]), 1);
        assert_eq!(leading_zero_bits(&[0x00, 0x80]), 8);
        assert_eq!(leading_zero_bits(&[0x00, 0x00, 0x10]), 19);
        assert_eq!(leading_zero_bits(&[0x00, 0x00, 0x00, 0x00]), 32);
    }

    #[test]
    fn determinism() {
        let v = vec![0.1f32, 0.2, 0.3];
        assert_eq!(vector_level(b"id-1", &v, 4), vector_level(b"id-1", &v, 4));
    }

    #[test]
    fn different_id_different_level() {
        // Trivially true for most pairs, but check at least that the function
        // varies with id.
        let v = vec![0.0f32, 0.0, 0.0];
        let mut seen_levels = std::collections::HashSet::new();
        for i in 0..1000u32 {
            let id = i.to_be_bytes();
            seen_levels.insert(vector_level(&id, &v, 4));
        }
        // We expect to see several distinct levels.
        assert!(
            seen_levels.len() > 1,
            "expected variety of levels, got {seen_levels:?}"
        );
    }

    #[test]
    fn level_bounded_by_max_level() {
        // Even with `level_bits = 1`, the level is capped.
        for i in 0..256u32 {
            let id = i.to_be_bytes();
            let v = vec![0.0f32; 4];
            let l = vector_level(&id, &v, 1);
            assert!(l <= MAX_LEVEL);
        }
    }

    #[test]
    fn level_distribution_matches_expected() {
        // With level_bits = 4, level L appears with probability 2^(-4L) per entry.
        // For 10_000 random entries, expected counts at level >= 0 = 10_000;
        // level >= 1 ≈ 625; level >= 2 ≈ 39. Just sanity-check the trend
        // (lots of zeros, very few at high levels).
        let mut counts: Vec<u32> = vec![0; (MAX_LEVEL as usize) + 1];
        for i in 0..10_000u32 {
            let id = i.to_be_bytes();
            let v = vec![i as f32, (i * 2) as f32];
            let l = vector_level(&id, &v, 4) as usize;
            counts[l] += 1;
        }
        // Sum should be the total
        assert_eq!(counts.iter().sum::<u32>(), 10_000);
        // Level 0 should be the most populous.
        let level_0 = counts[0];
        assert!(
            level_0 > 9_000,
            "expected most entries at level 0, got {level_0} / 10000"
        );
        // No entry should reach level > 8 for this dataset size.
        for &c in &counts[9..] {
            assert_eq!(c, 0);
        }
    }
}
