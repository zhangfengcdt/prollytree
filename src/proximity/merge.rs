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

//! Three-way merge of proximity-index entry sets (PR 3b).
//!
//! Provides:
//!
//! - [`ProximityConflict`] / [`ProximityResolution`] / [`ProximityConflictResolver`]:
//!   a vector-typed mirror of [`crate::diff::ConflictResolver`]. Resolvers
//!   receive `(id, base_vector, source_vector, destination_vector)` and return
//!   either a merged vector or a deletion.
//! - Four built-in resolvers: [`TakeSourceProximityResolver`],
//!   [`TakeDestinationProximityResolver`], [`LatestVectorResolver`],
//!   [`MeanVectorResolver`].
//! - [`merge_proximity_index_sets`]: three-way merge over `(id → vector)`
//!   maps using the same nine-case logic as
//!   `NamespacedKvStore::merge`, applied per id.
//!
//! Integration with `NamespacedKvStore::merge` (the Git-only namespaced merge
//! path) is **deferred to a future PR** — v1 scope keeps proximity indexes on
//! File and RocksDB backends where namespace-level merge is not currently
//! exposed.

use crate::proximity::distance::Metric;
use std::collections::{BTreeMap, BTreeSet};

/// One per-id conflict raised during [`merge_proximity_index_sets`].
///
/// At most one of `source_vector` / `destination_vector` may be `None`
/// (`None` on a side means that side deleted the id). The id appeared in
/// `base` if `base_vector.is_some()`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProximityConflict {
    pub id: Vec<u8>,
    pub base_vector: Option<Vec<f32>>,
    pub source_vector: Option<Vec<f32>>,
    pub destination_vector: Option<Vec<f32>>,
}

/// What a resolver chose to do with a conflict.
#[derive(Debug, Clone, PartialEq)]
pub enum ProximityResolution {
    /// Use this vector for the id (insert/update).
    Use(Vec<f32>),
    /// Remove the id from the merged index.
    Remove,
}

/// Vector-typed analogue of [`crate::diff::ConflictResolver`].
///
/// Implementations decide how to merge an id whose vector differs between
/// source and destination since the merge base. Returning `None` leaves the
/// conflict unresolved — [`merge_proximity_index_sets`] then surfaces it as
/// a [`MergeFailure`].
pub trait ProximityConflictResolver {
    fn resolve(&self, conflict: &ProximityConflict) -> Option<ProximityResolution>;
}

// ---------------------------------------------------------------------------
// Built-in resolvers
// ---------------------------------------------------------------------------

/// Always picks the source side. Equivalent to `git merge -X theirs` for one
/// id at a time.
#[derive(Debug, Clone, Default)]
pub struct TakeSourceProximityResolver;

impl ProximityConflictResolver for TakeSourceProximityResolver {
    fn resolve(&self, c: &ProximityConflict) -> Option<ProximityResolution> {
        Some(match &c.source_vector {
            Some(v) => ProximityResolution::Use(v.clone()),
            None => ProximityResolution::Remove,
        })
    }
}

/// Always picks the destination side. Equivalent to `git merge -X ours` for
/// one id at a time.
#[derive(Debug, Clone, Default)]
pub struct TakeDestinationProximityResolver;

impl ProximityConflictResolver for TakeDestinationProximityResolver {
    fn resolve(&self, c: &ProximityConflict) -> Option<ProximityResolution> {
        Some(match &c.destination_vector {
            Some(v) => ProximityResolution::Use(v.clone()),
            None => ProximityResolution::Remove,
        })
    }
}

/// Picks the most recently updated side using a user-supplied timestamp
/// extractor (e.g. a unix epoch stored alongside the vector, or extracted from
/// the id).
///
/// Higher returned values win. Ties: source side wins (deterministic).
pub struct LatestVectorResolver<F>
where
    F: Fn(&[u8], &[f32]) -> u64,
{
    extractor: F,
}

impl<F> std::fmt::Debug for LatestVectorResolver<F>
where
    F: Fn(&[u8], &[f32]) -> u64,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LatestVectorResolver").finish()
    }
}

