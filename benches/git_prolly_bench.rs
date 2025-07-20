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

#[cfg(all(feature = "git", feature = "sql"))]
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
#[cfg(all(feature = "git", feature = "sql"))]
use prollytree::git::GitNodeStorage;
#[cfg(all(feature = "git", feature = "sql"))]
use prollytree::git::VersionedKvStore;
#[cfg(all(feature = "git", feature = "sql"))]
use prollytree::config::TreeConfig;
#[cfg(all(feature = "git", feature = "sql"))]
use prollytree::tree::{ProllyTree, Tree};
#[cfg(all(feature = "git", feature = "sql"))]
use prollytree::sql::ProllyStorage;
#[cfg(all(feature = "git", feature = "sql"))]
use gluesql_core::prelude::Glue;
#[cfg(all(feature = "git", feature = "sql"))]
use tempfile::TempDir;

#[cfg(all(feature = "git", feature = "sql"))]
fn generate_versioned_data(versions: usize, records_per_version: usize) -> Vec<Vec<(String, String)>> {
    let mut data = Vec::new();
    
    for v in 0..versions {
        let mut version_data = Vec::new();
        for i in 0..records_per_version {
            version_data.push((
                format!("key_{:04}_v{}", i, v),
                format!("value_{:04}_version_{}", i, v)
            ));
        }
        data.push(version_data);
    }
    
    data
}

