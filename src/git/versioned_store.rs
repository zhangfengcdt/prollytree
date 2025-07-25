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
use crate::git::storage::GitNodeStorage;
use crate::git::types::*;
use crate::storage::{FileNodeStorage, InMemoryNodeStorage, NodeStorage};
use crate::tree::{ProllyTree, Tree};
use gix::prelude::*;
use std::collections::HashMap;
use std::path::Path;

#[cfg(feature = "rocksdb_storage")]
use crate::storage::RocksDBNodeStorage;

/// A versioned key-value store backed by Git and ProllyTree with configurable storage
///
/// This combines the efficient tree operations of ProllyTree with Git's
/// version control capabilities, providing a full-featured versioned
/// key-value store with branching, merging, and history.
pub struct VersionedKvStore<const N: usize, S: NodeStorage<N>> {
    tree: ProllyTree<N, S>,
    git_repo: gix::Repository,
    staging_area: HashMap<Vec<u8>, Option<Vec<u8>>>, // None = deleted
    current_branch: String,
    storage_backend: StorageBackend,
}

/// Type alias for backward compatibility (Git storage)
pub type GitVersionedKvStore<const N: usize> = VersionedKvStore<N, GitNodeStorage<N>>;

/// Type alias for InMemory storage
pub type InMemoryVersionedKvStore<const N: usize> = VersionedKvStore<N, InMemoryNodeStorage<N>>;

/// Type alias for File storage
pub type FileVersionedKvStore<const N: usize> = VersionedKvStore<N, FileNodeStorage<N>>;

/// Type alias for RocksDB storage
#[cfg(feature = "rocksdb_storage")]
pub type RocksDBVersionedKvStore<const N: usize> = VersionedKvStore<N, RocksDBNodeStorage<N>>;

impl<const N: usize, S: NodeStorage<N>> VersionedKvStore<N, S> {
    /// Find the git repository root by walking up the directory tree
    fn find_git_root<P: AsRef<Path>>(start_path: P) -> Option<std::path::PathBuf> {
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

    /// Check if we're running in the git repository root directory
    fn is_in_git_root<P: AsRef<Path>>(path: P) -> Result<bool, GitKvError> {
        let path = path
            .as_ref()
            .canonicalize()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to resolve path: {e}")))?;

        if let Some(git_root) = Self::find_git_root(&path) {
            let git_root = git_root.canonicalize().map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to resolve git root: {e}"))
            })?;
            Ok(path == git_root)
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

        // Persist the tree state
        self.tree.persist_root();

        // Save the updated configuration with the new root hash
        self.tree
            .save_config()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to save config: {e}")))?;

        // Create tree object in Git (this will include prolly metadata files)
        let tree_id = self.create_git_tree()?;

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

    /// Switch to a different branch
    pub fn checkout(&mut self, branch_or_commit: &str) -> Result<(), GitKvError> {
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
                // Update our internal tracking
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

        // Note: Tree reload is handled in Git-specific implementation

        Ok(())
    }

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

    /// Create a Git tree object from the current ProllyTree state
    fn create_git_tree(&self) -> Result<gix::ObjectId, GitKvError> {
        // Actually, we should let git handle the tree creation properly
        // Use git's index to stage files and create tree from the index

        // Get the git root directory
        let git_root = Self::find_git_root(self.git_repo.path().parent().unwrap()).unwrap();
        let current_dir = std::env::current_dir().map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to get current directory: {e}"))
        })?;

        // Get relative path from git root to current directory
        let relative_dir = current_dir.strip_prefix(&git_root).unwrap_or(&current_dir);

        // Stage the prolly metadata files using git add
        let config_file = "prolly_config_tree_config";
        let mapping_file = "prolly_hash_mappings";

