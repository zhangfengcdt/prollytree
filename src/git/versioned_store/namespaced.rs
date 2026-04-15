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

//! Namespace-aware versioned key-value store.
//!
//! [`NamespacedKvStore`] wraps a [`VersionedKvStore`] and adds native namespace
//! support where each namespace is backed by its own [`ProllyTree`] subtree.
//! A namespace registry maps namespace names to their subtree root hashes,
//! enabling O(1) change detection and efficient namespace-scoped operations.
//!
//! # Architecture
//!
//! ```text
//! NamespacedKvStore
//! ├── inner: VersionedKvStore   (git plumbing: commits, branches, refs)
//! ├── registry: HashMap<String, NamespaceEntry>  (namespace → root hash + config)
//! ├── namespaces: HashMap<String, ProllyTree>    (lazily loaded subtrees)
//! ├── namespace_staging: HashMap<String, ...>     (per-namespace staging areas)
//! └── dirty_namespaces: HashSet<String>           (modified since last commit)
//! ```
//!
//! All namespace trees share the same [`NodeStorage`] backend (content-addressed,
//! so there is no collision risk).

use super::{TreeConfigSaver, VersionedKvStore};
use crate::config::TreeConfig;
use crate::diff::{ConflictResolver, IgnoreConflictsResolver, MergeConflict};
use crate::digest::ValueDigest;
use crate::git::metadata::{GitMetadataBackend, MetadataBackend};
use crate::git::types::*;
use crate::storage::{GitNodeStorage, InMemoryNodeStorage, NodeStorage};
use crate::tree::{ProllyTree, Tree};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use crate::storage::FileNodeStorage;

#[cfg(feature = "rocksdb_storage")]
use crate::storage::RocksDBNodeStorage;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Storage format version for detecting V1 (flat) vs V2 (namespaced) stores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StoreFormatVersion {
    /// Legacy flat store — single ProllyTree, no namespaces.
    V1,
    /// Namespaced store — registry + per-namespace subtrees.
    V2,
}

/// Serializable entry for a single namespace in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceEntry<const N: usize> {
    /// Root hash of the namespace's ProllyTree (None if empty / newly created).
    pub root_hash: Option<ValueDigest<N>>,
    /// TreeConfig for this namespace's ProllyTree.
    pub config: TreeConfig<N>,
}

/// Report returned by [`NamespacedKvStore::migrate_v1_to_v2`].
#[derive(Debug, Clone)]
pub struct MigrationReport {
    pub keys_migrated: usize,
    pub namespaces_created: Vec<String>,
    pub storage_version: StoreFormatVersion,
}

/// The default namespace name used for backward-compatible flat API calls.
pub const DEFAULT_NAMESPACE: &str = "default";

// ---------------------------------------------------------------------------
// NamespacedKvStore
// ---------------------------------------------------------------------------

/// A namespace-aware versioned key-value store.
///
/// Each namespace is backed by its own [`ProllyTree`] subtree, with a registry
/// mapping namespace names to subtree root hashes. This enables:
///
/// - **O(1) change detection**: compare 32-byte root hashes per namespace.
/// - **Efficient scoped operations**: namespace list/delete without full scan.
/// - **Clean isolation**: keys in different namespaces live in separate trees.
/// - **Scalable merge**: skip unchanged namespaces entirely.
///
/// The struct wraps a [`VersionedKvStore`] which handles all git plumbing
/// (commits, branches, refs, staging files to git).
pub struct NamespacedKvStore<
    const N: usize,
    S: NodeStorage<N>,
    M: MetadataBackend = GitMetadataBackend,
> {
    /// Underlying versioned store — handles git metadata (commits, branches, HEAD).
    pub(crate) inner: VersionedKvStore<N, S, M>,
    /// Registry: namespace name → entry (root hash + config).
    pub(crate) registry: HashMap<String, NamespaceEntry<N>>,
    /// Lazily loaded per-namespace ProllyTree instances.
    pub(crate) namespaces: HashMap<String, ProllyTree<N, S>>,
    /// Per-namespace staging areas. `None` value = deletion.
    pub(crate) namespace_staging: HashMap<String, HashMap<Vec<u8>, Option<Vec<u8>>>>,
    /// Default namespace name for backward-compatible operations.
    pub(crate) default_namespace: String,
    /// Tracks which namespaces have been modified since last commit.
    pub(crate) dirty_namespaces: HashSet<String>,
    /// Store format version (V1 = flat/legacy, V2 = namespaced).
    pub(crate) format_version: StoreFormatVersion,
}

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

/// Type alias for Git-backed namespaced store.
pub type GitNamespacedKvStore<const N: usize> = NamespacedKvStore<N, GitNodeStorage<N>>;

/// Type alias for in-memory namespaced store.
pub type InMemoryNamespacedKvStore<const N: usize> = NamespacedKvStore<N, InMemoryNodeStorage<N>>;

/// Type alias for file-backed namespaced store.
pub type FileNamespacedKvStore<const N: usize> = NamespacedKvStore<N, FileNodeStorage<N>>;

/// Type alias for RocksDB-backed namespaced store.
#[cfg(feature = "rocksdb_storage")]
pub type RocksDBNamespacedKvStore<const N: usize> = NamespacedKvStore<N, RocksDBNodeStorage<N>>;

// ---------------------------------------------------------------------------
// Thread-safe wrapper
// ---------------------------------------------------------------------------

/// Thread-safe wrapper for [`NamespacedKvStore`].
///
/// Uses `Arc<parking_lot::Mutex<..>>` for safe multi-threaded access.
/// `NamespaceHandle` borrows cannot escape the lock scope, so this wrapper
/// provides direct `ns_*` methods instead.
pub struct ThreadSafeNamespacedKvStore<
    const N: usize,
    S: NodeStorage<N>,
    M: MetadataBackend = GitMetadataBackend,
> {
    pub(crate) inner: Arc<Mutex<NamespacedKvStore<N, S, M>>>,
}

/// Type alias for thread-safe Git-backed namespaced store.
pub type ThreadSafeGitNamespacedKvStore<const N: usize> =
    ThreadSafeNamespacedKvStore<N, GitNodeStorage<N>>;

/// Type alias for thread-safe in-memory namespaced store.
pub type ThreadSafeInMemoryNamespacedKvStore<const N: usize> =
    ThreadSafeNamespacedKvStore<N, InMemoryNodeStorage<N>>;

// ---------------------------------------------------------------------------
// NamespaceHandle — borrowed view into a single namespace
// ---------------------------------------------------------------------------

