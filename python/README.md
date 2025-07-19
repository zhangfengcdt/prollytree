# ProllyTree Python Bindings

This directory contains Python bindings for the ProllyTree Rust library, providing a Pythonic interface to the probabilistic tree data structure.

## Building the Python Package

### Prerequisites

- Rust toolchain (1.70 or later)
- Python 3.8 or later
- `maturin` build tool

### Installation

1. Install maturin:
```bash
pip install maturin
```

2. Build and install the package in development mode:
```bash
cd /path/to/prollytree
maturin develop --features python
```

3. Or build a wheel for distribution:
```bash
maturin build --release --features python
```

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

## Running Tests

```bash
cd python
python -m pytest tests/
```

## Running Examples

```bash
cd python
python examples/basic_usage.py
```

## Publishing to PyPI

### Prerequisites

1. **Get API Tokens**:
   - TestPyPI: https://test.pypi.org/manage/account/token/
   - PyPI: https://pypi.org/manage/account/token/

2. **Set Environment Variables**:
   ```bash
   export MATURIN_PYPI_TOKEN="pypi-your-token-here"
   # or for TestPyPI
   export TEST_PYPI_API_TOKEN="pypi-your-test-token-here"
   ```

### Manual Publishing

1. **Test on TestPyPI first**:
   ```bash
   ./publish_python.sh test
   ```

2. **Publish to production PyPI**:
   ```bash
   ./publish_python.sh prod
   ```

### Automated Publishing (GitHub Actions)

The repository includes a GitHub Actions workflow that automatically builds and publishes to PyPI when you push a version tag:

```bash
# Create and push a version tag
git tag v0.2.1
git push origin v0.2.1
```

**Setup Required**:
1. Add `PYPI_API_TOKEN` to GitHub repository secrets
2. Configure PyPI trusted publishing (recommended) or use API tokens

### Version Management

Update version in `pyproject.toml` before publishing:
```toml
[project]
version = "0.2.1"  # Update this
```