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

#[cfg(feature = "proximity")]
use crate::proximity::text_index::{
    dedup_chunk_hits_by_doc, doc_id_prefix, make_chunk_id, text_inner_proximity_name,
    text_state_key, validate_or_write_text_identity, OVERFETCH_MULTIPLIER,
};
#[cfg(feature = "proximity")]
use crate::proximity::{
    Chunker, Embedder, IdentityChunker, ProximityConfig, ProximityError, ProximityIndex,
    ProximityIndexEntry, TextHit, TextIndexConfig, TextIndexError,
};

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

/// Type alias for the per-target value transformer closure used by cascade.
/// Takes the raw primary-tree value bytes and returns `Some(text)` to embed,
/// or `None` to opt this id out of cascade for this index.
#[cfg(feature = "proximity")]
pub type ValueTransformer = Arc<dyn Fn(&[u8]) -> Option<String> + Send + Sync>;

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
    /// Lazily loaded per-namespace proximity (vector) indexes, keyed by
    /// `(namespace_name, index_name)`. See [`crate::proximity`].
    #[cfg(feature = "proximity")]
    pub(crate) proximity_indexes: HashMap<(String, String), ProximityIndex<N, S>>,
    /// Tracks which proximity indexes are dirty since last commit.
    #[cfg(feature = "proximity")]
    pub(crate) dirty_proximity_indexes: HashSet<(String, String)>,
    /// Centrally-owned embedders for text indexes, keyed by
    /// `(namespace, idx_name)`. Stored here (rather than in the
    /// `TextNamespaceHandle`) so the primary-tree `NamespaceHandle::insert`
    /// path can reach them for auto-cascade — PR 4d.
    #[cfg(feature = "proximity")]
    pub(crate) text_embedders: HashMap<(String, String), Arc<dyn Embedder>>,
    /// Centrally-owned chunkers for text indexes, parallel to
    /// `text_embedders`. Insert paths use the chunker to split a document
    /// into one or more text fragments; each fragment becomes a separate
    /// chunk-id in the underlying proximity index.
    #[cfg(feature = "proximity")]
    pub(crate) text_chunkers: HashMap<(String, String), Arc<dyn Chunker>>,
    /// Per-namespace cascade lists. When a namespace name is keyed here, every
    /// `NamespaceHandle::insert` / `::delete` against that namespace also
    /// applies the operation to each listed text sub-index. Targets that
    /// aren't currently loaded (no `text_index<E>()` call yet this process)
    /// are silently skipped — drift can be detected via `audit_text_index`.
    #[cfg(feature = "proximity")]
    pub(crate) cascade_lists: HashMap<String, Vec<String>>,
    /// Per-target value transformers consulted during cascade. When
    /// `(ns, idx_name)` is keyed here, the closure runs on the raw value
    /// bytes to produce the text that will be embedded. Returning `None`
    /// opts the id out of cascade for this index — useful for "this row
    /// isn't text-indexable" decisions on structured payloads. Absence of
    /// a transformer falls back to UTF-8 interpretation.
    #[cfg(feature = "proximity")]
    pub(crate) text_transformers: HashMap<(String, String), ValueTransformer>,
    /// Threshold (in bytes) above which inserted values are externalised as
    /// separate blobs via [`crate::storage::NodeStorage::insert_blob`].
    /// `None` (default) disables externalisation. Runtime config only —
    /// not persisted in the namespace registry; users set it per process.
    pub(crate) externalize_threshold: Option<usize>,
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
    /// Tree-side values are unwrapped through
    /// [`crate::storage::externalize::unwrap_value`], so externalised values
    /// transparently look like normal inline values to callers.
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        // Check namespace staging area first. Staged values are always
        // inline user bytes — externalisation only applies when staging
        // is drained into the tree on commit, so no unwrap needed here.
        if let Some(staging) = self.store.namespace_staging.get(&self.ns_name) {
            if let Some(staged_value) = staging.get(key) {
                return staged_value.clone();
            }
        }

        // Check committed tree. Raw bytes from the leaf may be an externalised
        // envelope; unwrap_value handles both cases (inline pass-through plus
        // graceful fallback when an envelope-shaped value's hash doesn't
        // resolve to a real blob).
        if let Some(tree) = self.store.namespaces.get(&self.ns_name) {
            return tree.find(key).and_then(|node| {
                node.keys.iter().position(|k| k == key).map(|idx| {
                    crate::storage::externalize::unwrap_value::<N, _>(
                        &node.values[idx],
                        &self.store.inner.tree.storage,
                    )
                })
            });
        }

        None
    }

    /// Insert a key-value pair within this namespace (stages the change).
    ///
    /// When the namespace has been configured for auto-cascade via
    /// `NamespacedKvStore::set_cascade`, this also embeds `value` (as UTF-8)
    /// and upserts the resulting vector into every cascading text sub-index
    /// that is currently loaded. Targets not loaded yet, and values that
    /// aren't valid UTF-8, are silently skipped.
    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), GitKvError> {
        crate::validation::validate_kv(&key, &value)?;
        // Auto-cascade into text indexes, if configured. Done BEFORE staging
        // the primary insert so an embed error surfaces before any state has
        // been touched.
        #[cfg(feature = "proximity")]
        self.cascade_insert(&key, &value);

        self.store
            .namespace_staging
            .entry(self.ns_name.clone())
            .or_default()
            .insert(key, Some(value));
        self.store.dirty_namespaces.insert(self.ns_name.clone());
        Ok(())
    }

    /// Delete a key within this namespace (stages the deletion).
    ///
    /// When the namespace has been configured for auto-cascade via
    /// `NamespacedKvStore::set_cascade`, this also removes the id from every
    /// cascading text sub-index that is currently loaded.
    pub fn delete(&mut self, key: &[u8]) -> Result<bool, GitKvError> {
        let exists = self.get(key).is_some();
        if exists {
            self.store
                .namespace_staging
                .entry(self.ns_name.clone())
                .or_default()
                .insert(key.to_vec(), None);
            self.store.dirty_namespaces.insert(self.ns_name.clone());

            #[cfg(feature = "proximity")]
            self.cascade_delete(key);
        }
        Ok(exists)
    }

    /// Cascade an insert to every configured text sub-index for this
    /// namespace. Per target the text to embed comes from:
    ///
    /// 1. A registered `ValueTransformer` (PR 4d follow-up) if one exists —
    ///    its `Option<String>` return value determines whether this id
    ///    participates (`None` opts out for this index).
    /// 2. Otherwise UTF-8 interpretation of the bytes (silent skip on
    ///    non-UTF-8).
    ///
    /// Silent no-op cases otherwise: no cascade list configured, target
    /// whose embedder/proximity-index isn't currently loaded, or the
    /// embedder itself returns an error.
    #[cfg(feature = "proximity")]
    fn cascade_insert(&mut self, key: &[u8], value: &[u8]) {
        let Some(cascade_list) = self.store.cascade_lists.get(&self.ns_name).cloned() else {
            return;
        };
        for idx_name in cascade_list {
            let target_key = (self.ns_name.clone(), idx_name.clone());
            let prox_key = (self.ns_name.clone(), text_inner_proximity_name(&idx_name));

            // 1. Determine the text to embed.
            let text: String = match self.store.text_transformers.get(&target_key).cloned() {
                Some(transformer) => match transformer(value) {
                    Some(t) => t,
                    None => continue, // transformer explicitly opted out
                },
                None => match std::str::from_utf8(value) {
                    Ok(s) => s.to_string(),
                    Err(_) => continue, // non-UTF-8 + no transformer
                },
            };

            // 2. Resolve the embedder + chunker (immutable borrow ends with the clones).
            let Some(embedder) = self.store.text_embedders.get(&target_key).cloned() else {
                continue;
            };
            let chunker: Arc<dyn Chunker> = self
                .store
                .text_chunkers
                .get(&target_key)
                .cloned()
                .unwrap_or_else(|| Arc::new(IdentityChunker));

            // 3. Chunk → embed → insert. Drop any previous chunks for this
            //    doc-id first so re-inserts with a different chunk count
            //    don't leak stale entries.
            //
            //    Borrow gymnastics: each chunk's embed happens outside the
            //    proximity-index borrow, then a fresh `get_mut` reborrows
            //    for the insert. Slight overhead, but keeps the embedder
            //    free to call into anything (incl. error-returning paths).
            Self::cascade_delete_chunks_for_doc(self.store, &prox_key, key);
            let chunks = chunker.split(&text);
            for (chunk_idx, chunk_text) in chunks.iter().enumerate() {
                let Ok(vec) = embedder.embed(chunk_text) else {
                    continue;
                };
                let chunk_id = make_chunk_id(key, chunk_idx as u32);
                if let Some(idx) = self.store.proximity_indexes.get_mut(&prox_key) {
                    let _ = idx.insert(chunk_id, vec);
                    self.store.dirty_proximity_indexes.insert(prox_key.clone());
                }
            }
        }
    }

    /// Cascade a delete to every configured text sub-index for this
    /// namespace. Removes every chunk for `key` from each target.
    #[cfg(feature = "proximity")]
    fn cascade_delete(&mut self, key: &[u8]) {
        let Some(cascade_list) = self.store.cascade_lists.get(&self.ns_name).cloned() else {
            return;
        };
        for idx_name in cascade_list {
            let prox_key = (self.ns_name.clone(), text_inner_proximity_name(&idx_name));
            Self::cascade_delete_chunks_for_doc(self.store, &prox_key, key);
        }
    }

    /// Static helper: prefix-scan and remove every chunk for `doc_id`
    /// from the proximity index at `prox_key`. Marks the index dirty if
    /// anything was removed.
    #[cfg(feature = "proximity")]
    fn cascade_delete_chunks_for_doc(
        store: &mut NamespacedKvStore<N, S, M>,
        prox_key: &(String, String),
        doc_id: &[u8],
    ) {
        let Some(idx) = store.proximity_indexes.get_mut(prox_key) else {
            return;
        };
        let prefix = doc_id_prefix(doc_id);
        let to_remove: Vec<Vec<u8>> = idx
            .entries_snapshot()
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        let mut any = false;
        for cid in to_remove {
            if idx.remove(&cid) {
                any = true;
            }
        }
        if any {
            store.dirty_proximity_indexes.insert(prox_key.clone());
        }
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
                // Ensure root_hash is set on the config so load_from_storage
                // can find the root node (entry.config.root_hash may be None
                // even when entry.root_hash is Some).
                let mut config = entry.config.clone();
                config.root_hash = entry.root_hash.clone();

                // Try loading from storage
                let tree = if entry.root_hash.is_some() {
                    ProllyTree::load_from_storage(self.inner.tree.storage.clone(), config.clone())
                        .unwrap_or_else(|| ProllyTree::new(self.inner.tree.storage.clone(), config))
                } else {
                    ProllyTree::new(self.inner.tree.storage.clone(), config)
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

    // -- Externalisation config (PR 0b) -------------------------------------

    /// Set the threshold (in bytes) above which inserted values are stored
    /// as separate content-addressed blobs via
    /// [`crate::storage::NodeStorage::insert_blob`] instead of inline in the
    /// leaf node.
    ///
    /// Passing `None` (the default) disables externalisation entirely —
    /// values are stored inline regardless of size.
    ///
    /// Runtime configuration only. Not persisted in the namespace registry,
    /// so callers must set this each time they open a store. The setting
    /// affects future inserts; previously-stored externalised values are
    /// always unwrapped transparently on read.
    pub fn set_externalize_threshold(&mut self, threshold: Option<usize>) {
        self.externalize_threshold = threshold;
    }

    /// Current externalisation threshold, if any.
    pub fn externalize_threshold(&self) -> Option<usize> {
        self.externalize_threshold
    }

    /// Borrow the inner content-addressed storage. Useful for integration
    /// tests that want to verify externalised values landed as blobs, and
    /// for advanced callers building on top of the blob API.
    pub fn inner_storage(&self) -> &S {
        &self.inner.tree.storage
    }

    /// Walk every loaded namespace, collect all blob hashes referenced by
    /// envelope-shaped leaf values, and delete every blob in storage that
    /// isn't on that list.
    ///
    /// Loads every namespace listed in the registry first, so blobs
    /// referenced by namespaces the user hasn't explicitly opened are still
    /// preserved.
    ///
    /// **Scope caveat:** GC walks only the current HEAD state. Blobs
    /// referenced by older commits but not by HEAD are deleted, so checking
    /// out an old commit after GC may fail to retrieve those values.
    /// History-aware GC (walking all reachable commits) is a future PR.
    pub fn gc_blobs(&mut self) -> Result<BlobGcReport, GitKvError> {
        // 1. Ensure every namespace from the registry is loaded in memory.
        let ns_names = self.list_namespaces();
        for name in &ns_names {
            let _ = self.namespace(name);
        }

        // 2. Walk every loaded namespace tree, collect referenced blob hashes.
        let mut referenced: HashSet<ValueDigest<N>> = HashSet::new();
        for tree in self.namespaces.values() {
            for key in tree.collect_keys() {
                let raw = tree.find(&key).and_then(|node| {
                    node.keys
                        .iter()
                        .position(|k| k == &key)
                        .map(|i| node.values[i].clone())
                });
                if let Some(bytes) = raw {
                    if let Some((hash, _size)) =
                        crate::storage::externalize::parse_envelope::<N>(&bytes)
                    {
                        referenced.insert(hash);
                    }
                }
            }
        }

        // 3. List every blob currently in storage.
        let all_blobs = self
            .inner
            .tree
            .storage
            .list_blobs()
            .map_err(|e| GitKvError::GitObjectError(format!("list_blobs: {e}")))?;

        // 4. Delete orphans.
        let mut report = BlobGcReport {
            total: all_blobs.len(),
            referenced: referenced.len(),
            removed: 0,
            errors: Vec::new(),
        };
        for h in all_blobs {
            if referenced.contains(&h) {
                continue;
            }
            match self.inner.tree.storage.delete_blob(&h) {
                Ok(()) => report.removed += 1,
                Err(e) => report.errors.push(format!("{h}: {e}")),
            }
        }
        Ok(report)
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
                                // Apply externalisation threshold: values longer
                                // than `externalize_threshold` are stored as
                                // separate blobs via `insert_blob`, with the
                                // in-tree value replaced by a 44-byte envelope
                                // (magic + content hash + original size). Smaller
                                // values stay inline, byte-for-byte unchanged.
                                let stored = match self.externalize_threshold {
                                    Some(threshold) if v.len() > threshold => {
                                        let hash = ValueDigest::<N>::new(&v);
                                        // Write the blob through the inner
                                        // storage (the shared backing) so a
                                        // fresh handle to the same path sees it.
                                        self.inner
                                            .tree
                                            .storage
                                            .insert_blob(hash.clone(), &v)
                                            .map_err(|e| {
                                                GitKvError::GitObjectError(format!(
                                                    "externalise insert_blob: {e}"
                                                ))
                                            })?;
                                        crate::storage::externalize::make_envelope::<N>(
                                            &hash,
                                            v.len() as u64,
                                        )
                                    }
                                    _ => v,
                                };
                                tree.insert(key, stored);
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

        // 3. Flush any dirty proximity sub-indexes through ProximityIndex::persist,
        //    which writes their canonical entries + root hash via NodeStorage::save_config.
        //    Each persist also calls NodeStorage::sync() so backends with deferred
        //    bookkeeping flush their mapping snapshots in time for the inner commit.
        #[cfg(feature = "proximity")]
        self.flush_dirty_proximity_indexes()?;

        // 4. Write namespace registry + version to dataset dir
        self.save_namespace_registry()?;

        // 5. Delegate to inner for the git commit
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

    /// Mirror of [`Self::merge_ns_hash_mappings_to_inner_git`] for proximity
    /// sub-indexes. Each proximity index holds a clone of the inner storage
    /// with its own `hash_to_object_id` map; this consolidates those maps
    /// into the inner store before `save_tree_config_to_git` snapshots the
    /// canonical `prolly_hash_mappings` file.
    #[cfg(feature = "proximity")]
    fn merge_proximity_hash_mappings_to_inner_git(&self) {
        for idx in self.proximity_indexes.values() {
            let mappings = idx.storage().get_hash_mappings();
            self.inner.tree.storage.merge_hash_mappings(mappings);
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
                            Some(v) => {
                                let stored = match self.externalize_threshold {
                                    Some(threshold) if v.len() > threshold => {
                                        let hash = ValueDigest::<N>::new(&v);
                                        self.inner
                                            .tree
                                            .storage
                                            .insert_blob(hash.clone(), &v)
                                            .map_err(|e| {
                                                GitKvError::GitObjectError(format!(
                                                    "externalise insert_blob: {e}"
                                                ))
                                            })?;
                                        crate::storage::externalize::make_envelope::<N>(
                                            &hash,
                                            v.len() as u64,
                                        )
                                    }
                                    _ => v,
                                };
                                tree.insert(key, stored);
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

        // 3. Flush any dirty proximity sub-indexes (writes their canonical
        //    entries + tree nodes via ProximityIndex::persist).
        #[cfg(feature = "proximity")]
        self.flush_dirty_proximity_indexes()?;

        // 4. Write namespace files
        self.save_namespace_registry()?;

        // 5. Merge namespace hash mappings into inner storage
        self.merge_ns_hash_mappings_to_inner_git();

        // 6. Merge proximity-index hash mappings into inner storage so the
        //    canonical `prolly_hash_mappings` written by inner.commit covers
        //    every proximity node we just wrote.
        #[cfg(feature = "proximity")]
        self.merge_proximity_hash_mappings_to_inner_git();

        // 7. Delegate to inner for git commit
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
            #[cfg(feature = "proximity")]
            proximity_indexes: HashMap::new(),
            #[cfg(feature = "proximity")]
            dirty_proximity_indexes: HashSet::new(),
            #[cfg(feature = "proximity")]
            text_embedders: HashMap::new(),
            #[cfg(feature = "proximity")]
            cascade_lists: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_transformers: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_chunkers: HashMap::new(),
            externalize_threshold: None,
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
            #[cfg(feature = "proximity")]
            proximity_indexes: HashMap::new(),
            #[cfg(feature = "proximity")]
            dirty_proximity_indexes: HashSet::new(),
            #[cfg(feature = "proximity")]
            text_embedders: HashMap::new(),
            #[cfg(feature = "proximity")]
            cascade_lists: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_transformers: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_chunkers: HashMap::new(),
            externalize_threshold: None,
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
            // No registry at this commit — either V1 or pre-namespace commit.
            // Mirror `open()`'s V1 handling by exposing the checked-out inner
            // flat tree through the default namespace view.
            self.format_version = StoreFormatVersion::V1;

            let mut kv_pairs = Vec::new();
            for key in self.inner.tree.collect_keys() {
                if let Some(value) = self.inner.get(&key) {
                    kv_pairs.push((key, value));
                }
            }
            if !kv_pairs.is_empty() {
                let mut default_tree =
                    ProllyTree::new(self.inner.tree.storage.clone(), TreeConfig::default());
                for (key, value) in kv_pairs {
                    default_tree.insert(key, value);
                }
                default_tree.persist_root();
                self.namespaces
                    .insert(DEFAULT_NAMESPACE.to_string(), default_tree);
            } else {
                let default_tree =
                    ProllyTree::new(self.inner.tree.storage.clone(), TreeConfig::default());
                self.namespaces
                    .insert(DEFAULT_NAMESPACE.to_string(), default_tree);
            }
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

            // Get dest KV — ensure namespace is loaded first (namespaces are lazily
            // loaded, so a namespace that exists in dest_registry but hasn't been
            // accessed would appear empty if we only checked self.namespaces).
            if !self.namespaces.contains_key(ns_name) {
                if let Some(entry) = dest_registry.get(ns_name) {
                    let mut config = entry.config.clone();
                    config.root_hash = entry.root_hash.clone();
                    let tree = if entry.root_hash.is_some() {
                        ProllyTree::load_from_storage(
                            self.inner.tree.storage.clone(),
                            config.clone(),
                        )
                        .unwrap_or_else(|| ProllyTree::new(self.inner.tree.storage.clone(), config))
                    } else {
                        ProllyTree::new(self.inner.tree.storage.clone(), config)
                    };
                    self.namespaces.insert(ns_name.clone(), tree);
                }
            }

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
                    (Some(b), None, Some(d)) => {
                        // Source deleted, dest may have modified
                        if b == d {
                            // Dest unchanged from base — safe to apply source deletion
                            merge_results.push(crate::diff::MergeResult::Removed(key.clone()));
                        } else {
                            // Dest modified while source deleted — conflict
                            let conflict = MergeConflict {
                                key: key.clone(),
                                base_value: Some(b.clone()),
                                source_value: None,
                                destination_value: Some(d.clone()),
                            };
                            merge_results.push(crate::diff::MergeResult::Conflict(conflict));
                        }
                    }
                    (Some(b), Some(s), None) => {
                        // Dest deleted, source may have modified
                        if b == s {
                            // Source unchanged from base — keep dest deletion (no-op)
                            continue;
                        } else {
                            // Source modified while dest deleted — conflict
                            let conflict = MergeConflict {
                                key: key.clone(),
                                base_value: Some(b.clone()),
                                source_value: Some(s.clone()),
                                destination_value: None,
                            };
                            merge_results.push(crate::diff::MergeResult::Conflict(conflict));
                        }
                    }
                    (Some(_), None, None) => {
                        // Both deleted — no-op
                        continue;
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

        // Consolidate namespace hash mappings into inner storage before
        // creating the merge commit, so prolly_hash_mappings includes all
        // blob mappings needed to reload merged namespace roots.
        self.merge_ns_hash_mappings_to_inner_git();

        self.inner.create_merge_commit_for_namespaced(source_branch)
    }

    /// Three-way merge that includes proximity (vector) sub-indexes.
    ///
    /// Compared to [`Self::merge`] (which only merges the primary KV tree per
    /// namespace), this method also runs
    /// [`crate::proximity::merge_proximity_index_sets`] on every proximity
    /// index **currently loaded into memory** (via earlier
    /// `NamespaceHandle::proximity_index` calls). Indexes that have never
    /// been opened in this process are not auto-discovered yet — open them
    /// first if you want them to participate in the merge.
    ///
    /// # Strategy
    ///
    /// 1. Pre-compute the proximity-merge result for every loaded index
    ///    against the base and source commits. If any index has unresolved
    ///    conflicts, the whole call fails with
    ///    [`GitKvError::ProximityMergeConflictError`] before mutating the
    ///    repository.
    /// 2. Run the existing [`Self::merge`] (primary-tree three-way merge,
    ///    creates a merge commit).
    /// 3. Apply the pre-computed proximity merges into the in-memory indexes
    ///    and create a **follow-up commit** that records the merged proximity
    ///    state.
    ///
    /// The two-commit shape (primary merge commit + proximity follow-up) is a
    /// known limitation; a future refactor can fold both into one commit by
    /// extracting helpers from `merge`.
    ///
    /// Returns the object id of the proximity follow-up commit when any
    /// proximity changes were applied, otherwise the primary merge commit.
    #[cfg(feature = "proximity")]
    pub fn merge_with_proximity_resolver<KR, PR>(
        &mut self,
        source_branch: &str,
        kv_resolver: &KR,
        proximity_resolver: &PR,
    ) -> Result<gix::ObjectId, GitKvError>
    where
        KR: ConflictResolver,
        PR: crate::proximity::ProximityConflictResolver,
    {
        let dest_branch = self.inner.current_branch.clone();
        let base_commit = self.find_merge_base(&dest_branch, source_branch)?;
        let source_commit = self.get_branch_commit(source_branch)?;

        // -- 1. Pre-compute proximity merges; fail fast on unresolved conflicts.
        let prox_keys: Vec<(String, String)> = self.proximity_indexes.keys().cloned().collect();
        let mut prox_merged: Vec<((String, String), ProximityEntrySet)> = Vec::new();
        let mut all_conflicts: Vec<crate::proximity::ProximityConflict> = Vec::new();

        for (ns_name, idx_name) in &prox_keys {
            let base = self
                .load_proximity_entries_at_commit(&base_commit, ns_name, idx_name)?
                .unwrap_or_default();
            let source = self
                .load_proximity_entries_at_commit(&source_commit, ns_name, idx_name)?
                .unwrap_or_default();
            let dest = self
                .proximity_indexes
                .get(&(ns_name.clone(), idx_name.clone()))
                .map(|i| i.entries_snapshot())
                .unwrap_or_default();

            match crate::proximity::merge_proximity_index_sets(
                &base,
                &source,
                &dest,
                proximity_resolver,
            ) {
                Ok(merged) => {
                    prox_merged.push(((ns_name.clone(), idx_name.clone()), merged));
                }
                Err(failure) => {
                    all_conflicts.extend(failure.conflicts);
                }
            }
        }

        if !all_conflicts.is_empty() {
            return Err(GitKvError::ProximityMergeConflictError(all_conflicts));
        }

        // -- 2. Run the existing primary-tree merge (creates the merge commit).
        let primary_commit = self.merge(source_branch, kv_resolver)?;

        // -- 3. Apply the pre-computed proximity merges + follow-up commit.
        if prox_merged.is_empty() {
            return Ok(primary_commit);
        }

        let mut any_changes = false;
        for ((ns_name, idx_name), merged_entries) in prox_merged {
            // Skip indexes whose three-way merge was a no-op (dest already
            // matched the merged result). This keeps the follow-up commit
            // empty in the no-op case.
            let dest_unchanged = self
                .proximity_indexes
                .get(&(ns_name.clone(), idx_name.clone()))
                .map(|i| i.entries_snapshot() == merged_entries)
                .unwrap_or(false);
            if dest_unchanged {
                continue;
            }
            if let Some(idx) = self
                .proximity_indexes
                .get_mut(&(ns_name.clone(), idx_name.clone()))
            {
                idx.replace_entries(merged_entries).map_err(|e| {
                    GitKvError::GitObjectError(format!(
                        "Failed to install merged proximity entries for {ns_name}:{idx_name}: {e}"
                    ))
                })?;
                self.dirty_proximity_indexes
                    .insert((ns_name.clone(), idx_name.clone()));
                any_changes = true;
            }
        }

        if !any_changes {
            return Ok(primary_commit);
        }

        self.commit("Proximity index merge follow-up")
    }

    /// Load the persisted `(id → vector)` entry set for a proximity sub-index
    /// at a specific commit, by reading the canonical
    /// `prolly_config_proximity:<ns>:<idx>:state` blob via git.
    ///
    /// Returns `Ok(None)` if the file doesn't exist at that commit (i.e. the
    /// index didn't exist there yet).
    #[cfg(feature = "proximity")]
    fn load_proximity_entries_at_commit(
        &self,
        commit_id: &gix::ObjectId,
        ns_name: &str,
        idx_name: &str,
    ) -> Result<Option<ProximityEntrySet>, GitKvError> {
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

        let key = format!("proximity:{ns_name}:{idx_name}:state");
        let path = if rel_str.is_empty() {
            format!("prolly_config_{key}")
        } else {
            format!("{rel_str}/prolly_config_{key}")
        };

        match self.inner.metadata.read_file_at_commit(commit_id, &path) {
            Ok(bytes) => {
                let state =
                    crate::proximity::deserialize_persisted_state::<N>(&bytes).map_err(|e| {
                        GitKvError::GitObjectError(format!(
                            "Failed to deserialize proximity state at commit: {e}"
                        ))
                    })?;
                Ok(Some(state.entries))
            }
            Err(_) => Ok(None),
        }
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
            #[cfg(feature = "proximity")]
            proximity_indexes: HashMap::new(),
            #[cfg(feature = "proximity")]
            dirty_proximity_indexes: HashSet::new(),
            #[cfg(feature = "proximity")]
            text_embedders: HashMap::new(),
            #[cfg(feature = "proximity")]
            cascade_lists: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_transformers: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_chunkers: HashMap::new(),
            externalize_threshold: None,
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
            #[cfg(feature = "proximity")]
            proximity_indexes: HashMap::new(),
            #[cfg(feature = "proximity")]
            dirty_proximity_indexes: HashSet::new(),
            #[cfg(feature = "proximity")]
            text_embedders: HashMap::new(),
            #[cfg(feature = "proximity")]
            cascade_lists: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_transformers: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_chunkers: HashMap::new(),
            externalize_threshold: None,
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
            #[cfg(feature = "proximity")]
            proximity_indexes: HashMap::new(),
            #[cfg(feature = "proximity")]
            dirty_proximity_indexes: HashSet::new(),
            #[cfg(feature = "proximity")]
            text_embedders: HashMap::new(),
            #[cfg(feature = "proximity")]
            cascade_lists: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_transformers: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_chunkers: HashMap::new(),
            externalize_threshold: None,
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
            #[cfg(feature = "proximity")]
            proximity_indexes: HashMap::new(),
            #[cfg(feature = "proximity")]
            dirty_proximity_indexes: HashSet::new(),
            #[cfg(feature = "proximity")]
            text_embedders: HashMap::new(),
            #[cfg(feature = "proximity")]
            cascade_lists: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_transformers: HashMap::new(),
            #[cfg(feature = "proximity")]
            text_chunkers: HashMap::new(),
            externalize_threshold: None,
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

// ---------------------------------------------------------------------------
// Proximity (vector) sub-index integration — PR 3a
// ---------------------------------------------------------------------------

/// Canonical `(id → vector)` entry set for a proximity sub-index.
/// Used during load + namespaced merge.
#[cfg(feature = "proximity")]
type ProximityEntrySet = std::collections::BTreeMap<Vec<u8>, Vec<f32>>;

//
// A namespace can own zero or more named proximity sub-indexes. Each lives in
// its own [`ProximityIndex<N, S>`] cached in
// [`NamespacedKvStore::proximity_indexes`] (keyed by `(namespace, index_name)`).
// Mutations mark the pair as dirty; [`NamespacedKvStore::commit_impl`] flushes
// every dirty index by calling [`ProximityIndex::persist`] under a namespaced
// key — that writes the tree's root node and saves the canonical entries
// `BTreeMap` (the source of truth for rebuilds) via
// [`crate::storage::NodeStorage::save_config`].
//
// On reopen, [`NamespaceHandle::proximity_index`] calls
// [`ProximityIndex::load`] with the same namespaced key to restore both the
// entries map and the root hash, and then resumes mutation.
//
// PR 3b (next) will add per-namespace 3-way merge for proximity sub-indexes.

#[cfg(feature = "proximity")]
fn proximity_save_name(ns_name: &str, idx_name: &str) -> String {
    format!("{ns_name}:{idx_name}")
}

/// Short-lived mutable handle on a proximity sub-index within a namespace.
///
/// Returned by [`NamespaceHandle::proximity_index`]. Holds a re-borrow of the
/// parent [`NamespacedKvStore`] and the keys identifying which index this
/// handle operates on.
#[cfg(feature = "proximity")]
pub struct ProximityNamespaceHandle<'a, const N: usize, S: NodeStorage<N>, M: MetadataBackend> {
    store: &'a mut NamespacedKvStore<N, S, M>,
    ns_name: String,
    idx_name: String,
}

#[cfg(feature = "proximity")]
impl<'a, const N: usize, S: NodeStorage<N>, M: MetadataBackend> std::fmt::Debug
    for ProximityNamespaceHandle<'a, N, S, M>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProximityNamespaceHandle")
            .field("ns_name", &self.ns_name)
            .field("idx_name", &self.idx_name)
            .finish()
    }
}

#[cfg(feature = "proximity")]
impl<'a, const N: usize, S: NodeStorage<N>, M: MetadataBackend>
    ProximityNamespaceHandle<'a, N, S, M>
{
    fn key(&self) -> (String, String) {
        (self.ns_name.clone(), self.idx_name.clone())
    }

    /// Insert or update `(id, vector)`.
    pub fn insert(&mut self, id: Vec<u8>, vector: Vec<f32>) -> Result<(), ProximityError> {
        let key = self.key();
        let idx = self
            .store
            .proximity_indexes
            .get_mut(&key)
            .expect("proximity index must be loaded by NamespaceHandle::proximity_index");
        idx.insert(id, vector)?;
        self.store.dirty_proximity_indexes.insert(key);
        Ok(())
    }

    /// Remove an id. Returns `true` if an entry was removed.
    pub fn remove(&mut self, id: &[u8]) -> bool {
        let key = self.key();
        let idx = match self.store.proximity_indexes.get_mut(&key) {
            Some(i) => i,
            None => return false,
        };
        let removed = idx.remove(id);
        if removed {
            self.store.dirty_proximity_indexes.insert(key);
        }
        removed
    }

    /// k-nearest-neighbour query. Triggers a rebuild if any mutation happened
    /// since the last query.
    pub fn knn(
        &mut self,
        query: &[f32],
        k: usize,
        ef: usize,
    ) -> Result<Vec<(Vec<u8>, f32)>, ProximityError> {
        let key = self.key();
        let idx = self
            .store
            .proximity_indexes
            .get_mut(&key)
            .expect("proximity index must be loaded");
        idx.knn(query, k, ef)
    }

    /// Number of distinct ids in this index.
    pub fn len(&self) -> usize {
        self.store
            .proximity_indexes
            .get(&self.key())
            .map_or(0, |i| i.len())
    }

    /// True when the index holds no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Root hash of the materialised proximity tree. Triggers a rebuild if dirty.
    pub fn root_hash(&mut self) -> Result<Option<ValueDigest<N>>, ProximityError> {
        let key = self.key();
        let idx = self
            .store
            .proximity_indexes
            .get_mut(&key)
            .expect("proximity index must be loaded");
        idx.root_hash().map(|opt| opt.cloned())
    }

    /// Read-only configuration view.
    pub fn config(&self) -> ProximityConfig {
        self.store
            .proximity_indexes
            .get(&self.key())
            .expect("proximity index must be loaded")
            .config()
            .clone()
    }

    /// Index name (within its namespace).
    pub fn name(&self) -> &str {
        &self.idx_name
    }

    /// Namespace name.
    pub fn namespace(&self) -> &str {
        &self.ns_name
    }
}

#[cfg(feature = "proximity")]
impl<'a, const N: usize, S: NodeStorage<N>, M: MetadataBackend> NamespaceHandle<'a, N, S, M>
where
    VersionedKvStore<N, S, M>: TreeConfigSaver<N>,
{
    /// Open or create a named proximity (vector) sub-index inside this
    /// namespace.
    ///
    /// On first call for a given `(namespace, idx_name)` pair this either
    /// loads the persisted state (if a prior commit recorded one) or creates
    /// a fresh empty index using `config`.
    ///
    /// When loading, the supplied `config.dim` must match the persisted
    /// dimension. Mismatch returns [`ProximityError::DimensionMismatch`].
    pub fn proximity_index<'b>(
        &'b mut self,
        idx_name: &str,
        config: ProximityConfig,
    ) -> Result<ProximityNamespaceHandle<'b, N, S, M>, ProximityError>
    where
        'a: 'b,
    {
        let key = (self.ns_name.clone(), idx_name.to_string());

        if !self.store.proximity_indexes.contains_key(&key) {
            let save_name = proximity_save_name(&self.ns_name, idx_name);
            let storage = self.store.inner.tree.storage.clone();

            let idx = match ProximityIndex::load(storage.clone(), &save_name) {
                Ok(loaded) => {
                    if loaded.config().dim != config.dim {
                        return Err(ProximityError::DimensionMismatch {
                            expected: loaded.config().dim,
                            got: config.dim,
                        });
                    }
                    loaded
                }
                Err(ProximityError::NotFound(_)) => ProximityIndex::new(storage, config),
                Err(e) => return Err(e),
            };
            self.store.proximity_indexes.insert(key.clone(), idx);
        }

        Ok(ProximityNamespaceHandle {
            store: &mut *self.store,
            ns_name: self.ns_name.clone(),
            idx_name: idx_name.to_string(),
        })
    }

    /// Drop a proximity sub-index. Returns whether an index was found and
    /// dropped from the in-memory cache.
    ///
    /// The persisted state (entries + tree nodes) is **not** eagerly deleted —
    /// content-addressed nodes become unreachable and are reclaimed by the
    /// backend's own GC. Future calls to `proximity_index` with the same name
    /// would create a fresh empty index.
    pub fn drop_proximity_index(&mut self, idx_name: &str) -> bool {
        let key = (self.ns_name.clone(), idx_name.to_string());
        self.store.dirty_proximity_indexes.remove(&key);
        self.store.proximity_indexes.remove(&key).is_some()
    }
}

#[cfg(feature = "proximity")]
impl<const N: usize, S: NodeStorage<N>, M: MetadataBackend> NamespacedKvStore<N, S, M>
where
    VersionedKvStore<N, S, M>: TreeConfigSaver<N>,
{
    /// Build a registry of `(namespace → {index_name → entry})` from the
    /// in-memory proximity-index state. Used by tests and PR 3b's merge logic.
    pub fn proximity_registry_snapshot(
        &mut self,
    ) -> Result<HashMap<String, HashMap<String, ProximityIndexEntry<N>>>, ProximityError> {
        let mut out: HashMap<String, HashMap<String, ProximityIndexEntry<N>>> = HashMap::new();
        let keys: Vec<(String, String)> = self.proximity_indexes.keys().cloned().collect();
        for (ns, idx_name) in keys {
            let entry = {
                let idx = self
                    .proximity_indexes
                    .get_mut(&(ns.clone(), idx_name.clone()))
                    .unwrap();
                ProximityIndexEntry {
                    root_hash: idx.root_hash()?.cloned(),
                    config: idx.config().clone(),
                }
            };
            out.entry(ns).or_default().insert(idx_name, entry);
        }
        Ok(out)
    }

    /// Flush every dirty proximity sub-index by calling
    /// [`ProximityIndex::persist`] with a namespaced save key. Called from
    /// [`Self::commit_impl`] before the inner commit.
    pub(crate) fn flush_dirty_proximity_indexes(&mut self) -> Result<(), GitKvError> {
        let dirty: Vec<(String, String)> = self.dirty_proximity_indexes.drain().collect();
        for (ns_name, idx_name) in dirty {
            let save_name = proximity_save_name(&ns_name, &idx_name);
            if let Some(idx) = self
                .proximity_indexes
                .get_mut(&(ns_name.clone(), idx_name.clone()))
            {
                idx.persist(&save_name).map_err(|e| {
                    GitKvError::GitObjectError(format!(
                        "Failed to persist proximity index {ns_name}:{idx_name}: {e}"
                    ))
                })?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// TextIndex sub-handle — PR 4a
// ---------------------------------------------------------------------------
//
// A namespaced text index reuses the existing proximity-index machinery for
// the underlying vector storage. The text-index identity (embedder id +
// version + dim + tuning) is persisted as a separate `text:<ns>:<idx>:state`
// blob via `NodeStorage::save_config`. The underlying vector tree lives in
// `proximity_indexes[(ns, "__text__<idx>")]` (the `__text__` prefix keeps it
// out of the user-facing proximity-index namespace).

/// Storage key under which the namespaced text-index identity blob lives.
#[cfg(feature = "proximity")]
fn text_index_state_key(ns_name: &str, idx_name: &str) -> String {
    text_state_key(&format!("{ns_name}:{idx_name}"))
}

/// Short-lived handle to a named text index inside a namespace.
///
/// Owns the [`Embedder`] for the duration of the handle. The underlying
/// vector index is borrowed (via re-borrow of the parent
/// [`NamespacedKvStore`]) for each operation.
#[cfg(feature = "proximity")]
pub struct TextNamespaceHandle<'a, const N: usize, S, M>
where
    S: NodeStorage<N>,
    M: MetadataBackend,
{
    store: &'a mut NamespacedKvStore<N, S, M>,
    ns_name: String,
    idx_name: String,
    /// Key into `store.proximity_indexes` — `(ns_name, "__text__<idx_name>")`.
    inner_idx_key: (String, String),
    /// Cheap reference into `store.text_embedders` (Arc bump only). PR 4d
    /// changed embedder ownership from "by value on the handle" to
    /// "centrally owned by the store", so the auto-cascade insert path
    /// can reach the embedder.
    embedder: Arc<dyn Embedder>,
    /// Cheap reference into `store.text_chunkers` — same rationale as the
    /// embedder. The chunker splits documents into one-or-more chunks on
    /// insert; the auto-cascade path uses the same chunker per target.
    chunker: Arc<dyn Chunker>,
}

#[cfg(feature = "proximity")]
impl<'a, const N: usize, S, M> std::fmt::Debug for TextNamespaceHandle<'a, N, S, M>
where
    S: NodeStorage<N>,
    M: MetadataBackend,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextNamespaceHandle")
            .field("ns_name", &self.ns_name)
            .field("idx_name", &self.idx_name)
            .field("embedder_id", &self.embedder.id())
            .field("dim", &self.embedder.dim())
            .field("chunker_id", &self.chunker.id())
            .finish()
    }
}

#[cfg(feature = "proximity")]
impl<'a, const N: usize, S, M> TextNamespaceHandle<'a, N, S, M>
where
    S: NodeStorage<N>,
    M: MetadataBackend,
{
    /// Insert or update `(id, text)`. The text is split through the
    /// configured chunker; each chunk is embedded and stored under its own
    /// chunk-id. Previous chunks for this `id` are removed first so that a
    /// re-insert with fewer chunks doesn't leak stale ones.
    pub fn insert(&mut self, id: &[u8], text: &str) -> Result<(), TextIndexError> {
        // Drop any previous chunks for this doc_id.
        self.delete_chunks_for_doc(id);

        let chunks = self.chunker.split(text);
        if chunks.is_empty() {
            return Ok(());
        }
        let idx = self
            .store
            .proximity_indexes
            .get_mut(&self.inner_idx_key)
            .expect("inner proximity index must be loaded");
        for (chunk_idx, chunk_text) in chunks.iter().enumerate() {
            let vec = self.embedder.embed(chunk_text)?;
            let chunk_id = make_chunk_id(id, chunk_idx as u32);
            idx.insert(chunk_id, vec)?;
        }
        self.store
            .dirty_proximity_indexes
            .insert(self.inner_idx_key.clone());
        Ok(())
    }

    /// Remove every chunk associated with `id`. Returns whether anything
    /// was removed.
    pub fn delete(&mut self, id: &[u8]) -> bool {
        self.delete_chunks_for_doc(id)
    }

    /// k-nearest-neighbour search by query text. Over-fetches chunks from
    /// the underlying KNN, then dedups by doc-id (keeping each doc's best
    /// chunk score). Returns top-k **documents**.
    pub fn search(&mut self, query: &str, k: usize) -> Result<Vec<TextHit>, TextIndexError> {
        if k == 0 {
            return Ok(Vec::new());
        }
        let q = self.embedder.embed(query)?;
        let raw_k = (k * OVERFETCH_MULTIPLIER).max(k);
        let ef = (raw_k * 4).max(32);
        let idx = self
            .store
            .proximity_indexes
            .get_mut(&self.inner_idx_key)
            .expect("inner proximity index must be loaded");
        let chunk_hits = idx.knn(&q, raw_k, ef)?;
        Ok(dedup_chunk_hits_by_doc(chunk_hits, k))
    }

    /// Number of stored **documents** (deduplicated across chunks). Under
    /// the default [`IdentityChunker`] this equals `chunk_count`.
    pub fn len(&self) -> usize {
        let idx = match self.store.proximity_indexes.get(&self.inner_idx_key) {
            Some(i) => i,
            None => return 0,
        };
        let mut docs: std::collections::HashSet<Vec<u8>> = std::collections::HashSet::new();
        for k in idx.entries_snapshot().keys() {
            match crate::proximity::text_index::parse_chunk_id(k) {
                Some((doc, _)) => {
                    docs.insert(doc);
                }
                None => {
                    docs.insert(k.clone());
                }
            }
        }
        docs.len()
    }

    /// Total chunk count in the underlying proximity index.
    pub fn chunk_count(&self) -> usize {
        self.store
            .proximity_indexes
            .get(&self.inner_idx_key)
            .map_or(0, |i| i.len())
    }

    /// True if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.chunk_count() == 0
    }

    /// Internal: remove every chunk for `doc_id`. Returns whether any were
    /// removed. Marks the underlying proximity index dirty on success.
    fn delete_chunks_for_doc(&mut self, doc_id: &[u8]) -> bool {
        let idx = match self.store.proximity_indexes.get_mut(&self.inner_idx_key) {
            Some(i) => i,
            None => return false,
        };
        let prefix = doc_id_prefix(doc_id);
        let to_remove: Vec<Vec<u8>> = idx
            .entries_snapshot()
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        let mut any = false;
        for cid in to_remove {
            if idx.remove(&cid) {
                any = true;
            }
        }
        if any {
            self.store
                .dirty_proximity_indexes
                .insert(self.inner_idx_key.clone());
        }
        any
    }

    /// Index name (within its namespace).
    pub fn name(&self) -> &str {
        &self.idx_name
    }

    /// Owning namespace name.
    pub fn namespace(&self) -> &str {
        &self.ns_name
    }

    /// Read-only access to the wrapped embedder (via dyn dispatch — the
    /// concrete embedder type is type-erased on namespaced handles to enable
    /// auto-cascade across multiple text indexes that may use different
    /// embedder types).
    pub fn embedder(&self) -> &dyn Embedder {
        &*self.embedder
    }
}

#[cfg(feature = "proximity")]
impl<'a, const N: usize, S: NodeStorage<N>, M: MetadataBackend> NamespaceHandle<'a, N, S, M>
where
    VersionedKvStore<N, S, M>: TreeConfigSaver<N>,
{
    /// Open or create a named **text** sub-index inside this namespace.
    ///
    /// On first call: persists the embedder identity blob via
    /// `NodeStorage::save_config` (key `text:<ns>:<idx>:state`) and creates a
    /// fresh underlying proximity index keyed
    /// `proximity_indexes[(ns, "__text__<idx>")]`.
    ///
    /// On subsequent calls (including after re-opening the store): loads the
    /// identity blob, validates the supplied embedder matches stored
    /// `id`/`version`/`dim`, and (re-)loads the underlying proximity index
    /// state from `NodeStorage::save_config`. Mismatch returns
    /// [`TextIndexError::EmbedderMismatch`] or
    /// [`TextIndexError::DimensionMismatch`] before mutating state.
    pub fn text_index<'b, E: Embedder + 'static>(
        &'b mut self,
        idx_name: &str,
        config: TextIndexConfig<E>,
    ) -> Result<TextNamespaceHandle<'b, N, S, M>, TextIndexError>
    where
        'a: 'b,
    {
        let state_key = text_index_state_key(&self.ns_name, idx_name);
        let storage = self.store.inner.tree.storage.clone();

        validate_or_write_text_identity::<N, _, _>(
            &storage,
            &state_key,
            &config.embedder,
            config.metric,
            config.level_bits,
            config.max_bucket_size,
        )?;

        let inner_local_name = text_inner_proximity_name(idx_name);
        let proximity_save_name_full = proximity_save_name(&self.ns_name, &inner_local_name);
        let inner_idx_key = (self.ns_name.clone(), inner_local_name);
        let embedder_key = (self.ns_name.clone(), idx_name.to_string());

        if !self.store.proximity_indexes.contains_key(&inner_idx_key) {
            let prox_config = ProximityConfig {
                dim: config.embedder.dim(),
                metric: config.metric,
                level_bits: config.level_bits,
                max_bucket_size: config.max_bucket_size,
            };
            let inner = match ProximityIndex::load(storage.clone(), &proximity_save_name_full) {
                Ok(loaded) => {
                    if loaded.config().dim != config.embedder.dim() {
                        return Err(TextIndexError::DimensionMismatch {
                            stored: loaded.config().dim,
                            got: config.embedder.dim(),
                        });
                    }
                    loaded
                }
                Err(ProximityError::NotFound(_)) => ProximityIndex::new(storage, prox_config),
                Err(e) => return Err(TextIndexError::Proximity(e)),
            };
            self.store
                .proximity_indexes
                .insert(inner_idx_key.clone(), inner);
        }

        // Register the embedder centrally so the cascade insert path can
        // reach it. Subsequent calls with the same `idx_name` replace the
        // entry — by this point, identity validation has already confirmed
        // the new embedder matches the persisted id/version, so swapping
        // the Arc is safe.
        let arc_embedder: Arc<dyn Embedder> = Arc::new(config.embedder);
        self.store
            .text_embedders
            .insert(embedder_key.clone(), arc_embedder.clone());
        // Register the chunker too — used by both this handle's insert and
        // the cascade insert path.
        let arc_chunker: Arc<dyn Chunker> = config.chunker;
        self.store
            .text_chunkers
            .insert(embedder_key, arc_chunker.clone());

        Ok(TextNamespaceHandle {
            store: &mut *self.store,
            ns_name: self.ns_name.clone(),
            idx_name: idx_name.to_string(),
            inner_idx_key,
            embedder: arc_embedder,
            chunker: arc_chunker,
        })
    }

    /// Drop a text sub-index from the in-memory cache. The persisted state
    /// (entries + identity blob + tree nodes) is **not** eagerly deleted —
    /// content-addressed nodes become unreachable for backend GC. A future
    /// call to `text_index` with the same name re-loads the same state.
    pub fn drop_text_index(&mut self, idx_name: &str) -> bool {
        let inner_idx_key = (self.ns_name.clone(), text_inner_proximity_name(idx_name));
        let embedder_key = (self.ns_name.clone(), idx_name.to_string());
        self.store.dirty_proximity_indexes.remove(&inner_idx_key);
        self.store.text_embedders.remove(&embedder_key);
        self.store.text_chunkers.remove(&embedder_key);
        self.store.text_transformers.remove(&embedder_key);
        self.store
            .proximity_indexes
            .remove(&inner_idx_key)
            .is_some()
    }
}

// ---------------------------------------------------------------------------
// Cascade configuration — PR 4d
// ---------------------------------------------------------------------------

#[cfg(feature = "proximity")]
impl<const N: usize, S: NodeStorage<N>, M: MetadataBackend> NamespacedKvStore<N, S, M>
where
    VersionedKvStore<N, S, M>: TreeConfigSaver<N>,
{
    /// Configure auto-cascade for a namespace. Every subsequent
    /// `NamespaceHandle::insert` against `ns_name` will additionally embed
    /// the value (interpreted as UTF-8) and upsert into every listed text
    /// sub-index. `NamespaceHandle::delete` cascades deletions to the same
    /// list.
    ///
    /// Targets that have not been loaded (no `text_index<E>()` call yet in
    /// this process) are silently skipped. Use
    /// [`Self::audit_text_index`] / [`Self::purge_text_index_orphans`] to
    /// detect and repair drift after the fact.
    ///
    /// Values that are not valid UTF-8 are silently skipped for cascade —
    /// a future PR will add a `value_transformer` hook for binary-to-text
    /// extraction.
    ///
    /// Runtime configuration only: not persisted in the namespace registry,
    /// so callers must set it each time they open the store.
    pub fn set_cascade(&mut self, ns_name: &str, text_indexes: Vec<String>) {
        self.cascade_lists.insert(ns_name.to_string(), text_indexes);
    }

    /// Clear any cascade configuration for the given namespace.
    pub fn clear_cascade(&mut self, ns_name: &str) {
        self.cascade_lists.remove(ns_name);
    }

    /// Current cascade list for a namespace, if any.
    pub fn cascade_for_namespace(&self, ns_name: &str) -> Option<&[String]> {
        self.cascade_lists.get(ns_name).map(|v| v.as_slice())
    }

    /// Register a value transformer for a `(namespace, text_index)` target.
    ///
    /// During cascade, the transformer runs on the raw primary-tree value
    /// bytes for each id. It returns either:
    ///
    /// - `Some(text)` — the text to embed and upsert into this index.
    /// - `None` — opt this id out of cascade for this index. The primary
    ///   tree still gets the bytes; only the named text index is skipped.
    ///
    /// Without a registered transformer, cascade falls back to interpreting
    /// the bytes as UTF-8 (and silently skips non-UTF-8 values).
    ///
    /// Runtime configuration only — not persisted; users register their
    /// transformers each process. Typical use: extract a field from a JSON
    /// payload, or stringify a Protobuf message.
    pub fn set_value_transformer<F>(&mut self, ns_name: &str, idx_name: &str, transformer: F)
    where
        F: Fn(&[u8]) -> Option<String> + Send + Sync + 'static,
    {
        self.text_transformers.insert(
            (ns_name.to_string(), idx_name.to_string()),
            Arc::new(transformer),
        );
    }

    /// Remove the value transformer for a `(namespace, text_index)` target.
    /// Returns whether one was registered.
    pub fn clear_value_transformer(&mut self, ns_name: &str, idx_name: &str) -> bool {
        self.text_transformers
            .remove(&(ns_name.to_string(), idx_name.to_string()))
            .is_some()
    }

    /// True when a value transformer is registered for the given target.
    pub fn has_value_transformer(&self, ns_name: &str, idx_name: &str) -> bool {
        self.text_transformers
            .contains_key(&(ns_name.to_string(), idx_name.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Drift management — PR 4c
// ---------------------------------------------------------------------------

/// Report returned by [`NamespacedKvStore::gc_blobs`] (PR 0c).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlobGcReport {
    /// Total blobs found in storage before GC.
    pub total: usize,
    /// Blobs found to be referenced by at least one envelope in some loaded
    /// namespace tree.
    pub referenced: usize,
    /// Blobs successfully deleted as orphans.
    pub removed: usize,
    /// Per-blob delete errors, formatted as `"hex_hash: error_message"`.
    pub errors: Vec<String>,
}

impl BlobGcReport {
    /// Convenience: count of blobs left after GC (`total - removed`).
    pub fn remaining(&self) -> usize {
        self.total - self.removed
    }
}

/// Report returned by [`NamespacedKvStore::audit_text_index`].
///
/// `orphans` and `missing` are ordered for stable iteration and for
/// deterministic test assertions.
#[cfg(feature = "proximity")]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextIndexAudit {
    /// Ids present in the text index but **not** in the primary KV tree.
    /// Result of (a) a delete that didn't cascade, (b) a manual primary-tree
    /// removal, or (c) the index was loaded from disk with stale state.
    pub orphans_in_index: Vec<Vec<u8>>,
    /// Ids present in the primary KV tree but **not** in the text index.
    /// Result of (a) primary inserts that weren't mirrored into the index,
    /// or (b) an index that was created after some primary writes already
    /// happened.
    pub missing_from_index: Vec<Vec<u8>>,
}

#[cfg(feature = "proximity")]
impl TextIndexAudit {
    /// True when the index is in perfect sync with the primary tree.
    pub fn is_in_sync(&self) -> bool {
        self.orphans_in_index.is_empty() && self.missing_from_index.is_empty()
    }
}

#[cfg(feature = "proximity")]
impl<const N: usize, S: NodeStorage<N>, M: MetadataBackend> NamespacedKvStore<N, S, M>
where
    VersionedKvStore<N, S, M>: TreeConfigSaver<N>,
{
    /// Compare the id set of a named text sub-index against the primary KV
    /// tree of the same namespace and report orphans + missing entries.
    ///
    /// Both namespace and index must already be loaded into memory (i.e. the
    /// user has called `namespace(ns)` and `text_index(idx, ...)` earlier in
    /// this process). Returns an error if either is missing.
    pub fn audit_text_index(
        &mut self,
        ns_name: &str,
        idx_name: &str,
    ) -> Result<TextIndexAudit, TextIndexError> {
        let inner_idx_key = (ns_name.to_string(), text_inner_proximity_name(idx_name));
        if !self.proximity_indexes.contains_key(&inner_idx_key) {
            return Err(TextIndexError::NotFound(format!("{ns_name}:{idx_name}")));
        }
        // Snapshot id sets from both sides.
        let index_ids: std::collections::BTreeSet<Vec<u8>> = self
            .proximity_indexes
            .get(&inner_idx_key)
            .unwrap()
            .entries_snapshot()
            .keys()
            .cloned()
            .collect();

        // Ensure the primary tree is loaded so we can list its keys.
        let handle = self.namespace(ns_name);
        let primary_ids: std::collections::BTreeSet<Vec<u8>> =
            handle.list_keys().into_iter().collect();
        // `handle` borrows `self`; drop before continuing.
        drop(handle);

        let mut orphans: Vec<Vec<u8>> = index_ids.difference(&primary_ids).cloned().collect();
        let mut missing: Vec<Vec<u8>> = primary_ids.difference(&index_ids).cloned().collect();
        orphans.sort();
        missing.sort();

        Ok(TextIndexAudit {
            orphans_in_index: orphans,
            missing_from_index: missing,
        })
    }

    /// Remove every text-index id that is not present in the primary KV tree
    /// of the same namespace. Returns the number of ids purged.
    ///
    /// Equivalent to "delete all `audit_text_index(...).orphans_in_index`
    /// entries from the text index" but does the audit + delete in one
    /// shot. Marks the underlying proximity index dirty so the deletions
    /// land on the next `commit()`.
    pub fn purge_text_index_orphans(
        &mut self,
        ns_name: &str,
        idx_name: &str,
    ) -> Result<usize, TextIndexError> {
        let report = self.audit_text_index(ns_name, idx_name)?;
        if report.orphans_in_index.is_empty() {
            return Ok(0);
        }

        let inner_idx_key = (ns_name.to_string(), text_inner_proximity_name(idx_name));
        let count = report.orphans_in_index.len();
        let idx = self
            .proximity_indexes
            .get_mut(&inner_idx_key)
            .expect("loaded by audit_text_index");
        for orphan in &report.orphans_in_index {
            idx.remove(orphan);
        }
        self.dirty_proximity_indexes.insert(inner_idx_key);
        Ok(count)
    }
}
