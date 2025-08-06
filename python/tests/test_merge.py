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

"""
Test merge functionality in VersionedKvStore
"""

import tempfile
import subprocess
import os
from prollytree import VersionedKvStore, ConflictResolution, MergeConflict


def setup_git_repo(tmpdir):
    """Setup git repository for testing"""
    # Initialize git in the root directory
    subprocess.run(['git', 'init'], cwd=tmpdir, check=True, capture_output=True)
    subprocess.run(['git', 'config', 'user.name', 'Test'], cwd=tmpdir, check=True, capture_output=True)
    subprocess.run(['git', 'config', 'user.email', 'test@test.com'], cwd=tmpdir, check=True, capture_output=True)

    # Create a subdirectory for the data store
    data_dir = os.path.join(tmpdir, 'data')
    os.makedirs(data_dir, exist_ok=True)

    return data_dir


def test_merge_no_conflicts():
    """Test merging branches with no conflicts"""
    with tempfile.TemporaryDirectory() as tmpdir:
        data_dir = setup_git_repo(tmpdir)

        # Initialize store
        store = VersionedKvStore(data_dir)

        # Add initial data
        store.insert(b"shared", b"initial_value")
        store.insert(b"key1", b"value1")
        store.commit("Initial commit")

        # Create feature branch
        store.create_branch("feature")

        # Make changes on feature branch
        store.insert(b"feature_key", b"feature_value")
        store.update(b"shared", b"feature_value")
        store.commit("Feature branch changes")

        # Switch back to main
        store.checkout("main")

        # Make different changes on main
        store.insert(b"main_key", b"main_value")
        store.commit("Main branch changes")

        # Merge feature into main
        merge_commit = store.merge("feature")
        assert merge_commit is not None

        # Verify merged state
        assert store.get(b"shared") == b"feature_value"  # Feature change applied
        assert store.get(b"feature_key") == b"feature_value"  # Feature addition applied
        assert store.get(b"main_key") == b"main_value"  # Main addition preserved
        assert store.get(b"key1") == b"value1"  # Unchanged key preserved


def test_merge_with_conflicts_ignore_all():
    """Test merging with conflicts using IgnoreAll resolution"""
    with tempfile.TemporaryDirectory() as tmpdir:
        data_dir = setup_git_repo(tmpdir)
        store = VersionedKvStore(data_dir)

        # Add initial data
        store.insert(b"conflict_key", b"initial_value")
        store.commit("Initial commit")

        # Create feature branch
        store.create_branch("feature")

        # Change on feature branch
        store.update(b"conflict_key", b"feature_value")
        store.commit("Feature change")

        # Switch back to main
        store.checkout("main")

        # Different change on main
        store.update(b"conflict_key", b"main_value")
        store.commit("Main change")

        # Merge with IgnoreAll (default) - should keep destination (main) value
        merge_commit = store.merge("feature", ConflictResolution.IgnoreAll)
        assert merge_commit is not None

        # Should keep main value due to IgnoreAll
        assert store.get(b"conflict_key") == b"main_value"


def test_merge_with_conflicts_take_source():
    """Test merging with conflicts using TakeSource resolution"""
    with tempfile.TemporaryDirectory() as tmpdir:
        data_dir = setup_git_repo(tmpdir)
        store = VersionedKvStore(data_dir)

        # Add initial data
        store.insert(b"conflict_key", b"initial_value")
        store.commit("Initial commit")

        # Create feature branch
        store.create_branch("feature")

        # Change on feature branch
        store.update(b"conflict_key", b"feature_value")
        store.commit("Feature change")

        # Switch back to main
        store.checkout("main")

        # Different change on main
        store.update(b"conflict_key", b"main_value")
        store.commit("Main change")

        # Merge with TakeSource - should take feature value
        merge_commit = store.merge("feature", ConflictResolution.TakeSource)
        assert merge_commit is not None

        # Should take feature value
        assert store.get(b"conflict_key") == b"feature_value"


def test_try_merge_basic():
    """Test try_merge functionality"""
    with tempfile.TemporaryDirectory() as tmpdir:
        data_dir = setup_git_repo(tmpdir)
        store = VersionedKvStore(data_dir)

        # Add initial data
        store.insert(b"conflict_key", b"initial_value")
        store.commit("Initial commit")

        # Create feature branch
        store.create_branch("feature")

        # Change on feature branch
        store.update(b"conflict_key", b"feature_value")
        store.commit("Feature change")

        # Switch back to main
        store.checkout("main")

        # Different change on main
        store.update(b"conflict_key", b"main_value")
        store.commit("Main change")

        # Try merge to detect conflicts
        success, conflicts = store.try_merge("feature")

        # Should have conflicts
        assert success is False
        assert len(conflicts) > 0

        # State should be unchanged
        assert store.get(b"conflict_key") == b"main_value"


if __name__ == "__main__":
    try:
        test_merge_no_conflicts()
        print("✅ test_merge_no_conflicts passed")

        test_merge_with_conflicts_ignore_all()
        print("✅ test_merge_with_conflicts_ignore_all passed")

        test_merge_with_conflicts_take_source()
        print("✅ test_merge_with_conflicts_take_source passed")

        test_try_merge_basic()
        print("✅ test_try_merge_basic passed")

        print("\n🎉 All merge tests passed!")

    except Exception as e:
        print(f"❌ Test failed: {e}")
        import traceback
        traceback.print_exc()
