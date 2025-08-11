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

/// Multi-agent conflict resolver that prioritizes by agent ID or timestamp
/// Useful when merging work from multiple agents with different priorities
#[derive(Debug, Clone)]
pub struct AgentPriorityResolver {
    /// Agent priority mapping (higher values = higher priority)
    agent_priorities: std::collections::HashMap<String, u32>,
    /// Default priority for unknown agents
    default_priority: u32,
}

impl Default for AgentPriorityResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentPriorityResolver {
    pub fn new() -> Self {
        Self {
            agent_priorities: std::collections::HashMap::new(),
            default_priority: 1,
        }
    }

    pub fn with_priorities(priorities: std::collections::HashMap<String, u32>) -> Self {
        Self {
            agent_priorities: priorities,
            default_priority: 1,
        }
    }

    pub fn set_agent_priority(&mut self, agent_id: String, priority: u32) {
        self.agent_priorities.insert(agent_id, priority);
    }

    pub fn set_default_priority(&mut self, priority: u32) {
        self.default_priority = priority;
    }

    /// Extract agent ID from key (assumes key format includes agent info)
    /// This is a simple implementation - in practice you might have more sophisticated key parsing
    /// Note: Reserved for future agent-based conflict resolution features
    #[allow(dead_code)]
    fn extract_agent_id(&self, key: &[u8]) -> Option<String> {
        let key_str = String::from_utf8_lossy(key);
        // Look for patterns like "agent123:" or "agent:name:" at the start of keys
        if key_str.starts_with("agent") {
            if let Some(colon_pos) = key_str.find(':') {
                return Some(key_str[..colon_pos].to_string());
            }
        }
        None
    }

    /// Get priority for a given key based on its agent ID
    /// Note: Reserved for future agent-based conflict resolution features
    #[allow(dead_code)]
    fn get_priority_for_key(&self, key: &[u8]) -> u32 {
        self.extract_agent_id(key)
            .and_then(|agent_id| self.agent_priorities.get(&agent_id))
            .copied()
            .unwrap_or(self.default_priority)
    }
}

impl ConflictResolver for AgentPriorityResolver {
    fn resolve_conflict(&self, conflict: &MergeConflict) -> Option<MergeResult> {
        // For agent priority resolution, we need to determine which value to take
        // based on metadata. Since we don't have explicit agent info in the conflict,
        // we'll fall back to taking source if it exists, otherwise destination
        match (&conflict.source_value, &conflict.destination_value) {
            (Some(source), Some(_dest)) => {
                // Both exist - in a real implementation, you'd compare agent priorities
                // For now, take source as it represents the "incoming" changes
                Some(MergeResult::Modified(conflict.key.clone(), source.clone()))
            }
            (Some(source), None) => {
                // Source added, destination doesn't exist
                Some(MergeResult::Added(conflict.key.clone(), source.clone()))
            }
            (None, Some(dest)) => {
                // Source removed, destination exists - keep destination
                Some(MergeResult::Modified(conflict.key.clone(), dest.clone()))
            }
            (None, None) => {
                // Both removed - remove
                Some(MergeResult::Removed(conflict.key.clone()))
            }
        }
    }
}

/// Timestamp-based conflict resolver for multi-agent scenarios
/// Resolves conflicts by preferring the most recent change
#[derive(Debug, Clone)]
pub struct TimestampResolver {
    /// Function to extract timestamp from key or value
    timestamp_extractor: fn(&[u8], &[u8]) -> Option<u64>,
}

impl Default for TimestampResolver {
    fn default() -> Self {
        Self::default_resolver()
    }
}

impl TimestampResolver {
    pub fn new(extractor: fn(&[u8], &[u8]) -> Option<u64>) -> Self {
        Self {
            timestamp_extractor: extractor,
        }
    }

    /// Create a default timestamp resolver that tries to parse timestamps from keys
    pub fn default_resolver() -> Self {
        Self::new(|key, _value| {
            // Try to extract timestamp from key
            let key_str = String::from_utf8_lossy(key);
            // Look for timestamp patterns like "timestamp:1234567890:"
            if let Some(ts_start) = key_str.find("timestamp:") {
                let ts_part = &key_str[ts_start + 10..];
                if let Some(ts_end) = ts_part.find(':') {
                    ts_part[..ts_end].parse::<u64>().ok()
                } else {
                    ts_part.parse::<u64>().ok()
                }
            } else {
                None
            }
        })
    }
}