/// A handle to a specific namespace within a [`NamespacedKvStore`].
///
/// This is a short-lived mutable reference that provides namespace-scoped
/// operations. It is created by [`NamespacedKvStore::namespace`].
pub struct NamespaceHandle<'a, const N: usize, S: NodeStorage<N>, M: MetadataBackend> {
    store: &'a mut NamespacedKvStore<N, S, M>,
    ns_name: String,
}

impl<'a, const N: usize, S: NodeStorage<N>, M: MetadataBackend> NamespaceHandle<'a, N, S, M> {
    /// Get a value by key within this namespace.
    ///
    /// Checks the namespace's staging area first, then the committed tree.
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        // Check namespace staging area first
        if let Some(staging) = self.store.namespace_staging.get(&self.ns_name) {
            if let Some(staged_value) = staging.get(key) {
                return staged_value.clone();
            }
        }

        // Check committed tree
        if let Some(tree) = self.store.namespaces.get(&self.ns_name) {
            return tree.find(key).and_then(|node| {
                node.keys
                    .iter()
                    .position(|k| k == key)
                    .map(|idx| node.values[idx].clone())
            });
        }

        None
    }

    /// Insert a key-value pair within this namespace (stages the change).
    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), GitKvError> {
        crate::validation::validate_kv(&key, &value)?;
        self.store
            .namespace_staging
            .entry(self.ns_name.clone())
            .or_default()
            .insert(key, Some(value));
        self.store.dirty_namespaces.insert(self.ns_name.clone());
        Ok(())
    }

    /// Delete a key within this namespace (stages the deletion).
    pub fn delete(&mut self, key: &[u8]) -> Result<bool, GitKvError> {
        let exists = self.get(key).is_some();
        if exists {
            self.store
                .namespace_staging
                .entry(self.ns_name.clone())
                .or_default()
                .insert(key.to_vec(), None);
            self.store.dirty_namespaces.insert(self.ns_name.clone());
        }
        Ok(exists)
    }

    /// List all keys within this namespace (includes staged changes).
    pub fn list_keys(&self) -> Vec<Vec<u8>> {
        let mut keys = HashSet::new();

        // Add keys from the committed tree
        if let Some(tree) = self.store.namespaces.get(&self.ns_name) {
            for key in tree.collect_keys() {
                keys.insert(key);
            }
        }

        // Apply staging area
        if let Some(staging) = self.store.namespace_staging.get(&self.ns_name) {
            for (key, value) in staging {
                if value.is_some() {
                    keys.insert(key.clone());
                } else {
                    keys.remove(key);
                }
            }
        }

        let mut result: Vec<Vec<u8>> = keys.into_iter().collect();
        result.sort();
        result
    }

    /// Get the root hash of this namespace's ProllyTree.
    pub fn root_hash(&self) -> Option<ValueDigest<N>> {
        self.store
            .namespaces
            .get(&self.ns_name)
            .and_then(|tree| tree.get_root_hash())
    }
}

// ---------------------------------------------------------------------------
// Core operations (generic over all storage backends)
// ---------------------------------------------------------------------------

