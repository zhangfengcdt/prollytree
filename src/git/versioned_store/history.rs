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

use super::{HistoricalAccess, TreeConfigSaver, VersionedKvStore};
use crate::config::TreeConfig;
use crate::diff::ConflictResolver;
use crate::digest::ValueDigest;
use crate::git::metadata::MetadataBackend;
use crate::git::types::*;
use crate::storage::NodeStorage;
use crate::tree::{ProllyTree, Tree};
use std::collections::HashMap;

impl<const N: usize, S: NodeStorage<N>, M: MetadataBackend> VersionedKvStore<N, S, M>
where
    Self: TreeConfigSaver<N>,
{
    /// Get the current storage backend type
    pub fn storage_backend(&self) -> &StorageBackend {
        &self.storage_backend
    }

    /// Resolve a reference (branch name, commit SHA, etc.) to a commit ID
    /// This is used by all storage types for historical access
    pub(super) fn resolve_commit(&self, reference: &str) -> Result<gix::ObjectId, GitKvError> {
        self.metadata.resolve_reference(reference)
    }

    /// Get the config file path relative to git root
    /// All backends now store dataset_dir, so this is consistent across all storage types
    fn get_config_file_path_relative_to_git_root(&self) -> Result<String, GitKvError> {
        let dataset_dir = self
            .dataset_dir
            .as_ref()
            .ok_or_else(|| GitKvError::GitObjectError("Dataset directory not set".to_string()))?;

        // Use work_dir() for worktree root (handles worktrees/submodules correctly),
        // falling back to find_git_root(dataset_dir) if work_dir is not available
        let git_root = self
            .metadata
            .work_dir()
            .or_else(|| Self::find_git_root(dataset_dir))
            .ok_or_else(|| GitKvError::GitObjectError("Could not find git root".to_string()))?;

        // Calculate relative path from git root to dataset dir
        let relative_path = dataset_dir.strip_prefix(&git_root).map_err(|_| {
            GitKvError::GitObjectError("Dataset directory is not inside git repository".to_string())
        })?;

        // Construct the file path and use '/' separators for git tree paths
        // (git uses forward slashes regardless of platform)
        let config_path = relative_path.join("prolly_config_tree_config");
        let path_str = config_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        Ok(path_str)
    }

    /// Read the tree config from a specific commit
    /// This gets the prolly_config_tree_config file from the commit to extract root hash
    pub(super) fn read_tree_config_from_commit(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<TreeConfig<N>, GitKvError> {
        let config_path = self.get_config_file_path_relative_to_git_root()?;

        match self.metadata.read_file_at_commit(commit_id, &config_path) {
            Ok(config_data) => {
                let tree_config: TreeConfig<N> =
                    serde_json::from_slice(&config_data).map_err(|e| {
                        GitKvError::GitObjectError(format!("Failed to parse tree config: {e}"))
                    })?;
                Ok(tree_config)
            }
            Err(_) => {
                eprintln!("Warning: prolly_config_tree_config not found in commit {commit_id}, using default config");
                Ok(TreeConfig::default())
            }
        }
    }

    /// Collect all key-value pairs from storage using a tree config (with root hash)
    /// This reconstructs the tree state for non-git storage types
    pub(super) fn collect_keys_from_config(
        &self,
        tree_config: &TreeConfig<N>,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Get the root hash from the config
        let root_hash = match tree_config.root_hash.as_ref() {
            Some(hash) => hash,
            None => {
                // If no root hash in config, return empty result
                // This can happen for initial commits or when config wasn't properly saved
                eprintln!("Warning: No root hash in tree config, returning empty key set");
                return Ok(HashMap::new());
            }
        };

        // Reconstruct the tree from storage using the root hash
        let root_node = match self.tree.storage.get_node_by_hash(root_hash) {
            Some(node) => node,
            None => {
                // Root node not found in storage, return empty result
                // This can happen if the historical state is not available in current storage
                eprintln!("Warning: Root node not found in storage for hash {root_hash:?}, returning empty key set");
                return Ok(HashMap::new());
            }
        };

        // Traverse the tree to collect all keys
        let mut result = HashMap::new();
        self.collect_keys_recursive(&root_node, &mut result)?;

        Ok(result)
    }

    /// Recursively collect keys from a node and its children
    pub(super) fn collect_keys_recursive(
        &self,
        node: &crate::node::ProllyNode<N>,
        result: &mut HashMap<Vec<u8>, Vec<u8>>,
    ) -> Result<(), GitKvError> {
        if node.is_leaf {
            // Leaf node: add all key-value pairs
            for (key, value) in node.keys.iter().zip(node.values.iter()) {
                result.insert(key.clone(), value.clone());
            }
        } else {
            // Internal node: recursively visit children
            for value in &node.values {
                // Value contains the hash of the child node
                if value.len() == N {
                    let mut hash_array = [0u8; N];
                    hash_array.copy_from_slice(value);
                    let child_hash = ValueDigest(hash_array);

                    if let Some(child_node) = self.tree.storage.get_node_by_hash(&child_hash) {
                        self.collect_keys_recursive(&child_node, result)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Get commit history for all storage types
    pub(super) fn get_commit_history_generic(&self) -> Result<Vec<CommitInfo>, GitKvError> {
        self.metadata.walk_history(1000)
    }

    /// Generic implementation for get_commits_for_key that works with all storage types
    pub(super) fn get_commits_for_key_generic(
        &self,
        key: &[u8],
    ) -> Result<Vec<CommitInfo>, GitKvError> {
        let mut commit_history = self.get_commit_history_generic()?;

        // Reverse to process in chronological order (oldest first)
        commit_history.reverse();

        let mut commits_with_key_changes = Vec::new();
        let mut previous_value: Option<Vec<u8>> = None; // None = key not present, Some(val) = key present with value

        for commit in commit_history {
            // Get the key value at this commit by reconstructing tree state
            let current_value = {
                if let Ok(tree_config) = self.read_tree_config_from_commit(&commit.id) {
                    if let Ok(keys_at_commit) = self.collect_keys_from_config(&tree_config) {
                        keys_at_commit.get(key).cloned()
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            // Check if the value changed from the previous commit
            let value_changed = previous_value != current_value;

            if value_changed {
                commits_with_key_changes.push(commit);
            }

            previous_value = current_value;
        }

        // Reverse back to newest first for the final result
        commits_with_key_changes.reverse();

        Ok(commits_with_key_changes)
    }

    // ============================================================================
    // Generic checkout, merge, try_merge implementations for all storage backends
    // All backends use Git for version control, so these operations work the same
    // ============================================================================

    /// Get commit ID for a branch (generic version)
    pub(super) fn get_branch_commit_generic(
        &self,
        branch: &str,
    ) -> Result<gix::ObjectId, GitKvError> {
        self.metadata.branch_commit_id(branch)
    }

    /// Get parents of a commit (generic version)
    pub(super) fn get_commit_parents_generic(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<Vec<gix::ObjectId>, GitKvError> {
        self.metadata.commit_parents(commit_id)
    }

    /// Find the merge base (common ancestor) of two branches (generic version)
    pub(super) fn find_merge_base_generic(
        &self,
        branch1: &str,
        branch2: &str,
    ) -> Result<gix::ObjectId, GitKvError> {
        let commit1 = self.get_branch_commit_generic(branch1)?;
        let commit2 = self.get_branch_commit_generic(branch2)?;

        let mut visited1 = std::collections::HashSet::new();
        let mut queue1 = std::collections::VecDeque::new();
        queue1.push_back(commit1);

        while let Some(commit_id) = queue1.pop_front() {
            if visited1.contains(&commit_id) {
                continue;
            }
            visited1.insert(commit_id);

            if let Ok(parents) = self.get_commit_parents_generic(&commit_id) {
                for parent in parents {
                    if !visited1.contains(&parent) {
                        queue1.push_back(parent);
                    }
                }
            }
        }

        let mut visited2 = std::collections::HashSet::new();
        let mut queue2 = std::collections::VecDeque::new();
        queue2.push_back(commit2);

        while let Some(commit_id) = queue2.pop_front() {
            if visited2.contains(&commit_id) {
                continue;
            }
            visited2.insert(commit_id);

            if visited1.contains(&commit_id) {
                return Ok(commit_id);
            }

            if let Ok(parents) = self.get_commit_parents_generic(&commit_id) {
                for parent in parents {
                    if !visited2.contains(&parent) {
                        queue2.push_back(parent);
                    }
                }
            }
        }

        Err(GitKvError::GitObjectError(
            "No common ancestor found".to_string(),
        ))
    }

    /// Reload the tree state from the current HEAD commit (generic version)
    pub(super) fn reload_tree_from_head_generic(&mut self) -> Result<(), GitKvError>
    where
        Self: HistoricalAccess<N>,
    {
        // Get the current HEAD commit
        let head_object_id = self.metadata.head_commit_id()?;

        // Load keys via HistoricalAccess::get_keys_at_ref so the Git backend reads
        // the per-commit `prolly_hash_mappings` blob (via `collect_keys_at_commit`)
        // rather than looking up the commit's root hash in the current in-memory
        // mappings, which can be narrower than the commit's view after a
        // `git reset` / working-tree switch (see GH-162).
        let keys_at_head = self.get_keys_at_ref(&head_object_id.to_hex().to_string())?;

        // Get the config from the commit
        let config = self.read_tree_config_from_commit(&head_object_id)?;

        // Clear and rebuild the tree
        self.tree.config = config;

        // Clear existing data and insert all keys from HEAD
        // Note: This creates a new tree with the same storage
        let storage = self.tree.storage.clone();
        let tree_config = self.tree.config.clone();
        self.tree = ProllyTree::new(storage, tree_config);

        for (key, value) in keys_at_head {
            self.tree.insert(key, value);
        }

        Ok(())
    }

    /// Switch to a different branch or commit (generic version for all backends)
    pub fn checkout_generic(&mut self, branch_or_commit: &str) -> Result<(), GitKvError>
    where
        Self: HistoricalAccess<N>,
    {
        // Clear staging area
        self.staging_area.clear();
        self.save_staging_area()?;

        // Verify the branch exists
        self.metadata.branch_commit_id(branch_or_commit)?;
        self.current_branch = branch_or_commit.to_string();

        // Update HEAD to point to the new branch
        self.metadata.update_head(branch_or_commit)?;

        // Reload the tree from the HEAD commit
        self.reload_tree_from_head_generic()?;

        Ok(())
    }

    /// Merge another branch into the current branch (generic version for all backends)
    pub fn merge_generic<R: ConflictResolver>(
        &mut self,
        source_branch: &str,
        resolver: &R,
    ) -> Result<gix::ObjectId, GitKvError>
    where
        Self: HistoricalAccess<N>,
    {
        let dest_branch = self.current_branch.clone();

        // Find common base commit
        let base_commit = self.find_merge_base_generic(&dest_branch, source_branch)?;

        // Get key-value data from each state via HistoricalAccess::get_keys_at_ref.
        //
        // The Git backend's specialization reads the per-commit `prolly_hash_mappings`
        // blob out of each commit's tree (see `collect_keys_at_commit`), so it works
        // even when the working-tree mappings file was narrowed by `git reset` /
        // `git checkout` back to a single branch's view (see GH-162). Using the
        // generic `collect_keys_from_config` path here would look the commit's root
        // hash up in the *current in-memory* mappings and spuriously return an empty
        // set for roots that only existed on the other branch.
        let source_commit = self.get_branch_commit_generic(source_branch)?;
        let base_kv = self.get_keys_at_ref(&base_commit.to_hex().to_string())?;
        let source_kv = self.get_keys_at_ref(&source_commit.to_hex().to_string())?;

        let mut dest_kv = HashMap::new();

        for key in self.tree.collect_keys() {
            if let Some(value) = self.get(&key) {
                dest_kv.insert(key, value);
            }
        }

        // Perform three-way merge at key-value level
        let mut merge_results = Vec::new();
        let mut all_keys = std::collections::HashSet::new();

        all_keys.extend(base_kv.keys().cloned());
        all_keys.extend(source_kv.keys().cloned());
        all_keys.extend(dest_kv.keys().cloned());

        for key in all_keys {
            let base_value = base_kv.get(&key);
            let source_value = source_kv.get(&key);
            let dest_value = dest_kv.get(&key);

            match (base_value, source_value, dest_value) {
                // Key exists in all three - check for modifications
                (Some(base), Some(source), Some(dest)) => {
                    if base == source && base == dest {
                        // No changes, skip
                        continue;
                    } else if base == dest && base != source {
                        // Only source changed - take source value
                        merge_results.push(crate::diff::MergeResult::Modified(
                            key.clone(),
                            source.clone(),
                        ));
                    } else if base == source && base != dest {
                        // Only dest changed - keep dest value (no-op)
                        continue;
                    } else if source == dest {
                        // Both changed to same value - keep it (no-op)
                        continue;
                    } else {
                        // Conflict: both branches modified differently
                        let conflict = crate::diff::MergeConflict {
                            key: key.clone(),
                            base_value: Some(base.clone()),
                            source_value: Some(source.clone()),
                            destination_value: Some(dest.clone()),
                        };
                        merge_results.push(crate::diff::MergeResult::Conflict(conflict));
                    }
                }
                // Key added in source, not in base or dest
                (None, Some(source), None) => {
                    merge_results
                        .push(crate::diff::MergeResult::Added(key.clone(), source.clone()));
                }
                // Key added in dest, not in base or source - keep it (no-op)
                (None, None, Some(_dest)) => {
                    continue;
                }
                // Key added in both source and dest - potential conflict
                (None, Some(source), Some(dest)) => {
                    if source == dest {
                        // Both added same value - keep it (no-op)
                        continue;
                    } else {
                        // Conflict: both branches added different values
                        let conflict = crate::diff::MergeConflict {
                            key: key.clone(),
                            base_value: None,
                            source_value: Some(source.clone()),
                            destination_value: Some(dest.clone()),
                        };
                        merge_results.push(crate::diff::MergeResult::Conflict(conflict));
                    }
                }
                // Key deleted in source, still exists in dest
                (Some(_base), None, Some(_dest)) => {
                    // Source deleted it - apply deletion
                    merge_results.push(crate::diff::MergeResult::Removed(key.clone()));
                }
                // Key deleted in dest, still exists in source - keep deletion (no-op)
                (Some(_base), Some(_source), None) => {
                    continue;
                }
                // Key deleted in both - no-op
                (Some(_base), None, None) => {
                    continue;
                }
                // All other cases - no action needed
                _ => continue,
            }
        }

        // Apply conflict resolution
        let mut resolved_results = Vec::new();
        let mut unresolved_conflicts = Vec::new();

        for result in merge_results {
            match result {
                crate::diff::MergeResult::Conflict(conflict) => {
                    if let Some(resolved_result) = resolver.resolve_conflict(&conflict) {
                        resolved_results.push(resolved_result);
                    } else {
                        unresolved_conflicts.push(conflict);
                    }
                }
                other => resolved_results.push(other),
            }
        }

        // If there are unresolved conflicts, return them
        if !unresolved_conflicts.is_empty() {
            return Err(GitKvError::MergeConflictError(unresolved_conflicts));
        }

        // Apply resolved merge results directly to current tree
        for result in resolved_results {
            match result {
                crate::diff::MergeResult::Added(key, value) => {
                    self.tree.insert(key, value);
                }
                crate::diff::MergeResult::Removed(key) => {
                    self.tree.delete(&key);
                }
                crate::diff::MergeResult::Modified(key, value) => {
                    self.tree.insert(key, value);
                }
                crate::diff::MergeResult::Conflict(_) => {
                    // Should not happen after resolution
                }
            }
        }

        // Persist tree changes
        self.tree.persist_root();
        self.tree
            .save_config()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to save config: {e}")))?;

        self.save_tree_config_to_git_internal()?;

        // Commit the merge
        let message = format!("Merge branch '{}' into {}", source_branch, dest_branch);
        let commit_id = self.commit(&message)?;

        Ok(commit_id)
    }

    /// Try to merge another branch and return conflicts (generic version)
    ///
    /// This method attempts to merge the source branch into the current branch.
    /// If there are no conflicts, the merge is applied and committed.
    /// If there are conflicts, the merge is not applied and the conflicts are returned.
    pub fn try_merge_generic(&mut self, source_branch: &str) -> Result<gix::ObjectId, GitKvError>
    where
        Self: HistoricalAccess<N> + TreeConfigSaver<N>,
    {
        // Use a NoOpResolver that leaves all conflicts unresolved
        struct NoOpResolver;
        impl crate::diff::ConflictResolver for NoOpResolver {
            fn resolve_conflict(
                &self,
                _conflict: &crate::diff::MergeConflict,
            ) -> Option<crate::diff::MergeResult> {
                None // Leave all conflicts unresolved
            }
        }

        self.merge_generic(source_branch, &NoOpResolver)
    }
}
