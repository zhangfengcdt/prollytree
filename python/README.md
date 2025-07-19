# ProllyTree Python Bindings

This directory contains Python bindings for the ProllyTree Rust library, providing a Pythonic interface to the probabilistic tree data structure.

## Usage Example

```python
from prollytree import ProllyTree, TreeConfig

# Create an in-memory tree
tree = ProllyTree(storage_type="memory")

# Insert key-value pairs
tree.insert(b"key1", b"value1")
tree.insert(b"key2", b"value2")

# Batch insert
items = [(b"key3", b"value3"), (b"key4", b"value4")]
tree.insert_batch(items)

# Find values
value = tree.find(b"key1")  # Returns b"value1"

# Update a value
tree.update(b"key1", b"new_value1")

# Delete keys
tree.delete(b"key2")

# Get tree properties
print(f"Size: {tree.size()}")
print(f"Depth: {tree.depth()}")
print(f"Root hash: {tree.get_root_hash().hex()}")

# Generate and verify Merkle proofs
proof = tree.generate_proof(b"key3")
is_valid = tree.verify_proof(proof, b"key3", b"value3")

# Compare trees
tree2 = ProllyTree()
tree2.insert(b"key1", b"different_value")
diff = tree.diff(tree2)

# File-based storage
config = TreeConfig(base=4, modulus=64)
file_tree = ProllyTree(storage_type="file", path="/tmp/my_tree", config=config)
file_tree.insert(b"persistent_key", b"persistent_value")
file_tree.save_config()
```

## API Reference

### TreeConfig

Configuration class for ProllyTree:

- `base`: Base for the rolling hash (default: 4)
- `modulus`: Modulus for the rolling hash (default: 64)
- `min_chunk_size`: Minimum chunk size (default: 1)
- `max_chunk_size`: Maximum chunk size (default: 4096)
- `pattern`: Pattern for chunk boundaries (default: 0)

### ProllyTree

Main tree class with the following methods:

- `__init__(storage_type="memory", path=None, config=None)`: Create a new tree
- `insert(key: bytes, value: bytes)`: Insert a key-value pair
- `insert_batch(items: List[Tuple[bytes, bytes]])`: Batch insert
- `find(key: bytes) -> Optional[bytes]`: Find a value by key
- `update(key: bytes, value: bytes)`: Update an existing key
- `delete(key: bytes)`: Delete a key
- `delete_batch(keys: List[bytes])`: Batch delete
- `size() -> int`: Get number of key-value pairs
- `depth() -> int`: Get tree depth
- `get_root_hash() -> bytes`: Get the root hash
- `stats() -> Dict[str, int]`: Get tree statistics
- `generate_proof(key: bytes) -> bytes`: Generate a Merkle proof
- `verify_proof(proof: bytes, key: bytes, expected_value: Optional[bytes]) -> bool`: Verify a proof
- `diff(other: ProllyTree) -> Dict`: Compare two trees
- `traverse() -> str`: Get string representation of tree structure
- `save_config()`: Save tree configuration to storage