impl<const N: usize, S: NodeStorage<N>, M: MetadataBackend> NamespacedKvStore<N, S, M>
where
    VersionedKvStore<N, S, M>: TreeConfigSaver<N>,
{
    // -- Namespace handle ---------------------------------------------------

    /// Get a mutable handle to a namespace. Creates the namespace lazily if
    /// it does not yet exist.
    pub fn namespace(&mut self, prefix: &str) -> NamespaceHandle<'_, N, S, M> {
        // Ensure the namespace ProllyTree exists in-memory
        if !self.namespaces.contains_key(prefix) {
            // Check if we have a registry entry to load from
            if let Some(entry) = self.registry.get(prefix) {
                // Try loading from storage
                let tree = if entry.root_hash.is_some() {
                    ProllyTree::load_from_storage(
                        self.inner.tree.storage.clone(),
                        entry.config.clone(),
                    )
                    .unwrap_or_else(|| {
                        ProllyTree::new(self.inner.tree.storage.clone(), entry.config.clone())
                    })
                } else {
                    ProllyTree::new(self.inner.tree.storage.clone(), entry.config.clone())
                };
                self.namespaces.insert(prefix.to_string(), tree);
            } else {
                // New namespace — create empty tree
                let tree = ProllyTree::new(self.inner.tree.storage.clone(), TreeConfig::default());
                self.namespaces.insert(prefix.to_string(), tree);
                self.dirty_namespaces.insert(prefix.to_string());
            }
        }

        // Ensure staging area exists
        self.namespace_staging
            .entry(prefix.to_string())
            .or_default();

        NamespaceHandle {
            store: self,
            ns_name: prefix.to_string(),
        }
    }

    // -- Registry operations ------------------------------------------------

    /// List all namespace names (sorted).
    pub fn list_namespaces(&self) -> Vec<String> {
        let mut names: HashSet<String> = HashSet::new();
        names.extend(self.registry.keys().cloned());
        names.extend(self.namespaces.keys().cloned());
        let mut result: Vec<String> = names.into_iter().collect();
        result.sort();
        result
    }

    /// Delete an entire namespace. Returns `true` if the namespace existed.
    ///
    /// The "default" namespace cannot be deleted.
    pub fn delete_namespace(&mut self, prefix: &str) -> Result<bool, GitKvError> {
        if prefix == self.default_namespace {
            return Err(GitKvError::GitObjectError(
                "Cannot delete the default namespace".to_string(),
            ));
        }
        let existed =
            self.registry.remove(prefix).is_some() || self.namespaces.remove(prefix).is_some();
        self.namespace_staging.remove(prefix);
        self.dirty_namespaces.remove(prefix);
        Ok(existed)
    }

    /// Get the root hash for a namespace (O(1) lookup).
    pub fn get_namespace_root_hash(&self, prefix: &str) -> Option<ValueDigest<N>> {
        // Check in-memory tree first (may have uncommitted changes)
        if let Some(tree) = self.namespaces.get(prefix) {
            return tree.get_root_hash();
        }
        // Fall back to registry entry
        self.registry
            .get(prefix)
            .and_then(|entry| entry.root_hash.clone())
    }

    // -- Backward-compatible flat API (operates on default namespace) -------

    /// Insert a key-value pair into the default namespace.
    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), GitKvError> {
        let ns = self.default_namespace.clone();
        self.namespace(&ns).insert(key, value)
    }

    /// Get a value by key from the default namespace.
    pub fn get(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        let ns = self.default_namespace.clone();
        self.namespace(&ns).get(key)
    }

    /// Delete a key from the default namespace.
    pub fn delete(&mut self, key: &[u8]) -> Result<bool, GitKvError> {
        let ns = self.default_namespace.clone();
        self.namespace(&ns).delete(key)
    }

    /// List all keys in the default namespace.
    pub fn list_keys(&mut self) -> Vec<Vec<u8>> {
        let ns = self.default_namespace.clone();
        self.namespace(&ns).list_keys()
    }

    // -- Git operations -----------------------------------------------------

    /// Get current branch name.
    pub fn current_branch(&self) -> &str {
        self.inner.current_branch()
    }

    /// List all branches.
    pub fn list_branches(&self) -> Result<Vec<String>, GitKvError> {
        self.inner.list_branches()
    }

    /// Get commit history.
    pub fn log(&self) -> Result<Vec<CommitInfo>, GitKvError> {
        self.inner.log()
    }

    /// Create a new branch from the current branch and switch to it.
    pub fn create_branch(&mut self, name: &str) -> Result<(), GitKvError> {
        self.inner.create_branch(name)?;
        // Clear per-namespace staging since we switched branches
        self.namespace_staging.clear();
        self.dirty_namespaces.clear();
        Ok(())
    }

    // -- Persistence helpers ------------------------------------------------

    /// Save namespace registry to the dataset directory.
    fn save_namespace_registry(&self) -> Result<(), GitKvError> {
        let dataset_dir = self
            .inner
            .dataset_dir
            .as_ref()
            .ok_or_else(|| GitKvError::GitObjectError("Dataset directory not set".into()))?;

        // Write version marker
        let version_path = dataset_dir.join("prolly_namespace_version");
        std::fs::write(&version_path, "V2").map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to write namespace version: {e}"))
        })?;

        // Build registry from current namespace state
        let mut registry_data: HashMap<String, NamespaceEntry<N>> = self.registry.clone();
        for (ns_name, tree) in &self.namespaces {
            registry_data.insert(
                ns_name.clone(),
                NamespaceEntry {
                    root_hash: tree.get_root_hash(),
                    config: tree.config.clone(),
                },
            );
        }

        // Write registry
        let registry_json = serde_json::to_string_pretty(&registry_data).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to serialize namespace registry: {e}"))
        })?;
        let registry_path = dataset_dir.join("prolly_namespace_registry");
        std::fs::write(&registry_path, registry_json).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to write namespace registry: {e}"))
        })?;

        Ok(())
    }

    /// Load namespace registry from the dataset directory.
    fn load_namespace_registry(&mut self) -> Result<(), GitKvError> {
        let dataset_dir = self
            .inner
            .dataset_dir
            .as_ref()
            .ok_or_else(|| GitKvError::GitObjectError("Dataset directory not set".into()))?
            .clone();

        let registry_path = dataset_dir.join("prolly_namespace_registry");
        if !registry_path.exists() {
            return Ok(());
        }

        let registry_json = std::fs::read_to_string(&registry_path).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to read namespace registry: {e}"))
        })?;

        let registry: HashMap<String, NamespaceEntry<N>> = serde_json::from_str(&registry_json)
            .map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to parse namespace registry: {e}"))
            })?;

        self.registry = registry;
        Ok(())
    }

    /// Detect store format version from the dataset directory.
    fn detect_format_version(dataset_dir: &Path) -> StoreFormatVersion {
        let version_path = dataset_dir.join("prolly_namespace_version");
        if version_path.exists() {
            if let Ok(content) = std::fs::read_to_string(version_path) {
                if content.trim() == "V2" {
                    return StoreFormatVersion::V2;
                }
            }
        }
        StoreFormatVersion::V1
    }

    // -- Commit -------------------------------------------------------------

    /// Core commit logic shared by all backends.
    ///
    /// 1. Drains each dirty namespace's staging into its ProllyTree.
    /// 2. Updates the registry with current root hashes.
    /// 3. Writes namespace config files to the dataset directory.
    /// 4. Delegates to `inner.commit()` for the actual git commit.
    ///
    /// Backend-specific `commit()` methods may wrap this with additional
    /// steps (e.g., merging hash mappings for Git storage).
    pub(crate) fn commit_impl(&mut self, message: &str) -> Result<gix::ObjectId, GitKvError> {
        // 1. For each dirty namespace, drain staging into tree
        let dirty: Vec<String> = self.dirty_namespaces.drain().collect();
        for ns_name in &dirty {
            if let Some(staging) = self.namespace_staging.get_mut(ns_name) {
                // Ensure the tree exists
                if !self.namespaces.contains_key(ns_name) {
                    let config = self
                        .registry
                        .get(ns_name)
                        .map(|e| e.config.clone())
                        .unwrap_or_default();
                    let tree = ProllyTree::new(self.inner.tree.storage.clone(), config);
                    self.namespaces.insert(ns_name.clone(), tree);
                }

                if let Some(tree) = self.namespaces.get_mut(ns_name) {
                    for (key, value) in staging.drain() {
                        match value {
                            Some(v) => {
                                tree.insert(key, v);
                            }
                            None => {
                                tree.delete(&key);
                            }
                        }
                    }
                    tree.persist_root();
                }
            }
        }

        // 2. Update registry entries with current root hashes
        for (ns_name, tree) in &self.namespaces {
            self.registry.insert(
                ns_name.clone(),
                NamespaceEntry {
                    root_hash: tree.get_root_hash(),
                    config: tree.config.clone(),
                },
            );
        }

        // 3. Write namespace registry + version to dataset dir
        self.save_namespace_registry()?;

        // 4. Delegate to inner for the git commit
        // (inner.commit drains its own empty staging, persists its placeholder tree,
        //  writes prolly_config_tree_config, stages ALL dataset_dir files via git add,
        //  creates the git commit, updates refs)
        //
        self.inner.commit(message)
    }
}

// ---------------------------------------------------------------------------
// Git-specific backend (init / open / checkout / merge)
// ---------------------------------------------------------------------------

impl<const N: usize> NamespacedKvStore<N, GitNodeStorage<N>, GitMetadataBackend> {
    /// Merge hash mappings from namespace trees into the inner store's
    /// Git storage, so all mappings are written to `prolly_hash_mappings`
    /// during commit.
    fn merge_ns_hash_mappings_to_inner_git(&self) {
        for tree in self.namespaces.values() {
            let ns_mappings = tree.storage.get_hash_mappings();
            self.inner.tree.storage.merge_hash_mappings(ns_mappings);
        }
    }

