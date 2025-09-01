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

"""Tests for the diff and current_commit functionality in VersionedKvStore."""

import tempfile
import shutil
import subprocess
import os
import pytest
from pathlib import Path

import prollytree


class TestDiffFunctionality:
    """Test diff and current_commit functions."""

    def setup_method(self):
        """Set up test fixtures."""
        self.temp_dir = tempfile.mkdtemp()

        # Initialize git repository in the temp directory
        subprocess.run(["git", "init"], cwd=self.temp_dir, check=True, capture_output=True)
        subprocess.run(["git", "config", "user.name", "Test User"], cwd=self.temp_dir, check=True)
        subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=self.temp_dir, check=True)

        # Create subdirectory for the store (not in git root)
        self.store_path = Path(self.temp_dir) / "data"
        self.store_path.mkdir(parents=True, exist_ok=True)

        # Change working directory to the store path for git operations
        self.original_cwd = os.getcwd()
        os.chdir(str(self.store_path))

    def teardown_method(self):
        """Clean up test fixtures."""
        # Restore original working directory
        os.chdir(self.original_cwd)
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_diff_between_commits(self):
        """Test diff between two commits."""
        # Initialize store
        store = prollytree.VersionedKvStore(str(self.store_path))

        # Create first commit
        store.insert(b"key1", b"value1")
        store.insert(b"key2", b"value2")
        commit1 = store.commit("Initial commit")

        # Create second commit with changes
        store.insert(b"key3", b"value3")  # Added
        store.update(b"key1", b"value1_modified")  # Modified
        store.delete(b"key2")  # Removed
        commit2 = store.commit("Second commit")

        # Get diff between commits
        diffs = store.diff(commit1, commit2)

        # Verify diff results
        diff_map = {diff.key: diff.operation for diff in diffs}

        # Check that we have all expected changes
        assert len(diffs) == 3
        assert b"key1" in diff_map
        assert b"key2" in diff_map
        assert b"key3" in diff_map

        # Verify operation types
        key1_op = diff_map[b"key1"]
        assert key1_op.operation_type == "Modified"
        assert key1_op.old_value == b"value1"
        assert key1_op.new_value == b"value1_modified"

        key2_op = diff_map[b"key2"]
        assert key2_op.operation_type == "Removed"
        assert key2_op.value == b"value2"

        key3_op = diff_map[b"key3"]
        assert key3_op.operation_type == "Added"
        assert key3_op.value == b"value3"

    def test_diff_between_branches(self):
        """Test diff between two branches."""
        # Initialize store
        store = prollytree.VersionedKvStore(str(self.store_path))

        # Create initial data on main branch
        store.insert(b"shared", b"initial")
        store.insert(b"main_only", b"main_value")
        store.commit("Initial commit on main")

        # Create feature branch
        store.create_branch("feature")

        # Make changes on feature branch
        store.update(b"shared", b"feature_value")
        store.insert(b"feature_only", b"feature_data")
        store.delete(b"main_only")
        store.commit("Changes on feature branch")

        # Get diff between branches
        diffs = store.diff("main", "feature")

        # Verify diff results
        assert len(diffs) == 3

        diff_map = {diff.key: diff.operation for diff in diffs}

        # Check shared key was modified
        shared_op = diff_map[b"shared"]
        assert shared_op.operation_type == "Modified"
        assert shared_op.old_value == b"initial"
        assert shared_op.new_value == b"feature_value"

        # Check main_only was removed
        main_only_op = diff_map[b"main_only"]
        assert main_only_op.operation_type == "Removed"

        # Check feature_only was added
        feature_only_op = diff_map[b"feature_only"]
        assert feature_only_op.operation_type == "Added"

    def test_current_commit(self):
        """Test getting current commit ID."""
        # Initialize store
        store = prollytree.VersionedKvStore(str(self.store_path))

        # Create first commit
        store.insert(b"key1", b"value1")
        commit1 = store.commit("First commit")

        # Get current commit
        current = store.current_commit()
        assert current == commit1

        # Create second commit
        store.insert(b"key2", b"value2")
        commit2 = store.commit("Second commit")

        # Current commit should be updated
        current = store.current_commit()
        assert current == commit2

        # Test with branch operations
        store.create_branch("test-branch")
        store.insert(b"key3", b"value3")
        commit3 = store.commit("Third commit on branch")

        # Current commit should be updated
        current = store.current_commit()
        assert current == commit3

        # Checkout back to main branch
        store.checkout("main")
        current = store.current_commit()
        assert current == commit2

    def test_diff_with_no_changes(self):
        """Test diff when there are no changes."""
        # Initialize store
        store = prollytree.VersionedKvStore(str(self.store_path))

        # Create a commit
        store.insert(b"key1", b"value1")
        commit1 = store.commit("First commit")

        # Get diff between same commit
        diffs = store.diff(commit1, commit1)

        # Should be empty
        assert len(diffs) == 0

    def test_diff_representation(self):
        """Test string representation of diff objects."""
        # Initialize store
        store = prollytree.VersionedKvStore(str(self.store_path))

        # Create commits with changes
        store.insert(b"key1", b"value1")
        commit1 = store.commit("First")

        store.update(b"key1", b"value2")
        commit2 = store.commit("Second")

        # Get diff
        diffs = store.diff(commit1, commit2)

        # Check representation
        assert len(diffs) == 1
        diff = diffs[0]

        # Test __repr__ methods
        repr_str = repr(diff)
        assert "key1" in repr_str
        assert "Modified" in repr_str

        op_repr = repr(diff.operation)
        assert "Modified" in op_repr
        assert "old_size" in op_repr
        assert "new_size" in op_repr

    def test_get_commits_for_key_functionality(self):
        """Test the get_commits_for_key function works correctly."""
        # Initialize store
        store = prollytree.VersionedKvStore(str(self.store_path))

        # Create commits with changes to a specific key
        store.insert(b"tracked_key", b"value1")
        store.insert(b"other_key", b"other_value")
        commit1 = store.commit("First commit")

        store.update(b"tracked_key", b"value2")
        commit2 = store.commit("Second commit - tracked_key changed")

        store.insert(b"another_key", b"another_value")
        commit3 = store.commit("Third commit - no tracked_key change")

        # Test get_commits_for_key functionality
        commits_for_key = store.get_commits_for_key(b"tracked_key")

        # Should return 2 commits that modified tracked_key
        assert len(commits_for_key) == 2

        # Verify the commit IDs match what we expect
        commit_ids = [commit['id'] for commit in commits_for_key]
        assert commit2 in commit_ids  # Most recent change
        assert commit1 in commit_ids  # First commit with this key
        assert commit3 not in [c['id'] for c in commits_for_key]  # Third commit didn't touch tracked_key

        # Verify commits are in reverse chronological order (newest first)
        assert commits_for_key[0]['id'] == commit2  # Most recent first


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
