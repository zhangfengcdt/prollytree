# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ProllyTree is a probabilistic tree data structure that combines B-trees and Merkle trees, implemented in Rust with Python bindings. It provides efficient data access with cryptographic verification, designed for distributed systems, version control, and AI memory systems.

## Core Architecture

### Language & Framework
- **Primary Language**: Rust (edition 2021)
- **Python Bindings**: Available via PyO3 (Python 3.8+)
- **Binary**: `git-prolly` CLI tool for Git-like versioned key-value storage

### Key Components
- **Core Tree**: `src/tree.rs` - Probabilistic B-tree implementation with Merkle hashing
- **Storage Backends**: In-memory, RocksDB, and Git-backed storage options
- **Git Integration**: `src/git/` - Git-like version control for key-value data
- **SQL Support**: `src/sql.rs` - GlueSQL integration for SQL queries on tree data
- **Agent Memory**: `src/agent/` - AI agent memory system with semantic, episodic, and working memory
- **Python Module**: `src/python.rs` - PyO3 bindings for Python integration

### Feature Flags
- `git`: Git-backed versioned storage
- `sql`: SQL query support via GlueSQL
- `rig`: Rig framework integration for AI agents
- `python`: Python bindings
- `rocksdb_storage`: RocksDB persistent storage
- `tui`: Terminal UI for interactive usage

## Common Commands

### Build & Development
```bash
# Build the project
cargo build

# Build with all features
cargo build --all-features

# Build release version with optimizations
cargo build --release

# Build specific features
cargo build --features "git sql"

# Build the git-prolly CLI tool
cargo build --features "git sql" --bin git-prolly
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run tests for specific module
cargo test --lib tree::tests

# Run with specific features
cargo test --features "git sql"

# Run worktree tests specifically
cargo test --features git worktree

# Run Python integration tests (requires Python bindings built)
python tests/test_worktree_integration.py

# Test merge functionality specifically
cargo test test_versioned_kv_store_merge --lib -- --nocapture
```

### Code Quality
```bash
# Format code
cargo fmt

# Check formatting without changes
cargo fmt -- --check

# Run linter
cargo clippy --all

# Check code without building
cargo check

# Generate documentation
cargo doc --document-private-items --no-deps
```

### Python Development
```bash
# Build Python bindings
./python/build_python.sh

# Build with SQL features
./python/build_python.sh --with-sql

# Build with all features
./python/build_python.sh --all-features

# Build and install Python bindings
./python/build_python.sh --install

# Run Python tests (after building)
python -m pytest python/tests/

# Run specific Python test
python python/tests/test_prollytree.py
python python/tests/test_sql.py
python python/tests/test_agent.py

python python/tests/test_worktree_integration.py
python python/tests/test_merge.py  # Merge functionality tests

# Run Python examples
cd python/examples && ./run_examples.sh

# Run specific example
cd python/examples && ./run_examples.sh langgraph_chronological.py
```

### Git-Prolly CLI Usage
```bash
# Initialize a new repository
./target/debug/git-prolly init

# Set key-value pairs
./target/debug/git-prolly set key1 value1
./target/debug/git-prolly set key2 value2

# Commit changes
./target/debug/git-prolly commit -m "Initial data"

# Branch operations
./target/debug/git-prolly checkout -b feature-branch
./target/debug/git-prolly checkout main

# List all keys
./target/debug/git-prolly list
./target/debug/git-prolly list --values  # Include values
./target/debug/git-prolly list --graph   # Show tree structure

# Get specific value
./target/debug/git-prolly get key1

# View commit history
./target/debug/git-prolly log
./target/debug/git-prolly log --limit 5

# SQL queries
./target/debug/git-prolly sql "CREATE TABLE users (id INTEGER, name TEXT)"
./target/debug/git-prolly sql "INSERT INTO users VALUES (1, 'Alice')"
./target/debug/git-prolly sql "SELECT * FROM users"
```

### Benchmarking
```bash
# Run tree benchmarks
cargo bench --bench tree

# Run SQL benchmarks
cargo bench --bench sql

# Run Git benchmarks
cargo bench --bench git

# Run storage benchmarks
cargo bench --bench storage
```

### Documentation
```bash
# Build Python documentation locally
cd python/docs && ./build_docs.sh

# Build Python documentation only (requires prollytree installed)
cd python/docs && sphinx-build -b html . _build/html

# Serve documentation locally
cd python/docs/_build/html && python -m http.server 8000
```

## Testing Patterns

### Rust Tests
- Unit tests are in the same file as the code using `#[cfg(test)]` modules
- Integration tests would go in `tests/` directory (currently not present)
- Use `RUST_BACKTRACE=1` for debugging test failures

### Python Tests
- Test files in `python/tests/`
- Use pytest framework
- Ensure Python bindings are built before running tests

## Important Implementation Details

### Multi-Layer Architecture
The codebase implements a layered architecture where each layer builds on the previous:

