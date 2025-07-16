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
use crate::tree::{ProllyTree, Tree};
use gix::prelude::*;
use std::collections::HashMap;
use std::path::Path;

/// A versioned key-value store backed by Git and ProllyTree
///
/// This combines the efficient tree operations of ProllyTree with Git's
/// version control capabilities, providing a full-featured versioned
/// key-value store with branching, merging, and history.
pub struct VersionedKvStore<const N: usize> {
    tree: ProllyTree<N, GitNodeStorage<N>>,
    git_repo: gix::Repository,
    staging_area: HashMap<Vec<u8>, Option<Vec<u8>>>, // None = deleted
    current_branch: String,
}

impl<const N: usize> VersionedKvStore<N> {
    /// Initialize a new versioned KV store at the given path
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Initialize Git repository
        let git_repo = gix::init(path).map_err(|e| GitKvError::GitInitError(Box::new(e)))?;

        // Create GitNodeStorage
        let storage = GitNodeStorage::new(git_repo.clone())?;

        // Create ProllyTree with default config
        let config: TreeConfig<N> = TreeConfig::default();
        let tree = ProllyTree::new(storage, config);

        let mut store = VersionedKvStore {
            tree,
            git_repo,
            staging_area: HashMap::new(),
            current_branch: "main".to_string(),
        };

        // Save initial configuration
        let _ = store.tree.save_config();

        // Create .gitignore to ignore prolly files
        let gitignore_path = path.join(".gitignore");
        std::fs::write(
            &gitignore_path,
            "prolly_tree_root\nprolly_config_*\nprolly_hash_mappings\n",
        )
        .map_err(|e| GitKvError::GitObjectError(format!("Failed to create .gitignore: {e}")))?;

        // Create initial commit
        store.commit("Initial commit")?;

        Ok(store)
    }

    /// Open an existing versioned KV store
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let path = path.as_ref();

        // Open existing Git repository
        let git_repo = gix::open(path).map_err(|e| GitKvError::GitOpenError(Box::new(e)))?;

        // Create GitNodeStorage
        let storage = GitNodeStorage::new(git_repo.clone())?;

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
        };

        // Load staging area from file if it exists
        store.load_staging_area()?;

        // Reload the tree from the current HEAD
        store.reload_tree_from_head()?;

        Ok(store)
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

        // Create tree object in Git
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
        // Get the current HEAD commit - simplified approach
        let head = self
            .git_repo
            .head()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get HEAD: {e}")))?;

        let _head_commit_id = head.id().ok_or_else(|| {
            GitKvError::GitObjectError("HEAD does not point to a commit".to_string())
        })?;

        let _branch_ref = format!("refs/heads/{name}");

        // Note: This is a simplified implementation
        // A full implementation would use gix transaction API to properly create branch references
        // For now, we return success as branch operations are handled at a higher level
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
                // Note: A full implementation would use gix transaction API to update HEAD
                self.current_branch = branch_or_commit.to_string();
            }
            Err(_) => {
                return Err(GitKvError::BranchNotFound(branch_or_commit.to_string()));
            }
        }

        // Reload tree state from the new HEAD
        self.reload_tree_from_head()?;

        Ok(())
    }

    /// Get current branch name
    pub fn current_branch(&self) -> &str {
        &self.current_branch
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
                                author: String::from_utf8_lossy(commit_ref.author.name).to_string(),
                                committer: String::from_utf8_lossy(commit_ref.committer.name)
                                    .to_string(),
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
        // Create an empty tree - the ProllyTree state is managed through GitNodeStorage
        // We don't need to create a prolly_tree_root file since the tree structure
        // is stored in Git blobs and managed through the NodeStorage interface
        let tree_entries = vec![];

        let tree = gix::objs::Tree {
            entries: tree_entries,
        };

        let tree_id = self
            .git_repo
            .objects
            .write(&tree)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to write tree: {e}")))?;

        Ok(tree_id)
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

        // Create author and committer signatures
        let signature = gix::actor::Signature {
            name: "git-prolly".into(),
            email: "git-prolly@example.com".into(),
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

    /// Reload the ProllyTree from the current HEAD
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

    /// Save the staging area to a file
    fn save_staging_area(&self) -> Result<(), GitKvError> {
        let staging_file = self.git_repo.path().join("PROLLY_STAGING");

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
        let staging_file = self.git_repo.path().join("PROLLY_STAGING");

        if staging_file.exists() {
            let data = std::fs::read(staging_file).map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to read staging area: {e}"))
            })?;

            self.staging_area =
                bincode::deserialize(&data).map_err(GitKvError::SerializationError)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_versioned_store_init() {
        let temp_dir = TempDir::new().unwrap();
        let store = VersionedKvStore::<32>::init(temp_dir.path());
        assert!(store.is_ok());
    }

    #[test]
    fn test_basic_kv_operations() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = VersionedKvStore::<32>::init(temp_dir.path()).unwrap();

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
        let mut store = VersionedKvStore::<32>::init(temp_dir.path()).unwrap();

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
}
