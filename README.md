# Prolly Tree
A Prolly Tree is a hybrid data structure that combines the features of B-trees and Merkle trees to provide 
both efficient data access and verifiable integrity. It is specifically designed to handle the requirements 
of distributed systems and large-scale databases, making indexes syncable and distributable over 
peer-to-peer (P2P) networks.

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
use prollytree::ProllyTree;

fn main() {
    // Step 1: Create and Wrap the Storage Backend
    let storage_backend = Arc::new(Mutex::new(HashMapNodeStorage::<32, Vec<u8>>::new()));

    // Step 2: Initialize the Root Node
    let root_node = Node::new(
        "root_key".as_bytes().to_vec(),
        "root_value".as_bytes().to_vec(),
        true,
        storage_backend,
    );

    // Step 3: Initialize the ProllyTree
    let mut tree = ProllyTree::new(root_node);

    // Step 4: Insert a New Key-Value Pair
    tree.insert(
        "new_key".as_bytes().to_vec(),
        "new_value".as_bytes().to_vec()
    );

    // Step 5: Update the Value for an Existing Key
    tree.update(
        "new_key".as_bytes().to_vec(),
        "updated_value".as_bytes().to_vec()
    );

    // Step 6: Find or Search for a Key
    let search_key = "new_key".as_bytes().to_vec();
    if let Some(_node) = tree.find(&search_key) {
        println!("Found node with key: {:?}", search_key);
    } else {
        println!("Node with key {:?} not found", search_key);
    }

    // Step 7: Delete a Key-Value Pair
    tree.delete(&search_key);
```

## Documentation

For detailed documentation and examples, please visit [docs.rs/prollytree](https://docs.rs/prollytree).

## Roadmap

The following features are for prollytree library:
- [X] Implement basic Prolly Tree structure
- [ ] Implement insertion and deletion operations
- [ ] Implement hashing and verification
- [ ] Add support for variable probabilistic balancing
- [ ] Add documentation and examples
- [ ] Add benchmarks and optimizations

The following features are for other prollytree projects:
- [ ] prolly cli tool
- [ ] integration with git
- [ ] integration with ipfs
- [ ] integration with blockchain systems (e.g., substrate)

## Contributing

Contributions are welcome! Please submit a pull request or open an issue to discuss improvements or features.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
