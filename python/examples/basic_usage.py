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
Basic usage example for ProllyTree Python bindings
"""

from prollytree import ProllyTree, TreeConfig

def main():
    # Create a new in-memory tree with default configuration
    print("Creating in-memory ProllyTree...")
    tree = ProllyTree(storage_type="memory")
    
    # Insert some key-value pairs
    print("\nInserting key-value pairs...")
    tree.insert(b"key1", b"value1")
    tree.insert(b"key2", b"value2")
    tree.insert(b"key3", b"value3")
    
    # Batch insert
    print("Batch inserting...")
    batch_items = [
        (b"key4", b"value4"),
        (b"key5", b"value5"),
        (b"key6", b"value6"),
    ]
    tree.insert_batch(batch_items)
    
    # Find values
    print("\nFinding values...")
    value = tree.find(b"key1")
    print(f"key1 -> {value.decode() if value else 'Not found'}")
    
    value = tree.find(b"key5")
    print(f"key5 -> {value.decode() if value else 'Not found'}")
    
    # Update a value
    print("\nUpdating key2...")
    tree.update(b"key2", b"updated_value2")
    value = tree.find(b"key2")
    print(f"key2 -> {value.decode() if value else 'Not found'}")
    
    # Get tree statistics
    print(f"\nTree size: {tree.size()}")
    print(f"Tree depth: {tree.depth()}")
    print(f"Root hash: {tree.get_root_hash().hex()}")
    
    stats = tree.stats()
    print(f"Tree stats: {stats}")
    
    # Generate and verify proof
    print("\nGenerating Merkle proof for key3...")
    proof = tree.generate_proof(b"key3")
    is_valid = tree.verify_proof(proof, b"key3", b"value3")
    print(f"Proof valid: {is_valid}")
    
    # Delete a key
    print("\nDeleting key4...")
    tree.delete(b"key4")
    value = tree.find(b"key4")
    print(f"key4 after deletion: {value.decode() if value else 'Not found'}")
    
    # Create another tree for diff comparison
    print("\nCreating second tree for comparison...")
    tree2 = ProllyTree(storage_type="memory")
    tree2.insert(b"key1", b"value1")
    tree2.insert(b"key2", b"different_value2")
    tree2.insert(b"key7", b"value7")
    
    # Compare trees - shows changes from tree to tree2
    print("\nComparing trees...")
    diff = tree.diff(tree2)
    
    print("Added in tree2:", {k.decode(): v.decode() for k, v in diff["added"].items()})
    print("Removed from tree:", {k.decode(): v.decode() for k, v in diff["removed"].items()})
    print("Modified keys:")
    for k, changes in diff["modified"].items():
        print(f"  {k.decode()}: {changes['old'].decode()} -> {changes['new'].decode()}")
    
    # File-based storage example
    print("\n\nCreating file-based ProllyTree...")
    config = TreeConfig(base=4, modulus=64, min_chunk_size=1, max_chunk_size=4096)
    file_tree = ProllyTree(storage_type="file", path="/tmp/prolly_tree_test", config=config)
    
    file_tree.insert(b"persistent_key", b"persistent_value")
    file_tree.save_config()
    print("File-based tree created and saved.")
    
    # Tree traversal
    print("\nTree structure:")
    print(tree.traverse())

if __name__ == "__main__":
    main()