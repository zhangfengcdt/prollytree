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

use super::{HistoricalAccess, HistoricalCommitAccess, TreeConfigSaver, VersionedKvStore};
use crate::config::TreeConfig;
use crate::diff::{ConflictResolver, IgnoreConflictsResolver};
use crate::digest::ValueDigest;
use crate::git::metadata::{GitMetadataBackend, MetadataBackend};
use crate::git::types::*;
use crate::storage::{FileNodeStorage, GitNodeStorage, InMemoryNodeStorage};
use crate::tree::{ProllyTree, Tree};
use gix::prelude::*;
use std::collections::HashMap;
use std::path::Path;

#[cfg(feature = "rocksdb_storage")]
use crate::storage::RocksDBNodeStorage;

// Implement TreeConfigSaver for GitNodeStorage
impl<const N: usize> TreeConfigSaver<N>
    for VersionedKvStore<N, GitNodeStorage<N>, GitMetadataBackend>
{
    fn save_tree_config_to_git_internal(&self) -> Result<(), GitKvError> {
        self.save_tree_config_to_git()
    }
}

// Storage-specific implementations
impl<const N: usize> VersionedKvStore<N, GitNodeStorage<N>, GitMetadataBackend> {
    /// Get access to the git repository (for internal use and backward compatibility)
    pub fn git_repo(&self) -> &gix::Repository {
        self.metadata.repo()
    }

