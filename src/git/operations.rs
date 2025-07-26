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
use crate::git::versioned_store::GitVersionedKvStore;
use gix::prelude::*;
use std::collections::HashMap;

/// Git operations for versioned KV store
pub struct GitOperations<const N: usize> {
    store: GitVersionedKvStore<N>,
}

impl<const N: usize> GitOperations<N> {
    pub fn new(store: GitVersionedKvStore<N>) -> Self {
        GitOperations { store }
    }

    /// Perform a merge between two branches, focusing on fast-forward merges
    pub fn merge(&mut self, other_branch: &str) -> Result<MergeResult, GitKvError> {
        // Get the current branch state
        let current_branch = self.store.current_branch();

        // Get the commit IDs for both branches
        let current_commit = self.get_branch_commit(current_branch)?;
        let other_commit = self.get_branch_commit(other_branch)?;

        // Check if they're the same (nothing to merge)
        if current_commit == other_commit {
            return Ok(MergeResult::FastForward(current_commit));
        }

        // Check if we can do a fast-forward merge
        if self.is_fast_forward_possible(&current_commit, &other_commit)? {
            // Fast-forward merge: just update HEAD to the other branch
            self.store.checkout(other_branch)?;
            return Ok(MergeResult::FastForward(other_commit));
        }

        // For now, we don't support three-way merges
        // Return a conflict indicating guide merge is needed
        let conflicts = vec![crate::git::types::KvConflict {
            key: b"<merge>".to_vec(),
            base_value: None,
            our_value: Some(b"Cannot automatically merge - guide merge required".to_vec()),
            their_value: Some(b"Use 'git merge' or resolve conflicts manually".to_vec()),
        }];

        Ok(MergeResult::Conflict(conflicts))
    }

    /// Check if a fast-forward merge is possible
    fn is_fast_forward_possible(
        &self,
        current_commit: &gix::ObjectId,
        other_commit: &gix::ObjectId,
    ) -> Result<bool, GitKvError> {
        // Fast-forward is possible if the other commit is a descendant of the current commit
        // This means the current commit should be an ancestor of the other commit
        self.is_ancestor(current_commit, other_commit)
    }

    /// Check if commit A is an ancestor of commit B
    fn is_ancestor(
        &self,
        ancestor: &gix::ObjectId,
        descendant: &gix::ObjectId,
    ) -> Result<bool, GitKvError> {
        // If they're the same, ancestor relationship is true
        if ancestor == descendant {
            return Ok(true);
        }

        // Walk through the parents of the descendant commit
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(*descendant);

        while let Some(current_commit) = queue.pop_front() {
            if visited.contains(&current_commit) {
                continue;
            }
            visited.insert(current_commit);

            // If we found the ancestor, return true
            if current_commit == *ancestor {
                return Ok(true);
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

        // If we didn't find the ancestor, return false
        Ok(false)
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
        let commit_obj = self
            .store
            .git_repo()
            .objects
            .find(&commit_id, &mut buffer)
            .map_err(|e| GitKvError::GitObjectError(format!("Commit not found: {e}")))?;

        let commit = match commit_obj.decode() {
            Ok(gix::objs::ObjectRef::Commit(commit)) => commit,
            _ => {
                return Err(GitKvError::GitObjectError(
                    "Object is not a commit".to_string(),
                ))
            }
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
        let current_head = self
            .store
            .git_repo()
            .head_id()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get HEAD: {e}")))?;

        if *commit_id == current_head {
            // For current HEAD, use the current state
            return self.get_current_kv_state();
        }

        // Reconstruct the ProllyTree state from the specific commit
        self.reconstruct_kv_state_from_commit(commit_id)
    }

    /// Reconstruct KV state from a specific commit by temporarily switching to it
    fn reconstruct_kv_state_from_commit(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Create a temporary store to reconstruct the state
        let current_dir = std::env::current_dir()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get current dir: {e}")))?;

        // Create a temporary clone of the versioned store
        let mut temp_store = GitVersionedKvStore::<N>::open(&current_dir)?;

        // Save current state
        let original_branch = temp_store.current_branch().to_string();

        // Switch to the target commit temporarily
        let result = self.checkout_commit_temporarily(&mut temp_store, commit_id);

        // Restore original state
        if let Err(e) = temp_store.checkout(&original_branch) {
            // Log error but continue with the result we got
            eprintln!("Warning: Failed to restore original branch {original_branch}: {e}");
        }

        result
    }

    /// Temporarily checkout a commit and extract its KV state
    fn checkout_commit_temporarily(
        &self,
        store: &mut GitVersionedKvStore<N>,
        commit_id: &gix::ObjectId,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Update the store to point to the specific commit
        // This is a simplified approach - we'll try to reconstruct from the commit

        // For now, we'll create a temporary directory and checkout the commit there
        // This is a workaround until we implement full historical state reconstruction
        let temp_dir = std::env::temp_dir().join(format!("prolly_temp_{}", commit_id.to_hex()));

        // Create temporary directory
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to create temp dir: {e}")))?;

        // Use git to checkout the specific commit in the temp directory
        let output = std::process::Command::new("git")
            .args([
                "clone",
                "--quiet",
                store.git_repo().path().to_str().unwrap_or("."),
                temp_dir.to_str().unwrap_or("."),
            ])
            .output()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to clone repo: {e}")))?;

        if !output.status.success() {
            return Err(GitKvError::GitObjectError(format!(
                "Git clone failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Checkout the specific commit
        let output = std::process::Command::new("git")
            .args(["checkout", "--quiet", &commit_id.to_hex().to_string()])
            .current_dir(&temp_dir)
            .output()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to checkout commit: {e}")))?;

        if !output.status.success() {
            // Clean up temp directory
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(GitKvError::GitObjectError(format!(
                "Git checkout failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Try to open the store at the temp location
        let dataset_dir = temp_dir.join("dataset");
        let result = if dataset_dir.exists() {
            match GitVersionedKvStore::<N>::open(&dataset_dir) {
                Ok(temp_store) => self.get_current_kv_state_from_store(&temp_store),
                Err(_) => {
                    // If we can't open the store, return empty state
                    Ok(HashMap::new())
                }
            }
        } else {
            // No dataset directory, return empty state
            Ok(HashMap::new())
        };

        // Clean up temp directory
        let _ = std::fs::remove_dir_all(&temp_dir);

        result
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
    fn get_current_kv_state_from_store(
        &self,
        store: &GitVersionedKvStore<N>,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
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
        let store = GitVersionedKvStore::<32>::init(&dataset_dir).unwrap();
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
        let store = GitVersionedKvStore::<32>::init(&dataset_dir).unwrap();
        let ops = GitOperations::new(store);

        // Test HEAD parsing
        let head_id = ops.parse_commit_id("HEAD");
        assert!(head_id.is_ok());
    }
}