impl<F> LatestVectorResolver<F>
where
    F: Fn(&[u8], &[f32]) -> u64,
{
    /// Build a resolver from a closure that extracts a comparable timestamp
    /// from `(id, vector)`.
    pub fn new(extractor: F) -> Self {
        Self { extractor }
    }
}

impl<F> ProximityConflictResolver for LatestVectorResolver<F>
where
    F: Fn(&[u8], &[f32]) -> u64,
{
    fn resolve(&self, c: &ProximityConflict) -> Option<ProximityResolution> {
        let s_ts = c.source_vector.as_ref().map(|v| (self.extractor)(&c.id, v));
        let d_ts = c
            .destination_vector
            .as_ref()
            .map(|v| (self.extractor)(&c.id, v));

        Some(match (s_ts, d_ts) {
            (Some(s), Some(d)) => {
                if s >= d {
                    ProximityResolution::Use(c.source_vector.as_ref().unwrap().clone())
                } else {
                    ProximityResolution::Use(c.destination_vector.as_ref().unwrap().clone())
                }
            }
            (Some(_), None) => ProximityResolution::Use(c.source_vector.as_ref().unwrap().clone()),
            (None, Some(_)) => {
                ProximityResolution::Use(c.destination_vector.as_ref().unwrap().clone())
            }
            (None, None) => ProximityResolution::Remove,
        })
    }
}

/// Averages the source and destination vectors element-wise. Only valid for
/// metrics where mean is meaningful — [`Metric::L2`] and [`Metric::Cosine`].
///
/// On dimension mismatch the resolver returns `None`, surfacing the conflict
/// rather than producing a malformed vector.
#[derive(Debug, Clone)]
pub struct MeanVectorResolver {
    metric: Metric,
}

impl MeanVectorResolver {
    /// Build a resolver for the given metric. Returns an error for
    /// [`Metric::InnerProduct`] since "average inner-product vectors" is not a
    /// meaningful operation.
    pub fn new(metric: Metric) -> Result<Self, MeanVectorResolverError> {
        match metric {
            Metric::L2 | Metric::Cosine => Ok(Self { metric }),
            Metric::InnerProduct => Err(MeanVectorResolverError::InvalidMetric(metric)),
        }
    }

    pub fn metric(&self) -> Metric {
        self.metric
    }
}

/// Error from [`MeanVectorResolver::new`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MeanVectorResolverError {
    #[error("MeanVectorResolver is not valid for metric {0:?} — averaging only makes sense for L2 and Cosine")]
    InvalidMetric(Metric),
}

impl ProximityConflictResolver for MeanVectorResolver {
    fn resolve(&self, c: &ProximityConflict) -> Option<ProximityResolution> {
        match (&c.source_vector, &c.destination_vector) {
            (Some(s), Some(d)) => {
                if s.len() != d.len() {
                    return None;
                }
                let mean: Vec<f32> = s.iter().zip(d).map(|(a, b)| (a + b) * 0.5).collect();
                Some(ProximityResolution::Use(mean))
            }
            (Some(s), None) => Some(ProximityResolution::Use(s.clone())),
            (None, Some(d)) => Some(ProximityResolution::Use(d.clone())),
            (None, None) => Some(ProximityResolution::Remove),
        }
    }
}

// ---------------------------------------------------------------------------
// Three-way merge
// ---------------------------------------------------------------------------

/// Result of a successful merge: the merged `(id → vector)` set in
/// ascending id order.
pub type MergedProximitySet = BTreeMap<Vec<u8>, Vec<f32>>;

/// Returned when one or more conflicts could not be resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeFailure {
    pub conflicts: Vec<ProximityConflict>,
}

impl std::fmt::Display for MergeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} unresolved proximity conflict{}",
            self.conflicts.len(),
            if self.conflicts.len() == 1 { "" } else { "s" }
        )
    }
}

impl std::error::Error for MergeFailure {}

