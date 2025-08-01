# Prolly Tree

[![Crates.io](https://img.shields.io/crates/v/prollytree.svg)](https://crates.io/crates/prollytree)
[![Documentation](https://docs.rs/prollytree/badge.svg)](https://docs.rs/prollytree)
[![License](https://img.shields.io/crates/l/prollytree.svg)](https://github.com/yourusername/prollytree/blob/main/LICENSE)
[![Downloads](https://img.shields.io/crates/d/prollytree.svg)](https://crates.io/crates/prollytree)

A Prolly Tree is a hybrid data structure that combines the features of B-trees and Merkle trees to provide 
both efficient data access and verifiable integrity. It is specifically designed to handle the requirements 
of distributed systems and large-scale databases, making indexes syncable and distributable over 
peer-to-peer (P2P) networks.

## Key Features

- **Balanced B-tree Structure**: O(log n) operations with shallow tree depth for high performance
- **Probabilistic Balancing**: Flexible mutations while maintaining efficiency without degradation
- **Merkle Tree Properties**: Cryptographic hashes provide verifiable data integrity and inclusion proofs
- **Efficient Data Access**: Optimized for both random access and ordered scans at scale
- **Distributed & Syncable**: Built for P2P networks with efficient diff, sync, and merge capabilities

## Use Cases

### AI & GenAI Applications
- **Agent Memory Systems**: Store conversation history and context with verifiable checkpoints, enabling rollback to previous states and audit trails for AI decision-making
- **Versioned Vector Databases**: Track embedding changes over time in RAG systems, compare different indexing strategies, and maintain reproducible search results
- **Model & Prompt Management**: Version control for LLM prompts, LoRA adapters, and fine-tuned models with diff capabilities to track performance changes

### Collaborative Systems
- **Real-time Document Editing**: Multiple users can edit simultaneously with automatic conflict resolution using Merkle proofs to verify changes
- **Distributed Development**: Code collaboration without central servers, enabling offline work with guaranteed merge consistency
- **Shared State Management**: Synchronize application state across devices with cryptographic verification of data integrity

### Data Infrastructure
- **Version Control for Databases**: Git-like branching and merging for structured data, time-travel queries, and verifiable audit logs
- **Distributed Ledgers**: Build blockchain-alternative systems with efficient state synchronization and tamper-proof history
- **Content-Addressed Storage**: Deduplication at the block level with verifiable data retrieval and efficient delta synchronization

## Getting Started

### Rust

Install from crates.io:

```toml
[dependencies]
prollytree = "0.2.0"
```

Build from source:

```sh
cargo build
```

## Performance

Benchmarks run on Apple M3 Pro, 18GB RAM using in-memory storage:

| Operation | 100 Keys | 1,000 Keys | 10,000 Keys |
|-----------|----------|------------|-------------|
| Insert (single) | 8.26 µs | 14.0 µs | 21.2 µs |
| Insert (batch) | 6.17 µs | 10.3 µs | 17.5 µs |
| Lookup | 1.15 µs | 2.11 µs | 2.47 µs |
| Delete | 11.2 µs | 22.4 µs | 29.8 µs |
| Mixed Ops* | 7.73 µs | 14.5 µs | 20.1 µs |

*Mixed operations: 60% lookups, 30% inserts, 10% deletes

### Key Performance Characteristics

- **O(log n) complexity** for all operations
- **Batch operations** are ~25% faster than individual operations
- **Lookup performance** scales sub-linearly due to efficient caching
- **Memory usage** is approximately 100 bytes per key-value pair

## Rust Examples

### Basic Usage

```rust
use prollytree::tree::ProllyTree;
use prollytree::storage::InMemoryNodeStorage;

fn main() {
    // Create tree with in-memory storage
    let storage = InMemoryNodeStorage::<32>::new();
    let mut tree = ProllyTree::new(storage, Default::default());

    // Insert key-value pairs
    tree.insert(b"user:alice".to_vec(), b"Alice Johnson".to_vec());
    tree.insert(b"user:bob".to_vec(), b"Bob Smith".to_vec());

    // Find value
    if let Some(value) = tree.find(b"user:alice") {
        println!("Found: {:?}", String::from_utf8(value).unwrap());
    }

    // Update value
    tree.update(b"user:alice".to_vec(), b"Alice Williams".to_vec());

    // Delete key
    tree.delete(b"user:bob");
}
```

### Git-like Version Control

```rust
use prollytree::git::GitVersionedKvStore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize git-backed store
    let mut store = GitVersionedKvStore::init("./my-data")?;
    
    // Set values (automatically stages changes)
    store.set(b"config/api_key", b"secret123")?;
    store.set(b"config/timeout", b"30")?;
    
    // Commit changes
    store.commit("Update API configuration")?;
    
    // Create a branch for experiments
    store.checkout_new_branch("feature/new-settings")?;
    store.set(b"config/timeout", b"60")?;
    store.commit("Increase timeout")?;
    
    // Switch back and see the difference
    store.checkout("main")?;
    let timeout = store.get(b"config/timeout")?; // Returns b"30"
    
    Ok(())
}
```

### SQL Queries on Versioned Data

```rust
use prollytree::sql::ProllyStorage;
use gluesql_core::prelude::Glue;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize SQL-capable storage
    let storage = ProllyStorage::<32>::init("./data")?;
    let mut glue = Glue::new(storage);
    
    // Create table and insert data
    glue.execute("CREATE TABLE users (id INTEGER, name TEXT, age INTEGER)").await?;
    glue.execute("INSERT INTO users VALUES (1, 'Alice', 30)").await?;
    glue.execute("INSERT INTO users VALUES (2, 'Bob', 25)").await?;
    
    // Query with SQL
    let result = glue.execute("SELECT * FROM users WHERE age > 26").await?;
    // Returns: [(1, 'Alice', 30)]
    
    // Time travel query (requires commit)
    glue.storage.commit("Initial user data").await?;
    glue.execute("UPDATE users SET age = 31 WHERE id = 1").await?;
    
    // Query previous version
    let old_data = glue.storage.query_at_commit("HEAD~1", "SELECT * FROM users").await?;
    
    Ok(())
}
```

### AI Agent Memory System

```rust
use prollytree::agent::{AgentMemorySystem, MemoryQuery, MemoryType, MemoryStore};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize with thread-safe git-backed persistence
    let mut memory = AgentMemorySystem::init_with_thread_safe_git(
        "./agent_memory", "assistant_001".to_string(), None
    )?;
    
    // Store conversation in short-term memory
    memory.short_term.store_conversation_turn(
        "session_123", "user", "What's the weather in Tokyo?", None
    ).await?;
    
    // Store facts in semantic memory
    memory.semantic.store_fact(
        "location", "tokyo",
        json!({"timezone": "JST", "temp": "22°C"}),
        0.9, "weather_api"
    ).await?;
    
    // Query memories
    let query = MemoryQuery {
        namespace: None,
        memory_types: Some(vec![MemoryType::Semantic]),
        tags: None,
        time_range: None,
        text_query: Some("Tokyo".to_string()),
        semantic_query: None,
        limit: Some(5),
        include_expired: false,
    };
    let results = memory.semantic.query(query).await?;
    
    // Create checkpoint
    let commit_id = memory.checkpoint("Weather session").await?;
    println!("Stored {} memories, checkpoint: {}", results.len(), commit_id);
    
    Ok(())
}
```

### Merkle Proofs for Verification

```rust
use prollytree::tree::ProllyTree;
use prollytree::storage::InMemoryNodeStorage;

fn main() {
    let storage = InMemoryNodeStorage::<32>::new();
    let mut tree = ProllyTree::new(storage, Default::default());
    
    // Insert sensitive data
    tree.insert(b"balance:alice".to_vec(), b"1000".to_vec());
    tree.insert(b"balance:bob".to_vec(), b"500".to_vec());
    
    // Generate cryptographic proof
    let proof = tree.generate_proof(b"balance:alice").unwrap();
    let root_hash = tree.root_hash();
    
    // Verify proof (can be done by third party)
    let is_valid = tree.verify_proof(&proof, b"balance:alice", b"1000");
    assert!(is_valid);
    
    // Root hash changes if any data changes
    tree.update(b"balance:alice".to_vec(), b"1100".to_vec());
    let new_root = tree.root_hash();
    assert_ne!(root_hash, new_root);
}
```

## Documentation

For detailed documentation and examples, please visit [docs.rs/prollytree](https://docs.rs/prollytree).

## Contributing

Contributions are welcome! Please submit a pull request or open an issue to discuss improvements or features.

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](LICENSE) file for details.