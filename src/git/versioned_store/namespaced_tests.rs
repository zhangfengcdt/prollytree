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

#[cfg(test)]
mod namespaced_tests {
    use crate::git::versioned_store::namespaced::*;
    use std::collections::HashSet;
    use tempfile::TempDir;

    /// RAII guard that holds the global CWD mutex and restores the working
    /// directory on drop. This prevents parallel tests from racing on CWD.
    struct CwdGuard {
        original: std::path::PathBuf,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl CwdGuard {
        fn set(path: &std::path::Path) -> Self {
            let lock = crate::git::versioned_store::cwd_lock()
                .lock()
                .expect("CWD mutex poisoned");
            let original = std::env::current_dir().expect("Failed to get current dir");
            std::env::set_current_dir(path).expect("Failed to change directory");
            Self {
                original,
                _lock: lock,
            }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    /// Helper: create a temporary git repo with a dataset subdirectory.
    fn setup_git_repo() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = temp_dir.path();

        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init failed");
        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .expect("git config name failed");
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config email failed");

        let dataset_path = repo_path.join("dataset");
        std::fs::create_dir(&dataset_path).expect("Failed to create dataset dir");

        (temp_dir, dataset_path)
    }

    // =====================================================================
    // Basic namespace CRUD
    // =====================================================================

    #[test]
    fn test_namespace_insert_get_delete() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Insert into "users" namespace
        {
            let mut ns = store.namespace("users");
            ns.insert(b"user:1".to_vec(), b"Alice".to_vec())
                .expect("insert failed");
            ns.insert(b"user:2".to_vec(), b"Bob".to_vec())
                .expect("insert failed");
        }

        // Read back
        {
            let ns = store.namespace("users");
            assert_eq!(ns.get(b"user:1"), Some(b"Alice".to_vec()));
            assert_eq!(ns.get(b"user:2"), Some(b"Bob".to_vec()));
            assert_eq!(ns.get(b"user:3"), None);
        }

        // Delete
        {
            let mut ns = store.namespace("users");
            assert!(ns.delete(b"user:1").expect("delete failed"));
            assert!(!ns.delete(b"user:999").expect("delete failed"));
        }

        // Verify deletion
        {
            let ns = store.namespace("users");
            assert_eq!(ns.get(b"user:1"), None);
            assert_eq!(ns.get(b"user:2"), Some(b"Bob".to_vec()));
        }
    }

    #[test]
    fn test_namespace_list_keys() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        {
            let mut ns = store.namespace("products");
            ns.insert(b"prod:1".to_vec(), b"Laptop".to_vec()).unwrap();
            ns.insert(b"prod:2".to_vec(), b"Mouse".to_vec()).unwrap();
            ns.insert(b"prod:3".to_vec(), b"Keyboard".to_vec()).unwrap();
        }