/// Three-way merge of three proximity-index entry sets (base, source,
/// destination) into a single merged set, using `resolver` for diverging ids.
///
/// The nine cases mirror `NamespacedKvStore::merge` exactly so the behaviour
/// is consistent with the byte-keyed primary tree merge:
///
/// | base | source | dest | outcome                                                          |
/// |------|--------|------|------------------------------------------------------------------|
/// | x    | x      | x    | no-op (no change anywhere)                                       |
/// | x    | y      | x    | take source `y` (only source changed)                            |
/// | x    | x      | y    | take dest `y` (only dest changed)                                |
/// | x    | y      | z    | conflict — resolver decides                                      |
/// | -    | y      | -    | added by source — insert                                         |
/// | -    | -      | y    | added by dest — keep                                             |
/// | -    | y      | y    | both added same value — keep                                     |
/// | -    | y      | z    | both added different — conflict                                  |
/// | x    | -      | x    | dest unchanged, source deleted — apply deletion                  |
/// | x    | -      | y    | dest modified, source deleted — conflict                         |
/// | x    | y      | -    | dest deleted, source unchanged — keep deletion                   |
/// | x    | y      | -    | dest deleted, source modified — conflict                         |
/// | x    | -      | -    | both deleted — keep deletion                                     |
pub fn merge_proximity_index_sets<R: ProximityConflictResolver>(
    base: &BTreeMap<Vec<u8>, Vec<f32>>,
    source: &BTreeMap<Vec<u8>, Vec<f32>>,
    dest: &BTreeMap<Vec<u8>, Vec<f32>>,
    resolver: &R,
) -> Result<MergedProximitySet, MergeFailure> {
    let mut merged: MergedProximitySet = BTreeMap::new();
    let mut unresolved: Vec<ProximityConflict> = Vec::new();

    let all_ids: BTreeSet<&Vec<u8>> = base
        .keys()
        .chain(source.keys())
        .chain(dest.keys())
        .collect();

    for id in all_ids {
        let b = base.get(id);
        let s = source.get(id);
        let d = dest.get(id);

        match (b, s, d) {
            // Triple presence — resolve based on which side(s) diverged.
            (Some(b), Some(s), Some(d)) => {
                if s == d {
                    // Both sides converged on the same value (whether or not it
                    // matches base).
                    merged.insert(id.clone(), d.clone());
                } else if b == d && b != s {
                    // Only source changed.
                    merged.insert(id.clone(), s.clone());
                } else if b == s {
                    // Only dest changed.
                    merged.insert(id.clone(), d.clone());
                } else {
                    // Both sides changed differently — conflict.
                    let conflict = ProximityConflict {
                        id: id.clone(),
                        base_vector: Some(b.clone()),
                        source_vector: Some(s.clone()),
                        destination_vector: Some(d.clone()),
                    };
                    apply_resolution(resolver, conflict, &mut merged, &mut unresolved);
                }
            }
            // Added by source only.
            (None, Some(s), None) => {
                merged.insert(id.clone(), s.clone());
            }
            // Added by dest only.
            (None, None, Some(d)) => {
                merged.insert(id.clone(), d.clone());
            }
            // Added on both sides.
            (None, Some(s), Some(d)) => {
                if s == d {
                    merged.insert(id.clone(), d.clone());
                } else {
                    let conflict = ProximityConflict {
                        id: id.clone(),
                        base_vector: None,
                        source_vector: Some(s.clone()),
                        destination_vector: Some(d.clone()),
                    };
                    apply_resolution(resolver, conflict, &mut merged, &mut unresolved);
                }
            }
            // Source deleted; dest still present.
            (Some(b), None, Some(d)) => {
                if b == d {
                    // Dest unchanged → safe to apply deletion.
                } else {
                    let conflict = ProximityConflict {
                        id: id.clone(),
                        base_vector: Some(b.clone()),
                        source_vector: None,
                        destination_vector: Some(d.clone()),
                    };
                    apply_resolution(resolver, conflict, &mut merged, &mut unresolved);
                }
            }
            // Dest deleted; source still present.
            (Some(b), Some(s), None) => {
                if b == s {
                    // Source unchanged → keep dest's deletion.
                } else {
                    let conflict = ProximityConflict {
                        id: id.clone(),
                        base_vector: Some(b.clone()),
                        source_vector: Some(s.clone()),
                        destination_vector: None,
                    };
                    apply_resolution(resolver, conflict, &mut merged, &mut unresolved);
                }
            }
            // Both deleted (or never existed beyond base).
            (Some(_), None, None) => { /* deletion converged */ }
            // Unreachable: at least one side must have the id (we iterated
            // their union).
            (None, None, None) => unreachable!("id appeared in iteration but exists nowhere"),
        }
    }

    if unresolved.is_empty() {
        Ok(merged)
    } else {
        Err(MergeFailure {
            conflicts: unresolved,
        })
    }
}