    /// Save both tree config and hash mappings to git for GitNodeStorage
    fn save_tree_config_to_git(&self) -> Result<(), GitKvError> {
        // For GitNodeStorage, we need to ensure the config and hash mappings are
        // available in the dataset directory so they can be committed to git

        // Get the current tree configuration
        let config = self.tree.config.clone();

        // Serialize the configuration to JSON
        let config_json = serde_json::to_string_pretty(&config)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to serialize config: {e}")))?;

        // Write config to the dataset directory
        let config_path = self
            .tree
            .storage
            .dataset_dir()
            .join("prolly_config_tree_config");
        std::fs::write(&config_path, config_json)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to write config file: {e}")))?;

        // Get hash mappings from storage and save them, sorted by hash bytes so that
        // the on-disk file is byte-deterministic for a given set of mappings. Without
        // this, HashMap iteration order varies between processes and `git status`
        // spuriously reports `prolly_hash_mappings` as modified after checkout /
        // reload even though the logical mapping set is unchanged (see GH-161).
        let mappings = self.tree.storage.get_hash_mappings();
        let mut entries: Vec<(String, String)> = mappings
            .iter()
            .map(|(hash, object_id)| {
                let hash_hex: String = hash.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
                (hash_hex, object_id.to_hex().to_string())
            })
            .collect();
        entries.sort();
        let mut mappings_content = String::new();
        for (hash_hex, object_hex) in &entries {
            mappings_content.push_str(&format!("{hash_hex}:{object_hex}\n"));
        }

        // Write mappings to the dataset directory
        let mappings_path = self.tree.storage.dataset_dir().join("prolly_hash_mappings");
        std::fs::write(&mappings_path, mappings_content).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to write mappings file: {e}"))
        })?;

        Ok(())
    }

    /// Git-specific checkout that reloads tree state from the target commit
    pub fn checkout(&mut self, branch_or_commit: &str) -> Result<(), GitKvError> {
        // Call the generic checkout to handle HEAD reference update
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
        match self.metadata.repo().refs.find(&target_ref) {
            Ok(_reference) => {
                // Update our internal tracking
                self.current_branch = branch_or_commit.to_string();

                // Update HEAD to point to the resolved reference
                let head_file = self.metadata.repo().path().join("HEAD");
                let head_content = format!("ref: {target_ref}\n");
                std::fs::write(&head_file, head_content).map_err(|e| {
                    GitKvError::GitObjectError(format!("Failed to update HEAD: {e}"))
                })?;
            }
            Err(_) => {
                return Err(GitKvError::BranchNotFound(branch_or_commit.to_string()));
            }
        }

        // Sync working tree and index under the dataset dir to match the new HEAD
        // so `git status` is clean afterward (see GH-161).
        self.sync_working_tree_to_head()?;

        // Git-specific: Reload the tree from the HEAD commit of the target branch
        self.reload_tree_from_head()?;

        Ok(())
    }

    /// Merge another branch into the current branch with configurable conflict resolution
    ///
    /// This method performs a three-way merge using the ProllyTree merge functionality.
    /// It merges changes from the source branch into the current (destination) branch.
    ///
    /// # Arguments
    ///
    /// * `source_branch` - The branch to merge from
    /// * `resolver` - Conflict resolver to handle merge conflicts
    ///
    /// # Returns
    ///
    /// Returns Ok(commit_id) if merge succeeded, or Err with unresolved conflicts
    pub fn merge<R: ConflictResolver>(
        &mut self,
        source_branch: &str,
        resolver: &R,
    ) -> Result<gix::ObjectId, GitKvError> {
        // Get current branch name
        let dest_branch = self.current_branch.clone();

        // Find common base commit
        let base_commit = self.find_merge_base(&dest_branch, source_branch)?;

        // Note: We don't need the tree root hashes for key-value level merge
        // let base_root = self.get_tree_root_at_commit(&base_commit)?;
        // let source_root = self.get_tree_root_at_branch(source_branch)?;
        // let dest_root = self.tree.get_root_hash().unwrap(); // Current tree

        // Perform the merge at the key-value level instead of tree level
        // Get the actual key-value data from each branch
        let base_kv = self.collect_keys_at_commit(&base_commit)?;
        let source_kv = self.collect_keys_at_commit(&self.get_branch_commit(source_branch)?)?;
        let mut dest_kv = HashMap::new();

        // Collect current tree data
        for key in self.tree.collect_keys() {
            if let Some(value) = self.get(&key) {
                dest_kv.insert(key, value);
            }
        }

        // Perform three-way merge at key-value level
        let mut merge_results = Vec::new();
        let mut all_keys = std::collections::HashSet::new();

        // Collect all keys from all three states
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
                        merge_results.push(crate::diff::MergeResult::Modified(key, source.clone()));
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
                    merge_results.push(crate::diff::MergeResult::Added(key, source.clone()));
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
                    merge_results.push(crate::diff::MergeResult::Removed(key));
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
                crate::diff::MergeResult::Modified(key, value) => {
                    self.tree.insert(key, value); // insert overwrites existing
                }
                crate::diff::MergeResult::Removed(key) => {
                    self.tree.delete(&key);
                }
                crate::diff::MergeResult::Conflict(_) => {
                    // Should not happen since we handled conflicts above
                    unreachable!("Conflicts should have been resolved");
                }
            }
        }

        // Clear staging area since we've applied changes directly to tree
        self.staging_area.clear();
        self.save_staging_area()?;

        // Create merge commit
        let message = format!("Merge branch '{source_branch}' into '{dest_branch}'");
        let merge_commit_id = self.create_merge_commit(&message, source_branch)?;

        Ok(merge_commit_id)
    }

    /// Convenience method to merge with default IgnoreConflictsResolver
    pub fn merge_ignore_conflicts(
        &mut self,
        source_branch: &str,
    ) -> Result<gix::ObjectId, GitKvError> {
        self.merge(source_branch, &IgnoreConflictsResolver)
    }

    /// Find the merge base (common ancestor) of two branches
    fn find_merge_base(&self, branch1: &str, branch2: &str) -> Result<gix::ObjectId, GitKvError> {
        // Get commit IDs for both branches
        let commit1 = self.get_branch_commit(branch1)?;
        let commit2 = self.get_branch_commit(branch2)?;

        // For now, use a simple approach: find common ancestor by walking history
        // This is a simplified implementation - a real merge-base algorithm would be more sophisticated
        let mut visited1 = std::collections::HashSet::new();
        let mut queue1 = std::collections::VecDeque::new();
        queue1.push_back(commit1);

        // Collect all ancestors of branch1
        while let Some(commit_id) = queue1.pop_front() {
            if visited1.contains(&commit_id) {
                continue;
            }
            visited1.insert(commit_id);

            // Add parents
            if let Ok(parents) = self.get_commit_parents(&commit_id) {
                for parent in parents {
                    if !visited1.contains(&parent) {
                        queue1.push_back(parent);
                    }
                }
            }
        }

        // Walk branch2 and find first common ancestor
        let mut visited2 = std::collections::HashSet::new();
        let mut queue2 = std::collections::VecDeque::new();
        queue2.push_back(commit2);

        while let Some(commit_id) = queue2.pop_front() {
            if visited2.contains(&commit_id) {
                continue;
            }
            visited2.insert(commit_id);

            // Check if this commit is in branch1's ancestors
            if visited1.contains(&commit_id) {
                return Ok(commit_id);
            }

            // Add parents
            if let Ok(parents) = self.get_commit_parents(&commit_id) {
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

    /// Get commit ID for a branch
    fn get_branch_commit(&self, branch: &str) -> Result<gix::ObjectId, GitKvError> {
        let branch_ref = format!("refs/heads/{branch}");
        match self.metadata.repo().refs.find(&branch_ref) {
            Ok(reference) => match reference.target.try_id() {
                Some(commit_id) => Ok(commit_id.to_owned()),
                None => Err(GitKvError::GitObjectError(format!(
                    "Branch {branch} does not point to a commit"
                ))),
            },
            Err(_) => Err(GitKvError::BranchNotFound(branch.to_string())),
        }
    }

    /// Get parents of a commit
    fn get_commit_parents(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<Vec<gix::ObjectId>, GitKvError> {
        let mut buffer = Vec::new();
        let commit_obj = self
            .metadata
            .repo()
            .objects
            .find(commit_id, &mut buffer)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to find commit: {e}")))?;

        let parents = match commit_obj.decode() {
            Ok(gix::objs::ObjectRef::Commit(commit)) => commit.parents().collect(),
            _ => {
                return Err(GitKvError::GitObjectError(
                    "Object is not a commit".to_string(),
                ))
            }
        };
        Ok(parents)
    }

    /// Create a merge commit with two parents (current HEAD + source branch)
    pub(crate) fn create_merge_commit(
        &mut self,
        message: &str,
        source_branch: &str,
    ) -> Result<gix::ObjectId, GitKvError> {
        // Get both parent commits
        let current_commit = self.metadata.head_commit_id()?;
        let source_commit = self.get_branch_commit(source_branch)?;

        // Save current tree state
        self.tree.persist_root();
        self.tree
            .save_config()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to save config: {e}")))?;

        // Save tree config to git
        self.save_tree_config_to_git_internal()?;

        // Stage files and write tree via metadata backend
        let dataset_dir = self
            .dataset_dir
            .as_ref()
            .ok_or_else(|| GitKvError::GitObjectError("Dataset directory not set".into()))?;
        let git_root = self
            .metadata
            .work_dir()
            .or_else(|| Self::find_git_root(dataset_dir))
            .ok_or_else(|| GitKvError::GitObjectError("Could not find git root".into()))?;

        let tree_id = self.metadata.stage_and_write_tree(&git_root)?;

        // Create merge commit with two parents using gix (bypasses shell hooks)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| GitKvError::GitObjectError(format!("System time error: {e}")))?
            .as_secs() as i64;

        let (name, email) = self.metadata.user_config();

        let signature = gix::actor::Signature {
            name: name.into(),
            email: email.into(),
            time: gix::date::Time {
                seconds: now,
                offset: 0,
            },
        };

        let commit = gix::objs::Commit {
            tree: tree_id,
            parents: vec![current_commit, source_commit].into(),
            author: signature.clone(),
            committer: signature,
            encoding: None,
            message: message.as_bytes().into(),
            extra_headers: vec![],
        };

        let commit_id = self
            .metadata
            .repo()
            .objects
            .write(&commit)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to write commit: {e}")))?;

        // Update branch ref and HEAD
        self.metadata
            .update_branch(&self.current_branch, commit_id)?;
        self.metadata.update_head(&self.current_branch)?;

        Ok(commit_id)
    }

    /// Reload the tree state from the current HEAD commit
    fn reload_tree_from_head(&mut self) -> Result<(), GitKvError> {
        // Get the current HEAD commit
        let head = self
            .metadata
            .repo()
            .head()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get HEAD: {e}")))?;

        let head_commit_id = head.id().ok_or_else(|| {
            GitKvError::GitObjectError("HEAD does not point to a commit".to_string())
        })?;

        // Convert gix::Id to gix::ObjectId
        let head_object_id = head_commit_id.detach();

        // Load all key-value pairs from the HEAD commit
        let keys_at_head = self.collect_keys_at_commit(&head_object_id)?;

        // Clear the current tree and rebuild it with the data from HEAD
        // Important: Create tree with config that already has the correct root hash to avoid triggering persist_root
        let mut config = self.tree.config.clone();

        // Get the config from the commit to get the correct root hash
        if let Ok(commit_config) = self.read_tree_config_from_commit(&head_object_id) {
            config.root_hash = commit_config.root_hash;
        }

        self.tree = ProllyTree::new(self.tree.storage.clone(), config);

        // Insert all the key-value pairs from the HEAD commit
        // Note: These insertions may still trigger auto-save in the tree
        for (key, value) in keys_at_head {
            self.tree.insert(key, value);
        }

        // Note: We don't save_config() or persist_root() here because this is a read operation.
        // The config files should only be saved during write operations (commit).
        // Saving here would overwrite the existing files with potentially stale data.

        Ok(())
    }

    /// Initialize a new versioned KV store with Git storage (default)
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Safety check: prevent initializing at git root to avoid `git add -A .` staging all files
        if Self::is_in_git_root(path)? {
            return Err(GitKvError::GitObjectError(
                "Cannot initialize git-prolly in git root directory. \
                Please use a subdirectory to create a dataset, or the commit operation \
                may accidentally stage all files in the repository."
                    .to_string(),
            ));
        }

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // For GitVersionedKvStore, use the user-provided path as the dataset directory
        // This allows config files to be versioned in git commits
        let dataset_dir = path.to_path_buf();
        std::fs::create_dir_all(&dataset_dir).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create dataset directory: {e}"))
        })?;

        // Check if the store is already initialized by looking for config files
        let config_path = dataset_dir.join("prolly_config_tree_config");
        let mappings_path = dataset_dir.join("prolly_hash_mappings");

        if config_path.exists() || mappings_path.exists() {
            // Store already exists, use open instead to load existing configuration
            return Self::open(path);
        }

        // Open the existing git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create GitNodeStorage with user-provided path for versioned files
        let storage = GitNodeStorage::new(git_repo.clone(), dataset_dir.clone())?;

        // Create ProllyTree with default config
        let config: TreeConfig<N> = TreeConfig::default();
        let tree = ProllyTree::new(storage, config);

        let mut store = VersionedKvStore {
            tree,
            metadata: GitMetadataBackend::new(git_repo),
            staging_area: HashMap::new(),
            current_branch: "main".to_string(),
            storage_backend: StorageBackend::Git,
            dataset_dir: Some(dataset_dir),
        };

        // Save initial configuration
        let _ = store.tree.save_config();

        // Create initial commit (which will include prolly metadata files)
        store.commit("Initial commit")?;

        Ok(store)
    }

    /// Open an existing versioned KV store with Git storage (default)
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Safety check: prevent opening at git root to avoid `git add -A .` staging all files
        if Self::is_in_git_root(path)? {
            return Err(GitKvError::GitObjectError(
                "Cannot open git-prolly in git root directory. \
                Please use a subdirectory for your dataset, or the commit operation \
                may accidentally stage all files in the repository."
                    .to_string(),
            ));
        }

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // For GitVersionedKvStore, use the user-provided path as the dataset directory
        // This allows config files to be versioned in git commits
        let dataset_dir = path.to_path_buf();

        // Open existing Git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create GitNodeStorage with user-provided path for versioned files
        let storage = GitNodeStorage::new(git_repo.clone(), dataset_dir.clone())?;

        // Load tree configuration from dataset directory
        let config_path = dataset_dir.join("prolly_config_tree_config");
        if !config_path.exists() {
            return Err(GitKvError::GitObjectError(
                "Config file not found. The store may not be initialized. \
                Call init() to create a new store."
                    .to_string(),
            ));
        }
        let config_data = std::fs::read_to_string(&config_path)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to read config file: {e}")))?;
        let config: TreeConfig<N> = serde_json::from_str(&config_data)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to parse config file: {e}")))?;

        // Try to load existing tree from storage
        let tree = if let Some(existing_tree) =
            ProllyTree::load_from_storage(storage.clone(), config.clone())
        {
            existing_tree
        } else if config.root_hash.is_some() {
            // We have a saved root hash but failed to load the tree
            // This could be due to missing hash mappings or git objects
            // For read-only operations, we should try to work with what we have
            // rather than creating a new empty tree that would overwrite the config
            eprintln!("Warning: Failed to load tree from saved root hash. This may indicate missing git objects or corrupted hash mappings.");
            eprintln!("Attempting to create tree with saved config to avoid data loss...");
            ProllyTree::new(storage, config)
        } else {
            // No saved root hash - this is a genuinely new/empty tree
            ProllyTree::new(storage, config)
        };

        // Get current branch
        let current_branch = git_repo
            .head_ref()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get head ref: {e}")))?
            .map(|r| r.name().shorten().to_string())
            .unwrap_or_else(|| "main".to_string());

        let mut store = VersionedKvStore {
            tree,
            metadata: GitMetadataBackend::new(git_repo),
            staging_area: HashMap::new(),
            current_branch,
            storage_backend: StorageBackend::Git,
            dataset_dir: Some(dataset_dir),
        };

        // Load staging area from file if it exists
        store.load_staging_area()?;

        // Note: We intentionally do NOT call reload_tree_from_head() here
        // because git-prolly commands should read from the current directory's
        // prolly_config_tree_config and mapping files, not from git HEAD.
        // The tree was already loaded from local storage by load_from_storage() above.

        Ok(store)
    }

    /// Get a reference to the underlying ProllyTree
    pub fn tree(&self) -> &ProllyTree<N, GitNodeStorage<N>> {
        &self.tree
    }

    /// Get a mutable reference to the underlying ProllyTree
    pub fn tree_mut(&mut self) -> &mut ProllyTree<N, GitNodeStorage<N>> {
        &mut self.tree
    }

    /// Collect all key-value pairs from the tree at a specific commit
    pub(crate) fn collect_keys_at_commit(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Build relative paths for the prolly files
        let dataset_dir = self.tree.storage.dataset_dir();
        let git_root = self
            .metadata
            .work_dir()
            .or_else(|| Self::find_git_root(dataset_dir))
            .ok_or_else(|| GitKvError::GitObjectError("Could not find git root".to_string()))?;

        let dataset_relative_path = dataset_dir
            .strip_prefix(&git_root)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get relative path: {e}")))?;

        let relative_path_str = dataset_relative_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        let config_path = format!("{}/prolly_config_tree_config", relative_path_str);
        let mapping_path = format!("{}/prolly_hash_mappings", relative_path_str);

        let config_result = self.metadata.read_file_at_commit(commit_id, &config_path);
        let mapping_result = self.metadata.read_file_at_commit(commit_id, &mapping_path);

        // If files are not found, this might be an initial empty commit, return empty
        if config_result.is_err() || mapping_result.is_err() {
            return Ok(HashMap::new());
        }

        let config_data = config_result?;
        let config: TreeConfig<N> = serde_json::from_slice(&config_data).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to deserialize config: {e}"))
        })?;

        // Load the hash mappings from the tree as string format and parse
        let mapping_data = mapping_result?;
        let mapping_str = String::from_utf8(mapping_data)
            .map_err(|e| GitKvError::GitObjectError(format!("Invalid UTF-8 in mappings: {e}")))?;

        // Check if this is a simple key-value mapping (for InMemory storage)
        // or a hash mapping (for Git storage)
        let mut key_values = HashMap::new();
        let mut hash_mappings = HashMap::new();
        let mut is_simple_mapping = false;

        for line in mapping_str.lines() {
            if let Some((prefix, rest)) = line.split_once(':') {
                if prefix == "key" {
                    // This is a simple key-value mapping from InMemory storage
                    is_simple_mapping = true;
                    if let Some((key_hex, value_hex)) = rest.split_once(':') {
                        if let (Ok(key), Ok(value)) = (hex::decode(key_hex), hex::decode(value_hex))
                        {
                            key_values.insert(key, value);
                        }
                    }
                } else {
                    // This is a hash mapping from Git storage
                    let hash_hex = prefix;
                    let object_hex = rest;

                    if hash_hex.len() == N * 2 {
                        let mut hash_bytes = Vec::new();
                        for i in 0..N {
                            if let Ok(byte) = u8::from_str_radix(&hash_hex[i * 2..i * 2 + 2], 16) {
                                hash_bytes.push(byte);
                            } else {
                                break;
                            }
                        }

                        if hash_bytes.len() == N {
                            if let Ok(object_id) = gix::ObjectId::from_hex(object_hex.as_bytes()) {
                                let hash = ValueDigest::raw_hash(&hash_bytes);
                                hash_mappings.insert(hash, object_id);
                            }
                        }
                    }
                }
            }
        }

        if is_simple_mapping {
            // For InMemory storage, return the directly stored key-value pairs
            return Ok(key_values);
        }

        // For Git storage, reconstruct the tree from hash mappings
        if hash_mappings.is_empty() {
            return Ok(HashMap::new());
        }

        // Create a temporary storage with the loaded mappings
        let temp_storage = GitNodeStorage::with_mappings(
            self.metadata.clone_repo(),
            self.tree.storage.dataset_dir().to_path_buf(),
            hash_mappings,
        )?;

        // Load the tree with the config
        let tree = ProllyTree::load_from_storage(temp_storage, config).ok_or_else(|| {
            GitKvError::GitObjectError("Failed to load tree from storage".to_string())
        })?;

        // Collect all key-value pairs
        let mut result_key_values = HashMap::new();
        for key in tree.collect_keys() {
            if let Some(node) = tree.find(&key) {
                // Find the value in the node
                if let Some(index) = node.keys.iter().position(|k| k == &key) {
                    result_key_values.insert(key, node.values[index].clone());
                }
            }
        }

        Ok(result_key_values)
    }
}

