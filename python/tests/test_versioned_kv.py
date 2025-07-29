#!/usr/bin/env python3
"""Test script for VersionedKvStore Python bindings."""

import tempfile
import os
import subprocess
from prollytree import VersionedKvStore, StorageBackend

def test_versioned_kv_store():
    """Test the versioned key-value store functionality."""
    
    # Create a temporary directory and initialize a git repository
    with tempfile.TemporaryDirectory() as tmpdir:
        print(f"📁 Creating test in: {tmpdir}")
        
        # Initialize git repository
        subprocess.run(["git", "init"], cwd=tmpdir, check=True, capture_output=True)
        subprocess.run(["git", "config", "user.name", "Test User"], cwd=tmpdir, check=True)
        subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=tmpdir, check=True)
        
        # Create a subdirectory for our dataset
        dataset_dir = os.path.join(tmpdir, "dataset")  
        os.makedirs(dataset_dir)
        os.chdir(dataset_dir)
        
        print("✅ Git repository initialized")
        
        # Test 1: Initialize VersionedKvStore
        print("\n🧪 Test 1: Initialize VersionedKvStore")
        store = VersionedKvStore(dataset_dir)
        print(f"   Storage backend: {store.storage_backend()}")
        print(f"   Current branch: {store.current_branch()}")
        
        # Test 2: Basic key-value operations
        print("\n🧪 Test 2: Basic key-value operations")
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
        print("\n🧪 Test 3: List keys and status")
        keys = store.list_keys()
        print(f"   Keys: {[k.decode() for k in keys]}")
        
        status = store.status()
        print("   Status:")
        for key, status_str in status:
            print(f"   - {key.decode()}: {status_str}")
        
        # Test 4: Commit changes
        print("\n🧪 Test 4: Commit changes")
        commit_hash = store.commit("Add initial user data")
        print(f"   Commit hash: {commit_hash}")
        
        # Check status after commit
        status = store.status()
        print(f"   Status after commit: {len(status)} staged changes")
        
        # Test 5: Update and delete operations
        print("\n🧪 Test 5: Update and delete operations")
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
        print("\n🧪 Test 6: Branch operations")
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
        print("\n🧪 Test 7: Switch back to main")
        store.checkout("main")
        print(f"   Current branch: {store.current_branch()}")
        
        # Check if feature key exists (should not exist on main)
        feature = store.get(b"feature")
        print(f"   Feature key on main: {feature}")
        
        # Test 8: Commit history
        print("\n🧪 Test 8: Commit history")
        history = store.log()
        print(f"   Commit history ({len(history)} commits):")
        for i, commit in enumerate(history[:3]):  # Show first 3 commits
            print(f"   {i+1}. {commit['id'][:8]} - {commit['message']}")
            print(f"      Author: {commit['author']}")
            print(f"      Timestamp: {commit['timestamp']}")
            
        print("\n✅ All VersionedKvStore tests completed successfully!")


if __name__ == "__main__":
    test_versioned_kv_store()