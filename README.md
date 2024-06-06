
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