// Implement HistoricalAccess for GitNodeStorage
impl<const N: usize> HistoricalAccess<N>
    for VersionedKvStore<N, GitNodeStorage<N>, GitMetadataBackend>
{
    fn get_keys_at_ref(&self, reference: &str) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        let commit_id = self.resolve_commit(reference)?;
        self.collect_keys_at_commit(&commit_id)
    }
}

// Implement HistoricalCommitAccess for GitNodeStorage
impl<const N: usize> HistoricalCommitAccess<N>
    for VersionedKvStore<N, GitNodeStorage<N>, GitMetadataBackend>
{
    fn get_commits_for_key(&self, key: &[u8]) -> Result<Vec<CommitInfo>, GitKvError> {
        let mut commit_history = self.get_commit_history()?;

        // Reverse to process in chronological order (oldest first)
        commit_history.reverse();

        let mut commits_with_key_changes = Vec::new();
        let mut previous_value: Option<Vec<u8>> = None; // None = key not present, Some(val) = key present with value

        for commit in commit_history {
            // Get the key value at this commit
            let current_value = self.collect_keys_at_commit(&commit.id)?.get(key).cloned();

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

    fn get_commit_history(&self) -> Result<Vec<CommitInfo>, GitKvError> {
        // Reuse the existing log method
        self.log()
    }
}

impl<const N: usize> VersionedKvStore<N, InMemoryNodeStorage<N>, GitMetadataBackend> {
    /// Initialize a new versioned KV store with InMemory storage
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Safety check: prevent initializing at git root to avoid `git add -A .` staging all files
        if Self::is_in_git_root(path)? {
            return Err(GitKvError::GitObjectError(
                "Cannot initialize in-memory store in git root directory. \
                Please use a subdirectory to create a dataset, or the commit operation \
                may accidentally stage all files in the repository."
                    .to_string(),
            ));
        }

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // Open the existing git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create dataset directory for config files
        let dataset_dir = path.to_path_buf();
        std::fs::create_dir_all(&dataset_dir).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create dataset directory: {e}"))
        })?;

        // Create InMemoryNodeStorage
        let storage = InMemoryNodeStorage::<N>::new();

        // Create ProllyTree with default config
        let config: TreeConfig<N> = TreeConfig::default();
        let tree = ProllyTree::new(storage, config);

        let mut store = VersionedKvStore {
            tree,
            metadata: GitMetadataBackend::new(git_repo),
            staging_area: HashMap::new(),
            current_branch: "main".to_string(),
            storage_backend: StorageBackend::InMemory,
            dataset_dir: Some(dataset_dir),
        };

        // Create initial commit
        store.commit("Initial commit")?;

        Ok(store)
    }

    /// Open an existing versioned KV store with InMemory storage
    /// Note: InMemory storage is volatile, so this creates a new empty store
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        // For InMemory storage, "opening" is the same as initializing
        // since data is not persistent
        Self::init(path)
    }
}

