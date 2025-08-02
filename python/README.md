# ProllyTree Python Bindings

This directory contains Python bindings for the ProllyTree Rust library, providing a comprehensive toolkit for:

- **Probabilistic Trees**: High-performance prolly trees for efficient data storage and retrieval
- **AI Agent Memory Systems**: Multi-layered memory systems for intelligent agents
- **Versioned Key-Value Storage**: Git-backed versioned storage with branching and history

## Overview

ProllyTree combines B-trees and Merkle trees to provide both efficient data access and verifiable integrity, making it ideal for distributed systems and applications requiring data verification.

## Usage Examples

### Basic ProllyTree Operations

```python
from prollytree import ProllyTree, TreeConfig

# Create an in-memory tree
tree = ProllyTree(storage_type="memory")

# Insert key-value pairs
tree.insert(b"key1", b"value1")
tree.insert(b"key2", b"value2")

# Batch operations
items = [(b"key3", b"value3"), (b"key4", b"value4")]
tree.insert_batch(items)

# Find and update values
value = tree.find(b"key1")  # Returns b"value1"
tree.update(b"key1", b"new_value1")
tree.delete(b"key2")

# Tree properties and verification
print(f"Size: {tree.size()}, Depth: {tree.depth()}")
proof = tree.generate_proof(b"key3")
is_valid = tree.verify_proof(proof, b"key3", b"value3")

# File-based storage
config = TreeConfig(base=4, modulus=64)
file_tree = ProllyTree(storage_type="file", path="/tmp/my_tree", config=config)
```

### AI Agent Memory System

```python
import json
from prollytree import AgentMemorySystem, MemoryType

# Initialize agent memory system
memory = AgentMemorySystem("/path/to/memory", "agent_001")

# Short-term memory (conversations)
memory.store_conversation_turn(
    "thread_123",
    "user",
    "What's the weather like?",
    {"session": "morning", "platform": "chat"}
)

memory.store_conversation_turn(
    "thread_123",
    "assistant",
    "I'd be happy to help with weather information!"
)

# Retrieve conversation history
history = memory.get_conversation_history("thread_123", limit=10)

# Semantic memory (facts about entities)
memory.store_fact(
    "person",
    "john_doe",
    json.dumps({
        "name": "John Doe",
        "role": "Software Engineer",
        "location": "San Francisco"
    }),
    confidence=0.95,
    source="user_profile"
)

# Get facts about an entity
facts = memory.get_entity_facts("person", "john_doe")

# Procedural memory (task instructions)
memory.store_procedure(
    "development",
    "code_review",
    "How to conduct a code review",
    [
        json.dumps({"step": 1, "action": "Check code style and formatting"}),
        json.dumps({"step": 2, "action": "Review logic and algorithms"}),
        json.dumps({"step": 3, "action": "Test edge cases and error handling"})
    ],
    prerequisites=["git_access", "reviewer_permissions"],
    priority=2
)

# Memory management
checkpoint_id = memory.checkpoint("Saved conversation and facts")
optimization_report = memory.optimize()
```

### Versioned Key-Value Store

```python
from prollytree import VersionedKvStore, StorageBackend

# Initialize in a git repository subdirectory
store = VersionedKvStore("/path/to/git/repo/dataset")

# Basic key-value operations (staged until commit)
store.insert(b"user:1", b'{"name": "Alice", "age": 30}')
store.insert(b"user:2", b'{"name": "Bob", "age": 25}')
store.update(b"user:1", b'{"name": "Alice", "age": 31}')

# Check staging status
status = store.status()  # Shows added/modified/deleted keys
keys = store.list_keys()

# Commit changes with message
commit_hash = store.commit("Add initial user data")

# Branch operations
store.create_branch("feature-branch")
store.insert(b"feature:1", b"experimental_data")
store.commit("Add experimental feature")

# Switch branches
store.checkout("main")
print(f"Current branch: {store.current_branch()}")
print(f"Available branches: {store.list_branches()}")

# View commit history
history = store.log()
for commit in history[:5]:  # Last 5 commits
    print(f"{commit['id'][:8]} - {commit['message']}")
    print(f"  Author: {commit['author']}")

# Storage backend info
backend = store.storage_backend()  # Returns StorageBackend.Git
```

## Publishing to PyPI

### Configuration

Copy the example PyPI configuration:
```bash
cp .pypirc.example ~/.pypirc
```

