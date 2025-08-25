# ProllyTree Python Bindings

[![Documentation](https://img.shields.io/badge/docs-read%20the%20docs-blue)](https://prollytree.readthedocs.io/en/latest/)
[![PyPI](https://img.shields.io/pypi/v/prollytree)](https://pypi.org/project/prollytree/)

Python bindings for ProllyTree - a probabilistic tree data structure that combines B-trees and Merkle trees for efficient, verifiable data storage.

## 🚀 Quick Start

### Installation

```bash
pip install prollytree
```

### Basic Usage

```python
from prollytree import ProllyTree

# Create a tree and insert data
tree = ProllyTree()
tree.insert(b"hello", b"world")
value = tree.find(b"hello")  # Returns b"world"
```

## 📚 Documentation

**👉 [Complete Documentation](https://prollytree.readthedocs.io/en/latest/)**

The full documentation includes:
- [Quickstart Guide](https://prollytree.readthedocs.io/en/latest/quickstart.html)
- [API Reference](https://prollytree.readthedocs.io/en/latest/api.html)
- [Examples](https://prollytree.readthedocs.io/en/latest/examples.html)
- [Advanced Usage](https://prollytree.readthedocs.io/en/latest/advanced.html)

## ✨ Features

- **🌳 Probabilistic Trees** - High-performance data storage with automatic balancing
- **🤖 AI Agent Memory** - Multi-layered memory systems for intelligent agents
- **📚 Versioned Storage** - Git-like version control for key-value data
- **🔐 Cryptographic Verification** - Merkle proofs for data integrity across trees and versioned storage
- **⚡ SQL Queries** - Query your data using SQL syntax

## 🔥 Key Use Cases

### Probabilistic Trees
```python
from prollytree import ProllyTree

tree = ProllyTree()
tree.insert(b"user:123", b"Alice")
tree.insert(b"user:456", b"Bob")

# Cryptographic verification
proof = tree.generate_proof(b"user:123")
is_valid = tree.verify_proof(proof, b"user:123", b"Alice")
```

### AI Agent Memory
```python
from prollytree import AgentMemorySystem

memory = AgentMemorySystem("./agent_memory", "agent_001")

# Store conversation
memory.store_conversation_turn("chat_123", "user", "Hello!")
memory.store_conversation_turn("chat_123", "assistant", "Hi there!")

# Store facts
memory.store_fact("person", "john", '{"name": "John", "age": 30}',
                  confidence=0.95, source="profile")
```

### Versioned Storage
```python
from prollytree import VersionedKvStore

store = VersionedKvStore("./data")
store.insert(b"config", b"production_settings")
commit_id = store.commit("Add production config")

# Branch and experiment
store.create_branch("experiment")
store.insert(b"feature", b"experimental_data")
store.commit("Add experimental feature")

# Cryptographic verification on versioned data
proof = store.generate_proof(b"config")
is_valid = store.verify_proof(proof, b"config", b"production_settings")
```

### SQL Queries
```python
from prollytree import ProllySQLStore

sql_store = ProllySQLStore("./database")
sql_store.execute("CREATE TABLE users (id INT, name TEXT)")
sql_store.execute("INSERT INTO users VALUES (1, 'Alice')")
results = sql_store.execute("SELECT * FROM users WHERE name = 'Alice'")
```

## 🛠️ Development

### Building from Source
```bash
git clone https://github.com/zhangfengcdt/prollytree
cd prollytree
./python/build_python.sh --all-features --install
```

### Running Tests
```bash
cd python/tests
python test_prollytree.py
```

## 📄 License

Licensed under the Apache License, Version 2.0