// Implement HistoricalAccess for InMemoryNodeStorage
impl<const N: usize> HistoricalAccess<N>
    for VersionedKvStore<N, InMemoryNodeStorage<N>, GitMetadataBackend>
{
    fn get_keys_at_ref(&self, reference: &str) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Resolve the reference to a commit ID
        let commit_id = self.resolve_commit(reference)?;

        // Get the tree config from the commit to extract root hash
        let tree_config = self.read_tree_config_from_commit(&commit_id)?;

        // Reconstruct the tree state from storage using the root hash
        self.collect_keys_from_config(&tree_config)
    }
}

// Implement HistoricalCommitAccess for InMemoryNodeStorage
impl<const N: usize> HistoricalCommitAccess<N>
    for VersionedKvStore<N, InMemoryNodeStorage<N>, GitMetadataBackend>
{
    fn get_commits_for_key(&self, key: &[u8]) -> Result<Vec<CommitInfo>, GitKvError> {
        self.get_commits_for_key_generic(key)
    }

    fn get_commit_history(&self) -> Result<Vec<CommitInfo>, GitKvError> {
        self.get_commit_history_generic()
    }
}

impl<const N: usize> VersionedKvStore<N, FileNodeStorage<N>, GitMetadataBackend> {
    /// Initialize a new versioned KV store with File storage
    ///
    /// Nodes are stored in `.git/prolly/nodes/files/` (shared, content-addressed).
    /// Config is stored in the dataset directory (committed to git for history).
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Safety check: prevent initializing at git root to avoid `git add -A .` staging all files
        if Self::is_in_git_root(path)? {
            return Err(GitKvError::GitObjectError(
                "Cannot initialize file store in git root directory. \
                Please use a subdirectory to create a dataset, or the commit operation \
                may accidentally stage all files in the repository."
                    .to_string(),
            ));
        }

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // Create prolly directory inside .git for node storage
        let prolly_dir = Self::ensure_prolly_dir(&git_root)?;

