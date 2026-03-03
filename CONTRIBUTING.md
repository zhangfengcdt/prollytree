# Contributing to ProllyTree

Thank you for your interest in contributing to ProllyTree! This document provides guidelines and instructions for contributing.

## Getting Started

### Prerequisites

- Rust toolchain (edition 2021)
- Git
- For Python bindings: Python 3.8+ and maturin (`pip install maturin`)

### Building the Project

```bash
# Clone the repository
git clone https://github.com/zhangfengcdt/prollytree.git
cd prollytree

# Build with default features
cargo build

# Build with all features
cargo build --all-features

# Run tests
cargo test
```

### Feature Flags

The project uses feature flags for optional functionality:

- `git`: Git-backed versioned storage
- `sql`: SQL query support via GlueSQL
- `agent`: AI agent memory system
- `python`: Python bindings via PyO3
- `rocksdb_storage`: RocksDB persistent storage
- `tui`: Terminal UI

## How to Contribute

### Reporting Issues

- Check existing issues before creating a new one
- Include clear reproduction steps
- Provide relevant environment details (OS, Rust version, feature flags)

### Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/your-feature`)
3. Make your changes
4. Ensure code quality:
   ```bash
   cargo fmt --all
   cargo clippy --all
   cargo test
   ```
5. Commit with clear messages
6. Push to your fork and open a PR

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Address all clippy warnings (`cargo clippy`)
- Add tests for new functionality
- Include Apache 2.0 license headers in new files

### Testing

```bash
# Run all tests
cargo test

# Run tests with specific features
cargo test --features "git sql"

# Run a specific test
cargo test test_name -- --nocapture
```

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
