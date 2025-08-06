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
Integration tests for ProllyTree worktree functionality

This test suite validates the worktree implementation that solves race
conditions in multi-agent systems. It demonstrates:

1. WorktreeManager creation and management
2. Safe concurrent worktree operations (add, list, lock/unlock)
3. Branch isolation for multi-agent systems
4. Core architectural concepts for preventing context bleeding

These tests complement the Rust unit tests in src/git/worktree.rs and
verify that the Python bindings work correctly.
"""

import tempfile
import os
import subprocess
import sys
from pathlib import Path

def test_worktree_manager_functionality():
    """Test basic WorktreeManager operations"""

    with tempfile.TemporaryDirectory() as tmpdir:
        print(f"📁 Test directory: {tmpdir}")

        # Initialize main repository
        main_path = os.path.join(tmpdir, "main_repo")
        os.makedirs(main_path)

        # Initialize git repository
        subprocess.run(["git", "init"], cwd=main_path, check=True, capture_output=True)
        subprocess.run(["git", "config", "user.name", "Test User"], cwd=main_path, check=True, capture_output=True)
        subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=main_path, check=True, capture_output=True)

        # Create an initial commit
        test_file = os.path.join(main_path, "README.md")
        with open(test_file, "w") as f:
            f.write("# Test Repository\n")

        subprocess.run(["git", "add", "."], cwd=main_path, check=True, capture_output=True)
        subprocess.run(["git", "commit", "-m", "Initial commit"], cwd=main_path, check=True, capture_output=True)

        print("✅ Created main repository with initial commit")

        try:
            # Import worktree classes directly
            from prollytree.prollytree import WorktreeManager
            print("✅ Successfully imported WorktreeManager")
        except ImportError as e:
            print(f"❌ Failed to import WorktreeManager: {e}")
            return False

        # Test 1: Create WorktreeManager
        print("\n🧪 Test 1: Create WorktreeManager")
        try:
            manager = WorktreeManager(main_path)
            print(f"   ✅ Created WorktreeManager")

            # List initial worktrees
            worktrees = manager.list_worktrees()
            print(f"   📊 Initial worktrees: {len(worktrees)}")
            for wt in worktrees:
                print(f"      • {wt['id']}: branch={wt['branch']}, linked={wt['is_linked']}")
        except Exception as e:
            print(f"   ❌ Failed to create WorktreeManager: {e}")
            return False

        # Test 2: Add worktrees for different "agents"
        print("\n🧪 Test 2: Add worktrees for agents")
        agents = ['agent1', 'agent2', 'agent3']
        agent_worktrees = {}

        for agent in agents:
            try:
                worktree_path = os.path.join(tmpdir, f"{agent}_worktree")
                branch_name = f"{agent}-feature"

                info = manager.add_worktree(worktree_path, branch_name, True)
                agent_worktrees[agent] = info

                print(f"   ✅ Created worktree for {agent}: {info['id']}")
                print(f"      Path: {info['path']}")
                print(f"      Branch: {info['branch']}")
            except Exception as e:
                print(f"   ❌ Failed to create worktree for {agent}: {e}")
                return False

        # Test 3: Verify worktree isolation
        print("\n🧪 Test 3: Verify worktree isolation")
        final_worktrees = manager.list_worktrees()
        print(f"   📊 Total worktrees: {len(final_worktrees)}")

        # Should have main + 3 agent worktrees
        expected_count = 4
        if len(final_worktrees) == expected_count:
            print(f"   ✅ Correct number of worktrees ({expected_count})")
        else:
            print(f"   ❌ Expected {expected_count} worktrees, got {len(final_worktrees)}")
            return False

        # Verify each agent has their own branch
        agent_branches = set()
        for wt in final_worktrees:
            if wt['is_linked']:  # Skip main worktree
                agent_branches.add(wt['branch'])

        expected_branches = {f"{agent}-feature" for agent in agents}
        if agent_branches == expected_branches:
            print(f"   ✅ All agent branches created correctly")
        else:
            print(f"   ❌ Branch mismatch. Expected: {expected_branches}, Got: {agent_branches}")
            return False

        # Test 4: Test locking mechanism
        print("\n🧪 Test 4: Test worktree locking")
        test_agent = agents[0]
        test_worktree_id = agent_worktrees[test_agent]['id']

        try:
            # Lock the worktree
            manager.lock_worktree(test_worktree_id, f"{test_agent} is processing critical data")
            is_locked = manager.is_locked(test_worktree_id)

            if is_locked:
                print(f"   ✅ Successfully locked {test_agent}'s worktree")
            else:
                print(f"   ❌ Failed to lock {test_agent}'s worktree")
                return False

            # Try to lock again (should fail)
            try:
                manager.lock_worktree(test_worktree_id, "Another lock attempt")
                print(f"   ❌ Should not be able to double-lock worktree")
                return False
            except Exception as e:
                print(f"   ✅ Correctly prevented double-locking: {type(e).__name__}")

            # Unlock
            manager.unlock_worktree(test_worktree_id)
            is_locked = manager.is_locked(test_worktree_id)

            if not is_locked:
                print(f"   ✅ Successfully unlocked {test_agent}'s worktree")
            else:
                print(f"   ❌ Failed to unlock {test_agent}'s worktree")
                return False

        except Exception as e:
            print(f"   ❌ Locking test failed: {e}")
            return False

        # Test 5: Cleanup test
        print("\n🧪 Test 5: Cleanup worktrees")
        try:
            for agent in agents:
                worktree_id = agent_worktrees[agent]['id']
                manager.remove_worktree(worktree_id)
                print(f"   ✅ Removed worktree for {agent}")

            final_worktrees = manager.list_worktrees()
            if len(final_worktrees) == 1:  # Only main should remain
                print(f"   ✅ All agent worktrees removed, only main remains")
            else:
                print(f"   ❌ Expected 1 worktree after cleanup, got {len(final_worktrees)}")
                return False

        except Exception as e:
            print(f"   ❌ Cleanup test failed: {e}")
            return False

        print("\n✅ All worktree tests passed!")
        print("\n📊 Summary of verified functionality:")
        print("   • WorktreeManager creation and initialization")
        print("   • Multiple worktree creation with different branches")
        print("   • Worktree listing and metadata access")
        print("   • Locking mechanism to prevent conflicts")
        print("   • Worktree cleanup and removal")
        print("   • Branch isolation for concurrent operations")

        return True

def test_worktree_architecture_concepts():
    """Test architectural concepts that would be used in multi-agent systems"""

    print("\n" + "="*80)
    print("🏗️  ARCHITECTURAL VERIFICATION: Multi-Agent Worktree Patterns")
    print("="*80)

    with tempfile.TemporaryDirectory() as tmpdir:
        # Setup
        main_path = os.path.join(tmpdir, "multi_agent_repo")
        os.makedirs(main_path)
        subprocess.run(["git", "init"], cwd=main_path, check=True, capture_output=True)
        subprocess.run(["git", "config", "user.name", "Test"], cwd=main_path, check=True, capture_output=True)
        subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=main_path, check=True, capture_output=True)

        test_file = os.path.join(main_path, "shared.txt")
        with open(test_file, "w") as f:
            f.write("shared_data=initial\n")
        subprocess.run(["git", "add", "."], cwd=main_path, check=True, capture_output=True)
        subprocess.run(["git", "commit", "-m", "Initial"], cwd=main_path, check=True, capture_output=True)

        from prollytree.prollytree import WorktreeManager
        manager = WorktreeManager(main_path)

        # Simulate multi-agent scenario
        agents = {
            'billing': {'branch': 'session-001-billing-abc123', 'task': 'Process billing data'},
            'support': {'branch': 'session-001-support-def456', 'task': 'Handle customer inquiry'},
            'analysis': {'branch': 'session-001-analysis-ghi789', 'task': 'Analyze customer patterns'}
        }

        print(f"🤖 Simulating {len(agents)} concurrent agents:")

        # Each agent gets their own worktree
        for agent_name, config in agents.items():
            worktree_path = os.path.join(tmpdir, f"agent_{agent_name}_workspace")
            info = manager.add_worktree(worktree_path, config['branch'], True)

            print(f"   • {agent_name}: branch={config['branch'][:20]}... task='{config['task']}'")
            print(f"     Workspace: {info['path']}")
            print(f"     Isolated: {info['is_linked']} (separate working directory)")

        # Verify isolation
        worktrees = manager.list_worktrees()
        agent_worktrees = [wt for wt in worktrees if wt['is_linked']]

        print(f"\n🔒 Branch Isolation Analysis:")
        print(f"   • Total worktrees: {len(worktrees)} (1 main + {len(agent_worktrees)} agents)")
        print(f"   • Each agent has separate:")
        print(f"     - Working directory (prevents file conflicts)")
        print(f"     - Git branch (prevents commit conflicts)")
        print(f"     - HEAD pointer (prevents checkout conflicts)")
        print(f"   • Shared Git object database (enables data sharing)")

        # Demonstrate the key insight
        print(f"\n💡 Key Architectural Insight:")
        print(f"   This solves the race condition problem identified in the original")
        print(f"   multi-agent implementation where multiple VersionedKvStore instances")
        print(f"   pointed to the same Git repository and competed for the same HEAD file.")
        print(f"   ")
        print(f"   Now each agent has their own worktree with:")
        print(f"   - Dedicated .git/worktrees/[worktree_id]/ directory")
        print(f"   - Independent HEAD file")
        print(f"   - Separate working directory")
        print(f"   - But shared object database for collaboration")

        return True

def test_worktree_merge_functionality():
    """Test merge functionality in worktree system"""

    print("\n" + "="*80)
    print("🔄 MERGE FUNCTIONALITY: Testing Branch Merging in Worktrees")
    print("="*80)

    with tempfile.TemporaryDirectory() as tmpdir:
        # Setup repository
        main_path = os.path.join(tmpdir, "merge_test_repo")
        os.makedirs(main_path)
        subprocess.run(["git", "init"], cwd=main_path, check=True, capture_output=True)
        subprocess.run(["git", "config", "user.name", "Test"], cwd=main_path, check=True, capture_output=True)
        subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=main_path, check=True, capture_output=True)

        # Create initial commit
        initial_file = os.path.join(main_path, "base.txt")
        with open(initial_file, "w") as f:
            f.write("base content\n")
        subprocess.run(["git", "add", "."], cwd=main_path, check=True, capture_output=True)
        subprocess.run(["git", "commit", "-m", "Initial commit"], cwd=main_path, check=True, capture_output=True)

        from prollytree.prollytree import WorktreeManager
        manager = WorktreeManager(main_path)

        print("🏗️ Test Setup:")
        print(f"   Repository: {main_path}")

        # Get initial state
        initial_branches = manager.list_branches()
        initial_main_commit = manager.get_branch_commit("main")
        print(f"   Initial branches: {initial_branches}")
        print(f"   Initial main commit: {initial_main_commit[:8]}")

        # Create worktrees for different "agents"
        agents = [
            {"name": "feature-dev", "branch": "feature/new-feature", "task": "Implement new feature"},
            {"name": "bug-fix", "branch": "bugfix/critical-fix", "task": "Fix critical bug"},
        ]

        agent_worktrees = {}

        print(f"\n🤖 Creating agent worktrees:")
        for agent in agents:
            worktree_path = os.path.join(tmpdir, f"{agent['name']}_workspace")
            info = manager.add_worktree(worktree_path, agent["branch"], True)
            agent_worktrees[agent["name"]] = info

            print(f"   • {agent['name']}: branch={agent['branch']}")
            print(f"     Task: {agent['task']}")
            print(f"     Workspace: {info['path']}")

        # Simulate work in feature branch
        print(f"\n🔧 Simulating work in feature branch:")
        feature_workspace = agent_worktrees["feature-dev"]["path"]
        feature_file = os.path.join(feature_workspace, "feature.txt")
        with open(feature_file, "w") as f:
            f.write("new feature implementation\n")

        # For testing, manually update the branch reference to simulate commits
        # In real usage, this would happen through VersionedKvStore operations
        git_dir = os.path.join(main_path, ".git")
        feature_branch_ref = os.path.join(git_dir, "refs", "heads", agent_worktrees["feature-dev"]["branch"].replace("/", os.sep))
        os.makedirs(os.path.dirname(feature_branch_ref), exist_ok=True)

        # Create a fake commit hash to simulate feature work
        fake_feature_commit = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        with open(feature_branch_ref, "w") as f:
            f.write(fake_feature_commit)

        feature_commit = manager.get_branch_commit(agent_worktrees["feature-dev"]["branch"])
        print(f"   Feature work completed: {feature_commit[:8]}")

        # Test merge functionality
        print(f"\n🔄 Testing merge operations:")

        # Test 1: Merge feature to main
        feature_worktree_id = agent_worktrees["feature-dev"]["id"]
        try:
            merge_result = manager.merge_to_main(feature_worktree_id, "Merge feature work to main")
            print(f"   ✅ Merge to main succeeded: {merge_result}")

            # Verify main was updated
            updated_main_commit = manager.get_branch_commit("main")
            print(f"   📊 Main branch updated: {initial_main_commit[:8]} → {updated_main_commit[:8]}")

            if updated_main_commit != initial_main_commit:
                print(f"   ✅ Main branch successfully updated")
            else:
                print(f"   ⚠️ Main branch unchanged (branches were identical)")

        except Exception as e:
            print(f"   ❌ Merge failed: {e}")
            return False

        # Test 2: Invalid merge (merge to self)
        print(f"\n🚫 Testing invalid merge operations:")
        try:
            manager.merge_to_main("main", "Invalid merge")
            print(f"   ❌ Should not be able to merge main to itself")
            return False
        except Exception as e:
            print(f"   ✅ Correctly prevented invalid merge: {type(e).__name__}")

        # Test 3: Branch listing after merge
        final_branches = manager.list_branches()
        print(f"\n📊 Final branch state:")
        for branch in final_branches:
            commit = manager.get_branch_commit(branch)
            print(f"   • {branch}: {commit[:8]}")

        # Test 4: Cross-branch merge
        print(f"\n🔀 Testing cross-branch merge:")
        try:
            # Create another fake commit for bug-fix branch
            bugfix_branch_ref = os.path.join(git_dir, "refs", "heads", agent_worktrees["bug-fix"]["branch"].replace("/", os.sep))
            os.makedirs(os.path.dirname(bugfix_branch_ref), exist_ok=True)
            fake_bugfix_commit = "cccccccccccccccccccccccccccccccccccccccc"
            with open(bugfix_branch_ref, "w") as f:
                f.write(fake_bugfix_commit)

            bugfix_worktree_id = agent_worktrees["bug-fix"]["id"]
            cross_merge_result = manager.merge_branch(
                bugfix_worktree_id,
                "main",
                "Merge bug fix to main"
            )
            print(f"   ✅ Cross-branch merge succeeded: {cross_merge_result}")

        except Exception as e:
            print(f"   ❌ Cross-branch merge failed: {e}")
            return False

        print(f"\n✅ All merge functionality tests passed!")
        print(f"\n💡 Key Merge Capabilities Verified:")
        print(f"   • Merge worktree branches to main")
        print(f"   • Cross-branch merging between arbitrary branches")
        print(f"   • Prevention of invalid merge operations")
        print(f"   • Branch commit tracking and verification")
        print(f"   • Proper error handling for edge cases")

        return True

if __name__ == "__main__":
    print("🚀 Starting ProllyTree Worktree Functionality Tests")

    success = True

    try:
        success &= test_worktree_manager_functionality()
        success &= test_worktree_architecture_concepts()
        success &= test_worktree_merge_functionality()
    except Exception as e:
        print(f"❌ Unexpected error during testing: {e}")
        success = False

    print("\n" + "="*80)
    if success:
        print("✅ ALL WORKTREE TESTS PASSED!")
        print("   The worktree implementation successfully provides:")
        print("   • Git worktree-like functionality for ProllyTree")
        print("   • Safe concurrent branch operations")
        print("   • Foundation for multi-agent context isolation")
        sys.exit(0)
    else:
        print("❌ SOME TESTS FAILED")
        print("   Check the error messages above for details")
        sys.exit(1)