    /// Commit staged changes across all namespaces.
    ///
    /// Before delegating to the generic commit logic, merges hash mappings
    /// from namespace subtrees into the inner store's `GitNodeStorage` so
    /// that `save_tree_config_to_git()` writes all mappings to
    /// `prolly_hash_mappings`.
    pub fn commit(&mut self, message: &str) -> Result<gix::ObjectId, GitKvError> {
        // First run the generic commit logic (drains staging, persists trees, etc.)
        // We need to call commit_impl first so trees are persisted before we merge mappings
        // Actually, commit_impl also calls inner.commit(). We need to merge BEFORE inner.commit.
        // So we split: do the namespace work ourselves, merge mappings, then call inner.commit.

        // 1. Drain staging into namespace trees
        let dirty: Vec<String> = self.dirty_namespaces.drain().collect();
        for ns_name in &dirty {
            if let Some(staging) = self.namespace_staging.get_mut(ns_name) {
                if !self.namespaces.contains_key(ns_name) {
                    let config = self
                        .registry
                        .get(ns_name)
                        .map(|e| e.config.clone())
                        .unwrap_or_default();
                    let tree = ProllyTree::new(self.inner.tree.storage.clone(), config);
                    self.namespaces.insert(ns_name.clone(), tree);
                }
                if let Some(tree) = self.namespaces.get_mut(ns_name) {
                    for (key, value) in staging.drain() {
                        match value {
                            Some(v) => tree.insert(key, v),
                            None => {
                                tree.delete(&key);
                            }
                        }
                    }
                    tree.persist_root();
                }
            }
        }

        // 2. Update registry
        for (ns_name, tree) in &self.namespaces {
            self.registry.insert(
                ns_name.clone(),
                NamespaceEntry {
                    root_hash: tree.get_root_hash(),
                    config: tree.config.clone(),
                },
            );
        }

        // 3. Write namespace files
        self.save_namespace_registry()?;

        // 4. Merge namespace hash mappings into inner storage
        self.merge_ns_hash_mappings_to_inner_git();

        // 5. Delegate to inner for git commit
        self.inner.commit(message)
    }

    /// Initialize a new namespaced store with Git storage.
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        // Delegate to VersionedKvStore for git setup
        let inner = VersionedKvStore::<N, GitNodeStorage<N>>::init(&path)?;

        let mut store = NamespacedKvStore {
            inner,
            registry: HashMap::new(),
            namespaces: HashMap::new(),
            namespace_staging: HashMap::new(),
            default_namespace: DEFAULT_NAMESPACE.to_string(),
            dirty_namespaces: HashSet::new(),
            format_version: StoreFormatVersion::V2,
        };

        // Create the default namespace with an empty tree
        let default_tree = ProllyTree::new(store.inner.tree.storage.clone(), TreeConfig::default());
        store
            .namespaces
            .insert(DEFAULT_NAMESPACE.to_string(), default_tree);

        // Commit initial state (writes V2 marker + registry)
        store.commit("Initial namespaced store")?;

