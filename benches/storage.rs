/*
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use prollytree::config::TreeConfig;
use prollytree::storage::{FileNodeStorage, InMemoryNodeStorage, NodeStorage};
use prollytree::tree::{ProllyTree, Tree};
use tempfile::TempDir;

#[cfg(feature = "rocksdb_storage")]
use prollytree::storage::RocksDBNodeStorage;

fn generate_test_data(size: usize) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut keys = Vec::new();
    let mut values = Vec::new();

    for i in 0..size {
        keys.push(format!("key_{:08}", i).into_bytes());
        values.push(format!("value_data_{:08}_padding_to_make_it_larger", i).into_bytes());
    }

    (keys, values)
}

fn bench_storage_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_insert");
    group.sample_size(10);

    for &size in &[1000, 5000, 10000] {
        let (keys, values) = generate_test_data(size);

        // InMemory Storage
        group.bench_with_input(
            BenchmarkId::new("InMemory", size),
            &(&keys, &values),
            |b, (keys, values)| {
                b.iter(|| {
                    let storage = InMemoryNodeStorage::<32>::new();
                    let config = TreeConfig::<32>::default();
                    let mut tree = ProllyTree::new(storage, config);

                    for (key, value) in keys.iter().zip(values.iter()) {
                        tree.insert(black_box(key.clone()), black_box(value.clone()));
                    }
                });
            },
        );

        // File Storage
        group.bench_with_input(
            BenchmarkId::new("File", size),
            &(&keys, &values),
            |b, (keys, values)| {
                let temp_dir = TempDir::new().unwrap();
                b.iter(|| {
                    let storage = FileNodeStorage::<32>::new(temp_dir.path().to_path_buf());
                    let config = TreeConfig::<32>::default();
                    let mut tree = ProllyTree::new(storage, config);

                    for (key, value) in keys.iter().zip(values.iter()) {
                        tree.insert(black_box(key.clone()), black_box(value.clone()));
                    }
                });
            },
        );

        // RocksDB Storage
        #[cfg(feature = "rocksdb_storage")]
        group.bench_with_input(
            BenchmarkId::new("RocksDB", size),
            &(&keys, &values),
            |b, (keys, values)| {
                let temp_dir = TempDir::new().unwrap();
                b.iter(|| {
                    let storage = RocksDBNodeStorage::<32>::new(temp_dir.path().join("rocksdb"))
                        .expect("Failed to create RocksDB storage");
                    let config = TreeConfig::<32>::default();
                    let mut tree = ProllyTree::new(storage, config);

                    for (key, value) in keys.iter().zip(values.iter()) {
                        tree.insert(black_box(key.clone()), black_box(value.clone()));
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_storage_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_read");
    group.sample_size(10);

    for &size in &[1000, 5000, 10000] {
        let (keys, values) = generate_test_data(size);

        // Prepare InMemory tree
        let inmem_storage = InMemoryNodeStorage::<32>::new();
        let config = TreeConfig::<32>::default();
        let mut inmem_tree = ProllyTree::new(inmem_storage, config.clone());
        for (key, value) in keys.iter().zip(values.iter()) {
            inmem_tree.insert(key.clone(), value.clone());
        }

        // Prepare File tree
        let file_dir = TempDir::new().unwrap();
        let file_storage = FileNodeStorage::<32>::new(file_dir.path().to_path_buf());
        let mut file_tree = ProllyTree::new(file_storage, config.clone());
        for (key, value) in keys.iter().zip(values.iter()) {
            file_tree.insert(key.clone(), value.clone());
        }

        // Prepare RocksDB tree
        #[cfg(feature = "rocksdb_storage")]
        let rocksdb_tree = {
            let rocksdb_dir = TempDir::new().unwrap();
            let rocksdb_storage =
                RocksDBNodeStorage::<32>::new(rocksdb_dir.path().join("rocksdb")).unwrap();
            let mut rocksdb_tree = ProllyTree::new(rocksdb_storage, config.clone());
            for (key, value) in keys.iter().zip(values.iter()) {
                rocksdb_tree.insert(key.clone(), value.clone());
            }
            (rocksdb_tree, rocksdb_dir)
        };

        // Benchmark InMemory reads
        group.bench_with_input(
            BenchmarkId::new("InMemory", size),
            &(&keys, &inmem_tree),
            |b, (keys, tree)| {
                b.iter(|| {
                    for key in keys.iter() {
                        black_box(tree.find(key));
                    }
                });
            },
        );

        // Benchmark File reads
        group.bench_with_input(
            BenchmarkId::new("File", size),
            &(&keys, &file_tree),
            |b, (keys, tree)| {
                b.iter(|| {
                    for key in keys.iter() {
                        black_box(tree.find(key));
                    }
                });
            },
        );

        // Benchmark RocksDB reads
        #[cfg(feature = "rocksdb_storage")]
        {
            let (rocksdb_tree, _dir) = &rocksdb_tree;
            group.bench_with_input(
                BenchmarkId::new("RocksDB", size),
                &(&keys, rocksdb_tree),
                |b, (keys, tree)| {
                    b.iter(|| {
                        for key in keys.iter() {
                            black_box(tree.find(key));
                        }
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_storage_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_batch_operations");
    group.sample_size(10);

    for &size in &[1000, 5000] {
        let (keys, values) = generate_test_data(size);

        // Benchmark batch insert for different storage backends
        group.bench_with_input(
            BenchmarkId::new("InMemory_batch_insert", size),
            &(&keys, &values),
            |b, (keys, values)| {
                b.iter(|| {
                    let storage = InMemoryNodeStorage::<32>::new();
                    let config = TreeConfig::<32>::default();
                    let mut tree = ProllyTree::new(storage, config);
                    tree.insert_batch(black_box(keys), black_box(values));
                });
            },
        );

        #[cfg(feature = "rocksdb_storage")]
        group.bench_with_input(
            BenchmarkId::new("RocksDB_batch_insert", size),
            &(&keys, &values),
            |b, (keys, values)| {
                let temp_dir = TempDir::new().unwrap();
                b.iter(|| {
                    let storage = RocksDBNodeStorage::<32>::new(temp_dir.path().join("rocksdb"))
                        .expect("Failed to create RocksDB storage");
                    let config = TreeConfig::<32>::default();
                    let mut tree = ProllyTree::new(storage, config);
                    tree.insert_batch(black_box(keys), black_box(values));
                });
            },
        );
    }

    group.finish();
}

fn bench_node_storage_direct(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_storage_direct");

    // Create test nodes
    let config = TreeConfig::<32>::default();
    let mut nodes = Vec::new();
    for i in 0..1000 {
        let node = prollytree::node::ProllyNode {
            keys: vec![format!("key_{:08}", i).into_bytes()],
            key_schema: config.key_schema.clone(),
            values: vec![format!("value_{:08}", i).into_bytes()],
            value_schema: config.value_schema.clone(),
            is_leaf: true,
            level: 0,
            base: config.base,
            modulus: config.modulus,
            min_chunk_size: config.min_chunk_size,
            max_chunk_size: config.max_chunk_size,
            pattern: config.pattern,
            split: false,
            merged: false,
            encode_types: Vec::new(),
            encode_values: Vec::new(),
        };
        nodes.push((node.get_hash(), node));
    }

    // Benchmark direct node insertions
    group.bench_function("InMemory_insert_nodes", |b| {
        b.iter(|| {
            let mut storage = InMemoryNodeStorage::<32>::new();
            for (hash, node) in &nodes {
                storage.insert_node(black_box(hash.clone()), black_box(node.clone()));
            }
        });
    });

    #[cfg(feature = "rocksdb_storage")]
    group.bench_function("RocksDB_insert_nodes", |b| {
        let temp_dir = TempDir::new().unwrap();
        b.iter(|| {
            let mut storage = RocksDBNodeStorage::<32>::new(temp_dir.path().join("rocksdb"))
                .expect("Failed to create RocksDB storage");
            for (hash, node) in &nodes {
                storage.insert_node(black_box(hash.clone()), black_box(node.clone()));
            }
        });
    });

    // Benchmark direct node reads
    let mut inmem_storage = InMemoryNodeStorage::<32>::new();
    for (hash, node) in &nodes {
        inmem_storage.insert_node(hash.clone(), node.clone());
    }

    group.bench_function("InMemory_read_nodes", |b| {
        b.iter(|| {
            for (hash, _) in &nodes {
                black_box(inmem_storage.get_node_by_hash(black_box(hash)));
            }
        });
    });

    #[cfg(feature = "rocksdb_storage")]
    {
        let temp_dir = TempDir::new().unwrap();
        let mut rocksdb_storage =
            RocksDBNodeStorage::<32>::new(temp_dir.path().join("rocksdb")).unwrap();
        for (hash, node) in &nodes {
            rocksdb_storage.insert_node(hash.clone(), node.clone());
        }

        group.bench_function("RocksDB_read_nodes", |b| {
            b.iter(|| {
                for (hash, _) in &nodes {
                    black_box(rocksdb_storage.get_node_by_hash(black_box(hash)));
                }
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_storage_insert,
    bench_storage_read,
    bench_storage_batch_operations,
    bench_node_storage_direct
);
criterion_main!(benches);