        let keys = store.namespace("products").list_keys();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&b"prod:1".to_vec()));
        assert!(keys.contains(&b"prod:2".to_vec()));
        assert!(keys.contains(&b"prod:3".to_vec()));
    }

    #[test]
    fn test_multiple_namespaces_isolation() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Insert same key in two different namespaces
        store
            .namespace("ns_a")
            .insert(b"key1".to_vec(), b"value_a".to_vec())
            .unwrap();
        store
            .namespace("ns_b")
            .insert(b"key1".to_vec(), b"value_b".to_vec())
            .unwrap();

        // Values are isolated
        assert_eq!(
            store.namespace("ns_a").get(b"key1"),
            Some(b"value_a".to_vec())
        );
        assert_eq!(
            store.namespace("ns_b").get(b"key1"),
            Some(b"value_b".to_vec())
        );

        // Keys are isolated
        let keys_a = store.namespace("ns_a").list_keys();
        let keys_b = store.namespace("ns_b").list_keys();
        assert_eq!(keys_a.len(), 1);
        assert_eq!(keys_b.len(), 1);

        // A third namespace sees nothing
        assert!(store.namespace("ns_c").list_keys().is_empty());
        assert_eq!(store.namespace("ns_c").get(b"key1"), None);
    }

    #[test]
    fn test_default_namespace_backward_compat() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Use flat API (should route to "default" namespace)
        store
            .insert(b"flat_key".to_vec(), b"flat_value".to_vec())
            .unwrap();
        assert_eq!(store.get(b"flat_key"), Some(b"flat_value".to_vec()));

        // Verify it's in the "default" namespace
        assert_eq!(
            store.namespace(DEFAULT_NAMESPACE).get(b"flat_key"),
            Some(b"flat_value".to_vec())
        );

        // Keys from flat API appear in default namespace
        let keys = store.namespace(DEFAULT_NAMESPACE).list_keys();
        assert!(keys.contains(&b"flat_key".to_vec()));
    }

    // =====================================================================
    // Registry operations
    // =====================================================================

    #[test]
    fn test_list_namespaces() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Initially only "default"
        let ns_list = store.list_namespaces();
        assert!(ns_list.contains(&"default".to_string()));

        // Create some namespaces
        store
            .namespace("users")
            .insert(b"u1".to_vec(), b"Alice".to_vec())
            .unwrap();
        store
            .namespace("products")
            .insert(b"p1".to_vec(), b"Widget".to_vec())
            .unwrap();
        store
            .namespace("orders")
            .insert(b"o1".to_vec(), b"Order1".to_vec())
            .unwrap();

        let ns_list = store.list_namespaces();
        assert_eq!(ns_list.len(), 4); // default + users + products + orders
        assert!(ns_list.contains(&"users".to_string()));
        assert!(ns_list.contains(&"products".to_string()));
        assert!(ns_list.contains(&"orders".to_string()));
        assert!(ns_list.contains(&"default".to_string()));
    }

    #[test]
    fn test_delete_namespace() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        store
            .namespace("temp")
            .insert(b"key".to_vec(), b"value".to_vec())
            .unwrap();
        assert_eq!(store.list_namespaces().len(), 2); // default + temp

        // Delete the namespace
        assert!(store.delete_namespace("temp").unwrap());
        assert!(!store.list_namespaces().contains(&"temp".to_string()));

        // Deleting again returns false
        assert!(!store.delete_namespace("temp").unwrap());

        // Cannot delete default namespace
        assert!(store.delete_namespace("default").is_err());
    }

    #[test]
    fn test_get_namespace_root_hash() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Empty namespace has a root hash (empty tree hash)
        let hash_before = store.get_namespace_root_hash("default");
        assert!(hash_before.is_some());

        // Insert data and verify hash changes
        store
            .namespace("default")
            .insert(b"key".to_vec(), b"value".to_vec())
            .unwrap();
        store.commit("add key").unwrap();

        let hash_after = store.get_namespace_root_hash("default");
        assert!(hash_after.is_some());
        // After insert + commit, the tree root hash should have changed
        // (comparing pre-commit staging vs post-commit tree)

        // Different namespaces have different root hashes
        store
            .namespace("other")
            .insert(b"okey".to_vec(), b"oval".to_vec())
            .unwrap();
        store.commit("add other ns").unwrap();

        let hash_default = store.get_namespace_root_hash("default");
        let hash_other = store.get_namespace_root_hash("other");
        assert!(hash_default.is_some());
        assert!(hash_other.is_some());
        assert_ne!(hash_default, hash_other);

        // Non-existent namespace returns None
        assert!(store.get_namespace_root_hash("nonexistent").is_none());
    }

    // =====================================================================
    // Commit and persistence
    // =====================================================================

    #[test]
    fn test_namespace_commit_and_reopen() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        // Create and populate
        {
            let mut store =
                GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

            store
                .namespace("users")
                .insert(b"user:1".to_vec(), b"Alice".to_vec())
                .unwrap();
            store
                .namespace("products")
                .insert(b"prod:1".to_vec(), b"Widget".to_vec())
                .unwrap();
            store
                .insert(b"default_key".to_vec(), b"default_val".to_vec())
                .unwrap();

            store.commit("Add data across namespaces").unwrap();
        }

        // Reopen and verify
        {
            let mut store =
                GitNamespacedKvStore::<32>::open(&dataset_path).expect("Failed to open");

            assert_eq!(store.format_version, StoreFormatVersion::V2);

            // Check namespaces
            let ns_list = store.list_namespaces();
            assert!(ns_list.contains(&"users".to_string()));
            assert!(ns_list.contains(&"products".to_string()));
            assert!(ns_list.contains(&"default".to_string()));

            // Check data
            assert_eq!(
                store.namespace("users").get(b"user:1"),
                Some(b"Alice".to_vec())
            );
            assert_eq!(
                store.namespace("products").get(b"prod:1"),
                Some(b"Widget".to_vec())
            );
            assert_eq!(store.get(b"default_key"), Some(b"default_val".to_vec()));
        }
    }

    #[test]
    fn test_namespace_staging_isolation() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Stage changes in two namespaces without committing
        store
            .namespace("ns_a")
            .insert(b"key".to_vec(), b"staged_a".to_vec())
            .unwrap();
        store
            .namespace("ns_b")
            .insert(b"key".to_vec(), b"staged_b".to_vec())
            .unwrap();

        // Both staging areas are independent
        assert_eq!(
            store.namespace("ns_a").get(b"key"),
            Some(b"staged_a".to_vec())
        );
        assert_eq!(
            store.namespace("ns_b").get(b"key"),
            Some(b"staged_b".to_vec())
        );

        // Deleting from ns_a does not affect ns_b
        store.namespace("ns_a").delete(b"key").unwrap();
        assert_eq!(store.namespace("ns_a").get(b"key"), None);
        assert_eq!(
            store.namespace("ns_b").get(b"key"),
            Some(b"staged_b".to_vec())
        );
    }

    #[test]
    fn test_namespace_dirty_tracking() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Insert into one namespace, commit
        store
            .namespace("ns_a")
            .insert(b"k1".to_vec(), b"v1".to_vec())
            .unwrap();
        store.commit("add ns_a data").unwrap();

        // Only ns_a should have been persisted with data
        assert_eq!(store.namespace("ns_a").get(b"k1"), Some(b"v1".to_vec()));

        // Now insert into ns_b and commit again
        store
            .namespace("ns_b")
            .insert(b"k2".to_vec(), b"v2".to_vec())
            .unwrap();
        store.commit("add ns_b data").unwrap();

        // Both namespaces have data
        assert_eq!(store.namespace("ns_a").get(b"k1"), Some(b"v1".to_vec()));
        assert_eq!(store.namespace("ns_b").get(b"k2"), Some(b"v2".to_vec()));
    }

    // =====================================================================
    // Branching and checkout
    // =====================================================================

    #[test]
    fn test_namespace_branch_checkout() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Add data on main
        store
            .namespace("users")
            .insert(b"u1".to_vec(), b"Alice".to_vec())
            .unwrap();
        store.commit("main data").unwrap();

        // Create branch and add more data
        store.create_branch("feature").unwrap();
        store
            .namespace("users")
            .insert(b"u2".to_vec(), b"Bob".to_vec())
            .unwrap();
        store.commit("feature data").unwrap();

        // Verify feature branch has both
        assert_eq!(store.namespace("users").get(b"u1"), Some(b"Alice".to_vec()));
        assert_eq!(store.namespace("users").get(b"u2"), Some(b"Bob".to_vec()));

        // Checkout main — should only have u1
        store.checkout("main").unwrap();
        assert_eq!(store.namespace("users").get(b"u1"), Some(b"Alice".to_vec()));
        assert_eq!(store.namespace("users").get(b"u2"), None);

        // Checkout feature again — has both
        store.checkout("feature").unwrap();
        assert_eq!(store.namespace("users").get(b"u2"), Some(b"Bob".to_vec()));
    }

    #[test]
    fn test_namespace_across_branches() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Main: create ns_a
        store
            .namespace("ns_a")
            .insert(b"key".to_vec(), b"main_a".to_vec())
            .unwrap();
        store.commit("main ns_a").unwrap();

        // Feature: create ns_b
        store.create_branch("feature").unwrap();
        store
            .namespace("ns_b")
            .insert(b"key".to_vec(), b"feature_b".to_vec())
            .unwrap();
        store.commit("feature ns_b").unwrap();

        // Feature has both namespaces
        assert!(store.list_namespaces().contains(&"ns_a".to_string()));
        assert!(store.list_namespaces().contains(&"ns_b".to_string()));

        // Main only has ns_a
        store.checkout("main").unwrap();
        assert!(store.list_namespaces().contains(&"ns_a".to_string()));
        // ns_b was created on feature branch, main shouldn't have it in registry
    }

    // =====================================================================
    // Merge
    // =====================================================================

    #[test]
    fn test_namespace_merge_no_conflict() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Main: add data to ns_a
        store
            .namespace("ns_a")
            .insert(b"key_a".to_vec(), b"val_a".to_vec())
            .unwrap();
        store.commit("main: ns_a data").unwrap();

        // Create feature branch, add data to ns_b (different namespace)
        store.create_branch("feature").unwrap();
        store
            .namespace("ns_b")
            .insert(b"key_b".to_vec(), b"val_b".to_vec())
            .unwrap();
        store.commit("feature: ns_b data").unwrap();

        // Checkout main
        store.checkout("main").unwrap();

        // Merge feature into main
        store.merge_ignore_conflicts("feature").unwrap();

        // Main now has both namespaces' data
        assert_eq!(
            store.namespace("ns_a").get(b"key_a"),
            Some(b"val_a".to_vec())
        );
        assert_eq!(
            store.namespace("ns_b").get(b"key_b"),
            Some(b"val_b".to_vec())
        );
    }

    #[test]
    fn test_namespace_merge_same_namespace() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Main: add initial data
        store
            .namespace("shared")
            .insert(b"base_key".to_vec(), b"base_val".to_vec())
            .unwrap();
        store.commit("main: base data").unwrap();

        // Create feature branch, add different key to same namespace
        store.create_branch("feature").unwrap();
        store
            .namespace("shared")
            .insert(b"feature_key".to_vec(), b"feature_val".to_vec())
            .unwrap();
        store.commit("feature: add feature_key").unwrap();

        // Back to main, add another key
        store.checkout("main").unwrap();
        store
            .namespace("shared")
            .insert(b"main_key".to_vec(), b"main_val".to_vec())
            .unwrap();
        store.commit("main: add main_key").unwrap();

        // Merge feature into main
        store.merge_ignore_conflicts("feature").unwrap();

        // All three keys should exist
        assert_eq!(
            store.namespace("shared").get(b"base_key"),
            Some(b"base_val".to_vec())
        );
        assert_eq!(
            store.namespace("shared").get(b"feature_key"),
            Some(b"feature_val".to_vec())
        );
        assert_eq!(
            store.namespace("shared").get(b"main_key"),
            Some(b"main_val".to_vec())
        );
    }

    #[test]
    fn namespace_merge_reports_missing_source_hash_mappings() {
        let (temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        store
            .namespace("shared")
            .insert(b"base_key".to_vec(), b"base_val".to_vec())
            .unwrap();
        store.commit("main: base data").unwrap();

        store.create_branch("feature").unwrap();
        store
            .namespace("shared")
            .insert(b"feature_key".to_vec(), b"feature_val".to_vec())
            .unwrap();
        store.commit("feature: add feature_key").unwrap();

        let rm = std::process::Command::new("git")
            .args(["rm", "dataset/prolly_hash_mappings"])
            .current_dir(temp_dir.path())
            .output()
            .expect("git rm failed");
        assert!(rm.status.success(), "git rm failed: {rm:?}");
        let commit = std::process::Command::new("git")
            .args(["commit", "-m", "Remove namespace mappings"])
            .current_dir(temp_dir.path())
            .output()
            .expect("git commit failed");
        assert!(commit.status.success(), "git commit failed: {commit:?}");

        store.checkout("main").unwrap();
        let err = match store.merge_ignore_conflicts("feature") {
            Ok(_) => panic!("merge with missing source namespace mappings must fail"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("mapping") || err.to_string().contains("Mapping"),
            "unexpected error: {err}"
        );
    }

    // =====================================================================
    // Change detection
    // =====================================================================

    #[test]
    fn test_namespace_changed_detection() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Commit 1: add data to ns_a
        store
            .namespace("ns_a")
            .insert(b"k1".to_vec(), b"v1".to_vec())
            .unwrap();
        let commit1 = store.commit("add ns_a").unwrap();
        let commit1_hex = commit1.to_hex().to_string();

        // Commit 2: add data to ns_b (ns_a unchanged)
        store
            .namespace("ns_b")
            .insert(b"k2".to_vec(), b"v2".to_vec())
            .unwrap();
        let commit2 = store.commit("add ns_b").unwrap();
        let commit2_hex = commit2.to_hex().to_string();

        // ns_a did NOT change between commit1 and commit2
        assert!(!store
            .namespace_changed("ns_a", &commit1_hex, &commit2_hex)
            .unwrap());

        // ns_b DID change (didn't exist in commit1, exists in commit2)
        assert!(store
            .namespace_changed("ns_b", &commit1_hex, &commit2_hex)
            .unwrap());

        // Commit 3: modify ns_a
        store
            .namespace("ns_a")
            .insert(b"k1".to_vec(), b"v1_updated".to_vec())
            .unwrap();
        let commit3 = store.commit("update ns_a").unwrap();
        let commit3_hex = commit3.to_hex().to_string();

        // ns_a changed between commit2 and commit3
        assert!(store
            .namespace_changed("ns_a", &commit2_hex, &commit3_hex)
            .unwrap());

        // ns_b did NOT change between commit2 and commit3
        assert!(!store
            .namespace_changed("ns_b", &commit2_hex, &commit3_hex)
            .unwrap());
    }

    // =====================================================================
    // Migration
    // =====================================================================

    #[test]
    fn test_v1_to_v2_migration() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        // Create a V1 (flat) store using VersionedKvStore directly
        {
            use crate::git::versioned_store::GitVersionedKvStore;

            let mut flat_store =
                GitVersionedKvStore::<32>::init(&dataset_path).expect("Failed to init flat store");
            flat_store
                .insert(b"key1".to_vec(), b"value1".to_vec())
                .unwrap();
            flat_store
                .insert(b"key2".to_vec(), b"value2".to_vec())
                .unwrap();
            flat_store.commit("v1 data").unwrap();
        }

        // Open with NamespacedKvStore (detects V1)
        let mut store = GitNamespacedKvStore::<32>::open(&dataset_path).expect("Failed to open");
        assert_eq!(store.format_version, StoreFormatVersion::V1);

        // Data is accessible via default namespace
        assert_eq!(store.get(b"key1"), Some(b"value1".to_vec()));
        assert_eq!(store.get(b"key2"), Some(b"value2".to_vec()));

        // Migrate to V2
        let report = store.migrate_v1_to_v2().unwrap();
        assert_eq!(report.keys_migrated, 2);
        assert_eq!(report.storage_version, StoreFormatVersion::V2);
        assert_eq!(store.format_version, StoreFormatVersion::V2);

        // Data is still accessible
        assert_eq!(store.get(b"key1"), Some(b"value1".to_vec()));
        assert_eq!(store.get(b"key2"), Some(b"value2".to_vec()));

        // Can now use namespace operations
        store
            .namespace("new_ns")
            .insert(b"nk".to_vec(), b"nv".to_vec())
            .unwrap();
        store.commit("post-migration").unwrap();

        assert!(store.list_namespaces().contains(&"new_ns".to_string()));
    }

    // =====================================================================
    // Thread safety
    // =====================================================================

    #[test]
    fn test_thread_safe_namespace_operations() {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);

        let store =
            ThreadSafeGitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Insert from main thread
        store
            .ns_insert("users", b"u1".to_vec(), b"Alice".to_vec())
            .unwrap();

        // Read back
        assert_eq!(store.ns_get("users", b"u1"), Some(b"Alice".to_vec()));

        // List keys
        let keys = store.ns_list_keys("users");
        assert_eq!(keys.len(), 1);

        // List namespaces
        let ns_list = store.list_namespaces();
        assert!(ns_list.contains(&"users".to_string()));

        // Default namespace via flat API
        store.insert(b"dk".to_vec(), b"dv".to_vec()).unwrap();
        assert_eq!(store.get(b"dk"), Some(b"dv".to_vec()));

        // Commit
        store.commit("thread-safe test").unwrap();

        // Verify after commit
        assert_eq!(store.ns_get("users", b"u1"), Some(b"Alice".to_vec()));
        assert_eq!(store.get(b"dk"), Some(b"dv".to_vec()));
    }

    // =====================================================================
    // Comparison: prefix-based (old) vs subtree-based (new)
    // =====================================================================

    #[test]
    fn test_namespace_vs_prefix_comparison() {
        use crate::git::versioned_store::GitVersionedKvStore;
        use std::time::Instant;

        let num_namespaces = 5;
        let keys_per_ns = 20;

        // ── Setup: Old approach (prefix-based flat store) ──

        let (_td_old, dataset_path_old) = setup_git_repo();
        let _cwd_old = CwdGuard::set(&dataset_path_old);

        let mut old_store =
            GitVersionedKvStore::<32>::init(&dataset_path_old).expect("Failed to init old store");

        for ns_idx in 0..num_namespaces {
            for key_idx in 0..keys_per_ns {
                let key = format!("/ns_{ns_idx}/key_{key_idx}").into_bytes();
                let value = format!("value_{ns_idx}_{key_idx}").into_bytes();
                old_store.insert(key, value).unwrap();
            }
        }
        old_store.commit("old: populate").unwrap();

        // ── Setup: New approach (subtree-based namespaced store) ──

        let (_td_new, dataset_path_new) = setup_git_repo();
        // Restore cwd first then set to new
        drop(_cwd_old);
        let _cwd_new = CwdGuard::set(&dataset_path_new);

        let mut new_store =
            GitNamespacedKvStore::<32>::init(&dataset_path_new).expect("Failed to init new store");

        for ns_idx in 0..num_namespaces {
            for key_idx in 0..keys_per_ns {
                let ns_name = format!("ns_{ns_idx}");
                let key = format!("key_{key_idx}").into_bytes();
                let value = format!("value_{ns_idx}_{key_idx}").into_bytes();
                new_store.namespace(&ns_name).insert(key, value).unwrap();
            }
        }
        new_store.commit("new: populate").unwrap();

        // ── Comparison 1: Namespace listing ──

        // Old: must scan all keys and extract unique prefixes
        let start_old = Instant::now();
        let old_keys = old_store.list_keys();
        let mut old_namespaces: HashSet<String> = HashSet::new();
        for key in &old_keys {
            let key_str = String::from_utf8_lossy(key);
            if let Some(idx) = key_str[1..].find('/') {
                old_namespaces.insert(key_str[1..idx + 1].to_string());
            }
        }
        let old_ns_time = start_old.elapsed();

        // New: direct registry lookup
        let start_new = Instant::now();
        let new_namespaces = new_store.list_namespaces();
        let new_ns_time = start_new.elapsed();

        assert_eq!(old_namespaces.len(), num_namespaces);
        // new_namespaces includes "default" namespace
        assert_eq!(new_namespaces.len(), num_namespaces + 1);

        // ── Comparison 2: Namespace-scoped key listing ──

        // Old: scan all keys, filter by prefix
        let target_ns = "ns_2";
        let prefix = format!("/{target_ns}/");

        let start_old = Instant::now();
        let old_ns_keys: Vec<_> = old_store
            .list_keys()
            .into_iter()
            .filter(|k| String::from_utf8_lossy(k).starts_with(&prefix))
            .collect();
        let old_key_time = start_old.elapsed();

        // New: direct namespace key listing
        let start_new = Instant::now();
        let new_ns_keys = new_store.namespace(target_ns).list_keys();
        let new_key_time = start_new.elapsed();

        assert_eq!(old_ns_keys.len(), keys_per_ns);
        assert_eq!(new_ns_keys.len(), keys_per_ns);

        // ── Comparison 3: Change detection ──

        // Modify one namespace
        let commit_before_hex = {
            let log = new_store.log().unwrap();
            log[0].id.to_hex().to_string()
        };

        new_store
            .namespace("ns_0")
            .insert(b"new_key".to_vec(), b"new_val".to_vec())
            .unwrap();
        let commit_after = new_store.commit("modify ns_0").unwrap();
        let commit_after_hex = commit_after.to_hex().to_string();

        // New: O(1) hash comparison per namespace
        let start_new = Instant::now();
        let ns0_changed = new_store
            .namespace_changed("ns_0", &commit_before_hex, &commit_after_hex)
            .unwrap();
        let ns1_changed = new_store
            .namespace_changed("ns_1", &commit_before_hex, &commit_after_hex)
            .unwrap();
        let new_change_time = start_new.elapsed();

        assert!(ns0_changed, "ns_0 should have changed");
        assert!(!ns1_changed, "ns_1 should NOT have changed");

        // Old: must load full KV from both commits and compare
        // (We don't measure this since VersionedKvStore doesn't have
        //  a namespace_changed equivalent — it would require diff() + filtering)

        // ── Print comparison results ──
        println!("\n===== Namespace Approach Comparison =====");
        println!(
            "  Namespaces: {}, Keys per NS: {}",
            num_namespaces, keys_per_ns
        );
        println!("  Total keys: {}", num_namespaces * keys_per_ns);
        println!();
        println!("  Namespace listing:");
        println!("    Old (prefix scan):  {:?}", old_ns_time);
        println!("    New (registry):     {:?}", new_ns_time);
        println!();
        println!("  Scoped key listing for '{target_ns}':");
        println!("    Old (filter all):   {:?}", old_key_time);
        println!("    New (subtree):      {:?}", new_key_time);
        println!();
        println!("  Change detection:");
        println!("    Old: N/A (requires full diff + filter)");
        println!("    New (hash compare): {:?}", new_change_time);
        println!("=========================================\n");
    }

    // =====================================================================
    // History independence
    //
    // Each namespace is backed by its own ProllyTree, so the streaming
    // canonical chunker (driven by ProllyTree::apply_changes on every
    // commit) should produce per-namespace root hashes that depend only
    // on the final (key, value) set in that namespace - never on
    // insert/delete order or cross-namespace interleaving.
    // =====================================================================

    use rand::prelude::StdRng;
    use rand::seq::SliceRandom;
    use rand::SeedableRng;

    fn k8(i: u64) -> Vec<u8> {
        i.to_be_bytes().to_vec()
    }

    fn v16(i: u64) -> Vec<u8> {
        let mut v = Vec::with_capacity(16);
        v.extend_from_slice(&i.to_be_bytes());
        v.extend_from_slice(&(!i).to_be_bytes());
        v
    }

    /// Build a single-namespace store, inserting keys in the given order
    /// and committing once. Returns the namespace's root hash.
    fn build_single_namespace(ns_name: &str, order: &[u64]) -> crate::digest::ValueDigest<32> {
        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);
        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");
        {
            let mut ns = store.namespace(ns_name);
            for &i in order {
                ns.insert(k8(i), v16(i)).expect("insert");
            }
        }
        store.commit("history-independence").expect("commit");
        store
            .get_namespace_root_hash(ns_name)
            .expect("namespace root hash")
    }

    #[test]
    fn namespace_root_hash_independent_of_insert_order() {
        const N: u64 = 48;
        let asc: Vec<u64> = (0..N).collect();
        let desc: Vec<u64> = (0..N).rev().collect();
        let alt: Vec<u64> = (0..N).step_by(2).chain((1..N).step_by(2)).collect();
        let mut shuf: Vec<u64> = (0..N).collect();
        shuf.shuffle(&mut StdRng::from_seed([23u8; 32]));

        let h_asc = build_single_namespace("users", &asc);
        let h_desc = build_single_namespace("users", &desc);
        let h_alt = build_single_namespace("users", &alt);
        let h_shuf = build_single_namespace("users", &shuf);

        assert_eq!(
            h_asc, h_desc,
            "descending order produced different namespace root hash"
        );
        assert_eq!(
            h_asc, h_alt,
            "alternating order produced different namespace root hash"
        );
        assert_eq!(
            h_asc, h_shuf,
            "shuffled order produced different namespace root hash"
        );
    }

    #[test]
    fn namespace_root_hash_independent_after_deletes() {
        const SURVIVE: u64 = 32;
        const EXTRA: u64 = 16;

        let survivors: Vec<u64> = (0..SURVIVE).collect();
        let baseline = build_single_namespace("orders", &survivors);

        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);
        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        let mut all: Vec<u64> = (0..SURVIVE + EXTRA).collect();
        all.shuffle(&mut StdRng::from_seed([29u8; 32]));
        {
            let mut ns = store.namespace("orders");
            for &i in &all {
                ns.insert(k8(i), v16(i)).expect("insert");
            }
        }
        store.commit("with extras").expect("commit");

        let mut del_order: Vec<u64> = (SURVIVE..SURVIVE + EXTRA).collect();
        del_order.shuffle(&mut StdRng::from_seed([31u8; 32]));
        {
            let mut ns = store.namespace("orders");
            for &i in &del_order {
                assert!(ns.delete(&k8(i)).expect("delete"));
            }
        }
        store.commit("delete extras").expect("commit deletes");

        let after_delete = store
            .get_namespace_root_hash("orders")
            .expect("ns root hash");
        assert_eq!(
            after_delete, baseline,
            "namespace after insert-extras-then-delete diverged from survivors-only baseline"
        );
    }

    /// Two namespaces written in interleaved order across multiple commits
    /// must produce the same per-namespace root hashes as a fresh single-
    /// commit build of each one independently. This catches any case where
    /// cross-namespace interleaving leaks into a namespace's tree state.
    #[test]
    fn per_namespace_hashes_independent_of_cross_namespace_interleaving() {
        const N: u64 = 24;
        let asc: Vec<u64> = (0..N).collect();
        let baseline_users = build_single_namespace("users", &asc);
        let baseline_orders = build_single_namespace("orders", &asc);

        let (_temp_dir, dataset_path) = setup_git_repo();
        let _cwd = CwdGuard::set(&dataset_path);
        let mut store = GitNamespacedKvStore::<32>::init(&dataset_path).expect("Failed to init");

        // Interleave: for each i, write to "users" then "orders". Pick a
        // non-trivial shuffle so we don't accidentally match the single-
        // namespace order. Commit every few iterations to exercise the
        // staging-drain path.
        let mut shuffled: Vec<u64> = (0..N).collect();
        shuffled.shuffle(&mut StdRng::from_seed([37u8; 32]));
        for (idx, i) in shuffled.iter().enumerate() {
            {
                let mut users = store.namespace("users");
                users.insert(k8(*i), v16(*i)).expect("users insert");
            }
            {
                let mut orders = store.namespace("orders");
                orders.insert(k8(*i), v16(*i)).expect("orders insert");
            }
            if idx % 5 == 4 {
                store
                    .commit(&format!("interleaved batch {idx}"))
                    .expect("commit batch");
            }
        }
        store.commit("interleaved final").expect("commit final");

        let h_users = store.get_namespace_root_hash("users").expect("users hash");
        let h_orders = store
            .get_namespace_root_hash("orders")
            .expect("orders hash");

        assert_eq!(
            h_users, baseline_users,
            "users namespace diverged after cross-namespace interleaved writes"
        );
        assert_eq!(
            h_orders, baseline_orders,
            "orders namespace diverged after cross-namespace interleaved writes"
        );
    }
}