#[cfg(all(feature = "git", feature = "sql"))]
fn bench_git_versioned_commits(c: &mut Criterion) {
    let mut group = c.benchmark_group("git_versioned_commits");
    group.sample_size(10);

    for size in &[10, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let store = VersionedKvStore::<32>::init(temp_dir.path()).unwrap();
                    (store, temp_dir, generate_versioned_data(5, size))
                },
                |(mut store, _temp_dir, data)| {
                    // Create multiple versions with commits
                    for (version, records) in data.iter().enumerate() {
                        for (key, value) in records {
                            store.insert(key.as_bytes().to_vec(), value.as_bytes().to_vec()).unwrap();
                        }
                        store.commit(&format!("Version {}", version)).unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(all(feature = "git", feature = "sql"))]
fn bench_git_sql_integration(c: &mut Criterion) {
    let mut group = c.benchmark_group("git_sql_integration");
    group.sample_size(10);

    for size in &[100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            
            b.iter_batched(
                || {
                    runtime.block_on(async {
                        let temp_dir = TempDir::new().unwrap();
                        let storage = ProllyStorage::<32>::init(temp_dir.path()).unwrap();
                        let mut glue = Glue::new(storage);
                        
                        // Create table with versioning in mind
                        glue.execute(
                            "CREATE TABLE versioned_data (
                                id INTEGER PRIMARY KEY,
                                key TEXT NOT NULL,
                                value TEXT NOT NULL,
                                version INTEGER NOT NULL,
                                timestamp TIMESTAMP
                            )"
                        ).await.unwrap();
                        
                        (glue, temp_dir)
                    })
                },
                |(mut glue, _temp_dir)| {
                    runtime.block_on(async {
                        // Insert versioned data
                        for v in 0..3 {
                            for i in 0..size {
                                let sql = format!(
                                    "INSERT INTO versioned_data (id, key, value, version, timestamp) 
                                     VALUES ({}, 'key_{}', 'value_{}_v{}', {}, TIMESTAMP '2024-01-{:02} 12:00:00')",
                                    v * size + i, i, i, v, v, (i % 28) + 1
                                );
                                glue.execute(&sql).await.unwrap();
                            }
                        }
                        
                        // Query latest version
                        let result = glue.execute(
                            "SELECT key, value, MAX(version) as latest_version 
                             FROM versioned_data 
                             GROUP BY key"
                        ).await.unwrap();
                        black_box(result);
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(all(feature = "git", feature = "sql"))]
fn bench_git_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("git_operations");
    group.sample_size(10);

    for size in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let repo_path = temp_dir.path().to_path_buf();
                    let repo = gix::init(&repo_path).unwrap();
                    let dataset_dir = repo_path.join("dataset");
                    std::fs::create_dir_all(&dataset_dir).unwrap();
                    let storage = GitNodeStorage::<32>::new(repo, dataset_dir).unwrap();
                    let tree = ProllyTree::new(storage, TreeConfig::<32>::default());
                    (tree, temp_dir)
                },
                |(mut tree, _temp_dir)| {
                    // Insert data
                    for i in 0..size {
                        let key = format!("git_key_{:06}", i).into_bytes();
                        let value = format!("git_value_{:06}", i).into_bytes();
                        tree.insert(key, value);
                    }
                    
                    // For benchmarking, we'll just measure the tree operations
                    // Git commit operations would require more complex setup
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(all(feature = "git", feature = "sql"))]
fn bench_git_branch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("git_branch_operations");
    group.sample_size(10);

    for size in &[50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let mut store = VersionedKvStore::<32>::init(temp_dir.path()).unwrap();
                    
                    // Initialize with some data
                    for i in 0..size {
                        store.insert(
                            format!("key_{:04}", i).into_bytes(),
                            format!("value_{:04}", i).into_bytes()
                        ).unwrap();
                    }
                    store.commit("Initial commit").unwrap();
                    
                    (store, temp_dir)
                },
                |(mut store, _temp_dir)| {
                    // Create and switch branches
                    for branch_num in 0..3 {
                        let branch_name = format!("feature-{}", branch_num);
                        store.create_branch(&branch_name).unwrap();
                        store.checkout(&branch_name).unwrap();
                        
                        // Make changes on branch
                        for i in 0..10 {
                            store.insert(
                                format!("branch_{}_key_{}", branch_num, i).into_bytes(),
                                format!("branch_{}_value_{}", branch_num, i).into_bytes()
                            ).unwrap();
                        }
                        
                        store.commit(&format!("Branch {} changes", branch_num)).unwrap();
                    }
                    
                    // Switch back to main
                    store.checkout("main").unwrap();
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(all(feature = "git", feature = "sql"))]
fn bench_sql_time_travel_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_time_travel");
    group.sample_size(10);

    for size in &[100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            
            b.iter_batched(
                || {
                    runtime.block_on(async {
                        let temp_dir = TempDir::new().unwrap();
                        let storage = ProllyStorage::<32>::init(temp_dir.path()).unwrap();
                        let mut glue = Glue::new(storage);
                        
                        // Create table
                        glue.execute(
                            "CREATE TABLE time_series (
                                id INTEGER PRIMARY KEY,
                                metric TEXT,
                                value DECIMAL,
                                timestamp TIMESTAMP
                            )"
                        ).await.unwrap();
                        
                        // Insert time series data
                        for i in 0..size {
                            for hour in 0..24 {
                                let sql = format!(
                                    "INSERT INTO time_series (id, metric, value, timestamp) 
                                     VALUES ({}, 'metric_{}', {}, TIMESTAMP '2024-01-01 {:02}:00:00')",
                                    i * 24 + hour, i % 10, 100.0 + (i as f64) + (hour as f64 * 0.1), hour
                                );
                                glue.execute(&sql).await.unwrap();
                            }
                        }
                        
                        (glue, temp_dir)
                    })
                },
                |(mut glue, _temp_dir)| {
                    runtime.block_on(async {
                        // Time-based aggregation query
                        let result = glue.execute(
                            "SELECT 
                                metric,
                                DATE_TRUNC('hour', timestamp) as hour,
                                AVG(value) as avg_value,
                                MIN(value) as min_value,
                                MAX(value) as max_value
                             FROM time_series
                             WHERE timestamp >= TIMESTAMP '2024-01-01 06:00:00'
                               AND timestamp <= TIMESTAMP '2024-01-01 18:00:00'
                             GROUP BY metric, DATE_TRUNC('hour', timestamp)
                             ORDER BY metric, hour"
                        ).await.unwrap();
                        black_box(result);
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(all(feature = "git", feature = "sql"))]
fn bench_concurrent_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_operations");
    group.sample_size(10);

    for size in &[50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            
            b.iter(|| {
                runtime.block_on(async {
                    let temp_dir = TempDir::new().unwrap();
                    let storage = ProllyStorage::<32>::init(temp_dir.path()).unwrap();
                    let mut glue = Glue::new(storage);
                    
                    // Create tables sequentially (GlueSQL doesn't support concurrent operations well)
                    for table_num in 0..4 {
                        // Create table
                        let create_sql = format!(
                            "CREATE TABLE table_{} (
                                id INTEGER PRIMARY KEY,
                                data TEXT
                            )",
                            table_num
                        );
                        glue.execute(&create_sql).await.unwrap();
                        
                        // Insert data
                        for i in 0..size {
                            let insert_sql = format!(
                                "INSERT INTO table_{} (id, data) VALUES ({}, 'data_{}')",
                                table_num, i, i
                            );
                            glue.execute(&insert_sql).await.unwrap();
                        }
                    }
                })
            });
        });
    }

    group.finish();
}

#[cfg(all(feature = "git", feature = "sql"))]
criterion_group!(
    git_prolly_benches,
    bench_git_versioned_commits,
    bench_git_sql_integration,
    bench_git_operations,
    bench_git_branch_operations,
    bench_sql_time_travel_queries,
    bench_concurrent_operations
);

#[cfg(all(feature = "git", feature = "sql"))]
criterion_main!(git_prolly_benches);

#[cfg(not(all(feature = "git", feature = "sql")))]
fn main() {
    println!("Git-Prolly benchmarks require both 'git' and 'sql' features to be enabled.");
    println!("Run with: cargo bench --features git,sql");
}