1. **Core Tree Layer** (`src/tree.rs`, `src/node.rs`): Probabilistic B-tree with Merkle properties
2. **Storage Layer** (`src/storage.rs`, `src/rocksdb/`, `src/git/storage.rs`): Pluggable backends
3. **Version Control Layer** (`src/git/versioned_store.rs`): Git-like operations on trees
4. **SQL Layer** (`src/sql.rs`): GlueSQL integration for query capabilities
5. **Agent Memory Layer** (`src/agent/`): AI-specific memory abstractions
6. **Language Bindings** (`src/python.rs`): Cross-language API exposure

### Cross-Language Integration
- **Rust-Python Boundary**: `src/python.rs` exposes all major features via PyO3
- **Type Translation**: Rust types are carefully mapped to Python equivalents
- **Error Handling**: Rust Results become Python exceptions with proper context
- **Memory Safety**: PyO3 handles reference counting and garbage collection boundaries

### Tree Operations
- The tree uses probabilistic balancing based on content hashes
- Node splitting is determined by hash thresholds, not fixed size
- All operations maintain Merkle tree properties for verification

### Storage Abstraction
- `NodeStorage` trait allows pluggable storage backends
- Each backend implements get/put operations for nodes
- Git backend stores nodes as Git objects for version control

### Memory Management
- Tree uses reference counting for node sharing
- LRU cache available for frequently accessed nodes
- Python bindings handle memory safely through PyO3

### Merge Operations & Conflict Resolution
- **Three-way merge**: Uses common base commit to intelligently merge branches
- **Key-value level merging**: Operates on actual data rather than tree structure for reliability
- **Conflict resolution strategies**:
  - `IgnoreConflictsResolver`: Keeps destination branch values (default for `merge_ignore_conflicts`)
  - `TakeSourceResolver`: Always prefers source branch values
  - `TakeDestinationResolver`: Always keeps current branch values
- **Python API**: Full merge support with `store.merge(branch, ConflictResolution.TakeSource)` and `store.try_merge(branch)` for conflict detection
- **Implementation**: `src/git/versioned_store.rs` contains merge logic, `src/diff.rs` defines conflict resolvers

### Concurrency
- Thread-safe variants available for multi-threaded access
- Agent memory system uses Tokio for async operations
- Git operations use file locking for concurrent access

## Common Pitfalls & Solutions

### Building Issues
- Ensure Rust toolchain is installed: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- For Python bindings, install maturin: `pip install maturin`
- RocksDB feature requires system libraries on some platforms

### Testing
- Some tests require Git to be configured: `git config user.name "Test"` and `git config user.email "test@example.com"`
- SQL tests may create temporary databases in `/tmp`
- Agent tests may require OPENAI_API_KEY environment variable (can be dummy value for tests)

### Performance
- Use batch operations when inserting multiple keys
- Enable LRU cache for read-heavy workloads
- Consider RocksDB backend for large datasets

## Project Structure

### Multi-Language Architecture
- **Rust Core**: High-performance tree implementation with multiple storage backends
- **Python Bindings**: Complete API coverage via PyO3 with SQL, versioning, and agent memory features
- **CLI Tool**: `git-prolly` command-line interface for Git-like operations
- **Documentation**: Auto-generated Sphinx docs at https://prollytree.readthedocs.io/

### Recent Additions
- **Branch Merging**: Three-way merge functionality with configurable conflict resolution strategies
- **Conflict Resolution**: Support for IgnoreAll, TakeSource, and TakeDestination merge strategies
- **Python Merge API**: Complete Python bindings for merge operations with MergeConflict detection
- **LangGraph Integration**: Examples showing AI agent workflows with ProllyTree memory
- **SQL API**: Complete SQL interface exposed to Python via GlueSQL
- **Historical Commit Access**: Track and retrieve commit history for specific keys
- **Enhanced Build System**: Feature-specific build flags for Python bindings
- **Comprehensive Documentation**: Read the Docs integration with auto-generated API reference

## Project Dependencies

### Critical Dependencies
- `sha2`: Cryptographic hashing for Merkle tree
- `serde` & `bincode`: Serialization for node storage
- `gix`: Git integration (optional feature)
- `gluesql-core`: SQL query engine (optional feature)
- `pyo3`: Python bindings (optional feature)
- `rocksdb`: Persistent storage backend (optional feature)
- `maturin`: Python extension building (for development)

# important-instruction-reminders
Do what has been asked; nothing more, nothing less.
NEVER create files unless they're absolutely necessary for achieving your goal.
ALWAYS prefer editing an existing file to creating a new one.
NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
NEVER perform `git push` or `git commit` operations without explicit instructions from the User.
ALWAYS add Apache 2.0 license headers to new files (Rust, Python, etc.). Format:
```
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
```

Note: This project has comprehensive documentation auto-generation via Sphinx at https://prollytree.readthedocs.io/ - prefer directing users there rather than creating new documentation files.
