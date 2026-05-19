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

//! Distance metrics for the proximity index.
//!
//! All metrics produce values where **smaller means closer**, so KNN and
//! beam-search can sort ascending uniformly. `InnerProduct` returns
//! `-dot(a, b)` to preserve that contract.

use serde::{Deserialize, Serialize};

/// Built-in distance metrics. Persisted as a single byte tag in
/// [`crate::proximity::ProximityNode`] so a query at any historical commit can
/// be validated against the index's original metric.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Metric {
    /// Standard Euclidean distance: `sqrt(sum((a_i - b_i)^2))`.
    L2 = 0,
    /// Cosine distance: `1 - cos(angle(a, b))`. Range `[0, 2]`; 0 = identical
    /// direction, 1 = orthogonal, 2 = opposite.
    Cosine = 1,
    /// Negated inner product: `-dot(a, b)`. Smaller is closer (matches the
    /// "smaller is better" contract used everywhere else).
    InnerProduct = 2,
}

impl Metric {
    pub const COUNT: u8 = 3;

    /// Parse a previously-persisted metric tag.
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Metric::L2),
            1 => Some(Metric::Cosine),
            2 => Some(Metric::InnerProduct),
            _ => None,
        }
    }
}

/// Pluggable distance function. Built-in [`Metric`] implements this;
/// callers can also provide their own implementation for custom metrics.
///
/// Implementations must satisfy:
/// 1. Smaller return value = closer.
/// 2. `distance(a, a) <= distance(a, b)` for any `b`.
/// 3. Determinism — same inputs return bit-identical outputs across calls
///    (so beam search is reproducible).
pub trait Distance: Send + Sync {
    /// Returns the distance between `a` and `b`. The two slices are guaranteed
    /// to have equal length (the index validates dimensions before calling).
    fn distance(&self, a: &[f32], b: &[f32]) -> f32;

    /// One-byte tag identifying this distance, persisted alongside the index.
    /// Custom implementations should pick a tag >= [`Metric::COUNT`] to avoid
    /// collision with the built-in metrics.
    fn metric_tag(&self) -> u8;
}

impl Distance for Metric {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len(), "distance dimensions must match");
        match self {
            Metric::L2 => {
                let mut sum = 0.0f32;
                for i in 0..a.len() {
                    let d = a[i] - b[i];
                    sum += d * d;
                }
                sum.sqrt()
            }
            Metric::Cosine => {
                let mut dot = 0.0f32;
                let mut na = 0.0f32;
                let mut nb = 0.0f32;
                for i in 0..a.len() {
                    dot += a[i] * b[i];
                    na += a[i] * a[i];
                    nb += b[i] * b[i];
                }
                let denom = (na * nb).sqrt();
                if denom == 0.0 {
                    1.0
                } else {
                    // Clamp to [-1, 1] to absorb floating-point drift, then
                    // map to [0, 2] cosine distance.
                    let sim = (dot / denom).clamp(-1.0, 1.0);
                    1.0 - sim
                }
            }
            Metric::InnerProduct => {
                let mut dot = 0.0f32;
                for i in 0..a.len() {
                    dot += a[i] * b[i];
                }
                -dot
            }
        }
    }

    fn metric_tag(&self) -> u8 {
        *self as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l2_identical_is_zero() {
        let a = [1.0, 2.0, 3.0];
        assert_eq!(Metric::L2.distance(&a, &a), 0.0);
    }

    #[test]
    fn l2_known_distance() {
        let a = [0.0, 0.0];
        let b = [3.0, 4.0];
        assert!((Metric::L2.distance(&a, &b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_identical_direction_is_zero() {
        let a = [1.0, 0.0, 0.0];
        let b = [2.0, 0.0, 0.0]; // same direction, different magnitude
        assert!(Metric::Cosine.distance(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_is_one() {
        let a = [1.0, 0.0];
        let b = [0.0, 1.0];
        assert!((Metric::Cosine.distance(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_opposite_is_two() {
        let a = [1.0, 0.0];
        let b = [-1.0, 0.0];
        assert!((Metric::Cosine.distance(&a, &b) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_zero_vector_returns_one() {
        let a = [0.0, 0.0];
        let b = [1.0, 0.0];
        assert_eq!(Metric::Cosine.distance(&a, &b), 1.0);
    }

    #[test]
    fn inner_product_smaller_is_more_aligned() {
        // Two vectors with larger dot product should yield a smaller distance.
        let q = [1.0, 0.0];
        let close = [1.0, 0.0]; // dot = 1
        let far = [-1.0, 0.0]; // dot = -1
        assert!(
            Metric::InnerProduct.distance(&q, &close) < Metric::InnerProduct.distance(&q, &far)
        );
    }

    #[test]
    fn metric_tag_roundtrip() {
        for m in [Metric::L2, Metric::Cosine, Metric::InnerProduct] {
            assert_eq!(Metric::from_tag(m.metric_tag()), Some(m));
        }
        assert_eq!(Metric::from_tag(99), None);
    }
}
