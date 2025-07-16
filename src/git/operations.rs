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

use crate::git::types::*;
use crate::git::versioned_store::VersionedKvStore;
use std::collections::HashMap;

/// Git operations for versioned KV store
pub struct GitOperations<const N: usize> {
    store: VersionedKvStore<N>,
}

impl<const N: usize> GitOperations<N> {
    pub fn new(store: VersionedKvStore<N>) -> Self {
        GitOperations { store }
    }

    /// Perform a three-way merge between two branches
    pub fn merge(&mut self, other_branch: &str) -> Result<MergeResult, GitKvError> {
        // Get the current branch state
        let current_branch = self.store.current_branch().to_string();

        // Find the common ancestor (merge base)
        let merge_base = self.find_merge_base(&current_branch, other_branch)?;

        // Get the states at each commit
        let base_state = self.get_kv_state_at_commit(&merge_base)?;
        let our_state = self.get_current_kv_state()?;
        let their_state = self.get_kv_state_at_branch(other_branch)?;

        // Perform three-way merge
        let merge_result = self.perform_three_way_merge(&base_state, &our_state, &their_state)?;

        match merge_result {
            MergeResult::Conflict(conflicts) => {
                // Return conflicts for user resolution
                Ok(MergeResult::Conflict(conflicts))
            }
            MergeResult::FastForward(commit_id) => {
                // Update HEAD to the target commit
                self.store.checkout(&commit_id.to_string())?;
                Ok(MergeResult::FastForward(commit_id))
            }
            MergeResult::ThreeWay(_commit_id) => {
                // The merge was successful, commit the result
                let final_commit = self
                    .store
                    .commit(&format!("Merge branch '{other_branch}'"))?;
                Ok(MergeResult::ThreeWay(final_commit))
            }
        }
    }

    /// Generate a diff between two branches or commits
    pub fn diff(&self, from: &str, to: &str) -> Result<Vec<KvDiff>, GitKvError> {
        let from_state = self.get_kv_state_at_branch(from)?;
        let to_state = self.get_kv_state_at_branch(to)?;

        let mut diffs = Vec::new();
        let mut all_keys = std::collections::HashSet::new();

        // Collect all keys from both states
        for key in from_state.keys() {
            all_keys.insert(key.clone());
        }
        for key in to_state.keys() {
            all_keys.insert(key.clone());
        }

        // Compare each key
        for key in all_keys {
            let from_value = from_state.get(&key);
            let to_value = to_state.get(&key);

            let operation = match (from_value, to_value) {
                (None, Some(value)) => DiffOperation::Added(value.clone()),
                (Some(value), None) => DiffOperation::Removed(value.clone()),
                (Some(old), Some(new)) => {
                    if old != new {
                        DiffOperation::Modified {
                            old: old.clone(),
                            new: new.clone(),
                        }
                    } else {
                        continue; // No change
                    }
                }
                (None, None) => continue, // Shouldn't happen
            };

            diffs.push(KvDiff { key, operation });
        }

        Ok(diffs)
    }

    /// Show the KV state at a specific commit
    pub fn show(&self, commit: &str) -> Result<CommitDetails, GitKvError> {
        // Parse commit ID
        let commit_id = self.parse_commit_id(commit)?;

        // Get commit info (simplified)
        let info = CommitInfo {
            id: commit_id,
            author: "Unknown".to_string(),
            committer: "Unknown".to_string(),
            message: "Commit".to_string(),
            timestamp: 0,
        };

        // Get parent commits (simplified)
        let parent_ids: Vec<gix::ObjectId> = vec![];

        // Generate diff from parent (if exists)
        let changes = if let Some(parent_id) = parent_ids.first() {
            self.diff(&parent_id.to_string(), commit)?
        } else {
            // Root commit - show all keys as added
            let state = self.get_kv_state_at_commit(&commit_id)?;
            state
                .iter()
                .map(|(key, value)| KvDiff {
                    key: key.clone(),
                    operation: DiffOperation::Added(value.clone()),
                })
                .collect()
        };

        Ok(CommitDetails {
            info,
            changes,
            parent_ids,
        })
    }

