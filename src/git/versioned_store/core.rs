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
use crate::git::types::*;
use crate::storage::NodeStorage;
use crate::tree::Tree;
use gix::prelude::*;
use std::path::Path;

impl<const N: usize, S: NodeStorage<N>> VersionedKvStore<N, S>
where
    Self: TreeConfigSaver<N>,
{
    /// Find the git repository root by walking up the directory tree
    pub(super) fn find_git_root<P: AsRef<Path>>(start_path: P) -> Option<std::path::PathBuf> {
        let mut current = start_path.as_ref().to_path_buf();
        loop {
            if current.join(".git").exists() {
                return Some(current);
            }
            if !current.pop() {
                break;
            }
        }
        None
    }

    /// Get the common git directory path, handling worktrees and submodules.
    /// For worktrees, this resolves to the main .git directory (not the per-worktree gitdir)
    /// to ensure shared node storage is placed in a common location.
    pub(super) fn resolve_git_dir<P: AsRef<Path>>(git_root: P) -> std::path::PathBuf {
        let git_path = git_root.as_ref().join(".git");

        // If .git is a file (worktree or submodule), read the gitdir path from it
        let gitdir = if git_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&git_path) {
                let mut resolved_gitdir = None;
                for line in content.lines() {
                    if let Some(gitdir_str) = line.strip_prefix("gitdir:") {
                        let gitdir_str = gitdir_str.trim();
                        // Handle both absolute and relative paths
                        let gitdir_path = std::path::Path::new(gitdir_str);
                        resolved_gitdir = Some(if gitdir_path.is_absolute() {
                            gitdir_path.to_path_buf()
                        } else {
                            git_root.as_ref().join(gitdir_str)
                        });
                        break;
                    }
                }
                resolved_gitdir.unwrap_or(git_path)
            } else {
                git_path
            }
        } else {
            // Default: .git is a directory
            git_path
        };

        // For linked worktrees, resolve to the common git directory
        // The commondir file contains the path to the main .git directory
        let commondir_path = gitdir.join("commondir");
        if commondir_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&commondir_path) {
                let commondir_str = content.trim();
                if !commondir_str.is_empty() {
                    let commondir = std::path::Path::new(commondir_str);
                    // Handle both absolute and relative paths
                    let resolved_commondir = if commondir.is_absolute() {
                        commondir.to_path_buf()
                    } else {
                        // Relative path is relative to the gitdir
                        gitdir.join(commondir)
                    };
                    // Canonicalize to resolve .. components
                    if let Ok(canonical) = resolved_commondir.canonicalize() {
                        return canonical;
                    }
                    return resolved_commondir;
                }
            }
        }

        gitdir
    }

    /// Get the prolly directory path inside the git directory
    /// This is where all ProllyTree data is stored to avoid accidental git versioning
    pub(super) fn get_prolly_dir<P: AsRef<Path>>(git_root: P) -> std::path::PathBuf {
        Self::resolve_git_dir(git_root).join("prolly")
    }

    /// Ensure the prolly directory structure exists
    pub(super) fn ensure_prolly_dir<P: AsRef<Path>>(
        git_root: P,
    ) -> Result<std::path::PathBuf, GitKvError> {
        let prolly_dir = Self::get_prolly_dir(&git_root);
        std::fs::create_dir_all(&prolly_dir).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create prolly directory: {e}"))
        })?;
        Ok(prolly_dir)
    }

    /// Check if the given path is the git repository root directory
    /// This is used to prevent initializing a dataset at the git root,
    /// which could cause `git add -A .` to stage unrelated files.
    pub(super) fn is_in_git_root<P: AsRef<Path>>(path: P) -> Result<bool, GitKvError> {
        let path = path.as_ref();

        // Try to canonicalize the path. If it doesn't exist, use the parent directory.
        let canonical_path = if path.exists() {
            path.canonicalize()
                .map_err(|e| GitKvError::GitObjectError(format!("Failed to resolve path: {e}")))?
        } else {
            // Path doesn't exist yet (common for init). Use parent + last component.
            let parent = path.parent().ok_or_else(|| {
                GitKvError::GitObjectError("Invalid path: no parent directory".to_string())
            })?;

            // If parent doesn't exist either, we can't proceed
            if !parent.exists() && !parent.as_os_str().is_empty() {
                return Err(GitKvError::GitObjectError(format!(
                    "Parent directory does not exist: {}",
                    parent.display()
                )));
            }

            // Canonicalize parent and append the last component
            let canonical_parent = if parent.as_os_str().is_empty() {
                std::env::current_dir().map_err(|e| {
                    GitKvError::GitObjectError(format!("Failed to get current directory: {e}"))
                })?
            } else {
                parent.canonicalize().map_err(|e| {
                    GitKvError::GitObjectError(format!("Failed to resolve parent path: {e}"))
                })?
            };

            // Append the file name to get the full path
            if let Some(file_name) = path.file_name() {
                canonical_parent.join(file_name)
            } else {
                canonical_parent
            }
        };

        // Find git root from the path (or its parent if path doesn't exist)
        let lookup_path = if path.exists() {
            canonical_path.clone()
        } else {
            canonical_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(canonical_path.clone())
        };

        if let Some(git_root) = Self::find_git_root(&lookup_path) {
            let git_root = git_root.canonicalize().map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to resolve git root: {e}"))
            })?;
            Ok(canonical_path == git_root)
        } else {
            Err(GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            ))
        }
    }

    /// Insert a key-value pair (stages the change)
    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), GitKvError> {
        self.staging_area.insert(key, Some(value));
        self.save_staging_area()?;
        Ok(())
    }

    /// Update an existing key-value pair (stages the change)
    pub fn update(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<bool, GitKvError> {
        let exists = self.get(&key).is_some();
        if exists {
            self.staging_area.insert(key, Some(value));
            self.save_staging_area()?;
        }
        Ok(exists)
    }

    /// Delete a key-value pair (stages the change)
    pub fn delete(&mut self, key: &[u8]) -> Result<bool, GitKvError> {
        let exists = self.get(key).is_some();
        if exists {
            self.staging_area.insert(key.to_vec(), None);
            self.save_staging_area()?;
        }
        Ok(exists)
    }

    /// Get a value by key (checks staging area first, then committed data)
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        // Check staging area first
        if let Some(staged_value) = self.staging_area.get(key) {
            return staged_value.clone();
        }

        // Check committed data
        self.tree.find(key).and_then(|node| {
            // Find the value in the node
            node.keys
                .iter()
                .position(|k| k == key)
                .map(|index| node.values[index].clone())
        })
    }

    /// List all keys (includes staged changes)
    pub fn list_keys(&self) -> Vec<Vec<u8>> {
        let mut keys = std::collections::HashSet::new();

        // Add keys from the committed ProllyTree
        for key in self.tree.collect_keys() {
            keys.insert(key);
        }

        // Add keys from staging area (overrides committed data)
        for (key, value) in &self.staging_area {
            if value.is_some() {
                keys.insert(key.clone());
            } else {
                keys.remove(key);
            }
        }

        keys.into_iter().collect()
    }

    /// Show current staging area status
    pub fn status(&self) -> Vec<(Vec<u8>, String)> {
        let mut status = Vec::new();

        for (key, value) in &self.staging_area {
            let status_str = match value {
                Some(_) => {
                    if self.tree.find(key).is_some() {
                        "modified".to_string()
                    } else {
                        "added".to_string()
                    }
                }
                None => "deleted".to_string(),
            };
            status.push((key.clone(), status_str));
        }

        status
    }

    /// Commit staged changes
    pub fn commit(&mut self, message: &str) -> Result<gix::ObjectId, GitKvError> {
        // Apply staged changes to the tree
        for (key, value) in self.staging_area.drain() {
            match value {
                Some(v) => {
                    self.tree.insert(key, v);
                }
                None => {
                    self.tree.delete(&key);
                }
            }
        }

        // Persist the tree state (including updating root hash and saving config)
        self.tree.persist_root();

        // For all storage types, also save the tree config to git for historical access
        self.save_tree_config_to_git_internal()?;

        // Create tree object in Git using git commands
        // Get the git root directory using work_dir() for worktree/submodule compatibility
        let dataset_dir = self
            .dataset_dir
            .as_ref()
            .ok_or_else(|| GitKvError::GitObjectError("Dataset directory not set".into()))?;
        let git_root = self
            .git_repo
            .work_dir()
            .map(|p| p.to_path_buf())
            .or_else(|| Self::find_git_root(dataset_dir))
            .ok_or_else(|| GitKvError::GitObjectError("Could not find git root".into()))?;

        // Stage all files in the current directory recursively
        let add_cmd = std::process::Command::new("git")
            .args(["add", "-A", "."])
            .current_dir(&git_root)
            .output()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to run git add: {e}")))?;

        if !add_cmd.status.success() {
            let stderr = String::from_utf8_lossy(&add_cmd.stderr);
            eprintln!("Warning: git add failed: {stderr}");
        }

        // Use git write-tree to create tree from the current index
        let write_tree_cmd = std::process::Command::new("git")
            .args(["write-tree"])
            .current_dir(&git_root)
            .output()
            .map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to run git write-tree: {e}"))
            })?;

        if !write_tree_cmd.status.success() {
            let stderr = String::from_utf8_lossy(&write_tree_cmd.stderr);
            return Err(GitKvError::GitObjectError(format!(
                "git write-tree failed: {stderr}"
            )));
        }

        let tree_hash = String::from_utf8_lossy(&write_tree_cmd.stdout)
            .trim()
            .to_string();
        let tree_id = gix::ObjectId::from_hex(tree_hash.as_bytes())
            .map_err(|e| GitKvError::GitObjectError(format!("Invalid tree hash: {e}")))?;

        // Create commit
        let commit_id = self.create_git_commit(tree_id, message)?;

        // Update HEAD
        self.update_head(commit_id)?;

        // Clear staging area file since we've committed
        self.save_staging_area()?;

        Ok(commit_id)
    }

    /// Create a new branch
    pub fn branch(&mut self, name: &str) -> Result<(), GitKvError> {
        // Get the current HEAD commit
        let head = self
            .git_repo
            .head()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get HEAD: {e}")))?;

        let head_commit_id = head.id().ok_or_else(|| {
            GitKvError::GitObjectError("HEAD does not point to a commit".to_string())
        })?;

        // Create the branch reference to point to the current HEAD
        let refs_dir = self.git_repo.path().join("refs").join("heads");
        std::fs::create_dir_all(&refs_dir).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create refs directory: {e}"))
        })?;

        let branch_file = refs_dir.join(name);

        // Create parent directories if the branch name contains slashes
        if let Some(parent) = branch_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to create branch directory: {e}"))
            })?;
        }

        std::fs::write(&branch_file, head_commit_id.to_hex().to_string()).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to write branch reference: {e}"))
        })?;

        Ok(())
    }

    /// Create a new branch from the current branch and switch to it
    pub fn create_branch(&mut self, name: &str) -> Result<(), GitKvError> {
        // First create the branch
        self.branch(name)?;

        // Then switch to it
        // Clear staging area
        self.staging_area.clear();
        self.save_staging_area()?;

        // Update our internal tracking to the new branch
        self.current_branch = name.to_string();

        // Update HEAD to point to the new branch
        let head_file = self.git_repo.path().join("HEAD");
        let head_content = format!("ref: refs/heads/{name}");
        std::fs::write(&head_file, head_content)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to update HEAD: {e}")))?;

        // Note: Tree reload is handled in Git-specific implementation

        Ok(())
    }

    // Note: checkout is implemented differently for each storage type
    // GitNodeStorage has its own implementation that reloads tree state

    /// Get current branch name
    pub fn current_branch(&self) -> &str {
        &self.current_branch
    }

    /// List all branches in the repository
    pub fn list_branches(&self) -> Result<Vec<String>, GitKvError> {
        let mut branches = Vec::new();

        // Get all refs under refs/heads/
        let refs = self
            .git_repo
            .refs
            .iter()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to iterate refs: {e}")))?;

        for reference in (refs
            .all()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get refs: {e}")))?)
        .flatten()
        {
            if let Some(name) = reference.name.as_bstr().strip_prefix(b"refs/heads/") {
                let branch_name = String::from_utf8_lossy(name).to_string();
                branches.push(branch_name);
            }
        }

        branches.sort();
        Ok(branches)
    }

    /// Get access to the git repository (for internal use)
    pub fn git_repo(&self) -> &gix::Repository {
        &self.git_repo
    }

    /// Get commit history
    pub fn log(&self) -> Result<Vec<CommitInfo>, GitKvError> {
        let mut history = Vec::new();

        // Get the current HEAD commit
        let head_commit = match self.git_repo.head_commit() {
            Ok(commit) => commit,
            Err(_) => return Ok(history), // No commits yet
        };

        // Use rev_walk to traverse the commit history
        let rev_walk = self.git_repo.rev_walk([head_commit.id()]);

        match rev_walk.all() {
            Ok(walk) => {
                for info in walk.take(100).flatten() {
                    // Limit to 100 commits
                    if let Ok(commit_obj) = info.object() {
                        if let Ok(commit_ref) = commit_obj.decode() {
                            let commit_info = CommitInfo {
                                id: commit_obj.id().into(),
                                author: format!(
                                    "{} <{}>",
                                    String::from_utf8_lossy(commit_ref.author.name),
                                    String::from_utf8_lossy(commit_ref.author.email)
                                ),
                                committer: format!(
                                    "{} <{}>",
                                    String::from_utf8_lossy(commit_ref.committer.name),
                                    String::from_utf8_lossy(commit_ref.committer.email)
                                ),
                                message: String::from_utf8_lossy(commit_ref.message).to_string(),
                                timestamp: commit_ref.author.time.seconds,
                            };
                            history.push(commit_info);
                        }
                    }
                }
            }
            Err(_) => {
                // Fallback to single commit if rev_walk fails
                let commit_info = CommitInfo {
                    id: head_commit.id().into(),
                    author: "Unknown".to_string(),
                    committer: "Unknown".to_string(),
                    message: "Commit".to_string(),
                    timestamp: 0,
                };
                history.push(commit_info);
            }
        }

        Ok(history)
    }

    /// Get git user configuration (name and email)
    fn get_git_user_config(&self) -> Result<(String, String), GitKvError> {
        let config = self.git_repo.config_snapshot();

        let name = config
            .string("user.name")
            .map(|n| n.to_string())
            .unwrap_or_else(|| "git-prolly".to_string());

        let email = config
            .string("user.email")
            .map(|e| e.to_string())
            .unwrap_or_else(|| "git-prolly@example.com".to_string());

        Ok((name, email))
    }

    /// Create a Git commit object
    fn create_git_commit(
        &self,
        tree_id: gix::ObjectId,
        message: &str,
    ) -> Result<gix::ObjectId, GitKvError> {
        // Get the current time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| GitKvError::GitObjectError(format!("System time error: {e}")))?
            .as_secs() as i64;

        // Get git user configuration
        let (name, email) = self.get_git_user_config()?;

        // Create author and committer signatures
        let signature = gix::actor::Signature {
            name: name.into(),
            email: email.into(),
            time: gix::date::Time {
                seconds: now,
                offset: 0,
                sign: gix::date::time::Sign::Plus,
            },
        };

        // Get parent commits (current HEAD if exists)
        let parent_ids = match self.git_repo.head_commit() {
            Ok(parent) => vec![parent.id().into()],
            Err(_) => vec![], // No parent for initial commit
        };

        // Create commit object
        let commit = gix::objs::Commit {
            tree: tree_id,
            parents: parent_ids.into(),
            author: signature.clone(),
            committer: signature,
            encoding: None,
            message: message.as_bytes().into(),
            extra_headers: vec![],
        };

        let commit_id = self
            .git_repo
            .objects
            .write(&commit)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to write commit: {e}")))?;

        Ok(commit_id)
    }

    /// Update HEAD to point to the new commit
    fn update_head(&mut self, commit_id: gix::ObjectId) -> Result<(), GitKvError> {
        // Update the current branch reference to point to the new commit
        let branch_ref = format!("refs/heads/{}", self.current_branch);

        // For now, use a simple implementation that writes directly to the file
        let refs_dir = self.git_repo.path().join("refs").join("heads");
        std::fs::create_dir_all(&refs_dir).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create refs directory: {e}"))
        })?;

        let branch_file = refs_dir.join(&self.current_branch);
        std::fs::write(&branch_file, commit_id.to_hex().to_string()).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to write branch reference: {e}"))
        })?;

        // Update HEAD to point to the branch
        let head_file = self.git_repo.path().join("HEAD");
        std::fs::write(&head_file, format!("ref: {branch_ref}\n")).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to write HEAD reference: {e}"))
        })?;

        Ok(())
    }

    /// Save the staging area to a file
    pub(super) fn save_staging_area(&self) -> Result<(), GitKvError> {
        let staging_file = self.get_staging_file_path()?;

        // Serialize the staging area
        let serialized =
            bincode::serialize(&self.staging_area).map_err(GitKvError::SerializationError)?;

        std::fs::write(staging_file, serialized).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to write staging area: {e}"))
        })?;

        Ok(())
    }

    /// Load the staging area from a file
    pub(super) fn load_staging_area(&mut self) -> Result<(), GitKvError> {
        let staging_file = self.get_staging_file_path()?;

        if staging_file.exists() {
            let data = std::fs::read(staging_file).map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to read staging area: {e}"))
            })?;

            self.staging_area =
                bincode::deserialize(&data).map_err(GitKvError::SerializationError)?;
        }

        Ok(())
    }

    /// Get the dataset-specific staging file path
    fn get_staging_file_path(&self) -> Result<std::path::PathBuf, GitKvError> {
        // Get the current directory relative to git root
        let current_dir = std::env::current_dir().map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to get current directory: {e}"))
        })?;

        let git_root = Self::find_git_root(&current_dir)
            .ok_or_else(|| GitKvError::GitObjectError("Not in a git repository".to_string()))?;

        // Create a dataset-specific identifier from the relative path
        let relative_path = current_dir
            .strip_prefix(&git_root)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get relative path: {e}")))?;

        // Use the relative path to create a unique staging file name
        let path_str = relative_path.to_string_lossy().replace(['/', '\\'], "_");
        let staging_filename = if path_str.is_empty() {
            "PROLLY_STAGING_root".to_string()
        } else {
            format!("PROLLY_STAGING_{path_str}")
        };

        Ok(self.git_repo.path().join(staging_filename))
    }

    /// Generate a cryptographic proof for a key's existence and value in the tree
    /// This proof can be used to verify the integrity of the key-value pair without
    /// requiring access to the entire tree structure.
    ///
    /// # Parameters
    /// - `key`: The key to generate proof for
    ///
    /// # Returns
    /// - A proof object containing the hash path from root to the target node
    pub fn generate_proof(&self, key: &[u8]) -> crate::proof::Proof<N> {
        self.tree.generate_proof(key)
    }

    /// Verify a cryptographic proof for a key-value pair
    /// This checks that the proof is valid and optionally verifies the expected value
    ///
    /// # Parameters
    /// - `proof`: The proof to verify
    /// - `key`: The key that the proof claims to prove
    /// - `expected_value`: Optional expected value to verify against
    ///
    /// # Returns
    /// - `true` if the proof is valid, `false` otherwise
    pub fn verify(
        &self,
        proof: crate::proof::Proof<N>,
        key: &[u8],
        expected_value: Option<&[u8]>,
    ) -> bool {
        self.tree.verify(proof, key, expected_value)
    }
}

