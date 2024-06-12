# Prolly Tree
A Prolly Tree is a hybrid data structure that combines the features of B-trees and Merkle trees to provide 
both efficient data access and verifiable integrity. It is specifically designed to handle the requirements 
of distributed systems and large-scale databases, making indexes syncable and distributable over peer-to-peer (P2P) networks.

## Key Characteristics:

- **Balanced Structure**: Prolly Trees inherit the balanced structure of B-trees, which ensures that operations such as insertions, deletions, and lookups are efficient. This is achieved by maintaining a balanced tree where each node can have multiple children, ensuring that the tree remains shallow and operations are logarithmic in complexity.

- **Probabilistic Balancing**: The "probabilistic" aspect refers to techniques used to maintain the balance of the tree in a way that is not strictly deterministic. This allows for more flexible handling of mutations (insertions and deletions) while still ensuring the tree remains efficiently balanced.

- **Merkle Properties**: Each node in a Prolly Tree contains a cryptographic hash that is computed based on its own content and the hashes of its children. This creates a verifiable structure where any modification to the data can be detected by comparing the root hash.
This Merkle hashing provides proofs of inclusion and exclusion, enabling efficient and secure verification of data.

- **Efficient Data Access**: Like B-trees, Prolly Trees support efficient random reads and writes as well as ordered scans. This makes them suitable for database-like operations where both random access and sequential access patterns are important.
The block size in Prolly Trees is tightly controlled, which helps in optimizing read and write operations.

- **Distributed and Syncable**: Prolly Trees are designed to be used in distributed environments. The Merkle tree properties enable efficient and correct synchronization, diffing, and merging of data across different nodes in a network.
This makes Prolly Trees ideal for applications where data needs to be distributed and kept in sync across multiple locations or devices.

## Advantages:
- **Verifiability**: The cryptographic hashing in Prolly Trees ensures data integrity and allows for verifiable proofs of inclusion/exclusion.
- **Performance**: The balanced tree structure provides efficient data access patterns similar to B-trees, ensuring high performance for both random and sequential access.
- **Scalability**: Prolly Trees are suitable for large-scale applications, providing efficient index maintenance and data distribution capabilities.
- **Flexibility**: The probabilistic balancing allows for handling various mutation patterns without degrading performance or structure.

## Use Cases:
- Distributed Databases: Efficiently maintain and synchronize indexes across distributed systems.
- Version Control Systems: Enable verifiable diff, sync, and merge operations for large datasets.
- Blockchain and P2P Networks: Manage and synchronize data with verifiable integrity.
- Real-time Collaborative Editing: Support multiple users making simultaneous changes with efficient merging.
- Cloud Storage Services: Manage file versions and ensure efficient data retrieval and synchronization.

## Prolly Tree v.s. Merkle Search Tree 

- **Structure**: Merkle Search Trees are binary trees with added security features. Prolly Trees are like B-trees (balanced trees) but with the same security features.
- **Efficiency**: Prolly Trees work like B-trees, which are efficient for large data sets. Merkle Search Trees work like binary trees.
- **Path Dependency**: Prolly Trees use a special balancing technique to avoid issues common in B-trees. Merkle Search Trees don't specifically address these issues.
- **Use Cases**: Prolly Trees are good for large databases and peer-to-peer networks. Merkle Search Trees are better for tasks needing binary tree features and security.
- Prolly Trees are efficient for maintaining large indexes like B-trees and can be shared and synchronized over a peer-to-peer network.

# Prolly Tree in Rust

## Features

- **Efficient Ordered Data Storage**: Optimized for handling large datasets with ordered entries.
- **Integrity Verification**: Uses hashing to ensure data integrity.
- **Probabilistic Balancing**: Employs probabilistic methods for balancing, providing performance benefits in certain scenarios.
- **Search Operations**: Efficient search operations with guaranteed data integrity.

Prolly tree writes are slightly more expensive than b tree writes because there is a small (~1/64) chance of
writing more than one node at each level.

Operation | B-Trees | Merkle Search Trees | Prolly Trees
--------- | ------- |---------------------| ------------
1 Random Read | 🎉logk(n) | 🎉logk(n)           | 🎉logk(n)
1 Random Write | 🎉logk(n) | ???                 | 👍(1+k/w)*logk(n)
Ordered scan of one item with size z | 🎉z/k | 🎉z/k                 | 🎉z/k
Calculate diff of size d | 💩n | 🎉d                 | 🎉d
Probabilistic Balancing | ❌ | ❌                   | 🙌
Block size | ❌ | fixed               | variable
Verification, proofs | ❌ | 🙌                  | 🙌
Structured sharing | ❌ | 🙌                  | 🙌

**†** assuming hashed keys, unhashed destroys perf — **n**: total leaf data in tree, **k**: average block size, **w**: window width

## Installation

To use this library, add the following to your `Cargo.toml`:

```toml
[dependencies]
prollytree = "0.1.0"
```

## Usage

Here is a simple example to get you started:

```rust
use prollytree::ProllyTree;

fn main() {
    let mut tree = ProllyTree::<32, String, String>::new();

    let key1 = "key1".to_string();
    let value1 = "value1".to_string();
    tree.insert(key1.clone(), value1);

    let key2 = "key2".to_string();
    let value2 = "value2".to_string();
    tree.insert(key2.clone(), value2);

    let root_hash = tree.root_hash();
    println!("Root Hash: {:?}", root_hash);
}
```

## Documentation

For detailed documentation and examples, please visit [docs.rs/prollytree](https://docs.rs/prollytree).

## Getting Started

Clone the repository:

```sh
git clone https://github.com/zhangfengcdt/prollytree.git
cd prollytree
```

Build the project:

```sh
cargo build
```

Run the tests:

```sh
cargo test
```

## Support

If you encounter any issues or have questions, please open an issue on GitHub.

## Contributing

Contributions are welcome! Please submit a pull request or open an issue to discuss improvements or features.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