        // Create dataset directory for config files (will be committed to git)
        let dataset_dir = path.to_path_buf();
        std::fs::create_dir_all(&dataset_dir).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create dataset directory: {e}"))
        })?;

        // Open the existing git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create FileNodeStorage inside .git/prolly/nodes/files/ (shared across datasets)
        let file_storage_path = prolly_dir.join("nodes").join("files");
        std::fs::create_dir_all(&file_storage_path).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create file storage directory: {e}"))
        })?;
        let storage = FileNodeStorage::<N>::new(file_storage_path).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create file storage: {e}"))
        })?;

        // Create ProllyTree with default config
        let config: TreeConfig<N> = TreeConfig::default();
        let tree = ProllyTree::new(storage, config);

        let mut store = VersionedKvStore {
            tree,
            metadata: GitMetadataBackend::new(git_repo),
            staging_area: HashMap::new(),
            current_branch: "main".to_string(),
            storage_backend: StorageBackend::File,
            dataset_dir: Some(dataset_dir),
        };

        // Create initial commit (which will save config to dataset_dir)
        store.commit("Initial commit")?;

        Ok(store)
    }

    /// Open an existing versioned KV store with File storage
    ///
    /// Nodes are loaded from `.git/prolly/nodes/files/` (shared, content-addressed).
    /// Config is loaded from the dataset directory.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();
        let dataset_dir = path.to_path_buf();

        // Safety check: prevent opening at git root to avoid `git add -A .` staging all files
        if Self::is_in_git_root(path)? {
            return Err(GitKvError::GitObjectError(
                "Cannot open file store in git root directory. \
                Please use a subdirectory for your dataset, or the commit operation \
                may accidentally stage all files in the repository."
                    .to_string(),
            ));
        }

        // Check if the dataset directory exists
        if !dataset_dir.exists() {
            return Err(GitKvError::GitObjectError(
                "Dataset directory not found. Call init() first to create the store.".to_string(),
            ));
        }

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // Get prolly directory inside .git
        let prolly_dir = Self::get_prolly_dir(&git_root);

        // Open existing Git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Open FileNodeStorage inside .git/prolly/nodes/files/
        let file_storage_path = prolly_dir.join("nodes").join("files");

        // Check if the file storage directory exists - if not, the store hasn't been initialized
        if !file_storage_path.exists() {
            return Err(GitKvError::GitObjectError(
                "File store not initialized. Call init() first to create the store.".to_string(),
            ));
        }

        let storage = FileNodeStorage::<N>::new(file_storage_path.clone()).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create file storage: {e}"))
        })?;

        // Load tree configuration from dataset directory (not from storage)
        // Config file must exist for open() - use init() to create new stores
        let config_path = dataset_dir.join("prolly_config_tree_config");
        if !config_path.exists() {
            return Err(GitKvError::GitObjectError(
                "Config file not found. The store may not be initialized. \
                Call init() to create a new store."
                    .to_string(),
            ));
        }
        let config_data = std::fs::read_to_string(&config_path)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to read config file: {e}")))?;
        let config: TreeConfig<N> = serde_json::from_str(&config_data)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to parse config file: {e}")))?;

        // Try to load existing tree from storage using the config's root hash
        let tree =
            if let Some(existing_tree) = ProllyTree::load_from_storage(storage, config.clone()) {
                existing_tree
            } else {
                // Create new storage instance since the original was consumed
                let new_storage = FileNodeStorage::<N>::new(file_storage_path).map_err(|e| {
                    GitKvError::GitObjectError(format!("Failed to create file storage: {e}"))
                })?;
                ProllyTree::new(new_storage, config)
            };

        // Get current branch
        let current_branch = git_repo
            .head_ref()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get head ref: {e}")))?
            .map(|r| r.name().shorten().to_string())
            .unwrap_or_else(|| "main".to_string());

        let mut store = VersionedKvStore {
            tree,
            metadata: GitMetadataBackend::new(git_repo),
            staging_area: HashMap::new(),
            current_branch,
            storage_backend: StorageBackend::File,
            dataset_dir: Some(dataset_dir),
        };

        // Load staging area from file if it exists
        store.load_staging_area()?;

        Ok(store)
    }
}

