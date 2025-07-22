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
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use prollytree::config::TreeConfig;
use prollytree::storage::InMemoryNodeStorage;
use prollytree::tree::{ProllyTree, Tree};

fn generate_test_data(size: usize) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut keys = Vec::new();
    let mut values = Vec::new();

    for i in 0..size {
        keys.push(format!("key_{:06}", i).into_bytes());
        values.push(format!("value_{:06}", i).into_bytes());
    }

    (keys, values)
}

fn bench_insert_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_single");
    group.sample_size(10);

    for &size in &[100, 1000, 10000] {
        let (keys, values) = generate_test_data(size);

        group.bench_with_input(format!("insert_single_{}", size), &size, |b, _| {
            b.iter(|| {
                let storage = InMemoryNodeStorage::<32>::new();
                let config = TreeConfig::<32>::default();
                let mut tree = ProllyTree::new(storage, config);

                for (key, value) in keys.iter().zip(values.iter()) {
                    tree.insert(black_box(key.clone()), black_box(value.clone()));
                }
            });
        });
    }

    group.finish();
}

fn bench_insert_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_batch");
    group.sample_size(10);

    for &size in &[100, 1000, 10000] {
        let (keys, values) = generate_test_data(size);

        group.bench_with_input(format!("insert_batch_{}", size), &size, |b, _| {
            b.iter(|| {
                let storage = InMemoryNodeStorage::<32>::new();
                let config = TreeConfig::<32>::default();
                let mut tree = ProllyTree::new(storage, config);

                tree.insert_batch(black_box(&keys), black_box(&values));
            });
        });
    }

    group.finish();
}

fn bench_delete_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_single");
    group.sample_size(10);

    for &size in &[100, 1000, 10000] {
        let (keys, values) = generate_test_data(size);

        group.bench_with_input(format!("delete_single_{}", size), &size, |b, _| {
            b.iter_batched(
                || {
                    let storage = InMemoryNodeStorage::<32>::new();
                    let config = TreeConfig::<32>::default();
                    let mut tree = ProllyTree::new(storage, config);
                    tree.insert_batch(&keys, &values);
                    tree
                },
                |mut tree| {
                    for key in keys.iter() {
                        tree.delete(black_box(key));
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_delete_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_batch");
    group.sample_size(10);

    for &size in &[100, 1000, 10000] {
        let (keys, values) = generate_test_data(size);

        group.bench_with_input(format!("delete_batch_{}", size), &size, |b, _| {
            b.iter_batched(
                || {
                    let storage = InMemoryNodeStorage::<32>::new();
                    let config = TreeConfig::<32>::default();
                    let mut tree = ProllyTree::new(storage, config);
                    tree.insert_batch(&keys, &values);
                    tree
                },
                |mut tree| {
                    tree.delete_batch(black_box(&keys));
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("get");
    group.sample_size(10);

    for &size in &[100, 1000, 10000] {
        let (keys, values) = generate_test_data(size);

        group.bench_with_input(format!("get_{}", size), &size, |b, _| {
            let storage = InMemoryNodeStorage::<32>::new();
            let config = TreeConfig::<32>::default();
            let mut tree = ProllyTree::new(storage, config);
            tree.insert_batch(&keys, &values);

            b.iter(|| {
                for key in keys.iter() {
                    black_box(tree.find(black_box(key)));
                }
            });
        });
    }

    group.finish();
}

fn bench_mixed_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_operations");
    group.sample_size(10);

    for &size in &[100, 1000, 10000] {
        let (keys, values) = generate_test_data(size);

        group.bench_with_input(format!("mixed_operations_{}", size), &size, |b, _| {
            b.iter_batched(
                || {
                    let storage = InMemoryNodeStorage::<32>::new();
                    let config = TreeConfig::<32>::default();
                    let mut tree = ProllyTree::new(storage, config);
                    tree.insert_batch(&keys[..size / 2], &values[..size / 2]);
                    tree
                },
                |mut tree| {
                    // Insert remaining half
                    for i in size / 2..size {
                        tree.insert(black_box(keys[i].clone()), black_box(values[i].clone()));
                    }

                    // Get some values
                    for i in 0..size / 4 {
                        black_box(tree.find(black_box(&keys[i])));
                    }

                    // Delete some values
                    for i in 0..size / 4 {
                        tree.delete(black_box(&keys[i]));
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_insert_single,
    bench_insert_batch,
    bench_delete_single,
    bench_delete_batch,
    bench_get,
    bench_mixed_operations
);
criterion_main!(benches);