        Ok(store)
    }

    /// Open an existing store. Automatically detects V1/V2 format.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = VersionedKvStore::<N, GitNodeStorage<N>>::open(&path)?;

        let dataset_dir = inner
            .dataset_dir
            .as_ref()
            .ok_or_else(|| GitKvError::GitObjectError("Dataset directory not set".into()))?;
        let format_version = Self::detect_format_version(dataset_dir);

        let mut store = NamespacedKvStore {
            inner,
            registry: HashMap::new(),
            namespaces: HashMap::new(),
            namespace_staging: HashMap::new(),
            default_namespace: DEFAULT_NAMESPACE.to_string(),
            dirty_namespaces: HashSet::new(),
            format_version: format_version.clone(),
        };

        match format_version {
            StoreFormatVersion::V2 => {
                // Load namespace registry; trees are loaded lazily on access
                store.load_namespace_registry()?;
            }
            StoreFormatVersion::V1 => {
                // V1: wrap inner tree data as the "default" namespace.
                // Collect current tree data into a new namespace tree.
                let mut kv_pairs = Vec::new();
                for key in store.inner.tree.collect_keys() {
                    if let Some(value) = store.inner.get(&key) {
                        kv_pairs.push((key, value));
                    }
                }

                if !kv_pairs.is_empty() {
                    let mut default_tree =
                        ProllyTree::new(store.inner.tree.storage.clone(), TreeConfig::default());
                    for (key, value) in kv_pairs {
                        default_tree.insert(key, value);
                    }
                    default_tree.persist_root();
                    store
                        .namespaces
                        .insert(DEFAULT_NAMESPACE.to_string(), default_tree);
                } else {
                    let default_tree =
                        ProllyTree::new(store.inner.tree.storage.clone(), TreeConfig::default());
                    store
                        .namespaces
                        .insert(DEFAULT_NAMESPACE.to_string(), default_tree);
                }
            }
        }

        Ok(store)
    }

    /// Checkout a branch or commit. Reloads namespace state from the target.
    pub fn checkout(&mut self, branch_or_commit: &str) -> Result<(), GitKvError> {
        // Clear all namespace state
        self.namespace_staging.clear();
        self.dirty_namespaces.clear();
        self.namespaces.clear();
        self.registry.clear();

        // Delegate to inner for git checkout (updates HEAD, reloads inner tree)
        self.inner.checkout(branch_or_commit)?;

        // Load the namespace registry from the HEAD commit (not from disk,
        // because the file on disk may reflect a different branch's last commit).
        let head_commit = self.inner.metadata.head_commit_id()?;
        let registry_at_head = self.load_registry_at_commit(&head_commit)?;

        if !registry_at_head.is_empty() {
            self.registry = registry_at_head;
            self.format_version = StoreFormatVersion::V2;

            // Also update the on-disk files to match the checked-out commit
            self.save_namespace_registry()?;
        } else {
            // No registry at this commit — either V1 or pre-namespace commit
            self.format_version = StoreFormatVersion::V1;
        }

        Ok(())
    }

    /// Merge another branch into the current branch with namespace-aware
    /// three-way merge.
    ///
    /// For each namespace:
    /// - If root hash unchanged between base and source: skip (no change).
    /// - If only source changed: take source namespace state.
    /// - If only dest changed: keep dest (no-op).
    /// - If both changed: perform key-level three-way merge within namespace.
    pub fn merge<R: ConflictResolver>(
        &mut self,
        source_branch: &str,
        resolver: &R,
    ) -> Result<gix::ObjectId, GitKvError> {
        let dest_branch = self.inner.current_branch.clone();

        // Find merge base
        let base_commit = self.find_merge_base(&dest_branch, source_branch)?;
        let source_commit = self.get_branch_commit(source_branch)?;

        // Load registries at base and source commits
        let base_registry = self.load_registry_at_commit(&base_commit)?;
        let source_registry = self.load_registry_at_commit(&source_commit)?;

        // Current (dest) registry
        let dest_registry = self.current_registry_snapshot();

        // Collect all namespace names
        let mut all_ns: HashSet<String> = HashSet::new();
        all_ns.extend(base_registry.keys().cloned());
        all_ns.extend(source_registry.keys().cloned());
        all_ns.extend(dest_registry.keys().cloned());

        let mut unresolved_conflicts: Vec<MergeConflict> = Vec::new();

        for ns_name in &all_ns {
            let base_hash = base_registry.get(ns_name).and_then(|e| e.root_hash.clone());
            let source_hash = source_registry
                .get(ns_name)
                .and_then(|e| e.root_hash.clone());
            let dest_hash = dest_registry.get(ns_name).and_then(|e| e.root_hash.clone());

            // If source hash == dest hash, no merge needed for this namespace
            if source_hash == dest_hash {
                continue;
            }

            // If base hash == dest hash (only source changed), take source
            if base_hash == dest_hash && source_hash != dest_hash {
                // Load source namespace state and replace dest
                let source_kv =
                    self.collect_ns_keys_at_commit(ns_name, &source_commit, &source_registry)?;
                let mut tree = ProllyTree::new(
                    self.inner.tree.storage.clone(),
                    source_registry
                        .get(ns_name)
                        .map(|e| e.config.clone())
                        .unwrap_or_default(),
                );
                for (key, value) in source_kv {
                    tree.insert(key, value);
                }
                tree.persist_root();
                self.namespaces.insert(ns_name.clone(), tree);
                continue;
            }

            // If base hash == source hash (only dest changed), keep dest (no-op)
            if base_hash == source_hash {
                continue;
            }

            // Both changed — key-level three-way merge within this namespace
            let base_kv = self.collect_ns_keys_at_commit(ns_name, &base_commit, &base_registry)?;
            let source_kv =
                self.collect_ns_keys_at_commit(ns_name, &source_commit, &source_registry)?;

            // Get dest KV from in-memory tree
            let mut dest_kv = HashMap::new();
            if let Some(tree) = self.namespaces.get(ns_name) {
                for key in tree.collect_keys() {
                    if let Some(node) = tree.find(&key) {
                        if let Some(idx) = node.keys.iter().position(|k| k == &key) {
                            dest_kv.insert(key, node.values[idx].clone());
                        }
                    }
                }
            }

            // Three-way merge at key level
            let mut all_keys: HashSet<Vec<u8>> = HashSet::new();
            all_keys.extend(base_kv.keys().cloned());
            all_keys.extend(source_kv.keys().cloned());
            all_keys.extend(dest_kv.keys().cloned());

            let mut merge_results = Vec::new();

            for key in &all_keys {
                let base_val = base_kv.get(key);
                let source_val = source_kv.get(key);
                let dest_val = dest_kv.get(key);

                match (base_val, source_val, dest_val) {
                    (Some(b), Some(s), Some(d)) => {
                        if b == s && b == d {
                            continue;
                        } else if b == d && b != s {
                            merge_results
                                .push(crate::diff::MergeResult::Modified(key.clone(), s.clone()));
                        } else if b == s || s == d {
                            continue;
                        } else {
                            let conflict = MergeConflict {
                                key: key.clone(),
                                base_value: Some(b.clone()),
                                source_value: Some(s.clone()),
                                destination_value: Some(d.clone()),
                            };
                            merge_results.push(crate::diff::MergeResult::Conflict(conflict));
                        }
                    }
                    (None, Some(s), None) => {
                        merge_results.push(crate::diff::MergeResult::Added(key.clone(), s.clone()));
                    }
                    (None, Some(s), Some(d)) => {
                        if s == d {
                            continue;
                        } else {
                            let conflict = MergeConflict {
                                key: key.clone(),
                                base_value: None,
                                source_value: Some(s.clone()),
                                destination_value: Some(d.clone()),
                            };
                            merge_results.push(crate::diff::MergeResult::Conflict(conflict));
                        }
                    }
                    (Some(_), None, Some(_)) => {
                        merge_results.push(crate::diff::MergeResult::Removed(key.clone()));
                    }
                    _ => continue,
                }
            }

            // Resolve conflicts
            let mut resolved = Vec::new();
            for result in merge_results {
                match result {
                    crate::diff::MergeResult::Conflict(conflict) => {
                        if let Some(resolved_result) = resolver.resolve_conflict(&conflict) {
                            resolved.push(resolved_result);
                        } else {
                            unresolved_conflicts.push(conflict);
                        }
                    }
                    other => resolved.push(other),
                }
            }

            // Apply to namespace tree
            // Ensure tree is loaded
            if !self.namespaces.contains_key(ns_name) {
                let tree = ProllyTree::new(self.inner.tree.storage.clone(), TreeConfig::default());
                self.namespaces.insert(ns_name.clone(), tree);
            }
            if let Some(tree) = self.namespaces.get_mut(ns_name) {
                for result in resolved {
                    match result {
                        crate::diff::MergeResult::Added(key, value)
                        | crate::diff::MergeResult::Modified(key, value) => {
                            tree.insert(key, value);
                        }
                        crate::diff::MergeResult::Removed(key) => {
                            tree.delete(&key);
                        }
                        crate::diff::MergeResult::Conflict(_) => unreachable!(),
                    }
                }
                tree.persist_root();
            }
        }

        if !unresolved_conflicts.is_empty() {
            return Err(GitKvError::MergeConflictError(unresolved_conflicts));
        }

        // Clear staging
        self.namespace_staging.clear();
        self.dirty_namespaces.clear();

        // Save namespace registry and create merge commit
        self.save_namespace_registry()?;

        // Update inner registry entries
        for (ns_name, tree) in &self.namespaces {
            self.registry.insert(
                ns_name.clone(),
                NamespaceEntry {
                    root_hash: tree.get_root_hash(),
                    config: tree.config.clone(),
                },
            );
        }

        self.inner.create_merge_commit_for_namespaced(source_branch)
    }

    /// Migrate a V1 (flat) store to V2 (namespaced) format.
    ///
    /// All existing key-value pairs are moved into the "default" namespace.
    pub fn migrate_v1_to_v2(&mut self) -> Result<MigrationReport, GitKvError> {
        if self.format_version == StoreFormatVersion::V2 {
            return Err(GitKvError::GitObjectError(
                "Store is already V2 format".to_string(),
            ));
        }

        // Collect all KV from the inner (flat) tree
        let mut kv_pairs = Vec::new();
        for key in self.inner.tree.collect_keys() {
            if let Some(value) = self.inner.get(&key) {
                kv_pairs.push((key, value));
            }
        }
        let keys_migrated = kv_pairs.len();

        // Create "default" namespace tree and insert all KV
        let mut default_tree =
            ProllyTree::new(self.inner.tree.storage.clone(), TreeConfig::default());
        for (key, value) in kv_pairs {
            default_tree.insert(key, value);
        }
        default_tree.persist_root();

        self.namespaces
            .insert(DEFAULT_NAMESPACE.to_string(), default_tree);
        self.format_version = StoreFormatVersion::V2;

        // Commit the migration
        self.commit("Migrate store from V1 (flat) to V2 (namespaced)")?;

        Ok(MigrationReport {
            keys_migrated,
            namespaces_created: vec![DEFAULT_NAMESPACE.to_string()],
            storage_version: StoreFormatVersion::V2,
        })
    }

    /// Convenience method to merge with default IgnoreConflictsResolver.
    pub fn merge_ignore_conflicts(
        &mut self,
        source_branch: &str,
    ) -> Result<gix::ObjectId, GitKvError> {
        self.merge(source_branch, &IgnoreConflictsResolver)
    }

    /// Check if a namespace changed between two commits.
    pub fn namespace_changed(
        &self,
        prefix: &str,
        commit_a: &str,
        commit_b: &str,
    ) -> Result<bool, GitKvError> {
        let commit_id_a = self.resolve_commit(commit_a)?;
        let commit_id_b = self.resolve_commit(commit_b)?;

        let registry_a = self.load_registry_at_commit(&commit_id_a)?;
        let registry_b = self.load_registry_at_commit(&commit_id_b)?;

        let hash_a = registry_a.get(prefix).and_then(|e| e.root_hash.clone());
        let hash_b = registry_b.get(prefix).and_then(|e| e.root_hash.clone());

        Ok(hash_a != hash_b)
    }

    // -- Internal helpers ---------------------------------------------------

    /// Resolve a reference (branch name, commit hex) to a commit ID.
    fn resolve_commit(&self, reference: &str) -> Result<gix::ObjectId, GitKvError> {
        // Try as branch
        let branch_ref = format!("refs/heads/{reference}");
        if let Ok(r) = self.inner.metadata.repo().refs.find(&branch_ref) {
            if let Some(id) = r.target.try_id() {
                return Ok(id.to_owned());
            }
        }
        // Try as hex commit
        gix::ObjectId::from_hex(reference.as_bytes())
            .map_err(|e| GitKvError::GitObjectError(format!("Invalid reference: {e}")))
    }

    /// Get commit ID for a branch.
    fn get_branch_commit(&self, branch: &str) -> Result<gix::ObjectId, GitKvError> {
        let branch_ref = format!("refs/heads/{branch}");
        match self.inner.metadata.repo().refs.find(&branch_ref) {
            Ok(reference) => match reference.target.try_id() {
                Some(id) => Ok(id.to_owned()),
                None => Err(GitKvError::GitObjectError(format!(
                    "Branch {branch} does not point to a commit"
                ))),
            },
            Err(_) => Err(GitKvError::BranchNotFound(branch.to_string())),
        }
    }

    /// Find the merge base (common ancestor) of two branches.
    fn find_merge_base(&self, branch1: &str, branch2: &str) -> Result<gix::ObjectId, GitKvError> {
        let commit1 = self.get_branch_commit(branch1)?;
        let commit2 = self.get_branch_commit(branch2)?;

        // Walk branch1 ancestors
        let mut visited1 = HashSet::new();
        let mut queue1 = std::collections::VecDeque::new();
        queue1.push_back(commit1);
        while let Some(cid) = queue1.pop_front() {
            if !visited1.insert(cid) {
                continue;
            }
            if let Ok(parents) = self.inner.metadata.commit_parents(&cid) {
                for p in parents {
                    if !visited1.contains(&p) {
                        queue1.push_back(p);
                    }
                }
            }
        }

        // Walk branch2 and find first common ancestor
        let mut visited2 = HashSet::new();
        let mut queue2 = std::collections::VecDeque::new();
        queue2.push_back(commit2);
        while let Some(cid) = queue2.pop_front() {
            if !visited2.insert(cid) {
                continue;
            }
            if visited1.contains(&cid) {
                return Ok(cid);
            }
            if let Ok(parents) = self.inner.metadata.commit_parents(&cid) {
                for p in parents {
                    if !visited2.contains(&p) {
                        queue2.push_back(p);
                    }
                }
            }
        }

        Err(GitKvError::GitObjectError(
            "No common ancestor found".to_string(),
        ))
    }

    /// Load namespace registry from a specific git commit.
    fn load_registry_at_commit(
        &self,
        commit_id: &gix::ObjectId,
    ) -> Result<HashMap<String, NamespaceEntry<N>>, GitKvError> {
        let dataset_dir = self.inner.tree.storage.dataset_dir();
        let git_root = self
            .inner
            .metadata
            .work_dir()
            .or_else(|| VersionedKvStore::<N, GitNodeStorage<N>>::find_git_root(dataset_dir))
            .ok_or_else(|| GitKvError::GitObjectError("Could not find git root".to_string()))?;

        let dataset_relative = dataset_dir
            .strip_prefix(&git_root)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get relative path: {e}")))?;

        let rel_str = dataset_relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        let registry_path = format!("{rel_str}/prolly_namespace_registry");

        match self
            .inner
            .metadata
            .read_file_at_commit(commit_id, &registry_path)
        {
            Ok(data) => {
                let registry: HashMap<String, NamespaceEntry<N>> = serde_json::from_slice(&data)
                    .map_err(|e| {
                        GitKvError::GitObjectError(format!(
                            "Failed to parse registry at commit: {e}"
                        ))
                    })?;
                Ok(registry)
            }
            Err(_) => {
                // No registry at this commit — probably V1 or initial commit
                Ok(HashMap::new())
            }
        }
    }

    /// Build a snapshot of the current registry (from in-memory trees + saved registry).
    fn current_registry_snapshot(&self) -> HashMap<String, NamespaceEntry<N>> {
        let mut snapshot = self.registry.clone();
        for (ns_name, tree) in &self.namespaces {
            snapshot.insert(
                ns_name.clone(),
                NamespaceEntry {
                    root_hash: tree.get_root_hash(),
                    config: tree.config.clone(),
                },
            );
        }
        snapshot
    }

    /// Collect KV pairs for a namespace at a specific commit.
    fn collect_ns_keys_at_commit(
        &self,
        ns_name: &str,
        commit_id: &gix::ObjectId,
        registry: &HashMap<String, NamespaceEntry<N>>,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, GitKvError> {
        let entry = match registry.get(ns_name) {
            Some(e) => e,
            None => return Ok(HashMap::new()),
        };

        let root_hash = match &entry.root_hash {
            Some(h) => h,
            None => return Ok(HashMap::new()),
        };

        // Load hash mappings at this commit
        let dataset_dir = self.inner.tree.storage.dataset_dir();
        let git_root = self
            .inner
            .metadata
            .work_dir()
            .or_else(|| VersionedKvStore::<N, GitNodeStorage<N>>::find_git_root(dataset_dir))
            .ok_or_else(|| GitKvError::GitObjectError("Could not find git root".to_string()))?;

        let dataset_relative = dataset_dir
            .strip_prefix(&git_root)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get relative path: {e}")))?;

        let rel_str = dataset_relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        // Load hash mappings — try namespace-specific first, then fall back to global
        let ns_mapping_path = format!("{rel_str}/prolly_ns_hash_mappings");
        let global_mapping_path = format!("{rel_str}/prolly_hash_mappings");

        let mut hash_mappings: HashMap<ValueDigest<N>, gix::ObjectId> = HashMap::new();

        // Load from both mapping files
        for path in [&ns_mapping_path, &global_mapping_path] {
            if let Ok(data) = self.inner.metadata.read_file_at_commit(commit_id, path) {
                let mapping_str = String::from_utf8(data).unwrap_or_default();
                for line in mapping_str.lines() {
                    if let Some((hash_hex, object_hex)) = line.split_once(':') {
                        if hash_hex.len() == N * 2 {
                            let mut hash_bytes = Vec::new();
                            for i in 0..N {
                                if let Ok(byte) =
                                    u8::from_str_radix(&hash_hex[i * 2..i * 2 + 2], 16)
                                {
                                    hash_bytes.push(byte);
                                } else {
                                    break;
                                }
                            }
                            if hash_bytes.len() == N {
                                if let Ok(object_id) =
                                    gix::ObjectId::from_hex(object_hex.as_bytes())
                                {
                                    let hash = ValueDigest::raw_hash(&hash_bytes);
                                    hash_mappings.insert(hash, object_id);
                                }
                            }
                        }
                    }
                }
            }
        }

        if hash_mappings.is_empty() {
            return Ok(HashMap::new());
        }

        // Create temp storage and try to load tree
        let temp_storage = GitNodeStorage::with_mappings(
            self.inner.metadata.clone_repo(),
            self.inner.tree.storage.dataset_dir().to_path_buf(),
            hash_mappings,
        )?;

        let mut config = entry.config.clone();
        config.root_hash = Some(root_hash.clone());

        let tree = match ProllyTree::load_from_storage(temp_storage, config) {
            Some(t) => t,
            None => return Ok(HashMap::new()),
        };

        let mut kv = HashMap::new();
        for key in tree.collect_keys() {
            if let Some(node) = tree.find(&key) {
                if let Some(idx) = node.keys.iter().position(|k| k == &key) {
                    kv.insert(key, node.values[idx].clone());
                }
            }
        }
        Ok(kv)
    }
}