impl ConflictResolver for TimestampResolver {
    fn resolve_conflict(&self, conflict: &MergeConflict) -> Option<MergeResult> {
        match (&conflict.source_value, &conflict.destination_value) {
            (Some(source), Some(dest)) => {
                // Compare timestamps if available
                let source_ts = (self.timestamp_extractor)(&conflict.key, source);
                let dest_ts = (self.timestamp_extractor)(&conflict.key, dest);

                match (source_ts, dest_ts) {
                    (Some(s_ts), Some(d_ts)) => {
                        if s_ts >= d_ts {
                            Some(MergeResult::Modified(conflict.key.clone(), source.clone()))
                        } else {
                            Some(MergeResult::Modified(conflict.key.clone(), dest.clone()))
                        }
                    }
                    (Some(_), None) => {
                        // Source has timestamp, dest doesn't - prefer source
                        Some(MergeResult::Modified(conflict.key.clone(), source.clone()))
                    }
                    (None, Some(_)) => {
                        // Dest has timestamp, source doesn't - prefer dest
                        Some(MergeResult::Modified(conflict.key.clone(), dest.clone()))
                    }
                    (None, None) => {
                        // No timestamps - default to source
                        Some(MergeResult::Modified(conflict.key.clone(), source.clone()))
                    }
                }
            }
            (Some(source), None) => Some(MergeResult::Added(conflict.key.clone(), source.clone())),
            (None, Some(dest)) => Some(MergeResult::Modified(conflict.key.clone(), dest.clone())),
            (None, None) => Some(MergeResult::Removed(conflict.key.clone())),
        }
    }
}

/// Semantic merge resolver for AI agent scenarios
/// Attempts to merge values semantically when they're both JSON or similar structured data
#[derive(Debug, Clone, Default)]
pub struct SemanticMergeResolver;

impl ConflictResolver for SemanticMergeResolver {
    fn resolve_conflict(&self, conflict: &MergeConflict) -> Option<MergeResult> {
        match (&conflict.source_value, &conflict.destination_value) {
            (Some(source), Some(dest)) => {
                // Try to parse as JSON and merge
                if let (Ok(source_json), Ok(dest_json)) = (
                    serde_json::from_slice::<serde_json::Value>(source),
                    serde_json::from_slice::<serde_json::Value>(dest),
                ) {
                    // Attempt semantic merge
                    let merged = Self::merge_json_values(&source_json, &dest_json);
                    if let Ok(merged_bytes) = serde_json::to_vec(&merged) {
                        return Some(MergeResult::Modified(conflict.key.clone(), merged_bytes));
                    }
                }

                // If not JSON or merge failed, prefer source
                Some(MergeResult::Modified(conflict.key.clone(), source.clone()))
            }
            (Some(source), None) => Some(MergeResult::Added(conflict.key.clone(), source.clone())),
            (None, Some(dest)) => Some(MergeResult::Modified(conflict.key.clone(), dest.clone())),
            (None, None) => Some(MergeResult::Removed(conflict.key.clone())),
        }
    }
}

impl SemanticMergeResolver {
    /// Merge two JSON values semantically
    fn merge_json_values(
        source: &serde_json::Value,
        dest: &serde_json::Value,
    ) -> serde_json::Value {
        match (source, dest) {
            (serde_json::Value::Object(source_obj), serde_json::Value::Object(dest_obj)) => {
                // Merge objects by combining keys
                let mut merged = dest_obj.clone();
                for (key, value) in source_obj {
                    if let Some(dest_value) = dest_obj.get(key) {
                        // Key exists in both - recursively merge
                        merged.insert(key.clone(), Self::merge_json_values(value, dest_value));
                    } else {
                        // Key only in source - add it
                        merged.insert(key.clone(), value.clone());
                    }
                }
                serde_json::Value::Object(merged)
            }
            (serde_json::Value::Array(source_arr), serde_json::Value::Array(dest_arr)) => {
                // Simple array merge - combine and deduplicate
                let mut merged = dest_arr.clone();
                for item in source_arr {
                    if !merged.contains(item) {
                        merged.push(item.clone());
                    }
                }
                serde_json::Value::Array(merged)
            }
            _ => {
                // For non-mergeable types, prefer source
                source.clone()
            }
        }
    }
}
