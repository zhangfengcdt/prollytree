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

use crate::config::TreeConfig;
use crate::digest::ValueDigest;
use crate::git::types::*;
use crate::git::versioned_store::GitVersionedKvStore;
use crate::node::ProllyNode;
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

    /// Reconstruct KV state from a specific commit using git objects directly
    fn reconstruct_kv_state_from_commit(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Use git's object database to read prolly config directly from the commit
        // This is much more efficient than checking out the entire commit
        self.reconstruct_state_from_git_objects(commit_id)
    }

    /// Reconstruct KV state using git's tree and blob objects directly
    /// This uses the current working directory's config files and git object database
    fn reconstruct_state_from_git_objects(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // For now, fall back to the working but less optimal method
        // The optimization needs more debugging to handle hex decoding correctly
        self.checkout_commit_temporarily_isolated(commit_id)

        // TODO: Fix the direct git object approach
        // The issue seems to be with hex decoding in the hash mappings
        // Once fixed, uncomment the optimized version below:

        /*
        // Try to read prolly config and hash mappings directly from the commit
        let current_dir = std::env::current_dir()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get current dir: {e}")))?;

        // Get the dataset directory name relative to git root
        let git_root = self.store.git_repo().work_dir()
            .ok_or_else(|| GitKvError::GitObjectError("Not in a working directory".to_string()))?;

        let relative_path = current_dir.strip_prefix(git_root)
            .map_err(|_| GitKvError::GitObjectError("Current directory not within git repository".to_string()))?;

        let dataset_name = relative_path.to_string_lossy();

        // Try different possible file paths
        let config_paths = vec![
            format!("{}/prolly_config_tree_config", dataset_name),
            "prolly_config_tree_config".to_string(),
        ];

        let mapping_paths = vec![
            format!("{}/prolly_hash_mappings", dataset_name),
            "prolly_hash_mappings".to_string(),
        ];

        // Try to read the prolly config from the commit
        let mut tree_config = None;
        for path in &config_paths {
            if let Ok(config) = self.read_prolly_config_from_commit(commit_id, path) {
                tree_config = Some(config);
                break;
            }
        }

        let tree_config = tree_config.ok_or_else(|| {
            GitKvError::GitObjectError("Could not find prolly_config_tree_config in commit".to_string())
        })?;

        // Try to read the hash mappings from the commit
        let mut hash_mappings = None;
        for path in &mapping_paths {
            if let Ok(mappings) = self.read_hash_mappings_from_commit(commit_id, path) {
                hash_mappings = Some(mappings);
                break;
            }
        }

        let hash_mappings = hash_mappings.ok_or_else(|| {
            GitKvError::GitObjectError("Could not find prolly_hash_mappings in commit".to_string())
        })?;

        // Collect all key-value pairs from the root hash
        let root_hash = tree_config.root_hash.ok_or_else(|| {
            GitKvError::GitObjectError("Tree config has no root hash".to_string())
        })?;
        self.collect_keys_from_root_hash(&root_hash, &hash_mappings)
        */
    }

    /// Read prolly config file content from a specific git commit
    fn read_prolly_config_from_commit(
        &self,
        commit_id: &gix::ObjectId,
        file_path: &str,
    ) -> Result<TreeConfig<N>, GitKvError> {
        let file_content = self.read_file_from_git_commit(commit_id, file_path)?;
        let config: TreeConfig<N> = serde_json::from_slice(&file_content)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to parse tree config: {e}")))?;
        Ok(config)
    }

    /// Read hash mappings file from a specific git commit
    fn read_hash_mappings_from_commit(
        &self,
        commit_id: &gix::ObjectId,
        file_path: &str,
    ) -> Result<HashMap<ValueDigest<N>, gix::ObjectId>, GitKvError> {
        let file_content = self.read_file_from_git_commit(commit_id, file_path)?;
        let content = String::from_utf8_lossy(&file_content);

        let mut mappings = HashMap::new();
        for line in content.lines() {
            if let Some((hash_str, object_id_str)) = line.split_once(':') {
                // Parse prolly hash (decode hex string manually)
                if let Ok(hash_bytes) = self.decode_hex(hash_str) {
                    if hash_bytes.len() == N {
                        let mut hash_array = [0u8; N];
                        hash_array.copy_from_slice(&hash_bytes);
                        let prolly_hash = ValueDigest::new(&hash_array);

                        // Parse git object ID
                        if let Ok(git_object_id) = gix::ObjectId::from_hex(object_id_str.as_bytes())
                        {
                            mappings.insert(prolly_hash, git_object_id);
                        }
                    }
                }
            }
        }

        Ok(mappings)
    }

    /// Read a file from a specific git commit using gix
    /// Supports nested paths like "dataset/prolly_config_tree_config"
    fn read_file_from_git_commit(
        &self,
        commit_id: &gix::ObjectId,
        file_path: &str,
    ) -> Result<Vec<u8>, GitKvError> {
        // Get the commit object
        let mut buffer = Vec::new();
        let commit = self
            .store
            .git_repo()
            .objects
            .find(commit_id, &mut buffer)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to find commit: {e}")))?;

        let commit_ref = commit
            .decode()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to decode commit: {e}")))?
            .into_commit()
            .ok_or_else(|| GitKvError::GitObjectError("Object is not a commit".to_string()))?;

        // Get the root tree from the commit
        let tree_id = commit_ref.tree();

        // Split the path into components
        let path_parts: Vec<&str> = file_path.split('/').collect();

        // Navigate through the tree structure
        self.find_file_in_tree(&tree_id, &path_parts, 0)
    }

    /// Recursively find a file in a git tree, following directory structure
    fn find_file_in_tree(
        &self,
        tree_id: &gix::ObjectId,
        path_parts: &[&str],
        depth: usize,
    ) -> Result<Vec<u8>, GitKvError> {
        if depth >= path_parts.len() {
            return Err(GitKvError::GitObjectError(
                "Path traversal error".to_string(),
            ));
        }

        let current_part = path_parts[depth];
        let is_final = depth == path_parts.len() - 1;

        // Read the tree object
        let mut tree_buffer = Vec::new();
        let tree = self
            .store
            .git_repo()
            .objects
            .find(tree_id, &mut tree_buffer)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to find tree: {e}")))?;

        let tree_ref = tree
            .decode()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to decode tree: {e}")))?
            .into_tree()
            .ok_or_else(|| GitKvError::GitObjectError("Object is not a tree".to_string()))?;

        // Look for the current path component in the tree entries
        for entry in tree_ref.entries {
            if entry.filename == current_part.as_bytes() {
                if is_final {
                    // This should be a blob (file)
                    if entry.mode.is_blob() {
                        let mut blob_buffer = Vec::new();
                        let blob_oid = gix::ObjectId::from(entry.oid);
                        let blob = self
                            .store
                            .git_repo()
                            .objects
                            .find(&blob_oid, &mut blob_buffer)
                            .map_err(|e| {
                                GitKvError::GitObjectError(format!("Failed to find blob: {e}"))
                            })?;

                        let blob_ref = blob
                            .decode()
                            .map_err(|e| {
                                GitKvError::GitObjectError(format!("Failed to decode blob: {e}"))
                            })?
                            .into_blob()
                            .ok_or_else(|| {
                                GitKvError::GitObjectError("Object is not a blob".to_string())
                            })?;

                        return Ok(blob_ref.data.to_vec());
                    } else {
                        return Err(GitKvError::GitObjectError(format!(
                            "Expected file but found directory: {}",
                            current_part
                        )));
                    }
                } else {
                    // This should be a tree (directory) - recurse into it
                    if entry.mode.is_tree() {
                        let tree_oid = gix::ObjectId::from(entry.oid);
                        return self.find_file_in_tree(&tree_oid, path_parts, depth + 1);
                    } else {
                        return Err(GitKvError::GitObjectError(format!(
                            "Expected directory but found file: {}",
                            current_part
                        )));
                    }
                }
            }
        }

        Err(GitKvError::GitObjectError(format!(
            "Path component '{}' not found in tree (depth: {}, full path: {})",
            current_part,
            depth,
            path_parts.join("/")
        )))
    }

    /// Reconstruct key-value pairs from a prolly tree root hash and hash mappings
    fn collect_keys_from_root_hash(
        &self,
        root_hash: &ValueDigest<N>,
        hash_mappings: &HashMap<ValueDigest<N>, gix::ObjectId>,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Get the git object ID for the root hash
        let root_git_id = hash_mappings.get(root_hash).ok_or_else(|| {
            GitKvError::GitObjectError("Root hash not found in mappings".to_string())
        })?;

        // Read the root node from git
        let mut buffer = Vec::new();
        let root_blob = self
            .store
            .git_repo()
            .objects
            .find(root_git_id, &mut buffer)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to find root node: {e}")))?;

        let blob_ref = root_blob
            .decode()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to decode root node: {e}")))?
            .into_blob()
            .ok_or_else(|| GitKvError::GitObjectError("Root object is not a blob".to_string()))?;

        // Deserialize the prolly node
        let root_node: ProllyNode<N> = bincode::deserialize(blob_ref.data).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to deserialize root node: {e}"))
        })?;

        // Traverse the tree and collect all key-value pairs
        let mut result = HashMap::new();
        self.collect_keys_from_node(&root_node, hash_mappings, &mut result)?;

        Ok(result)
    }

    /// Recursively collect key-value pairs from a prolly tree node
    fn collect_keys_from_node(
        &self,
        node: &ProllyNode<N>,
        hash_mappings: &HashMap<ValueDigest<N>, gix::ObjectId>,
        result: &mut HashMap<Vec<u8>, Vec<u8>>,
    ) -> Result<(), GitKvError> {
        if node.is_leaf {
            // Leaf node: add all key-value pairs
            for (i, key) in node.keys.iter().enumerate() {
                if let Some(value) = node.values.get(i) {
                    result.insert(key.clone(), value.clone());
                }
            }
        } else {
            // Internal node: recursively process child nodes
            for value in &node.values {
                let child_hash = ValueDigest::raw_hash(value);
                if let Some(child_git_id) = hash_mappings.get(&child_hash) {
                    // Read child node from git
                    let mut buffer = Vec::new();
                    let child_blob = self
                        .store
                        .git_repo()
                        .objects
                        .find(child_git_id, &mut buffer)
                        .map_err(|e| {
                            GitKvError::GitObjectError(format!("Failed to find child node: {e}"))
                        })?;

                    let blob_ref = child_blob
                        .decode()
                        .map_err(|e| {
                            GitKvError::GitObjectError(format!("Failed to decode child node: {e}"))
                        })?
                        .into_blob()
                        .ok_or_else(|| {
                            GitKvError::GitObjectError("Child object is not a blob".to_string())
                        })?;

                    let child_node: ProllyNode<N> =
                        bincode::deserialize(blob_ref.data).map_err(|e| {
                            GitKvError::GitObjectError(format!(
                                "Failed to deserialize child node: {e}"
                            ))
                        })?;

                    // Recursively collect from child
                    self.collect_keys_from_node(&child_node, hash_mappings, result)?;
                }
            }
        }

        Ok(())
    }

    /// Simple hex decoder (replaces need for hex crate dependency)
    fn decode_hex(&self, hex_str: &str) -> Result<Vec<u8>, GitKvError> {
        if hex_str.len() % 2 != 0 {
            return Err(GitKvError::GitObjectError(
                "Invalid hex string length".to_string(),
            ));
        }

        let mut bytes = Vec::with_capacity(hex_str.len() / 2);
        for chunk in hex_str.as_bytes().chunks(2) {
            let hex_byte = std::str::from_utf8(chunk)
                .map_err(|_| GitKvError::GitObjectError("Invalid hex characters".to_string()))?;
            let byte = u8::from_str_radix(hex_byte, 16)
                .map_err(|_| GitKvError::GitObjectError("Invalid hex digit".to_string()))?;
            bytes.push(byte);
        }

        Ok(bytes)
    }

    /// Reconstruct KV state from a commit using an isolated temporary directory
    /// This method does not modify any files in the working directory
    fn checkout_commit_temporarily_isolated(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        // Create a unique temporary directory
        let temp_dir = std::env::temp_dir().join(format!("prolly_show_{}", commit_id.to_hex()));

        // Create temporary directory
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to create temp dir: {e}")))?;

        // Get the current git repository path
        let git_repo_path = self.store.git_repo().path();

        // Clone the repository to temp directory with a subdirectory structure
        let temp_repo_dir = temp_dir.join("repo");
        let output = std::process::Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--no-checkout",
                git_repo_path.to_str().unwrap_or("."),
                temp_repo_dir.to_str().unwrap_or("temp"),
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
            .current_dir(&temp_repo_dir)
            .output()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to checkout commit: {e}")))?;

        if !output.status.success() {
            return Err(GitKvError::GitObjectError(format!(
                "Git checkout failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Create a subdirectory for the dataset to avoid git root restriction
        let temp_dataset_dir = temp_repo_dir.join("temp_dataset");
        std::fs::create_dir_all(&temp_dataset_dir).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create dataset dir: {e}"))
        })?;

        // Copy prolly config files to the temp dataset directory
        let current_dir = std::env::current_dir()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get current dir: {e}")))?;

        // Find prolly config files in the checked out commit
        if let Ok(config_files) = std::fs::read_dir(&temp_repo_dir) {
            for entry in config_files.flatten() {
                let file_name = entry.file_name();
                if file_name.to_string_lossy().starts_with("prolly_") {
                    let src = entry.path();
                    let dst = temp_dataset_dir.join(&file_name);
                    let _ = std::fs::copy(&src, &dst);
                }
            }
        }

        // Also check for prolly files in any subdirectories that match the current working directory name
        if let Some(current_dir_name) = current_dir.file_name() {
            let subdir_path = temp_repo_dir.join(current_dir_name);
            if let Ok(subdir_files) = std::fs::read_dir(&subdir_path) {
                for entry in subdir_files.flatten() {
                    let file_name = entry.file_name();
                    if file_name.to_string_lossy().starts_with("prolly_") {
                        let src = entry.path();
                        let dst = temp_dataset_dir.join(&file_name);
                        let _ = std::fs::copy(&src, &dst);
                    }
                }
            }
        }

        // Now create a GitVersionedKvStore in the temporary dataset directory
        // This won't affect the original working directory
        let temp_store = GitVersionedKvStore::<N>::open(&temp_dataset_dir)?;

        // Extract all key-value pairs from the temporary store
        let mut state = HashMap::new();
        let keys = temp_store.list_keys();
        for key in keys {
            if let Some(value) = temp_store.get(&key) {
                state.insert(key, value);
            }
        }

        // Clean up temporary directory
        let _ = std::fs::remove_dir_all(&temp_dir);

        Ok(state)
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
