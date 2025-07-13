# Prolly Tree
A Prolly Tree is a hybrid data structure that combines the features of B-trees and Merkle trees to provide 
both efficient data access and verifiable integrity. It is specifically designed to handle the requirements 
of distributed systems and large-scale databases, making indexes syncable and distributable over 
peer-to-peer (P2P) networks.

## Getting Started

Build the project:

```sh
cargo build
```

Run the tests:

```sh
cargo test
```

Check formats and styles:

```sh
cargo fmt
cargo clippy
```

## Key Characteristics:

- **Balanced Structure**: Prolly Trees inherit the balanced structure of B-trees, which ensures that operations 
such as insertions, deletions, and lookups are efficient. This is achieved by maintaining a balanced tree 
where each node can have multiple children, ensuring that the tree remains shallow and operations are 
logarithmic in complexity.

- **Probabilistic Balancing**: The "probabilistic" aspect refers to techniques used to maintain the balance of 
the tree in a way that is not strictly deterministic. This allows for more flexible handling of mutations 
(insertions and deletions) while still ensuring the tree remains efficiently balanced.

- **Merkle Properties**: Each node in a Prolly Tree contains a cryptographic hash that is computed based 
on its own content and the hashes of its children. This creates a verifiable structure where any modification 
to the data can be detected by comparing the root hash.
This Merkle hashing provides proofs of inclusion and exclusion, enabling efficient and secure verification of data.

- **Efficient Data Access**: Like B-trees, Prolly Trees support efficient random reads and writes as well as 
ordered scans. This makes them suitable for database-like operations where both random access and sequential 
access patterns are important. The block size in Prolly Trees is tightly controlled, which helps in optimizing 
read and write operations.

- **Distributed and Syncable**: Prolly Trees are designed to be used in distributed environments. 
The Merkle tree properties enable efficient and correct synchronization, diffing, and merging of data across 
different nodes in a network. This makes Prolly Trees ideal for applications where data needs to be distributed 
and kept in sync across multiple locations or devices.

## Advantages:
- **Verifiability**: The cryptographic hashing in Prolly Trees ensures data integrity and allows for 
verifiable proofs of inclusion/exclusion.
- **Performance**: The balanced tree structure provides efficient data access patterns similar to 
B-trees, ensuring high performance for both random and sequential access.
- **Scalability**: Prolly Trees are suitable for large-scale applications, providing efficient index maintenance 
and data distribution capabilities.
- **Flexibility**: The probabilistic balancing allows for handling various mutation patterns without degrading 
performance or structure.

## Use Cases:
- AI Agent Memory & Long-Term Context: Serve as a structured, versioned memory store for AI agents, enabling efficient diffing, rollback, and verifiable state transitions.
- Versioned Vector Indexes for GenAI: Manage evolving embedding databases in RAG systems or vector search pipelines with Git-like tracking and time-travel queries.
- Prompt and Model Versioning: Track changes to prompts, fine-tuned adapters, or LoRA modules, supporting collaborative AI workflows with history and merge capabilities.
- Real-time Collaborative Editing: Support multiple users or agents making simultaneous changes with efficient merging and conflict resolution.
- Version Control Databases: Enable verifiable diff, sync, and merge operations for large structured datasets, similar to Git but for tabular or document-based data.
- Distributed Databases: Maintain and synchronize ordered indexes efficiently across distributed nodes with structural consistency.
- Blockchain and P2P Networks: Provide verifiable, tamper-proof data structures for syncing state and ensuring data integrity.
- Cloud Storage Services: Manage file versions and enable efficient synchronization, deduplication, and data retrieval across clients.

## Usage

To use this library, add the following to your `Cargo.toml`:

```toml
[dependencies]
prollytree = "0.1.0-beta.1"
```

```rust
use prollytree::tree::ProllyTree;

fn main() {
    // 1. Create a custom tree config
    let config = TreeConfig {
        base: 131,
        modulus: 1_000_000_009,
        min_chunk_size: 4,
        max_chunk_size: 8 * 1024,
        pattern: 0b101,
        root_hash: None,
    };

    // 2. Create and Wrap the Storage Backend
    let storage = InMemoryNodeStorage::<32>::new();

    // 3. Create the Prolly Tree
    let mut tree = ProllyTree::new(storage, config);

    // 4. Insert New Key-Value Pairs
    tree.insert(b"key1".to_vec(), b"value1".to_vec());
    tree.insert(b"key2".to_vec(), b"value2".to_vec());

    // 5. Traverse the Tree with a Custom Formatter
    let traversal = tree.formatted_traverse(|node| {
        let keys_as_strings: Vec<String> = node.keys.iter().map(|k| format!("{:?}", k)).collect();
        format!("[L{}: {}]", node.level, keys_as_strings.join(", "))
    });
    println!("Traversal: {}", traversal);

    // 6. Update the Value for an Existing Key
    tree.update(b"key1".to_vec(), b"new_value1".to_vec());

    // 7. Find or Search for a Key
    if let Some(node) = tree.find(b"key1") {
        println!("Found key1 with value: {:?}", node);
    } else {
        println!("key1 not found");
    }

    // 8. Delete a Key-Value Pair
    if tree.delete(b"key2") {
        println!("key2 deleted");
    } else {
        println!("key2 not found");
    }

    // 9. Print tree stats
    println!("Size: {}", tree.size());
    println!("Depth: {}", tree.depth());
    println!("Summary: {}", tree.summary());

    // 10. Print tree structure
    println!("{:?}", tree.root.print_tree(&tree.storage));    
}

```

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

## Documentation

For detailed documentation and examples, please visit [docs.rs/prollytree](https://docs.rs/prollytree).

## Roadmap

The following features are for Prolly tree library for Version 0.1.0:
- [X] Implement basic Prolly Tree structure
- [X] Implement insertion and deletion operations
- [X] Implement tree traversal and search
- [X] Implement tree size and depth calculation
- [X] Implement tree configuration and tree meta data handling
- [X] Implement proof generation and verification
- [X] Batch insertion and deletion

The following features are for Prolly tree library for Version 0.2.0:
- [X] Arrow block encoding and decoding
- [X] Parquet/Avro block encoding and decoding

The following features are for Prolly tree library for Version 0.2.1:
- [ ] tree diffing and merging
- [ ] show history of changes of the tree (git logs style) using `gitoxide` crate
- [ ] support build database index using Prolly Tree

The following features are for Prolly tree library for Version 0.2.2:
- [ ] version-controlled databases
  - [ ] prolly tree backend for git databases using `gitoxide` crate
  - [ ] support for IPFS (InterPlanetary File System) for distributed storage
  - [ ] advanced probabilistic splitting algorithms
- [ ] decentralized databases
  - [ ] add support for cryptographic hash functions like SHA-256, BLAKE3, etc.
  - [ ] support ZK-friendly hashes such as Poseidon, MiMC, etc.
  - [ ] supporting a rollup-style L2 for a decentralized database

## Contributing

Contributions are welcome! Please submit a pull request or open an issue to discuss improvements or features.

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](LICENSE) file for details.
