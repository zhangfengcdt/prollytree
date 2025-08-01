# Prolly Tree

[![Crates.io](https://img.shields.io/crates/v/prollytree.svg)](https://crates.io/crates/prollytree)
[![Documentation](https://docs.rs/prollytree/badge.svg)](https://docs.rs/prollytree)
[![License](https://img.shields.io/crates/l/prollytree.svg)](https://github.com/yourusername/prollytree/blob/main/LICENSE)
[![Downloads](https://img.shields.io/crates/d/prollytree.svg)](https://crates.io/crates/prollytree)

A Prolly Tree is a hybrid data structure that combines the features of B-trees and Merkle trees to provide 
both efficient data access and verifiable integrity. It is specifically designed to handle the requirements 
of distributed systems and large-scale databases, making indexes syncable and distributable over 
peer-to-peer (P2P) networks.

## Prolly Tree Structure Example

Here is an example of a Prolly Tree structure with 3 levels:

```
root:
└── *[0, 23, 63, 85]
    ├── *[0, 2, 7, 13]
    │   ├── [0, 1]
    │   ├── [2, 3, 4, 5, 6]
    │   ├── [7, 8, 9, 10, 11, 12]
    │   └── [13, 14, 15, 16, 17, 18, 19, 20, 21, 22]
    ├── *[23, 29, 36, 47, 58]
    │   ├── [23, 24, 25, 26, 27, 28]
    │   ├── [29, 30, 31, 32, 33, 34, 35]
    │   ├── [36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46]
    │   ├── [47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57]
    │   └── [58, 59, 60, 61, 62]
    ├── *[63, 77, 80]
    │   ├── [63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76]
    │   ├── [77, 78, 79]
    │   └── [80, 81, 82, 83, 84]
    └── *[85, 89, 92, 98]
        ├── [85, 86, 87, 88]
        ├── [89, 90, 91]
        ├── [92, 93, 94, 95, 96, 97]
        └── [98, 99, 100]

Note: *[keys] indicates internal node, [keys] indicates leaf node
```
This can be generated using the `print_tree` method on the root node of the tree.

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
use prollytree::agent::{SearchableMemoryStore, MemoryQuery, MemoryType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize agent memory
    let mut memory = SearchableMemoryStore::new("./agent_memory")?;
    
    // Store different types of memories
    memory.store_memory(
        "conversation",
        "User asked about weather in Tokyo",
        MemoryType::ShortTerm,
        json!({"intent": "weather_query", "location": "Tokyo"})
    ).await?;
    
    memory.store_memory(
        "learned_fact",
        "Tokyo is 9 hours ahead of UTC",
        MemoryType::LongTerm,
        json!({"category": "timezone", "confidence": 0.95})
    ).await?;
    
    // Query memories with semantic search
    let query = MemoryQuery {
        text: Some("What do I know about Tokyo?"),
        memory_type: Some(MemoryType::LongTerm),
        limit: 5,
        ..Default::default()
    };
    
    let memories = memory.search_memories(query).await?;
    for mem in memories {
        println!("Found: {} (relevance: {:.2})", mem.content, mem.relevance);
    }
    
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