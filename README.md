
# Prolly Tree in Rust

A Prolly Tree (probabilistic tree) implementation in Rust for efficient storage, retrieval, and modification of ordered data with integrity guarantees.

## Features

- **Efficient Ordered Data Storage**: Optimized for handling large datasets with ordered entries.
- **Integrity Verification**: Uses hashing to ensure data integrity.
- **Probabilistic Balancing**: Employs probabilistic methods for balancing, providing performance benefits in certain scenarios.
- **Search Operations**: Efficient search operations with guaranteed data integrity.

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
    let mut tree = ProllyTree::new();

    // Insert some key-value pairs
    tree.insert("key1", "value1");
    tree.insert("key2", "value2");
    tree.insert("key3", "value3");

    // Retrieve a value
    if let Some(value) = tree.get("key2") {
        println!("Found: {}", value);
    } else {
        println!("Key not found");
    }

    // Verify integrity
    let root_hash = tree.root_hash();
    println!("Root hash: {}", root_hash);
}
```

## Documentation

For detailed documentation and examples, please visit [docs.rs/prollytree](https://docs.rs/prollytree).

## Contributing

Contributions are welcome! Please submit a pull request or open an issue to discuss improvements or features.

### Running Tests

To run tests, use the following command:

```sh
cargo test
```

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Acknowledgements

- [Rust Programming Language](https://www.rust-lang.org/)
- [Cargo - Rust's Package Manager](https://doc.rust-lang.org/cargo/)

## Repository Structure

```
prollytree/
├── src/
│   ├── lib.rs
│   ├── prollytree.rs
│   └── hash.rs
├── tests/
│   └── prollytree_tests.rs
├── Cargo.toml
└── README.md
```

- `src/lib.rs`: The main library file.
- `src/prollytree.rs`: The implementation of the Prolly Tree.
- `src/hash.rs`: Hashing functions and utilities.
- `tests/prollytree_tests.rs`: Unit tests for the Prolly Tree implementation.
- `Cargo.toml`: Cargo configuration file.
- `README.md`: This file.

## Getting Started

Clone the repository:

```sh
git clone https://github.com/yourusername/prollytree.git
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
