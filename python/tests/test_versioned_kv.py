#!/usr/bin/env python3

# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""Test script for VersionedKvStore Python bindings."""

import tempfile
import os
import subprocess
from prollytree import VersionedKvStore, StorageBackend

def test_versioned_kv_store():
    """Test the versioned key-value store functionality."""

    # Save original directory
    original_dir = os.getcwd()

    # Create a temporary directory and initialize a git repository
    with tempfile.TemporaryDirectory() as tmpdir:
        try:
            print(f"[DIR] Creating test in: {tmpdir}")

            # Initialize git repository
            subprocess.run(["git", "init"], cwd=tmpdir, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.name", "Test User"], cwd=tmpdir, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=tmpdir, check=True)

            # Create a subdirectory for our dataset
            dataset_dir = os.path.join(tmpdir, "dataset")
            os.makedirs(dataset_dir)
            os.chdir(dataset_dir)

            print("[OK] Git repository initialized")

            # Test 1: Initialize VersionedKvStore
            print("\n[TEST] Test 1: Initialize VersionedKvStore")
            store = VersionedKvStore(dataset_dir)
            print(f"   Storage backend: {store.storage_backend()}")
            print(f"   Current branch: {store.current_branch()}")

            # Test 2: Basic key-value operations
            print("\n[TEST] Test 2: Basic key-value operations")
            store.insert(b"name", b"Alice")
            store.insert(b"age", b"30")
            store.insert(b"city", b"San Francisco")

            # Check values
            name = store.get(b"name")
            age = store.get(b"age")
            city = store.get(b"city")

            print(f"   name: {name}")
            print(f"   age: {age}")
            print(f"   city: {city}")

            # Test 3: List keys and status
            print("\n[TEST] Test 3: List keys and status")
            keys = store.list_keys()
            print(f"   Keys: {[k.decode() for k in keys]}")

            status = store.status()
            print("   Status:")
            for key, status_str in status:
                print(f"   - {key.decode()}: {status_str}")

            # Test 4: Commit changes
            print("\n[TEST] Test 4: Commit changes")
            commit_hash = store.commit("Add initial user data")
            print(f"   Commit hash: {commit_hash}")

            # Check status after commit
            status = store.status()
            print(f"   Status after commit: {len(status)} staged changes")

            # Test 5: Update and delete operations
            print("\n[TEST] Test 5: Update and delete operations")
            updated = store.update(b"age", b"31")
            print(f"   Updated age: {updated}")

            deleted = store.delete(b"city")
            print(f"   Deleted city: {deleted}")

            # Add new key
            store.insert(b"country", b"USA")

            # Check status
            status = store.status()
            print("   Status after changes:")
            for key, status_str in status:
                print(f"   - {key.decode()}: {status_str}")

            # Test 6: Branch operations
            print("\n[TEST] Test 6: Branch operations")
            store.create_branch("feature-branch")
            print("   Created and switched to feature-branch")
            print(f"   Current branch: {store.current_branch()}")

            # Make changes on feature branch
            store.insert(b"feature", b"new-feature")
            store.commit("Add feature on feature branch")

            # List all branches
            branches = store.list_branches()
            print(f"   Available branches: {branches}")

            # Test 7: Switch back to main
            print("\n[TEST] Test 7: Switch back to main")
            store.checkout("main")
            print(f"   Current branch: {store.current_branch()}")

            # Check if feature key exists (should not exist on main)
            feature = store.get(b"feature")
            print(f"   Feature key on main: {feature}")

            # Test 8: Commit history
            print("\n[TEST] Test 8: Commit history")
            history = store.log()
            print(f"   Commit history ({len(history)} commits):")
            for i, commit in enumerate(history[:3]):  # Show first 3 commits
                print(f"   {i+1}. {commit['id'][:8]} - {commit['message']}")
                print(f"      Author: {commit['author']}")
                print(f"      Timestamp: {commit['timestamp']}")

            print("\n[OK] All VersionedKvStore tests completed successfully!")
        finally:
            os.chdir(original_dir)


def test_storage_backends():
    """Test different storage backends.

    Note: All storage backends (Git, File, InMemory) require being inside a git
    repository because they all use git for version control metadata.
    """

    with tempfile.TemporaryDirectory() as tmpdir:
        print(f"\n[DIR] Testing storage backends in: {tmpdir}")

        # Save original directory
        original_dir = os.getcwd()

        try:
            # Initialize git repository at the root
            subprocess.run(["git", "init"], cwd=tmpdir, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.name", "Test User"], cwd=tmpdir, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=tmpdir, check=True, capture_output=True)

            # Test 1: Git backend (default)
            print("\n[TEST] Test: Git backend (default)")
            git_dir = os.path.join(tmpdir, "git_data")
            os.makedirs(git_dir)
            # gix discovers repos from cwd, so we need to change directory
            os.chdir(git_dir)
            store_git = VersionedKvStore(git_dir)
            assert store_git.storage_backend() == StorageBackend.Git
            store_git.insert(b"key1", b"value1")
            store_git.commit("Git backend test")
            print(f"   Backend: {store_git.storage_backend()}")
            print("   [OK] Git backend works")

            # Test 2: File backend
            print("\n[TEST] Test: File backend")
            file_dir = os.path.join(tmpdir, "file_data")
            os.makedirs(file_dir)
            store_file = VersionedKvStore(file_dir, StorageBackend.File)
            assert store_file.storage_backend() == StorageBackend.File
            store_file.insert(b"key1", b"value1")
            store_file.commit("File backend test")
            assert store_file.get(b"key1") == b"value1"
            print(f"   Backend: {store_file.storage_backend()}")
            print("   [OK] File backend works")

            # Test 3: InMemory backend
            print("\n[TEST] Test: InMemory backend")
            mem_dir = os.path.join(tmpdir, "mem_data")
            os.makedirs(mem_dir)
            store_mem = VersionedKvStore(mem_dir, StorageBackend.InMemory)
            assert store_mem.storage_backend() == StorageBackend.InMemory
            store_mem.insert(b"key1", b"value1")
            store_mem.commit("InMemory backend test")
            assert store_mem.get(b"key1") == b"value1"
            print(f"   Backend: {store_mem.storage_backend()}")
            print("   [OK] InMemory backend works")

            print("\n[OK] All storage backend tests completed successfully!")
        finally:
            os.chdir(original_dir)


def test_git_only_operations_error_handling():
    """Test that Git-only operations raise errors on non-Git backends.

    Note: All storage backends require being inside a git repository because
    they all use git for version control metadata.
    """

    # Save original directory
    original_dir = os.getcwd()

    with tempfile.TemporaryDirectory() as tmpdir:
        try:
            print(f"\n[DIR] Testing Git-only operation error handling in: {tmpdir}")

            # Initialize git repository (needed for all backends)
            subprocess.run(["git", "init"], cwd=tmpdir, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.name", "Test User"], cwd=tmpdir, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=tmpdir, check=True, capture_output=True)
            os.chdir(tmpdir)

            # Create File backend store
            file_dir = os.path.join(tmpdir, "file_data")
            os.makedirs(file_dir)
            store = VersionedKvStore(file_dir, StorageBackend.File)
            store.insert(b"key1", b"value1")
            store.commit("Initial")

            # Test that checkout raises error on File backend
            print("\n[TEST] Test: checkout raises error on File backend")
            try:
                store.checkout("main")
                print("   [FAIL] Expected ValueError but checkout succeeded")
                assert False, "Expected ValueError"
            except ValueError as e:
                print(f"   [OK] checkout raised ValueError as expected: {e}")

            # Test that merge raises error on File backend
            print("\n[TEST] Test: merge raises error on File backend")
            try:
                store.merge("feature")
                print("   [FAIL] Expected ValueError but merge succeeded")
                assert False, "Expected ValueError"
            except ValueError as e:
                print(f"   [OK] merge raised ValueError as expected: {e}")

            # Test that diff raises error on File backend
            print("\n[TEST] Test: diff raises error on File backend")
            try:
                store.diff("main", "feature")
                print("   [FAIL] Expected ValueError but diff succeeded")
                assert False, "Expected ValueError"
            except ValueError as e:
                print(f"   [OK] diff raised ValueError as expected: {e}")

            # Test that try_merge raises error on File backend
            print("\n[TEST] Test: try_merge raises error on File backend")
            try:
                store.try_merge("feature")
                print("   [FAIL] Expected ValueError but try_merge succeeded")
                assert False, "Expected ValueError"
            except ValueError as e:
                print(f"   [OK] try_merge raised ValueError as expected: {e}")

            print("\n[OK] All Git-only operation error handling tests completed successfully!")
        finally:
            os.chdir(original_dir)


if __name__ == "__main__":
    test_versioned_kv_store()
    test_storage_backends()
    test_git_only_operations_error_handling()