// Implement HistoricalAccess for FileNodeStorage
impl<const N: usize> HistoricalAccess<N>
    for VersionedKvStore<N, FileNodeStorage<N>, GitMetadataBackend>
{
    fn get_keys_at_ref(&self, reference: &str) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Resolve the reference to a commit ID
        let commit_id = self.resolve_commit(reference)?;

        // Get the tree config from the commit to extract root hash
        let tree_config = self.read_tree_config_from_commit(&commit_id)?;

        // Reconstruct the tree state from storage using the root hash
        self.collect_keys_from_config(&tree_config)
    }
}

// Implement HistoricalCommitAccess for FileNodeStorage
impl<const N: usize> HistoricalCommitAccess<N>
    for VersionedKvStore<N, FileNodeStorage<N>, GitMetadataBackend>
{
    fn get_commits_for_key(&self, key: &[u8]) -> Result<Vec<CommitInfo>, GitKvError> {
        self.get_commits_for_key_generic(key)
    }

    fn get_commit_history(&self) -> Result<Vec<CommitInfo>, GitKvError> {
        self.get_commit_history_generic()
    }
}

#[cfg(feature = "rocksdb_storage")]
impl<const N: usize> VersionedKvStore<N, RocksDBNodeStorage<N>, GitMetadataBackend> {
    /// Initialize a new versioned KV store with RocksDB storage
    ///
    /// Nodes are stored in `.git/prolly/nodes/rocksdb/` (shared, content-addressed).
    /// Config is stored in the dataset directory (committed to git for history).
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Safety check: prevent initializing at git root to avoid `git add -A .` staging all files
        if Self::is_in_git_root(path)? {
            return Err(GitKvError::GitObjectError(
                "Cannot initialize RocksDB store in git root directory. \
                Please use a subdirectory to create a dataset, or the commit operation \
                may accidentally stage all files in the repository."
                    .to_string(),
            ));
        }

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // Create prolly directory inside .git for node storage
        let prolly_dir = Self::ensure_prolly_dir(&git_root)?;

