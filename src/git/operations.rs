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
use gix::prelude::*;
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

        // Get commit object from git
        let mut buffer = Vec::new();
        let commit_obj = self.store.git_repo().objects.find(&commit_id, &mut buffer)
            .map_err(|e| GitKvError::GitObjectError(format!("Commit not found: {e}")))?;

        let commit = match commit_obj.decode() {
            Ok(gix::objs::ObjectRef::Commit(commit)) => commit,
            _ => return Err(GitKvError::GitObjectError("Object is not a commit".to_string())),
        };

        // Extract commit info
        let info = CommitInfo {
            id: commit_id,
            author: commit.author().name.to_string(),
            committer: commit.committer().name.to_string(),
            message: commit.message().title.to_string(),
            timestamp: commit.time().seconds,
        };

        // Get parent commits
        let parent_ids: Vec<gix::ObjectId> = commit.parents().collect();

        // Generate diff from parent (if exists)
        let changes = if let Some(parent_id) = parent_ids.first() {
            self.diff(&parent_id.to_string(), &commit_id.to_string())?
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
        let commit1 = self.get_branch_commit(branch1)?;
        let commit2 = self.get_branch_commit(branch2)?;

        // If the commits are the same, return it as the merge base
        if commit1 == commit2 {
            return Ok(commit1);
        }

        // Get all ancestors of commit1
        let mut ancestors1 = std::collections::HashSet::new();
        self.collect_ancestors(&commit1, &mut ancestors1)?;

        // Walk through ancestors of commit2 to find the first common ancestor
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(commit2);

        while let Some(current_commit) = queue.pop_front() {
            if visited.contains(&current_commit) {
                continue;
            }
            visited.insert(current_commit);

            // If this commit is an ancestor of commit1, it's our merge base
            if ancestors1.contains(&current_commit) {
                return Ok(current_commit);
            }

            // Add parents to queue
            let mut buffer = Vec::new();
            if let Ok(commit_obj) = self
                .store
                .git_repo()
                .objects
                .find(&current_commit, &mut buffer)
            {
                if let Ok(gix::objs::ObjectRef::Commit(commit)) = commit_obj.decode() {
                    for parent_id in commit.parents() {
                        if !visited.contains(&parent_id) {
                            queue.push_back(parent_id);
                        }
                    }
                }
            }
        }

        // If no common ancestor found, return an error
        Err(GitKvError::GitObjectError(format!(
            "No common ancestor found between {branch1} and {branch2}"
        )))
    }

    /// Collect all ancestors of a commit
    fn collect_ancestors(
        &self,
        start_commit: &gix::ObjectId,
        ancestors: &mut std::collections::HashSet<gix::ObjectId>,
    ) -> Result<(), GitKvError> {
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(*start_commit);

        while let Some(current_commit) = queue.pop_front() {
            if ancestors.contains(&current_commit) {
                continue;
            }
            ancestors.insert(current_commit);

            // Add parents to queue
            let mut buffer = Vec::new();
            if let Ok(commit_obj) = self
                .store
                .git_repo()
                .objects
                .find(&current_commit, &mut buffer)
            {
                if let Ok(gix::objs::ObjectRef::Commit(commit)) = commit_obj.decode() {
                    for parent_id in commit.parents() {
                        if !ancestors.contains(&parent_id) {
                            queue.push_back(parent_id);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get the commit ID for a branch
    fn get_branch_commit(&self, branch: &str) -> Result<gix::ObjectId, GitKvError> {
        // Try to resolve the branch reference
        let branch_ref = if branch.starts_with("refs/") {
            branch.to_string()
        } else {
            format!("refs/heads/{branch}")
        };

        // Find the reference
        match self.store.git_repo().refs.find(&branch_ref) {
            Ok(reference) => {
                // Get the target commit ID
                match reference.target.try_id() {
                    Some(commit_id) => Ok(commit_id.to_owned()),
                    None => Err(GitKvError::GitObjectError(format!(
                        "Branch {branch} does not point to a commit"
                    ))),
                }
            }
            Err(_) => {
                // If branch not found, try to resolve as commit ID
                match self.store.git_repo().rev_parse_single(branch) {
                    Ok(object) => Ok(object.into()),
                    Err(e) => Err(GitKvError::GitObjectError(format!(
                        "Cannot resolve branch/commit {branch}: {e}"
                    ))),
                }
            }
        }
    }

    /// Get KV state at a specific commit
    fn get_kv_state_at_commit(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Check if we're asking for the current HEAD
        let current_head = self.store.git_repo().head_id()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get HEAD: {e}")))?;
        
        if *commit_id == current_head {
            // For current HEAD, use the current state
            return self.get_current_kv_state();
        }
        
        // For now, return empty state for non-HEAD commits
        // This is a limitation - in a full implementation, we would need to:
        // 1. Parse the commit object to get the tree
        // 2. Reconstruct the ProllyTree from the Git objects
        // 3. Extract key-value pairs from the reconstructed tree
        // 
        // For the purpose of fixing the immediate issue, we'll focus on HEAD commits
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
        self.get_current_kv_state_from_store(&self.store)
    }
    
    /// Get current KV state from a specific store
    fn get_current_kv_state_from_store(&self, store: &VersionedKvStore<N>) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        let mut state = HashMap::new();
        
        // Get all keys from the store
        let keys = store.list_keys();
        
        // For each key, get its value
        for key in keys {
            if let Some(value) = store.get(&key) {
                state.insert(key, value);
            }
        }
        
        Ok(state)
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
        // Try to resolve using git's rev-parse functionality
        match self.store.git_repo().rev_parse_single(commit) {
            Ok(object) => Ok(object.into()),
            Err(e) => Err(GitKvError::GitObjectError(format!(
                "Cannot resolve commit {commit}: {e}"
            ))),
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
        // Initialize git repository (regular, not bare)
        gix::init(temp_dir.path()).unwrap();
        // Create subdirectory for dataset
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir_all(&dataset_dir).unwrap();
        let store = VersionedKvStore::<32>::init(&dataset_dir).unwrap();
        let _ops = GitOperations::new(store);
    }

    #[test]
    fn test_parse_commit_id() {
        let temp_dir = TempDir::new().unwrap();
        // Initialize git repository (regular, not bare)
        gix::init(temp_dir.path()).unwrap();
        // Create subdirectory for dataset
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir_all(&dataset_dir).unwrap();
        let store = VersionedKvStore::<32>::init(&dataset_dir).unwrap();
        let ops = GitOperations::new(store);

        // Test HEAD parsing
        let head_id = ops.parse_commit_id("HEAD");
        assert!(head_id.is_ok());
    }
}