// -- Helper on inner store for creating merge commits -----------------------

impl<const N: usize> VersionedKvStore<N, GitNodeStorage<N>, GitMetadataBackend> {
    /// Create a merge commit for the namespaced store (public within crate).
    pub(crate) fn create_merge_commit_for_namespaced(
        &mut self,
        source_branch: &str,
    ) -> Result<gix::ObjectId, GitKvError> {
        let dest_branch = self.current_branch.clone();
        let message = format!("Merge branch '{source_branch}' into '{dest_branch}'");
        self.create_merge_commit(&message, source_branch)
    }
}

// ---------------------------------------------------------------------------
// InMemory backend
// ---------------------------------------------------------------------------

impl<const N: usize> NamespacedKvStore<N, InMemoryNodeStorage<N>, GitMetadataBackend> {
    /// Commit staged changes (in-memory backend).
    pub fn commit(&mut self, message: &str) -> Result<gix::ObjectId, GitKvError> {
        self.commit_impl(message)
    }

    /// Initialize a new namespaced store with in-memory storage.
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = VersionedKvStore::<N, InMemoryNodeStorage<N>>::init(&path)?;

        let mut store = NamespacedKvStore {
            inner,
            registry: HashMap::new(),
            namespaces: HashMap::new(),
            namespace_staging: HashMap::new(),
            default_namespace: DEFAULT_NAMESPACE.to_string(),
            dirty_namespaces: HashSet::new(),
            format_version: StoreFormatVersion::V2,
        };