fn apply_resolution<R: ProximityConflictResolver>(
    resolver: &R,
    conflict: ProximityConflict,
    merged: &mut MergedProximitySet,
    unresolved: &mut Vec<ProximityConflict>,
) {
    match resolver.resolve(&conflict) {
        Some(ProximityResolution::Use(v)) => {
            merged.insert(conflict.id, v);
        }
        Some(ProximityResolution::Remove) => { /* drop */ }
        None => unresolved.push(conflict),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn b(s: &str) -> Vec<u8> {
        s.as_bytes().to_vec()
    }

    fn v(xs: &[f32]) -> Vec<f32> {
        xs.to_vec()
    }

    fn map(pairs: &[(&str, &[f32])]) -> BTreeMap<Vec<u8>, Vec<f32>> {
        pairs.iter().map(|(k, vs)| (b(k), v(vs))).collect()
    }

    struct AlwaysUnresolved;
    impl ProximityConflictResolver for AlwaysUnresolved {
        fn resolve(&self, _: &ProximityConflict) -> Option<ProximityResolution> {
            None
        }
    }

    // ---- TakeSource / TakeDestination ------------------------------------

    #[test]
    fn take_source_picks_source_or_removes() {
        let r = TakeSourceProximityResolver;
        let c = ProximityConflict {
            id: b("x"),
            base_vector: None,
            source_vector: Some(v(&[1.0, 2.0])),
            destination_vector: Some(v(&[3.0, 4.0])),
        };
        assert_eq!(
            r.resolve(&c),
            Some(ProximityResolution::Use(v(&[1.0, 2.0])))
        );

        let c = ProximityConflict {
            id: b("x"),
            base_vector: Some(v(&[0.0, 0.0])),
            source_vector: None,
            destination_vector: Some(v(&[3.0, 4.0])),
        };
        assert_eq!(r.resolve(&c), Some(ProximityResolution::Remove));
    }

    #[test]
    fn take_destination_picks_dest_or_removes() {
        let r = TakeDestinationProximityResolver;
        let c = ProximityConflict {
            id: b("x"),
            base_vector: None,
            source_vector: Some(v(&[1.0, 2.0])),
            destination_vector: Some(v(&[3.0, 4.0])),
        };
        assert_eq!(
            r.resolve(&c),
            Some(ProximityResolution::Use(v(&[3.0, 4.0])))
        );

        let c = ProximityConflict {
            id: b("x"),
            base_vector: Some(v(&[0.0])),
            source_vector: Some(v(&[1.0])),
            destination_vector: None,
        };
        assert_eq!(r.resolve(&c), Some(ProximityResolution::Remove));
    }

    // ---- LatestVectorResolver --------------------------------------------

    #[test]
    fn latest_picks_higher_timestamp_side() {
        // Timestamp = first f32 (bit-cast to bits then to u64 for ordering)
        // Use the simpler: timestamp = floor(first element * 1000).
        let r = LatestVectorResolver::new(|_id, v| (v[0] * 1000.0).max(0.0) as u64);

        // source has higher ts
        let c = ProximityConflict {
            id: b("x"),
            base_vector: None,
            source_vector: Some(v(&[5.0, 1.0])),
            destination_vector: Some(v(&[2.0, 1.0])),
        };
        assert_eq!(
            r.resolve(&c),
            Some(ProximityResolution::Use(v(&[5.0, 1.0])))
        );

        // dest has higher ts
        let c = ProximityConflict {
            id: b("x"),
            base_vector: None,
            source_vector: Some(v(&[1.0, 1.0])),
            destination_vector: Some(v(&[3.0, 1.0])),
        };
        assert_eq!(
            r.resolve(&c),
            Some(ProximityResolution::Use(v(&[3.0, 1.0])))
        );

        // tie — source wins (deterministic)
        let c = ProximityConflict {
            id: b("x"),
            base_vector: None,
            source_vector: Some(v(&[2.0, 9.0])),
            destination_vector: Some(v(&[2.0, 8.0])),
        };
        assert_eq!(
            r.resolve(&c),
            Some(ProximityResolution::Use(v(&[2.0, 9.0])))
        );
    }

    #[test]
    fn latest_handles_one_sided_presence() {
        let r = LatestVectorResolver::new(|_, v| v[0] as u64);

        let c = ProximityConflict {
            id: b("x"),
            base_vector: Some(v(&[5.0])),
            source_vector: None,
            destination_vector: Some(v(&[3.0])),
        };
        assert_eq!(r.resolve(&c), Some(ProximityResolution::Use(v(&[3.0]))));

        let c = ProximityConflict {
            id: b("x"),
            base_vector: Some(v(&[5.0])),
            source_vector: Some(v(&[3.0])),
            destination_vector: None,
        };
        assert_eq!(r.resolve(&c), Some(ProximityResolution::Use(v(&[3.0]))));
    }

    // ---- MeanVectorResolver ---------------------------------------------

    #[test]
    fn mean_averages_elementwise() {
        let r = MeanVectorResolver::new(Metric::L2).unwrap();
        let c = ProximityConflict {
            id: b("x"),
            base_vector: Some(v(&[0.0, 0.0])),
            source_vector: Some(v(&[2.0, 4.0])),
            destination_vector: Some(v(&[4.0, 8.0])),
        };
        assert_eq!(
            r.resolve(&c),
            Some(ProximityResolution::Use(v(&[3.0, 6.0])))
        );
    }

    #[test]
    fn mean_rejects_inner_product() {
        assert!(MeanVectorResolver::new(Metric::InnerProduct).is_err());
    }

    #[test]
    fn mean_returns_none_on_dim_mismatch() {
        let r = MeanVectorResolver::new(Metric::Cosine).unwrap();
        let c = ProximityConflict {
            id: b("x"),
            base_vector: None,
            source_vector: Some(v(&[1.0, 2.0])),
            destination_vector: Some(v(&[1.0, 2.0, 3.0])),
        };
        assert_eq!(r.resolve(&c), None); // surfaces as unresolved
    }

    // ---- merge_proximity_index_sets — case coverage ----------------------

    #[test]
    fn merge_no_changes() {
        let base = map(&[("a", &[1.0]), ("b", &[2.0])]);
        let merged =
            merge_proximity_index_sets(&base, &base, &base, &TakeSourceProximityResolver).unwrap();
        assert_eq!(merged, base);
    }

    #[test]
    fn merge_disjoint_inserts() {
        // base empty; source adds 'a'; dest adds 'b'; both should land.
        let base = map(&[]);
        let source = map(&[("a", &[1.0])]);
        let dest = map(&[("b", &[2.0])]);
        let merged =
            merge_proximity_index_sets(&base, &source, &dest, &TakeSourceProximityResolver)
                .unwrap();
        assert_eq!(merged, map(&[("a", &[1.0]), ("b", &[2.0])]));
    }

    #[test]
    fn merge_only_source_changed_takes_source() {
        let base = map(&[("a", &[1.0])]);
        let source = map(&[("a", &[2.0])]);
        let dest = map(&[("a", &[1.0])]);
        let merged =
            merge_proximity_index_sets(&base, &source, &dest, &TakeDestinationProximityResolver)
                .unwrap();
        // dest unchanged, source changed → source wins regardless of resolver
        assert_eq!(merged, map(&[("a", &[2.0])]));
    }

    #[test]
    fn merge_only_dest_changed_takes_dest() {
        let base = map(&[("a", &[1.0])]);
        let source = map(&[("a", &[1.0])]);
        let dest = map(&[("a", &[3.0])]);
        let merged =
            merge_proximity_index_sets(&base, &source, &dest, &TakeSourceProximityResolver)
                .unwrap();
        // source unchanged → dest wins
        assert_eq!(merged, map(&[("a", &[3.0])]));
    }

    #[test]
    fn merge_both_changed_routes_through_resolver() {
        let base = map(&[("a", &[1.0])]);
        let source = map(&[("a", &[2.0])]);
        let dest = map(&[("a", &[3.0])]);
        let r = TakeSourceProximityResolver;
        let merged = merge_proximity_index_sets(&base, &source, &dest, &r).unwrap();
        assert_eq!(merged, map(&[("a", &[2.0])]));

        let r = TakeDestinationProximityResolver;
        let merged = merge_proximity_index_sets(&base, &source, &dest, &r).unwrap();
        assert_eq!(merged, map(&[("a", &[3.0])]));

        let r = MeanVectorResolver::new(Metric::L2).unwrap();
        let merged = merge_proximity_index_sets(&base, &source, &dest, &r).unwrap();
        assert_eq!(merged, map(&[("a", &[2.5])]));
    }

    #[test]
    fn merge_source_deleted_dest_unchanged_applies_deletion() {
        let base = map(&[("a", &[1.0])]);
        let source = map(&[]); // a deleted
        let dest = map(&[("a", &[1.0])]); // unchanged
        let merged =
            merge_proximity_index_sets(&base, &source, &dest, &TakeSourceProximityResolver)
                .unwrap();
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_source_deleted_dest_modified_is_conflict() {
        let base = map(&[("a", &[1.0])]);
        let source = map(&[]); // a deleted
        let dest = map(&[("a", &[5.0])]); // a modified
        let result = merge_proximity_index_sets(&base, &source, &dest, &AlwaysUnresolved);
        let fail = result.unwrap_err();
        assert_eq!(fail.conflicts.len(), 1);
        assert_eq!(fail.conflicts[0].id, b("a"));
        assert_eq!(fail.conflicts[0].source_vector, None);
        assert_eq!(fail.conflicts[0].destination_vector, Some(v(&[5.0])));
    }

    #[test]
    fn merge_both_added_same_value_no_conflict() {
        let base = map(&[]);
        let source = map(&[("a", &[1.0, 2.0])]);
        let dest = map(&[("a", &[1.0, 2.0])]);
        let merged = merge_proximity_index_sets(&base, &source, &dest, &AlwaysUnresolved).unwrap();
        assert_eq!(merged, map(&[("a", &[1.0, 2.0])]));
    }

    #[test]
    fn merge_both_added_different_is_conflict() {
        let base = map(&[]);
        let source = map(&[("a", &[1.0])]);
        let dest = map(&[("a", &[2.0])]);
        let result = merge_proximity_index_sets(&base, &source, &dest, &AlwaysUnresolved);
        let fail = result.unwrap_err();
        assert_eq!(fail.conflicts.len(), 1);
        assert_eq!(fail.conflicts[0].base_vector, None);
    }

    #[test]
    fn merge_both_deleted_converges_to_deletion() {
        let base = map(&[("a", &[1.0])]);
        let source = map(&[]);
        let dest = map(&[]);
        let merged = merge_proximity_index_sets(&base, &source, &dest, &AlwaysUnresolved).unwrap();
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_unresolved_returns_all_conflicts() {
        let base = map(&[("a", &[1.0]), ("b", &[2.0])]);
        let source = map(&[("a", &[10.0]), ("b", &[20.0])]);
        let dest = map(&[("a", &[100.0]), ("b", &[200.0])]);
        let fail =
            merge_proximity_index_sets(&base, &source, &dest, &AlwaysUnresolved).unwrap_err();
        assert_eq!(fail.conflicts.len(), 2);
        // Iteration order is BTreeSet-sorted by id.
        assert_eq!(fail.conflicts[0].id, b("a"));
        assert_eq!(fail.conflicts[1].id, b("b"));
    }
}
