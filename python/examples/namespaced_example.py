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
Example: NamespacedKvStore - multiple isolated KV namespaces in one git repo.

NamespacedKvStore is the multi-tree counterpart of VersionedKvStore. Every
namespace is backed by its own prolly tree, but they all share one git history,
so commits, branches, and merges span every namespace atomically.

This example walks through:
  1. Creating a store and writing into multiple namespaces.
  2. Listing namespaces and querying per-namespace state.
  3. Branching + per-namespace commits.
  4. Switching branches and observing the per-namespace data flip.
"""

import os
import shutil
import subprocess
import sys
import tempfile

from prollytree import NamespacedKvStore


def setup_example_repo():
    """Create a temp git repo + dataset directory the way the store expects."""
    tmpdir = tempfile.mkdtemp(prefix="prollytree_ns_example_")
    print(f"Created temporary directory: {tmpdir}")

    subprocess.run(["git", "init"], cwd=tmpdir, check=True, capture_output=True)
    subprocess.run(
        ["git", "config", "user.name", "Example User"],
        cwd=tmpdir,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "config", "user.email", "user@example.com"],
        cwd=tmpdir,
        check=True,
        capture_output=True,
    )

    dataset_dir = os.path.join(tmpdir, "dataset")
    os.makedirs(dataset_dir, exist_ok=True)
    return tmpdir, dataset_dir


def demo_multiple_namespaces():
    """Show how to write into separate namespaces in the same store."""
    print("\nDemo 1: Two namespaces, one store, one commit")
    print("=" * 60)

    tmpdir, dataset_dir = setup_example_repo()
    try:
        store = NamespacedKvStore(dataset_dir)

        # "users" namespace holds account records.
        store.ns_insert("users", b"u:alice", b"Alice <alice@example.com>")
        store.ns_insert("users", b"u:bob", b"Bob <bob@example.com>")

        # "settings" namespace holds configuration. Same keys would be fine
        # here because each namespace has its own tree.
        store.ns_insert("settings", b"theme", b"dark")
        store.ns_insert("settings", b"locale", b"en_US")

        commit_id = store.commit("seed users + settings")
        print(f"Single commit covering both namespaces: {commit_id[:10]}")
        print(f"Namespaces in store: {store.list_namespaces()}")

        # Per-namespace reads.
        print(f"\nusers/u:alice = {store.ns_get('users', b'u:alice')!r}")
        print(f"settings/theme = {store.ns_get('settings', b'theme')!r}")

        # ns_list_keys gives the key set inside one namespace.
        print(f"\nusers keys: {store.ns_list_keys('users')}")
        print(f"settings keys: {store.ns_list_keys('settings')}")

        # Independent root hashes — useful for change detection.
        print(f"\nusers root hash: {store.get_namespace_root_hash('users')[:8].hex()}...")
        print(f"settings root hash: {store.get_namespace_root_hash('settings')[:8].hex()}...")
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


def demo_branching_per_namespace():
    """Show that branches isolate writes across every namespace."""
    print("\nDemo 2: Branching is store-wide; both namespaces follow")
    print("=" * 60)

    tmpdir, dataset_dir = setup_example_repo()
    try:
        store = NamespacedKvStore(dataset_dir)

        # Seed on main.
        store.ns_insert("users", b"u:alice", b"Alice")
        store.ns_insert("settings", b"theme", b"dark")
        store.commit("seed on main")
        print(f"main branch seeded; current branch = {store.current_branch}")

        # Create + switch to an experiment branch and diverge BOTH namespaces.
        store.branch("experiment")
        store.ns_insert("users", b"u:carol", b"Carol")
        store.ns_insert("settings", b"theme", b"light")
        store.commit("experiment: add carol, switch to light theme")
        print(f"experiment branch: users = {store.ns_list_keys('users')}")
        print(f"experiment branch: theme = {store.ns_get('settings', b'theme')!r}")

        # Switch back to main — both namespaces snap back to the pre-experiment state.
        store.checkout("main")
        print(f"\nback on main; users = {store.ns_list_keys('users')}")
        print(f"main branch theme = {store.ns_get('settings', b'theme')!r}")
        print(f"main does NOT see u:carol: {store.ns_get('users', b'u:carol')!r}")
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


def demo_isolation_between_namespaces():
    """Same key in two namespaces resolves to independent values."""
    print("\nDemo 3: Namespaces are fully isolated")
    print("=" * 60)

    tmpdir, dataset_dir = setup_example_repo()
    try:
        store = NamespacedKvStore(dataset_dir)

        # Same logical key 'name', different meaning per namespace.
        store.ns_insert("users", b"name", b"Alice")
        store.ns_insert("products", b"name", b"Widget")
        store.commit("collision-free across namespaces")

        print(f"users/name    = {store.ns_get('users', b'name')!r}")
        print(f"products/name = {store.ns_get('products', b'name')!r}")
        print("Each namespace owns its own key space — no collision.")
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


def main():
    print("NamespacedKvStore example")
    print("=" * 60)
    print("Multiple isolated prolly trees in one git-versioned store.")

    try:
        demo_multiple_namespaces()
        demo_branching_per_namespace()
        demo_isolation_between_namespaces()
        print("\nAll demos completed successfully.")
        print("\nKey takeaways:")
        print("- One NamespacedKvStore can hold many independent prolly trees.")
        print("- ns_insert / ns_get / ns_delete take an extra namespace argument.")
        print("- A single commit() lands every dirty namespace atomically.")
        print("- branch + checkout flip every namespace at once.")
        print("- For text search on a namespace, see text_index_example.py.")
    except KeyboardInterrupt:
        print("\nExample interrupted by user.")
        sys.exit(1)
    except Exception as exc:  # pragma: no cover - illustrative only
        print(f"\nExample failed: {exc}")
        import traceback

        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
