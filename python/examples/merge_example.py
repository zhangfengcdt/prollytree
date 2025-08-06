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

"""
Example: Branch merging with conflict resolution in VersionedKvStore

This example demonstrates how to use the merge functionality in ProllyTree's
VersionedKvStore with different conflict resolution strategies.
"""

import tempfile
import subprocess
import os
import sys
import shutil

# Add the parent directory to the path so we can import prollytree
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from prollytree import VersionedKvStore, ConflictResolution, MergeConflict


def setup_example_repo():
    """Set up a temporary git repository for the example"""
    tmpdir = tempfile.mkdtemp(prefix="prollytree_merge_example_")
    print(f"📁 Created temporary directory: {tmpdir}")

    # Initialize git repository
    subprocess.run(['git', 'init'], cwd=tmpdir, check=True, capture_output=True)
    subprocess.run(['git', 'config', 'user.name', 'Example User'], cwd=tmpdir, check=True, capture_output=True)
    subprocess.run(['git', 'config', 'user.email', 'user@example.com'], cwd=tmpdir, check=True, capture_output=True)

    # Create data subdirectory
    data_dir = os.path.join(tmpdir, 'data')
    os.makedirs(data_dir, exist_ok=True)

    return tmpdir, data_dir


def demo_basic_merge():
    """Demonstrate basic merge without conflicts"""
    print("\n🔀 Demo: Basic merge without conflicts")
    print("=" * 50)

    tmpdir, data_dir = setup_example_repo()

    try:
        # Initialize the store
        store = VersionedKvStore(data_dir)

        # Create initial data on main branch
        print("📝 Setting up initial data on main branch...")
        store.insert(b"users:alice", b"Alice Smith")
        store.insert(b"users:bob", b"Bob Jones")
        store.insert(b"config:theme", b"light")
        store.commit("Initial user data")

        # Create and switch to feature branch
        print("🌿 Creating feature branch...")
        store.create_branch("add-user-charlie")

        # Add new user and update config on feature branch
        print("✍️  Making changes on feature branch...")
        store.insert(b"users:charlie", b"Charlie Brown")
        store.update(b"config:theme", b"dark")  # This will create a conflict later
        store.commit("Add Charlie and switch to dark theme")

        # Switch back to main and make different changes
        print("🔄 Switching back to main branch...")
        store.checkout("main")

        print("✍️  Making changes on main branch...")
        store.insert(b"users:diana", b"Diana Prince")
        store.commit("Add Diana")

        # Show status before merge
        print("\n📊 Status before merge:")
        print(f"Current branch: {store.current_branch()}")
        print("Users on main:", {k.decode(): v.decode() for k, v in
                               [(k, store.get(k)) for k in [b"users:alice", b"users:bob", b"users:diana"]]
                               if v})
        print(f"Theme: {store.get(b'config:theme').decode()}")

        # Perform merge
        print("\n🔀 Merging feature branch into main...")
        merge_commit = store.merge("add-user-charlie", ConflictResolution.TakeSource)
        print(f"✅ Merge successful! Commit: {merge_commit[:8]}")

        # Show final state
        print("\n📊 Final state after merge:")
        all_keys = store.list_keys()
        for key in sorted(all_keys):
            value = store.get(key)
            print(f"  {key.decode()}: {value.decode()}")

    finally:
        shutil.rmtree(tmpdir)
        print(f"🧹 Cleaned up {tmpdir}")


def demo_conflict_resolution():
    """Demonstrate different conflict resolution strategies"""
    print("\n⚔️  Demo: Conflict resolution strategies")
    print("=" * 50)

    for strategy_name, strategy in [
        ("IgnoreAll", ConflictResolution.IgnoreAll),
        ("TakeSource", ConflictResolution.TakeSource),
        ("TakeDestination", ConflictResolution.TakeDestination)
    ]:
        print(f"\n🛡️  Testing {strategy_name} strategy...")

        tmpdir, data_dir = setup_example_repo()

        try:
            store = VersionedKvStore(data_dir)

            # Set up conflict scenario
            store.insert(b"shared_key", b"initial_value")
            store.commit("Initial commit")

            # Feature branch changes
            store.create_branch("feature")
            store.update(b"shared_key", b"feature_value")
            store.commit("Feature change")

            # Main branch changes
            store.checkout("main")
            store.update(b"shared_key", b"main_value")
            store.commit("Main change")

            # Apply merge with strategy
            merge_commit = store.merge("feature", strategy)
            final_value = store.get(b"shared_key").decode()

            print(f"  Result with {strategy_name}: '{final_value}'")

        finally:
            shutil.rmtree(tmpdir)


def demo_conflict_detection():
    """Demonstrate conflict detection with try_merge"""
    print("\n🔍 Demo: Conflict detection with try_merge")
    print("=" * 50)

    tmpdir, data_dir = setup_example_repo()

    try:
        store = VersionedKvStore(data_dir)

        # Create conflict scenario
        store.insert(b"config:database_url", b"sqlite:///prod.db")
        store.insert(b"config:debug", b"false")
        store.commit("Production config")

        # Feature branch: development config
        store.create_branch("dev-config")
        store.update(b"config:database_url", b"sqlite:///dev.db")
        store.update(b"config:debug", b"true")
        store.insert(b"config:dev_tools", b"enabled")
        store.commit("Development configuration")

        # Main branch: staging config
        store.checkout("main")
        store.update(b"config:database_url", b"postgresql://staging-db")
        store.insert(b"config:cache", b"redis://cache-server")
        store.commit("Staging configuration")

        # Try merge to detect conflicts
        print("🔍 Checking for merge conflicts...")
        success, conflicts = store.try_merge("dev-config")

        if success:
            print("✅ No conflicts detected - merge would succeed")
        else:
            print(f"⚠️  Conflicts detected! Found {len(conflicts)} conflict(s):")
            for i, conflict in enumerate(conflicts, 1):
                print(f"\n  Conflict {i}: {conflict.key.decode()}")
                if conflict.base_value:
                    print(f"    Base:        '{conflict.base_value.decode()}'")
                if conflict.source_value:
                    print(f"    Source:      '{conflict.source_value.decode()}'")
                if conflict.destination_value:
                    print(f"    Destination: '{conflict.destination_value.decode()}'")

        print("\n💡 State remains unchanged after try_merge:")
        print(f"  database_url: {store.get(b'config:database_url').decode()}")
        print(f"  debug: {store.get(b'config:debug').decode()}")

    finally:
        shutil.rmtree(tmpdir)
        print(f"🧹 Cleaned up {tmpdir}")


def main():
    """Run all merge examples"""
    print("🌳 ProllyTree Merge Examples")
    print("=" * 50)

    print("This example demonstrates branch merging with conflict resolution")
    print("in ProllyTree's VersionedKvStore.")

    try:
        demo_basic_merge()
        demo_conflict_resolution()
        demo_conflict_detection()

        print("\n🎉 All examples completed successfully!")
        print("\nKey takeaways:")
        print("• Use store.merge(branch, strategy) to merge branches")
        print("• ConflictResolution.IgnoreAll keeps destination values")
        print("• ConflictResolution.TakeSource prefers source branch values")
        print("• ConflictResolution.TakeDestination keeps current branch values")
        print("• Use store.try_merge(branch) to detect conflicts without applying changes")

    except KeyboardInterrupt:
        print("\n⏸️  Example interrupted by user")
        sys.exit(1)
    except Exception as e:
        print(f"\n❌ Example failed: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