        // Create dataset directory for config files (will be committed to git)
        let dataset_dir = path.to_path_buf();
        std::fs::create_dir_all(&dataset_dir).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create dataset directory: {e}"))
        })?;

        // Open the existing git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create RocksDBNodeStorage inside .git/prolly/nodes/rocksdb/ (shared across datasets)
        let rocksdb_path = prolly_dir.join("nodes").join("rocksdb");
        std::fs::create_dir_all(&rocksdb_path).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create RocksDB directory: {e}"))
        })?;
        let storage = RocksDBNodeStorage::<N>::new(rocksdb_path)
            .map_err(|e| GitKvError::GitObjectError(format!("RocksDB creation failed: {e}")))?;

        // Create ProllyTree with default config
        let config: TreeConfig<N> = TreeConfig::default();
        let tree = ProllyTree::new(storage, config);

        let mut store = VersionedKvStore {
            tree,
            metadata: GitMetadataBackend::new(git_repo),
            staging_area: HashMap::new(),
            current_branch: "main".to_string(),
            storage_backend: StorageBackend::RocksDB,
            dataset_dir: Some(dataset_dir),
        };

        // Create initial commit (which will save config to dataset_dir)
        store.commit("Initial commit")?;

        Ok(store)
    }

    /// Open an existing versioned KV store with RocksDB storage
    ///
    /// Nodes are loaded from `.git/prolly/nodes/rocksdb/` (shared, content-addressed).
    /// Config is loaded from the dataset directory.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();
        let dataset_dir = path.to_path_buf();

        // Safety check: prevent opening at git root to avoid `git add -A .` staging all files
        if Self::is_in_git_root(path)? {
            return Err(GitKvError::GitObjectError(
                "Cannot open RocksDB store in git root directory. \
                Please use a subdirectory for your dataset, or the commit operation \
                may accidentally stage all files in the repository."
                    .to_string(),
            ));
        }

        // Check if the dataset directory exists
        if !dataset_dir.exists() {
            return Err(GitKvError::GitObjectError(
                "Dataset directory not found. Call init() first to create the store.".to_string(),
            ));
        }

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // Get prolly directory inside .git
        let prolly_dir = Self::get_prolly_dir(&git_root);

        // Open existing Git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Open RocksDBNodeStorage inside .git/prolly/nodes/rocksdb/
        let rocksdb_path = prolly_dir.join("nodes").join("rocksdb");

        // Check if the RocksDB directory exists - if not, the store hasn't been initialized
        if !rocksdb_path.exists() {
            return Err(GitKvError::GitObjectError(
                "RocksDB store not initialized. Call init() first to create the store.".to_string(),
            ));
        }

        let storage = RocksDBNodeStorage::<N>::new(rocksdb_path)
            .map_err(|e| GitKvError::GitObjectError(format!("RocksDB creation failed: {e}")))?;

        // Load tree configuration from dataset directory (not from storage)
        let config_path = dataset_dir.join("prolly_config_tree_config");
        if !config_path.exists() {
            return Err(GitKvError::GitObjectError(
                "Config file not found. The store may not be initialized. \
                Call init() to create a new store."
                    .to_string(),
            ));
        }
        let config_data = std::fs::read_to_string(&config_path)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to read config file: {e}")))?;
        let config: TreeConfig<N> = serde_json::from_str(&config_data)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to parse config file: {e}")))?;

        // Try to load existing tree from storage using the config's root hash
        let tree = ProllyTree::load_from_storage(storage.clone(), config.clone())
            .unwrap_or_else(|| ProllyTree::new(storage, config));

        // Get current branch
        let current_branch = git_repo
            .head_ref()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get head ref: {e}")))?
            .map(|r| r.name().shorten().to_string())
            .unwrap_or_else(|| "main".to_string());

        let mut store = VersionedKvStore {
            tree,
            metadata: GitMetadataBackend::new(git_repo),
            staging_area: HashMap::new(),
            current_branch,
            storage_backend: StorageBackend::RocksDB,
            dataset_dir: Some(dataset_dir),
        };

        // Load staging area from file if it exists
        store.load_staging_area()?;

        Ok(store)
    }
}

// Implement HistoricalAccess for RocksDBNodeStorage
#[cfg(feature = "rocksdb_storage")]
impl<const N: usize> HistoricalAccess<N>
    for VersionedKvStore<N, RocksDBNodeStorage<N>, GitMetadataBackend>
{
    fn get_keys_at_ref(&self, reference: &str) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Resolve the reference to a commit ID
        let commit_id = self.resolve_commit(reference)?;

        // Get the tree config from the commit to extract root hash
        let tree_config = self.read_tree_config_from_commit(&commit_id)?;

        // Reconstruct the tree state from storage using the root hash
        self.collect_keys_from_config(&tree_config)
    }
}