Edit `~/.pypirc` and add your API tokens:
- Get TestPyPI token: https://test.pypi.org/manage/account/token/
- Get PyPI token: https://pypi.org/manage/account/token/

### Publishing

Test on TestPyPI first:
```bash
./publish_python.sh test
```

Publish to production PyPI:
```bash
./publish_python.sh prod
```

## Installation

```bash
pip install prollytree
```

## API Reference

### ProllyTree Classes

#### TreeConfig
Configuration for ProllyTree instances:
- `base`: Rolling hash base (default: 4)
- `modulus`: Rolling hash modulus (default: 64)
- `min_chunk_size`: Minimum chunk size (default: 1)
- `max_chunk_size`: Maximum chunk size (default: 4096)
- `pattern`: Chunk boundary pattern (default: 0)

#### ProllyTree
High-performance probabilistic tree with Merkle verification:

**Core Operations:**
- `insert(key: bytes, value: bytes)`: Insert key-value pair
- `find(key: bytes) -> Optional[bytes]`: Find value by key
- `update(key: bytes, value: bytes)`: Update existing key
- `delete(key: bytes)`: Delete key
- `insert_batch(items)`, `delete_batch(keys)`: Batch operations

**Properties & Verification:**
- `size() -> int`, `depth() -> int`: Tree metrics
- `get_root_hash() -> bytes`: Cryptographic root hash
- `generate_proof(key) -> bytes`: Create Merkle proof
- `verify_proof(proof, key, value) -> bool`: Verify proof
- `stats() -> Dict`: Detailed tree statistics

### Agent Memory System

#### AgentMemorySystem
Comprehensive memory system for AI agents with multiple memory types:

**Initialization:**
- `AgentMemorySystem(path: str, agent_id: str)`: Create new system
- `AgentMemorySystem.open(path: str, agent_id: str)`: Open existing

**Short-term Memory (Conversations):**
- `store_conversation_turn(thread_id, role, content, metadata=None) -> str`
- `get_conversation_history(thread_id, limit=None) -> List[Dict]`

**Semantic Memory (Facts & Knowledge):**
- `store_fact(entity_type, entity_id, facts_json, confidence, source) -> str`
- `get_entity_facts(entity_type, entity_id) -> List[Dict]`

**Procedural Memory (Instructions & Procedures):**
- `store_procedure(category, name, description, steps, prerequisites=None, priority=1) -> str`
- `get_procedures_by_category(category) -> List[Dict]`

**System Operations:**
- `checkpoint(message: str) -> str`: Create memory snapshot
- `optimize() -> Dict[str, int]`: Cleanup and consolidation

#### MemoryType
Enum for memory classification: `ShortTerm`, `Semantic`, `Episodic`, `Procedural`

### Versioned Key-Value Store

#### VersionedKvStore
Git-backed versioned storage with full branching support:

**Initialization:**
- `VersionedKvStore(path: str)`: Initialize new store
- `VersionedKvStore.open(path: str)`: Open existing store

**Basic Operations:**
- `insert(key: bytes, value: bytes)`: Stage insertion
- `get(key: bytes) -> Optional[bytes]`: Get current value
- `update(key: bytes, value: bytes) -> bool`: Stage update
- `delete(key: bytes) -> bool`: Stage deletion
- `list_keys() -> List[bytes]`: List all keys

**Git Operations:**
- `commit(message: str) -> str`: Commit staged changes
- `status() -> List[Tuple[bytes, str]]`: Show staging status
- `branch(name: str)`: Create branch
- `create_branch(name: str)`: Create and switch to branch
- `checkout(branch_or_commit: str)`: Switch branch/commit
- `current_branch() -> str`: Get current branch
- `list_branches() -> List[str]`: List all branches
- `log() -> List[Dict]`: Commit history

#### StorageBackend
Enum for storage types: `InMemory`, `File`, `Git`

## Requirements

- **VersionedKvStore**: Requires git repository (must be run within a git repo)
- **AgentMemorySystem**: Requires filesystem access for persistence
- **ProllyTree**: Works with memory, file, or custom storage backends

## Testing

Run the comprehensive test suite:

```bash
cd python/tests
python test_prollytree.py      # Basic ProllyTree functionality
python test_agent.py           # Agent memory system
python test_versioned_kv.py    # Versioned key-value store
```