        let default_tree = ProllyTree::new(store.inner.tree.storage.clone(), TreeConfig::default());
        store
            .namespaces
            .insert(DEFAULT_NAMESPACE.to_string(), default_tree);

        store.commit("Initial namespaced store")?;
        Ok(store)
    }

    /// Open an existing namespaced store with in-memory storage.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = VersionedKvStore::<N, InMemoryNodeStorage<N>>::open(&path)?;

        let dataset_dir = inner
            .dataset_dir
            .as_ref()
            .ok_or_else(|| GitKvError::GitObjectError("Dataset directory not set".into()))?;
        let format_version = Self::detect_format_version(dataset_dir);

        let mut store = NamespacedKvStore {
            inner,
            registry: HashMap::new(),
            namespaces: HashMap::new(),
            namespace_staging: HashMap::new(),
            default_namespace: DEFAULT_NAMESPACE.to_string(),
            dirty_namespaces: HashSet::new(),
            format_version: format_version.clone(),
        };

        match format_version {
            StoreFormatVersion::V2 => {
                store.load_namespace_registry()?;
            }
            StoreFormatVersion::V1 => {
                let default_tree =
                    ProllyTree::new(store.inner.tree.storage.clone(), TreeConfig::default());
                store
                    .namespaces
                    .insert(DEFAULT_NAMESPACE.to_string(), default_tree);
            }
        }

        Ok(store)
    }
}

// ---------------------------------------------------------------------------
// File backend
// ---------------------------------------------------------------------------

impl<const N: usize> NamespacedKvStore<N, FileNodeStorage<N>, GitMetadataBackend> {
    /// Commit staged changes (file backend).
    pub fn commit(&mut self, message: &str) -> Result<gix::ObjectId, GitKvError> {
        self.commit_impl(message)
    }

