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
- Distributed Databases: Efficiently maintain and synchronize indexes across distributed systems.
- Version Control Systems: Enable verifiable diff, sync, and merge operations for large datasets.
- Blockchain and P2P Networks: Manage and synchronize data with verifiable integrity.
- Real-time Collaborative Editing: Support multiple users making simultaneous changes with efficient merging.
- Cloud Storage Services: Manage file versions and ensure efficient data retrieval and synchronization.

## Prolly Tree v.s. Merkle Search Tree 

- **Structure**: Merkle Search Trees are binary trees with added security features. Prolly Trees are like 
B-trees (balanced trees) but with the same security features.
- **Efficiency**: Prolly Trees work like B-trees, which are efficient for large data sets. 
Merkle Search Trees work like binary trees.
- **Path Dependency**: Prolly Trees use a special balancing technique to avoid issues common in B-trees. 
Merkle Search Trees don't specifically address these issues.
- **Use Cases**: Prolly Trees are good for large databases and peer-to-peer networks. Merkle Search Trees 
are better for tasks needing binary tree features and security.
- Prolly Trees are efficient for maintaining large indexes like B-trees and can be shared and synchronized 
over a peer-to-peer network.

## Usage

To use this library, add the following to your `Cargo.toml`:

```toml
[dependencies]
prollytree = "0.1.0"
```

## Usage

Here is a simple example to get you started:

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

## Documentation

For detailed documentation and examples, please visit [docs.rs/prollytree](https://docs.rs/prollytree).

## Roadmap

The following features are for Prolly Tree library:
- [X] Implement basic Prolly Tree structure
- [X] Implement insertion and deletion operations
- [X] Implement tree traversal and search
- [X] Implement tree size and depth calculation
- [X] Implement tree configuration and tree meta data handling
- [X] Implement proof generation and verification
- [ ] Support Arrow block encoding and decoding with block metadata
- [ ] Support FILE block store (rocksdb)
- [ ] Support IPFS / IPLD block store
- [ ] Support S3 block store
- [ ] Support Prolly Tree Indexes 
- [ ] Support batch insertion and deletion
- [ ] Support probabilistic tree balancing with cdf estimation
- [ ] Benchmarks and optimizations
- [ ] Add documentation and examples

The following features are for other Prolly Tree projects:
- [ ] prolly-cli tool
- [ ] prolly-wasm npm package 
- [ ] prolly-sql with datafusion integration (query engine) 
- [ ] integration with postgres (cdc)
- [ ] integration with git
- [ ] integration with blockchain systems (e.g., substrate)

## Contributing

Contributions are welcome! Please submit a pull request or open an issue to discuss improvements or features.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
