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
use crate::git::types::*;
use crate::storage::NodeStorage;
use crate::tree::{ProllyTree, Tree};
use gix::prelude::*;
use std::collections::HashMap;

impl<const N: usize, S: NodeStorage<N>> VersionedKvStore<N, S>
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
        // Try to resolve as a branch first
        if let Ok(mut branch_ref) = self
            .git_repo
            .find_reference(&format!("refs/heads/{reference}"))
        {
            // Try to peel the reference to get the commit ID
            if let Ok(peeled) = branch_ref.peel_to_id_in_place() {
                return Ok(peeled.detach());
            }
        }

        // Try to resolve as a commit SHA
        if let Ok(commit_id) = gix::ObjectId::from_hex(reference.as_bytes()) {
            // Verify the commit exists by trying to find it
            let mut buffer = Vec::new();
            if self.git_repo.objects.find(&commit_id, &mut buffer).is_ok() {
                return Ok(commit_id);
            }
        }

        // Try other reference formats (tags, etc.)
        if let Ok(mut reference) = self.git_repo.find_reference(reference) {
            // Try to peel the reference to get the commit ID
            if let Ok(peeled) = reference.peel_to_id_in_place() {
                return Ok(peeled.detach());
            }
        }

        Err(GitKvError::InvalidCommit(format!(
            "Reference '{reference}' not found"
        )))
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
            .git_repo
            .work_dir()
            .map(|p| p.to_path_buf())
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
        // Get the config file path relative to git root
        let config_path = self.get_config_file_path_relative_to_git_root()?;

        // Get the commit object
        let mut commit_buffer = Vec::new();
        let commit_obj = self
            .git_repo
            .objects
            .find(commit_id, &mut commit_buffer)
            .map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to find commit {commit_id}: {e}"))
            })?;

        let commit = match commit_obj.kind {
            gix::object::Kind::Commit => gix::objs::CommitRef::from_bytes(commit_obj.data)
                .map_err(|e| GitKvError::GitObjectError(format!("Failed to parse commit: {e}")))?,
            _ => {
                return Err(GitKvError::InvalidCommit(format!(
                    "{commit_id} is not a commit"
                )))
            }
        };

        // Get the tree object
        let tree_id = commit.tree();

        // Try to read the config file, with fallback to current config if not found
        match self.read_file_from_tree(&tree_id, &config_path) {
            Ok(config_data) => {
                // Parse the config
                let tree_config: TreeConfig<N> =
                    serde_json::from_slice(&config_data).map_err(|e| {
                        GitKvError::GitObjectError(format!("Failed to parse tree config: {e}"))
                    })?;
                Ok(tree_config)
            }
            Err(_) => {
                // If config file is not found in commit, create a default config
                // This can happen for commits that don't have prolly config saved
                // or for initial commits before the config system was in place
                eprintln!("Warning: prolly_config_tree_config not found in commit {commit_id}, using default config");
                Ok(TreeConfig::default())
            }
        }
    }

    /// Read a file from a git tree (helper for all storage types)
    pub(super) fn read_file_from_tree(
        &self,
        tree_id: &gix::ObjectId,
        file_path: &str,
    ) -> Result<Vec<u8>, GitKvError> {
        let mut tree_buffer = Vec::new();
        let tree_obj = self
            .git_repo
            .objects
            .find(tree_id, &mut tree_buffer)
            .map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to find tree {tree_id}: {e}"))
            })?;

        let tree = match tree_obj.kind {
            gix::object::Kind::Tree => gix::objs::TreeRef::from_bytes(tree_obj.data)
                .map_err(|e| GitKvError::GitObjectError(format!("Failed to parse tree: {e}")))?,
            _ => {
                return Err(GitKvError::GitObjectError(format!(
                    "{tree_id} is not a tree"
                )))
            }
        };

        // Handle file paths with subdirectories
        let path_parts: Vec<&str> = file_path.split('/').collect();
        self.read_file_from_tree_recursive(&tree, &path_parts, 0)
    }

    /// Recursively read a file from a tree, handling subdirectories
    pub(super) fn read_file_from_tree_recursive(
        &self,
        tree: &gix::objs::TreeRef,
        path_parts: &[&str],
        part_index: usize,
    ) -> Result<Vec<u8>, GitKvError> {
        if part_index >= path_parts.len() {
            return Err(GitKvError::GitObjectError(
                "Path traversal error".to_string(),
            ));
        }

        let current_part = path_parts[part_index];

        // Search for the current path part in the tree
        for entry in &tree.entries {
            if entry.filename == current_part.as_bytes() {
                if part_index == path_parts.len() - 1 {
                    // This is the final part (the file), read its content
                    let mut file_buffer = Vec::new();
                    let file_obj = self
                        .git_repo
                        .objects
                        .find(entry.oid, &mut file_buffer)
                        .map_err(|e| {
                            GitKvError::GitObjectError(format!("Failed to find file object: {e}"))
                        })?;

                    match file_obj.kind {
                        gix::object::Kind::Blob => return Ok(file_obj.data.to_vec()),
                        _ => {
                            return Err(GitKvError::GitObjectError(
                                "File is not a blob".to_string(),
                            ))
                        }
                    }
                } else {
                    // This is a directory, recurse into it
                    let mut subtree_buffer = Vec::new();
                    let subtree_obj = self
                        .git_repo
                        .objects
                        .find(entry.oid, &mut subtree_buffer)
                        .map_err(|e| {
                            GitKvError::GitObjectError(format!(
                                "Failed to find subtree object: {e}"
                            ))
                        })?;

                    match subtree_obj.kind {
                        gix::object::Kind::Tree => {
                            let subtree = gix::objs::TreeRef::from_bytes(subtree_obj.data)
                                .map_err(|e| {
                                    GitKvError::GitObjectError(format!(
                                        "Failed to parse subtree: {e}"
                                    ))
                                })?;
                            return self.read_file_from_tree_recursive(
                                &subtree,
                                path_parts,
                                part_index + 1,
                            );
                        }
                        _ => {
                            return Err(GitKvError::GitObjectError(
                                "Expected directory but found file".to_string(),
                            ))
                        }
                    }
                }
            }
        }

        Err(GitKvError::GitObjectError(format!(
            "Path component '{current_part}' not found in tree"
        )))
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

    /// Get commit history for all storage types using Git
    pub(super) fn get_commit_history_generic(&self) -> Result<Vec<CommitInfo>, GitKvError> {
        let mut commit_infos = Vec::new();

        // Get HEAD commit
        let mut head_ref = self
            .git_repo
            .head_ref()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get HEAD: {e}")))?
            .ok_or_else(|| GitKvError::GitObjectError("HEAD not found".to_string()))?;

        // Peel the reference to get the commit ID
        let peeled_head = head_ref
            .peel_to_id_in_place()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to peel HEAD: {e}")))?;
        let mut current_commit_id = peeled_head.detach();

        // Walk through the commit history
        loop {
            let mut commit_buffer = Vec::new();
            let commit_obj = self
                .git_repo
                .objects
                .find(&current_commit_id, &mut commit_buffer)
                .map_err(|e| GitKvError::GitObjectError(format!("Failed to find commit: {e}")))?;

            let commit = match commit_obj.kind {
                gix::object::Kind::Commit => gix::objs::CommitRef::from_bytes(commit_obj.data)
                    .map_err(|e| {
                        GitKvError::GitObjectError(format!("Failed to parse commit: {e}"))
                    })?,
                _ => break,
            };

            // Create CommitInfo
            let commit_info = CommitInfo {
                id: current_commit_id,
                author: commit.author().name.to_string(),
                committer: commit.committer().name.to_string(),
                message: String::from_utf8_lossy(commit.message).to_string(),
                timestamp: commit.author().time.seconds,
            };

            commit_infos.push(commit_info);

            // Move to parent commit
            if let Some(parent_id) = commit.parents.first() {
                if let Ok(parent_oid) = gix::ObjectId::from_hex(parent_id) {
                    current_commit_id = parent_oid;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(commit_infos)
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
        let branch_ref = format!("refs/heads/{branch}");
        match self.git_repo.refs.find(&branch_ref) {
            Ok(reference) => match reference.target.try_id() {
                Some(commit_id) => Ok(commit_id.to_owned()),
                None => Err(GitKvError::GitObjectError(format!(
                    "Branch {branch} does not point to a commit"
                ))),
            },
            Err(_) => Err(GitKvError::BranchNotFound(branch.to_string())),
        }
    }

    /// Get parents of a commit (generic version)
    pub(super) fn get_commit_parents_generic(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<Vec<gix::ObjectId>, GitKvError> {
        let mut buffer = Vec::new();
        let commit = self
            .git_repo
            .objects
            .find(commit_id, &mut buffer)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to find commit: {e}")))?;

        let commit_ref = commit
            .decode()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to decode commit: {e}")))?
            .into_commit()
            .ok_or_else(|| GitKvError::GitObjectError("Object is not a commit".to_string()))?;

        Ok(commit_ref.parents().collect())
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
        let head = self
            .git_repo
            .head()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get HEAD: {e}")))?;

        let head_commit_id = head.id().ok_or_else(|| {
            GitKvError::GitObjectError("HEAD does not point to a commit".to_string())
        })?;

        let head_object_id = head_commit_id.detach();

        // Load all key-value pairs from the HEAD commit using HistoricalAccess
        let keys_at_head = self.collect_keys_from_commit_generic(&head_object_id)?;

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

    /// Collect all key-value pairs from a specific commit (generic version)
    pub(super) fn collect_keys_from_commit_generic(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Read the tree config from the commit
        let tree_config = self.read_tree_config_from_commit(commit_id)?;

        // Use the generic collect_keys_from_config which works for all storage types
        self.collect_keys_from_config(&tree_config)
    }

    /// Switch to a different branch or commit (generic version for all backends)
    pub fn checkout_generic(&mut self, branch_or_commit: &str) -> Result<(), GitKvError>
    where
        Self: HistoricalAccess<N>,
    {
        // Clear staging area
        self.staging_area.clear();
        self.save_staging_area()?;

        // Update HEAD to point to the new branch/commit
        let target_ref = if branch_or_commit.starts_with("refs/") {
            branch_or_commit.to_string()
        } else {
            format!("refs/heads/{branch_or_commit}")
        };

        // Check if the reference exists
        match self.git_repo.refs.find(&target_ref) {
            Ok(_reference) => {
                self.current_branch = branch_or_commit.to_string();

                // Update HEAD to point to the new branch
                let head_file = self.git_repo.path().join("HEAD");
                let head_content = format!("ref: refs/heads/{branch_or_commit}");
                std::fs::write(&head_file, head_content).map_err(|e| {
                    GitKvError::GitObjectError(format!("Failed to update HEAD: {e}"))
                })?;
            }
            Err(_) => {
                return Err(GitKvError::BranchNotFound(branch_or_commit.to_string()));
            }
        }

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

        // Get key-value data from each state
        let base_kv = self.collect_keys_from_commit_generic(&base_commit)?;
        let source_commit = self.get_branch_commit_generic(source_branch)?;
        let source_kv = self.collect_keys_from_commit_generic(&source_commit)?;
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
