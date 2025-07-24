# ProllyTree Benchmarks

This directory contains comprehensive benchmarks for ProllyTree, including core operations, SQL functionality, and git-prolly integration.

## Available Benchmarks

### 1. Core ProllyTree Operations (`prollytree_bench.rs`)
Basic tree operations benchmarks:
- **Insert**: Single and batch insertions
- **Delete**: Single and batch deletions
- **Get**: Key lookups
- **Mixed operations**: Combined insert/get/delete operations

### 2. SQL Operations (`sql_bench.rs`)
GlueSQL integration benchmarks:
- **Insert**: SQL INSERT operations
- **Select**: Basic SELECT queries
- **Join**: JOIN operations between tables
- **Aggregation**: GROUP BY with aggregate functions
- **Update**: UPDATE operations
- **Delete**: DELETE operations
- **Index**: CREATE INDEX and indexed queries
- **Transaction**: Transaction performance
- **Complex queries**: Subqueries and complex SQL

### 3. Git-Prolly Integration (`git_prolly_bench.rs`)
Git versioning and SQL integration:
- **Versioned commits**: Multiple version commits
- **Git-SQL integration**: Combined git versioning with SQL queries
- **Git operations**: Basic git operations (commit, branch)
- **Branch operations**: Creating and switching branches
- **Time travel queries**: Historical data queries
- **Concurrent operations**: Parallel table operations

### 4. Storage Backend Comparison (`storage_bench.rs`)
Compares different storage backend implementations:
- **Insert performance**: Sequential inserts across storage backends
- **Read performance**: Random key lookups
- **Batch operations**: Bulk insert performance
- **Direct node operations**: Low-level storage API performance

Supported backends:
- InMemoryNodeStorage (default)
- FileNodeStorage (default)
- RocksDBNodeStorage (requires `rocksdb_storage` feature)
- GitNodeStorage (requires `git` feature)

## Running Benchmarks

### Run all benchmarks:
```bash
cargo bench
```

### Run specific benchmark suite:
```bash
# Core benchmarks only
cargo bench --bench prollytree_bench

# SQL benchmarks only (requires sql feature)
cargo bench --bench sql_bench --features sql

# Git-Prolly benchmarks (requires both git and sql features)
cargo bench --bench git_prolly_bench --features git,sql

# Storage backend benchmarks
cargo bench --bench storage_bench

# Storage benchmarks with RocksDB included
cargo bench --bench storage_bench --features rocksdb_storage
```

### Run specific benchmark within a suite:
```bash
# Run only insert benchmarks
cargo bench insert

# Run only SQL join benchmarks
cargo bench sql_join

# Run only git versioning benchmarks
cargo bench git_versioned
```

### Generate HTML reports:
```bash
# Results will be in target/criterion/report/index.html
cargo bench -- --verbose
```

### Compare with baseline:
```bash
# Save current results as baseline
cargo bench -- --save-baseline my_baseline

# Compare against baseline
cargo bench -- --baseline my_baseline
```

## Benchmark Configuration

Benchmarks use different data sizes to test scalability:
- Small: 100 records
- Medium: 500-1000 records  
- Large: 10,000 records

Sample sizes and iterations are configured per benchmark group for optimal runtime.

## Interpreting Results

Results show:
- **Time**: Average time per operation
- **Throughput**: Operations per second
- **Variance**: Consistency of performance

Lower times and higher throughput indicate better performance.

## Adding New Benchmarks

1. Add benchmark function following the pattern:
```rust
fn bench_my_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("my_operation");
    // ... benchmark code
}
```

2. Add to appropriate criterion group at the bottom of the file

3. Update this README with the new benchmark description