        for filename in &[config_file, mapping_file] {
            let file_path = current_dir.join(filename);
            if file_path.exists() {
                // Get relative path from git root
                let relative_path = relative_dir.join(filename);
                let relative_path_str = relative_path.to_string_lossy();

                let add_cmd = std::process::Command::new("git")
                    .args(["add", &relative_path_str])
                    .current_dir(&git_root)
                    .output()
                    .map_err(|e| {
                        GitKvError::GitObjectError(format!("Failed to run git add: {e}"))
                    })?;

                if !add_cmd.status.success() {
                    let stderr = String::from_utf8_lossy(&add_cmd.stderr);
                    eprintln!("Warning: git add failed for {filename}: {stderr}");
                }
            }
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

        Ok(tree_id)
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
            .unwrap()
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
    fn save_staging_area(&self) -> Result<(), GitKvError> {
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
    fn load_staging_area(&mut self) -> Result<(), GitKvError> {
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
}

// Storage-specific implementations
impl<const N: usize> VersionedKvStore<N, GitNodeStorage<N>> {
    /// Initialize a new versioned KV store with Git storage (default)
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Reject if trying to initialize in git root directory
        if Self::is_in_git_root(path)? {
            return Err(GitKvError::GitObjectError(
                "Cannot initialize git-prolly in git root directory. Please run from a subdirectory to create a dataset.".to_string()
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

        // Create GitNodeStorage
        let storage = GitNodeStorage::new(git_repo.clone(), path.to_path_buf())?;

        // Create ProllyTree with default config
        let config: TreeConfig<N> = TreeConfig::default();
        let tree = ProllyTree::new(storage, config);

        let mut store = VersionedKvStore {
            tree,
            git_repo,
            staging_area: HashMap::new(),
            current_branch: "main".to_string(),
            storage_backend: StorageBackend::Git,
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

        // Reject if trying to open in git root directory
        if Self::is_in_git_root(path)? {
            return Err(GitKvError::GitObjectError(
                "Cannot run git-prolly in git root directory. Please run from a subdirectory containing a dataset.".to_string()
            ));
        }

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // Open existing Git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create GitNodeStorage
        let storage = GitNodeStorage::new(git_repo.clone(), path.to_path_buf())?;

        // Load tree configuration from storage
        let config: TreeConfig<N> = ProllyTree::load_config(&storage).unwrap_or_default();

        // Try to load existing tree from storage, or create new one
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
            git_repo,
            staging_area: HashMap::new(),
            current_branch,
            storage_backend: StorageBackend::Git,
        };

        // Load staging area from file if it exists
        store.load_staging_area()?;

        // Reload the tree from the current HEAD
        store.reload_tree_from_head()?;

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

    /// Reload the ProllyTree from the current HEAD (Git-specific)
    fn reload_tree_from_head(&mut self) -> Result<(), GitKvError> {
        // Since we're no longer storing prolly_tree_root in the Git tree,
        // we need to reload the tree state from the GitNodeStorage

        // Load tree configuration from storage
        let config: TreeConfig<N> = ProllyTree::load_config(&self.tree.storage).unwrap_or_default();

        // Try to load existing tree from storage, or create new one
        let storage = self.tree.storage.clone();
        self.tree = ProllyTree::load_from_storage(storage.clone(), config.clone())
            .unwrap_or_else(|| ProllyTree::new(storage, config));

        Ok(())
    }
}

impl<const N: usize> VersionedKvStore<N, InMemoryNodeStorage<N>> {
    /// Initialize a new versioned KV store with InMemory storage
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // Open the existing git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create InMemoryNodeStorage
        let storage = InMemoryNodeStorage::<N>::new();

        // Create ProllyTree with default config
        let config: TreeConfig<N> = TreeConfig::default();
        let tree = ProllyTree::new(storage, config);

        let mut store = VersionedKvStore {
            tree,
            git_repo,
            staging_area: HashMap::new(),
            current_branch: "main".to_string(),
            storage_backend: StorageBackend::InMemory,
        };

        // Note: InMemory storage doesn't persist config
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

impl<const N: usize> VersionedKvStore<N, FileNodeStorage<N>> {
    /// Initialize a new versioned KV store with File storage
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // Open the existing git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create FileNodeStorage with a subdirectory for file storage
        let file_storage_path = path.join("file_storage");
        let storage = FileNodeStorage::<N>::new(file_storage_path);

        // Create ProllyTree with default config
        let config: TreeConfig<N> = TreeConfig::default();
        let tree = ProllyTree::new(storage, config);

        let mut store = VersionedKvStore {
            tree,
            git_repo,
            staging_area: HashMap::new(),
            current_branch: "main".to_string(),
            storage_backend: StorageBackend::File,
        };

        // Save initial configuration
        let _ = store.tree.save_config();

        // Create initial commit
        store.commit("Initial commit")?;

        Ok(store)
    }

    /// Open an existing versioned KV store with File storage
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // Open existing Git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create FileNodeStorage with a subdirectory for file storage
        let file_storage_path = path.join("file_storage");
        let storage = FileNodeStorage::<N>::new(file_storage_path.clone());

        // Load tree configuration from storage
        let config: TreeConfig<N> = ProllyTree::load_config(&storage).unwrap_or_default();

        // Try to load existing tree from storage, or create new one
        let tree =
            if let Some(existing_tree) = ProllyTree::load_from_storage(storage, config.clone()) {
                existing_tree
            } else {
                // Create new storage instance since the original was consumed
                let new_storage = FileNodeStorage::<N>::new(file_storage_path);
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
            git_repo,
            staging_area: HashMap::new(),
            current_branch,
            storage_backend: StorageBackend::File,
        };

        // Load staging area from file if it exists
        store.load_staging_area()?;

        // Note: File storage data is loaded directly, no need to reload from HEAD

        Ok(store)
    }
}

#[cfg(feature = "rocksdb_storage")]
impl<const N: usize> VersionedKvStore<N, RocksDBNodeStorage<N>> {
    /// Initialize a new versioned KV store with RocksDB storage
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // Open the existing git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create RocksDBNodeStorage with a subdirectory for RocksDB
        let rocksdb_path = path.join("rocksdb");
        let storage = RocksDBNodeStorage::<N>::new(rocksdb_path)
            .map_err(|e| GitKvError::GitObjectError(format!("RocksDB creation failed: {e}")))?;

        // Create ProllyTree with default config
        let config: TreeConfig<N> = TreeConfig::default();
        let tree = ProllyTree::new(storage, config);

        let mut store = VersionedKvStore {
            tree,
            git_repo,
            staging_area: HashMap::new(),
            current_branch: "main".to_string(),
            storage_backend: StorageBackend::RocksDB,
        };

        // Save initial configuration
        let _ = store.tree.save_config();

        // Create initial commit
        store.commit("Initial commit")?;

        Ok(store)
    }

    /// Open an existing versioned KV store with RocksDB storage
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Find the git repository
        let git_root = Self::find_git_root(path).ok_or_else(|| {
            GitKvError::GitObjectError(
                "Not inside a git repository. Please run from within a git repository.".to_string(),
            )
        })?;

        // Open existing Git repository
        let git_repo = gix::open(&git_root).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create RocksDBNodeStorage with a subdirectory for RocksDB
        let rocksdb_path = path.join("rocksdb");
        let storage = RocksDBNodeStorage::<N>::new(rocksdb_path)
            .map_err(|e| GitKvError::GitObjectError(format!("RocksDB creation failed: {e}")))?;

        // Load tree configuration from storage
        let config: TreeConfig<N> = ProllyTree::load_config(&storage).unwrap_or_default();

        // Try to load existing tree from storage, or create new one
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
            git_repo,
            staging_area: HashMap::new(),
            current_branch,
            storage_backend: StorageBackend::RocksDB,
        };

        // Load staging area from file if it exists
        store.load_staging_area()?;

        // Note: RocksDB storage data is loaded directly, no need to reload from HEAD

        Ok(store)
    }
}

// Generic implementations for all storage types
impl<const N: usize, S: NodeStorage<N>> VersionedKvStore<N, S> {
    /// Get the current storage backend type
    pub fn storage_backend(&self) -> &StorageBackend {
        &self.storage_backend
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_versioned_store_init() {
        let temp_dir = TempDir::new().unwrap();
        // Initialize git repository (regular, not bare)
        gix::init(temp_dir.path()).unwrap();
        // Create subdirectory for dataset
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir_all(&dataset_dir).unwrap();
        let store = GitVersionedKvStore::<32>::init(&dataset_dir);
        assert!(store.is_ok());
    }

    #[test]
    fn test_basic_kv_operations() {
        let temp_dir = TempDir::new().unwrap();
        // Initialize git repository (regular, not bare)
        gix::init(temp_dir.path()).unwrap();
        // Create subdirectory for dataset
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir_all(&dataset_dir).unwrap();
        let mut store = GitVersionedKvStore::<32>::init(&dataset_dir).unwrap();

        // Test insert and get
        store.insert(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        assert_eq!(store.get(b"key1"), Some(b"value1".to_vec()));

        // Test update
        store
            .update(b"key1".to_vec(), b"new_value1".to_vec())
            .unwrap();
        assert_eq!(store.get(b"key1"), Some(b"new_value1".to_vec()));

        // Test delete
        store.delete(b"key1").unwrap();
        assert_eq!(store.get(b"key1"), None);
    }

    #[test]
    fn test_commit_workflow() {
        let temp_dir = TempDir::new().unwrap();
        // Initialize git repository (regular, not bare)
        gix::init(temp_dir.path()).unwrap();
        // Create subdirectory for dataset
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir_all(&dataset_dir).unwrap();
        let mut store = GitVersionedKvStore::<32>::init(&dataset_dir).unwrap();

        // Stage changes
        store.insert(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        store.insert(b"key2".to_vec(), b"value2".to_vec()).unwrap();

        // Check status
        let status = store.status();
        assert_eq!(status.len(), 2);

        // Commit
        let commit_id = store.commit("Add initial data").unwrap();
        // Now we have a real implementation that returns valid commit IDs
        assert!(!commit_id.is_null());

        // Check that staging area is clear
        let status = store.status();
        assert_eq!(status.len(), 0);
    }

    #[test]
    fn test_single_commit_behavior() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repository
        gix::init(temp_dir.path()).unwrap();

        // Create subdirectory for dataset
        let dataset_dir = temp_dir.path().join("dataset");
        std::fs::create_dir_all(&dataset_dir).unwrap();

        let mut store = GitVersionedKvStore::<32>::init(&dataset_dir).unwrap();

        // Get initial commit count
        let log_output = std::process::Command::new("git")
            .args(&["log", "--oneline"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        let initial_commits = String::from_utf8_lossy(&log_output.stdout).lines().count();

        // Insert some data and commit
        store
            .insert(b"test_key".to_vec(), b"test_value".to_vec())
            .unwrap();
        store.commit("Test single commit").unwrap();

        // Get commit count after our commit
        let log_output = std::process::Command::new("git")
            .args(&["log", "--oneline"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        let final_commits = String::from_utf8_lossy(&log_output.stdout).lines().count();

        // Should have exactly one more commit (no separate metadata commit)
        assert_eq!(
            final_commits,
            initial_commits + 1,
            "Expected exactly one new commit, but got {} new commits",
            final_commits - initial_commits
        );

        // Verify the prolly metadata files exist in the dataset directory
        let config_path = dataset_dir.join("prolly_config_tree_config");
        let mapping_path = dataset_dir.join("prolly_hash_mappings");
        assert!(
            config_path.exists(),
            "prolly_config_tree_config should exist"
        );
        assert!(mapping_path.exists(), "prolly_hash_mappings should exist");
    }
}