    /// Revert a commit
    pub fn revert(&mut self, commit: &str) -> Result<(), GitKvError> {
        let _commit_id = self.parse_commit_id(commit)?;

        // Get the changes in the commit
        let details = self.show(commit)?;

        // Apply the reverse of each change
        for diff in details.changes {
            match diff.operation {
                DiffOperation::Added(_) => {
                    // If it was added, delete it
                    self.store.delete(&diff.key)?;
                }
                DiffOperation::Removed(value) => {
                    // If it was removed, add it back
                    self.store.insert(diff.key, value)?;
                }
                DiffOperation::Modified { old, new: _ } => {
                    // If it was modified, revert to old value
                    self.store.insert(diff.key, old)?;
                }
            }
        }

        // Commit the revert
        let message = format!("Revert \"{}\"", details.info.message);
        self.store.commit(&message)?;

        Ok(())
    }

    /// Find the merge base between two branches
    fn find_merge_base(&self, branch1: &str, branch2: &str) -> Result<gix::ObjectId, GitKvError> {
        // Simplified implementation - just return the first common ancestor
        // In a real implementation, you'd use proper merge base algorithms
        let commit1 = self.get_branch_commit(branch1)?;
        let _commit2 = self.get_branch_commit(branch2)?;

        // For now, just return the first commit as a placeholder
        Ok(commit1)
    }

    /// Get the commit ID for a branch
    fn get_branch_commit(&self, _branch: &str) -> Result<gix::ObjectId, GitKvError> {
        // Simplified implementation
        Ok(gix::ObjectId::null(gix::hash::Kind::Sha1))
    }

    /// Get KV state at a specific commit
    fn get_kv_state_at_commit(
        &self,
        _commit_id: &gix::ObjectId,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // This is a simplified implementation
        // In reality, we'd need to reconstruct the ProllyTree from the Git objects
        Ok(HashMap::new())
    }

    /// Get KV state at a specific branch
    fn get_kv_state_at_branch(
        &self,
        branch: &str,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        let commit_id = self.get_branch_commit(branch)?;
        self.get_kv_state_at_commit(&commit_id)
    }

    /// Get current KV state
    fn get_current_kv_state(&self) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // This would collect all current KV pairs
        Ok(HashMap::new())
    }

    /// Perform a three-way merge
    fn perform_three_way_merge(
        &self,
        base: &HashMap<Vec<u8>, Vec<u8>>,
        ours: &HashMap<Vec<u8>, Vec<u8>>,
        theirs: &HashMap<Vec<u8>, Vec<u8>>,
    ) -> Result<MergeResult, GitKvError> {
        let mut conflicts = Vec::new();

        // Collect all keys
        let mut all_keys = std::collections::HashSet::new();
        for key in base.keys() {
            all_keys.insert(key.clone());
        }
        for key in ours.keys() {
            all_keys.insert(key.clone());
        }
        for key in theirs.keys() {
            all_keys.insert(key.clone());
        }

        // Check for conflicts
        for key in all_keys {
            let base_value = base.get(&key);
            let our_value = ours.get(&key);
            let their_value = theirs.get(&key);

            // Detect conflicts
            if base_value != our_value && base_value != their_value && our_value != their_value {
                conflicts.push(KvConflict {
                    key: key.clone(),
                    base_value: base_value.cloned(),
                    our_value: our_value.cloned(),
                    their_value: their_value.cloned(),
                });
            }
        }

        if conflicts.is_empty() {
            // No conflicts, create merge commit
            Ok(MergeResult::ThreeWay(gix::ObjectId::null(
                gix::hash::Kind::Sha1,
            )))
        } else {
            Ok(MergeResult::Conflict(conflicts))
        }
    }

    /// Parse a commit ID from a string
    fn parse_commit_id(&self, commit: &str) -> Result<gix::ObjectId, GitKvError> {
        // Handle special cases (simplified)
        match commit {
            "HEAD" => {
                // Return a placeholder for HEAD
                Ok(gix::ObjectId::null(gix::hash::Kind::Sha1))
            }
            _ => {
                // Try to parse as hex string
                gix::ObjectId::from_hex(commit.as_bytes())
                    .map_err(|e| GitKvError::InvalidCommit(format!("Invalid commit ID: {e}")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_git_operations_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = VersionedKvStore::<32>::init(temp_dir.path()).unwrap();
        let _ops = GitOperations::new(store);
    }

    #[test]
    fn test_parse_commit_id() {
        let temp_dir = TempDir::new().unwrap();
        let store = VersionedKvStore::<32>::init(temp_dir.path()).unwrap();
        let ops = GitOperations::new(store);

        // Test HEAD parsing
        let head_id = ops.parse_commit_id("HEAD");
        assert!(head_id.is_ok());
    }
}
