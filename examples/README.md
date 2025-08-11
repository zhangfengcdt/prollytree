# ProllyTree Examples and Use Cases

This directory contains practical examples demonstrating how to use ProllyTree in various scenarios. Each example showcases different features and real-world applications.

## Use Cases

### AI & GenAI Applications

#### Agent Memory Systems
Store conversation history and context with verifiable checkpoints, enabling rollback to previous states and audit trails for AI decision-making.

**Key Benefits:**
- Versioned conversation history
- Rollback to previous AI states
- Cryptographic verification of decisions
- Distributed agent coordination

#### Versioned Vector Databases
Track embedding changes over time in RAG systems, compare different indexing strategies, and maintain reproducible search results.

**Key Benefits:**
- Version control for embeddings
- A/B testing of indexing strategies
- Reproducible search experiments
- Semantic memory evolution tracking

#### Model & Prompt Management
Version control for LLM prompts, LoRA adapters, and fine-tuned models with diff capabilities to track performance changes.

**Key Benefits:**
- Prompt versioning and rollback
- Model checkpoint management
- Performance diff tracking
- Collaborative prompt engineering

### Collaborative Systems

#### Real-time Document Editing
Multiple users can edit simultaneously with automatic conflict resolution using Merkle proofs to verify changes.

**Key Benefits:**
- Conflict-free collaborative editing
- Cryptographic change verification
- Offline-first synchronization
- Distributed document state

#### Distributed Development
Code collaboration without central servers, enabling offline work with guaranteed merge consistency.

**Key Benefits:**
- Decentralized version control
- Offline development support
- Guaranteed merge correctness
- P2P code synchronization

#### Shared State Management
Synchronize application state across devices with cryptographic verification of data integrity.

**Key Benefits:**
- Cross-device synchronization
- State integrity verification
- Conflict resolution
- Offline state management

### Data Infrastructure

#### Version Control for Databases
Git-like branching and merging for structured data, time-travel queries, and verifiable audit logs.

**Key Benefits:**
- Database branching and merging
- Time-travel queries
- Verifiable audit trails
- Schema evolution tracking

#### Distributed Ledgers
Build blockchain-alternative systems with efficient state synchronization and tamper-proof history.

**Key Benefits:**
- Efficient state synchronization
- Tamper-proof history
- Scalable distributed consensus
- Energy-efficient verification

#### Content-Addressed Storage
Deduplication at the block level with verifiable data retrieval and efficient delta synchronization.

**Key Benefits:**
- Automatic deduplication
- Verifiable data retrieval
- Efficient delta sync
- Content-addressed indexing

## Available Examples

### Rust Examples

#### Basic Tree Operations
- **File:** `examples/storage.rs`
- **Description:** Demonstrates basic tree operations with different storage backends
- **Run:** `cargo run --example storage`

#### Git-like Version Control
- **File:** `examples/versioning.rs`
- **Description:** Shows how to use git-backed versioned key-value store
- **Run:** `cargo run --example versioning --features git`

#### SQL Queries on Versioned Data
- **File:** `examples/sql.rs`
- **Description:** Demonstrates SQL capabilities with time-travel queries
- **Run:** `cargo run --example sql --features "git sql"`

#### AI Agent Memory System
- **File:** `examples/agent.rs`
- **Description:** Complete agent memory system with semantic, episodic, and working memory
- **Run:** `OPENAI_API_KEY=your_key cargo run --example agent --features "git sql rig"`

#### Cryptographic Proofs
- **File:** `examples/proof.rs`
- **Description:** Generate and verify Merkle proofs for data integrity
- **Run:** `cargo run --example proof`

#### Worktree Operations
- **File:** `examples/worktree.rs`
- **Description:** Advanced git worktree operations for branching workflows
- **Run:** `cargo run --example worktree --features git`

### Python Examples

Located in `python/examples/`:

#### Basic Usage
- **File:** `python/examples/basic_usage.py`
- **Description:** Basic tree operations in Python
- **Run:** `cd python/examples && python basic_usage.py`

#### LangGraph Integration
- **File:** `python/examples/langgraph_chronological.py`
- **Description:** AI agent workflows with chronological memory
- **Run:** `cd python/examples && python langgraph_chronological.py`

#### Multi-Agent Branching
- **File:** `python/examples/langgraph_multi_agent_branching.py`
- **Description:** Complex multi-agent systems with branched memory
- **Run:** `cd python/examples && python langgraph_multi_agent_branching.py`

#### SQL Operations
- **File:** `python/examples/sql_example.py`
- **Description:** SQL queries and time-travel operations
- **Run:** `cd python/examples && python sql_example.py`

#### Merge Operations
- **File:** `python/examples/merge_example.py`
- **Description:** Three-way merging with conflict resolution
- **Run:** `cd python/examples && python merge_example.py`

## Performance Benchmarks

Benchmarks are available in the `benches/` directory:

- **Tree Operations:** `cargo bench --bench tree`
- **Storage Backends:** `cargo bench --bench storage`
- **Git Operations:** `cargo bench --bench git --features git`
- **SQL Queries:** `cargo bench --bench sql --features "git sql"`

## Sample Data

The `examples/data/` directory contains sample datasets for testing:

- **Conversation Data:** JSON files with sample AI conversations
- **Financial Data:** Sample financial records for SQL examples
- **Simple Test Data:** Basic key-value pairs for quick testing

## Getting Started

1. **Choose your use case** from the list above
2. **Find the relevant example** in this directory
3. **Install dependencies** as needed (see main README.md)
4. **Run the example** using the provided commands
5. **Modify for your needs** - examples are designed to be starting points

## Feature Matrix

| Example | Storage | Git | SQL | Agent | Proofs |
|---------|---------|-----|-----|-------|--------|
| storage.rs | ✓ | | | | |
| versioning.rs | ✓ | ✓ | | | |
| sql.rs | ✓ | ✓ | ✓ | | |
| agent.rs | ✓ | ✓ | ✓ | ✓ | |
| proof.rs | ✓ | | | | ✓ |
| worktree.rs | ✓ | ✓ | | | |

## Contributing Examples

To contribute a new example:

1. Create a new `.rs` file in `examples/` for Rust or `.py` in `python/examples/` for Python
2. Add comprehensive comments explaining the use case
3. Include error handling and clean resource management
4. Add an entry to this README with description and run command
5. Test with `cargo run --example your_example` or `python your_example.py`

## Need Help?

- Check the [main documentation](https://docs.rs/prollytree) for API details
- Look at existing examples for patterns and best practices
- Open an issue for questions about specific use cases
- Join discussions about new example ideas