// Implement HistoricalCommitAccess for RocksDBNodeStorage
#[cfg(feature = "rocksdb_storage")]
impl<const N: usize> HistoricalCommitAccess<N>
    for VersionedKvStore<N, RocksDBNodeStorage<N>, GitMetadataBackend>
{
    fn get_commits_for_key(&self, key: &[u8]) -> Result<Vec<CommitInfo>, GitKvError> {
        self.get_commits_for_key_generic(key)
    }

    fn get_commit_history(&self) -> Result<Vec<CommitInfo>, GitKvError> {
        self.get_commit_history_generic()
    }
}

// Implement TreeConfigSaver for InMemoryNodeStorage
impl<const N: usize> TreeConfigSaver<N>
    for VersionedKvStore<N, InMemoryNodeStorage<N>, GitMetadataBackend>
{
    fn save_tree_config_to_git_internal(&self) -> Result<(), GitKvError> {
        self.save_tree_config_to_git()
    }
}

// Specialized implementation for InMemoryNodeStorage
impl<const N: usize> VersionedKvStore<N, InMemoryNodeStorage<N>, GitMetadataBackend> {
    /// Save tree config to git for InMemoryNodeStorage
    ///
    /// Writes only the config (with root hash) to dataset directory.
    /// The config is committed to git for historical access.
    /// Nodes are kept in memory - historical access works within the same session
    /// by loading nodes from the in-memory storage using the root hash.
    /// After restart, nodes are lost (expected for in-memory storage).
    fn save_tree_config_to_git(&self) -> Result<(), GitKvError> {
        let dataset_dir = self
            .dataset_dir
            .as_ref()
            .ok_or_else(|| GitKvError::GitObjectError("Dataset directory not set".to_string()))?;

        // Get the current tree configuration (includes root_hash)
        let config = self.tree.config.clone();

        // Serialize the config to JSON
        let config_json = serde_json::to_string_pretty(&config)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to serialize config: {e}")))?;

        // Write only the config file to the dataset directory
        // No need for prolly_hash_mappings - nodes are in memory and accessed by root_hash
        // Historical access uses root_hash to traverse the tree from in-memory storage
        let config_path = dataset_dir.join("prolly_config_tree_config");
        std::fs::write(&config_path, config_json)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to write config file: {e}")))?;

        Ok(())
    }
}

// Implement TreeConfigSaver for FileNodeStorage
impl<const N: usize> TreeConfigSaver<N>
    for VersionedKvStore<N, FileNodeStorage<N>, GitMetadataBackend>
{
    fn save_tree_config_to_git_internal(&self) -> Result<(), GitKvError> {
        self.save_tree_config_to_git()
    }
}

// Specialized implementation for FileNodeStorage
impl<const N: usize> VersionedKvStore<N, FileNodeStorage<N>, GitMetadataBackend> {
    /// Save tree config to git for FileNodeStorage
    ///
    /// Writes only the config (with root hash) to dataset directory.
    /// The config is committed to git for historical access.
    /// Nodes remain in .git/prolly/nodes/files/ (shared, content-addressed).
    /// Historical access reconstructs tree by loading nodes from storage using root hash.
    fn save_tree_config_to_git(&self) -> Result<(), GitKvError> {
        let dataset_dir = self
            .dataset_dir
            .as_ref()
            .ok_or_else(|| GitKvError::GitObjectError("Dataset directory not set".to_string()))?;

        // Get the current tree configuration (includes root_hash)
        let config = self.tree.config.clone();

        // Serialize the config to JSON
        let config_json = serde_json::to_string_pretty(&config)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to serialize config: {e}")))?;

        // Write only the config file to the dataset directory
        // No need for prolly_hash_mappings - nodes are content-addressed in .git/prolly/nodes/files/
        // Historical access uses root_hash to traverse the tree from node storage
        let config_path = dataset_dir.join("prolly_config_tree_config");
        std::fs::write(&config_path, config_json)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to write config file: {e}")))?;

        Ok(())
    }
}

// Implement TreeConfigSaver for RocksDBNodeStorage
#[cfg(feature = "rocksdb_storage")]
impl<const N: usize> TreeConfigSaver<N>
    for VersionedKvStore<N, RocksDBNodeStorage<N>, GitMetadataBackend>
{
    fn save_tree_config_to_git_internal(&self) -> Result<(), GitKvError> {
        self.save_tree_config_to_git()
    }
}

// Specialized implementation for RocksDBNodeStorage
#[cfg(feature = "rocksdb_storage")]
impl<const N: usize> VersionedKvStore<N, RocksDBNodeStorage<N>, GitMetadataBackend> {
    /// Save tree config to git for RocksDBNodeStorage
    ///
    /// Writes only the config (with root hash) to dataset directory.
    /// The config is committed to git for historical access.
    /// Nodes remain in .git/prolly/nodes/rocksdb/ (shared, content-addressed).
    /// Historical access reconstructs tree by loading nodes from storage using root hash.
    fn save_tree_config_to_git(&self) -> Result<(), GitKvError> {
        let dataset_dir = self
            .dataset_dir
            .as_ref()
            .ok_or_else(|| GitKvError::GitObjectError("Dataset directory not set".to_string()))?;

        // Get the current tree configuration (includes root_hash)
        let config = self.tree.config.clone();

        // Serialize the config to JSON
        let config_json = serde_json::to_string_pretty(&config)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to serialize config: {e}")))?;

        // Write only the config file to the dataset directory
        // No need for prolly_hash_mappings - nodes are content-addressed in .git/prolly/nodes/rocksdb/
        // Historical access uses root_hash to traverse the tree from node storage
        let config_path = dataset_dir.join("prolly_config_tree_config");
        std::fs::write(&config_path, config_json)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to write config file: {e}")))?;

        Ok(())
    }
}