// Generic diff functionality for all storage types
impl<const N: usize, S: NodeStorage<N>> VersionedKvStore<N, S>
where
    VersionedKvStore<N, S>: HistoricalAccess<N>,
{
    /// Compare two commits or branches and return all keys that are added, updated or deleted
    pub fn diff(&self, from: &str, to: &str) -> Result<Vec<KvDiff>, GitKvError> {
        // Get all keys from both references
        let from_keys = self.get_keys_at_ref(from)?;
        let to_keys = self.get_keys_at_ref(to)?;

        let mut diffs = Vec::new();

        // Check for added or modified keys
        for (key, to_value) in &to_keys {
            match from_keys.get(key) {
                None => {
                    // Key was added
                    diffs.push(KvDiff {
                        key: key.clone(),
                        operation: DiffOperation::Added(to_value.clone()),
                    });
                }
                Some(from_value) => {
                    if from_value != to_value {
                        // Key was modified
                        diffs.push(KvDiff {
                            key: key.clone(),
                            operation: DiffOperation::Modified {
                                old: from_value.clone(),
                                new: to_value.clone(),
                            },
                        });
                    }
                }
            }
        }

        // Check for removed keys
        for (key, from_value) in &from_keys {
            if !to_keys.contains_key(key) {
                diffs.push(KvDiff {
                    key: key.clone(),
                    operation: DiffOperation::Removed(from_value.clone()),
                });
            }
        }

        // Sort diffs by key for consistent output
        diffs.sort_by(|a, b| a.key.cmp(&b.key));

        Ok(diffs)
    }
}

// Generic commit history functionality for all storage types
impl<const N: usize, S: NodeStorage<N>> VersionedKvStore<N, S>
where
    VersionedKvStore<N, S>: HistoricalCommitAccess<N>,
{
    /// Get all commits that contain changes to a specific key
    /// Returns commits in reverse chronological order (newest first), similar to `git log -- <file>`
    pub fn get_commits(&self, key: &[u8]) -> Result<Vec<CommitInfo>, GitKvError> {
        self.get_commits_for_key(key)
    }

    /// Get the current HEAD commit ID
    pub fn current_commit(&self) -> Result<gix::ObjectId, GitKvError> {
        let head = self
            .git_repo
            .head()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get HEAD: {e}")))?;

        let head_commit_id = head.id().ok_or_else(|| {
            GitKvError::GitObjectError("HEAD does not point to a commit".to_string())
        })?;

        Ok(head_commit_id.detach())
    }
}