    /// Initialize a new namespaced store with file-based storage.
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = VersionedKvStore::<N, FileNodeStorage<N>>::init(&path)?;

        let mut store = NamespacedKvStore {
            inner,
            registry: HashMap::new(),
            namespaces: HashMap::new(),
            namespace_staging: HashMap::new(),
            default_namespace: DEFAULT_NAMESPACE.to_string(),
            dirty_namespaces: HashSet::new(),
            format_version: StoreFormatVersion::V2,
        };

        let default_tree = ProllyTree::new(store.inner.tree.storage.clone(), TreeConfig::default());
        store
            .namespaces
            .insert(DEFAULT_NAMESPACE.to_string(), default_tree);

        store.commit("Initial namespaced store")?;
        Ok(store)
    }

    /// Open an existing namespaced store with file-based storage.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let inner = VersionedKvStore::<N, FileNodeStorage<N>>::open(&path)?;

        let dataset_dir = inner
            .dataset_dir
            .as_ref()
            .ok_or_else(|| GitKvError::GitObjectError("Dataset directory not set".into()))?;
        let format_version = Self::detect_format_version(dataset_dir);

        let mut store = NamespacedKvStore {
            inner,
            registry: HashMap::new(),
            namespaces: HashMap::new(),
            namespace_staging: HashMap::new(),
            default_namespace: DEFAULT_NAMESPACE.to_string(),
            dirty_namespaces: HashSet::new(),
            format_version: format_version.clone(),
        };

        match format_version {
            StoreFormatVersion::V2 => {
                store.load_namespace_registry()?;
            }
            StoreFormatVersion::V1 => {
                let default_tree =
                    ProllyTree::new(store.inner.tree.storage.clone(), TreeConfig::default());
                store
                    .namespaces
                    .insert(DEFAULT_NAMESPACE.to_string(), default_tree);
            }
        }

        Ok(store)
    }
}

// ---------------------------------------------------------------------------
// ThreadSafeNamespacedKvStore implementation
// ---------------------------------------------------------------------------

impl<const N: usize, S: NodeStorage<N>, M: MetadataBackend> Clone
    for ThreadSafeNamespacedKvStore<N, S, M>
{
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

// Send + Sync are auto-derived from Arc<Mutex<..>>

impl<const N: usize, S: NodeStorage<N>, M: MetadataBackend> ThreadSafeNamespacedKvStore<N, S, M>
where
    VersionedKvStore<N, S, M>: TreeConfigSaver<N>,
{
    /// Wrap a `NamespacedKvStore` in a thread-safe handle.
    pub fn new(store: NamespacedKvStore<N, S, M>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(store)),
        }
    }

    /// Insert a key-value pair into a specific namespace.
    pub fn ns_insert(
        &self,
        namespace: &str,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<(), GitKvError> {
        self.inner.lock().namespace(namespace).insert(key, value)
    }

    /// Get a value by key from a specific namespace.
    pub fn ns_get(&self, namespace: &str, key: &[u8]) -> Option<Vec<u8>> {
        self.inner.lock().namespace(namespace).get(key)
    }

    /// Delete a key from a specific namespace.
    pub fn ns_delete(&self, namespace: &str, key: &[u8]) -> Result<bool, GitKvError> {
        self.inner.lock().namespace(namespace).delete(key)
    }

    /// List all keys in a specific namespace.
    pub fn ns_list_keys(&self, namespace: &str) -> Vec<Vec<u8>> {
        self.inner.lock().namespace(namespace).list_keys()
    }

    /// List all namespace names.
    pub fn list_namespaces(&self) -> Vec<String> {
        self.inner.lock().list_namespaces()
    }

    /// Delete a namespace.
    pub fn delete_namespace(&self, prefix: &str) -> Result<bool, GitKvError> {
        self.inner.lock().delete_namespace(prefix)
    }

    /// Get namespace root hash.
    pub fn get_namespace_root_hash(&self, prefix: &str) -> Option<ValueDigest<N>> {
        self.inner.lock().get_namespace_root_hash(prefix)
    }

    /// Insert into default namespace.
    pub fn insert(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), GitKvError> {
        self.inner.lock().insert(key, value)
    }

    /// Get from default namespace.
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.inner.lock().get(key)
    }

    /// Delete from default namespace.
    pub fn delete(&self, key: &[u8]) -> Result<bool, GitKvError> {
        self.inner.lock().delete(key)
    }

    /// List keys in default namespace.
    pub fn list_keys(&self) -> Vec<Vec<u8>> {
        self.inner.lock().list_keys()
    }

    /// Create a new branch and switch to it.
    pub fn create_branch(&self, name: &str) -> Result<(), GitKvError> {
        self.inner.lock().create_branch(name)
    }

    /// Get current branch name.
    pub fn current_branch(&self) -> String {
        self.inner.lock().current_branch().to_string()
    }

    /// Get commit history.
    pub fn log(&self) -> Result<Vec<CommitInfo>, GitKvError> {
        self.inner.lock().log()
    }
}

// Git-specific thread-safe methods
impl<const N: usize> ThreadSafeNamespacedKvStore<N, GitNodeStorage<N>, GitMetadataBackend> {
    /// Commit all staged changes (Git backend).
    pub fn commit(&self, message: &str) -> Result<gix::ObjectId, GitKvError> {
        self.inner.lock().commit(message)
    }

    /// Initialize a new thread-safe namespaced store with Git storage.
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let store = GitNamespacedKvStore::init(path)?;
        Ok(Self::new(store))
    }

    /// Open an existing thread-safe namespaced store with Git storage.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GitKvError> {
        let store = GitNamespacedKvStore::open(path)?;
        Ok(Self::new(store))
    }

    /// Checkout a branch.
    pub fn checkout(&self, branch_or_commit: &str) -> Result<(), GitKvError> {
        self.inner.lock().checkout(branch_or_commit)
    }

    /// Merge with conflict resolution.
    pub fn merge<R: ConflictResolver>(
        &self,
        source_branch: &str,
        resolver: &R,
    ) -> Result<gix::ObjectId, GitKvError> {
        self.inner.lock().merge(source_branch, resolver)
    }

    /// Check if namespace changed between two commits.
    pub fn namespace_changed(
        &self,
        prefix: &str,
        commit_a: &str,
        commit_b: &str,
    ) -> Result<bool, GitKvError> {
        self.inner
            .lock()
            .namespace_changed(prefix, commit_a, commit_b)
    }
}
