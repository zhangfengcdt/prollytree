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

#[derive(Debug, PartialEq)]
pub enum DiffResult {
    Added(Vec<u8>, Vec<u8>),
    Removed(Vec<u8>, Vec<u8>),
    Modified(Vec<u8>, Vec<u8>, Vec<u8>),
}

#[derive(Debug, PartialEq, Clone)]
pub enum MergeResult {
    Added(Vec<u8>, Vec<u8>),
    Removed(Vec<u8>),
    Modified(Vec<u8>, Vec<u8>),
    Conflict(MergeConflict),
}

#[derive(Debug, PartialEq, Clone)]
pub struct MergeConflict {
    pub key: Vec<u8>,
    pub base_value: Option<Vec<u8>>,
    pub source_value: Option<Vec<u8>>,
    pub destination_value: Option<Vec<u8>>,
}

/// Trait for resolving merge conflicts
pub trait ConflictResolver {
    /// Resolve a conflict by returning the desired MergeResult
    /// Returns None if the conflict cannot be resolved and should remain as a conflict
    fn resolve_conflict(&self, conflict: &MergeConflict) -> Option<MergeResult>;
}

/// Default conflict resolver that ignores all conflicts (treats them as no-ops)
#[derive(Debug, Clone, Default)]
pub struct IgnoreConflictsResolver;

impl ConflictResolver for IgnoreConflictsResolver {
    fn resolve_conflict(&self, conflict: &MergeConflict) -> Option<MergeResult> {
        // Ignore conflicts by keeping the destination value (no change applied)
        // This effectively means "do nothing" for this key
        match &conflict.destination_value {
            Some(value) => Some(MergeResult::Modified(conflict.key.clone(), value.clone())),
            None => Some(MergeResult::Removed(conflict.key.clone())),
        }
    }
}

/// Conflict resolver that always takes the source value
#[derive(Debug, Clone, Default)]
pub struct TakeSourceResolver;

impl ConflictResolver for TakeSourceResolver {
    fn resolve_conflict(&self, conflict: &MergeConflict) -> Option<MergeResult> {
        match &conflict.source_value {
            Some(value) => Some(MergeResult::Modified(conflict.key.clone(), value.clone())),
            None => Some(MergeResult::Removed(conflict.key.clone())),
        }
    }
}

/// Conflict resolver that always takes the destination value
#[derive(Debug, Clone, Default)]
pub struct TakeDestinationResolver;

impl ConflictResolver for TakeDestinationResolver {
    fn resolve_conflict(&self, conflict: &MergeConflict) -> Option<MergeResult> {
        match &conflict.destination_value {
            Some(value) => Some(MergeResult::Modified(conflict.key.clone(), value.clone())),
            None => Some(MergeResult::Removed(conflict.key.clone())),
        }
    }
}
