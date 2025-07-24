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

#[cfg(feature = "sql")]
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
#[cfg(feature = "sql")]
use gluesql_core::prelude::Glue;
#[cfg(feature = "sql")]
use prollytree::sql::ProllyStorage;
#[cfg(feature = "sql")]
use tempfile::TempDir;

#[cfg(feature = "sql")]
async fn setup_database(record_count: usize) -> (Glue<ProllyStorage<32>>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let storage = ProllyStorage::<32>::init(temp_dir.path()).unwrap();
    let mut glue = Glue::new(storage);

    // Create table
    let create_sql = r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER,
            city TEXT,
            created_at TIMESTAMP
        )
    "#;
    glue.execute(create_sql).await.unwrap();

    // Insert test data
    for i in 0..record_count {
        let insert_sql = format!(
            "INSERT INTO users (id, name, email, age, city, created_at) 
             VALUES ({}, 'User{}', 'user{}@example.com', {}, 'City{}', TIMESTAMP '2024-01-{:02} 12:00:00')",
            i, i, i, 20 + (i % 50), i % 10, (i % 28) + 1
        );
        glue.execute(&insert_sql).await.unwrap();
    }

    (glue, temp_dir)
}

#[cfg(feature = "sql")]
fn bench_sql_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_insert");
    group.sample_size(10);

    for size in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            b.iter(|| {
                runtime.block_on(async {
                    let temp_dir = TempDir::new().unwrap();
                    let storage = ProllyStorage::<32>::init(temp_dir.path()).unwrap();
                    let mut glue = Glue::new(storage);

                    // Create table
                    glue.execute("CREATE TABLE bench_table (id INTEGER PRIMARY KEY, data TEXT)")
                        .await
                        .unwrap();

                    // Insert records
                    for i in 0..size {
                        let sql = format!(
                            "INSERT INTO bench_table (id, data) VALUES ({}, 'data_{}')",
                            i, i
                        );
                        glue.execute(&sql).await.unwrap();
                    }
                })
            });
        });
    }

    group.finish();
}

#[cfg(feature = "sql")]
fn bench_sql_select(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_select");
    group.sample_size(10);

    for size in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            b.iter_batched(
                || runtime.block_on(setup_database(size)),
                |(mut glue, _temp_dir)| {
                    runtime.block_on(async {
                        // Simple SELECT
                        let result = glue
                            .execute("SELECT * FROM users WHERE age > 30")
                            .await
                            .unwrap();
                        black_box(result);
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(feature = "sql")]
fn bench_sql_join(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_join");
    group.sample_size(10);

    for size in &[100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            b.iter_batched(
                || {
                    runtime.block_on(async {
                        let (mut glue, temp_dir) = setup_database(size).await;

                        // Create orders table
                        glue.execute(
                            "CREATE TABLE orders (
                                id INTEGER PRIMARY KEY,
                                user_id INTEGER,
                                amount DECIMAL,
                                status TEXT
                            )",
                        )
                        .await
                        .unwrap();

                        // Insert orders
                        for i in 0..size * 2 {
                            let sql = format!(
                                "INSERT INTO orders (id, user_id, amount, status) 
                                 VALUES ({}, {}, {}, '{}')",
                                i,
                                i % size,
                                100.0 + (i as f64),
                                if i % 2 == 0 { "completed" } else { "pending" }
                            );
                            glue.execute(&sql).await.unwrap();
                        }

                        (glue, temp_dir)
                    })
                },
                |(mut glue, _temp_dir)| {
                    runtime.block_on(async {
                        let result = glue
                            .execute(
                                "SELECT u.name, COUNT(o.id) as order_count, SUM(o.amount) as total
                             FROM users u
                             JOIN orders o ON u.id = o.user_id
                             WHERE o.status = 'completed'
                             GROUP BY u.name",
                            )
                            .await
                            .unwrap();
                        black_box(result);
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(feature = "sql")]
fn bench_sql_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_aggregation");
    group.sample_size(10);

    for size in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            b.iter_batched(
                || runtime.block_on(setup_database(size)),
                |(mut glue, _temp_dir)| {
                    runtime.block_on(async {
                        let result = glue
                            .execute(
                                "SELECT city, 
                                    COUNT(*) as user_count,
                                    AVG(age) as avg_age,
                                    MIN(age) as min_age,
                                    MAX(age) as max_age
                             FROM users
                             GROUP BY city
                             HAVING COUNT(*) > 5
                             ORDER BY user_count DESC",
                            )
                            .await
                            .unwrap();
                        black_box(result);
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(feature = "sql")]
fn bench_sql_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_update");
    group.sample_size(10);

    for size in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            b.iter_batched(
                || runtime.block_on(setup_database(size)),
                |(mut glue, _temp_dir)| {
                    runtime.block_on(async {
                        // Update multiple records
                        let result = glue
                            .execute(
                                "UPDATE users 
                             SET age = age + 1, 
                                 city = 'UpdatedCity'
                             WHERE age < 30",
                            )
                            .await
                            .unwrap();
                        black_box(result);
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(feature = "sql")]
fn bench_sql_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_delete");
    group.sample_size(10);

    for size in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            b.iter_batched(
                || runtime.block_on(setup_database(size)),
                |(mut glue, _temp_dir)| {
                    runtime.block_on(async {
                        // Delete records
                        let result = glue
                            .execute("DELETE FROM users WHERE age > 50")
                            .await
                            .unwrap();
                        black_box(result);
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(feature = "sql")]
fn bench_sql_index_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_index");
    group.sample_size(10);

    for size in &[100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            b.iter_batched(
                || runtime.block_on(setup_database(size)),
                |(mut glue, _temp_dir)| {
                    runtime.block_on(async {
                        // Create index
                        glue.execute("CREATE INDEX idx_users_age ON users(age)")
                            .await
                            .unwrap();

                        // Query using index
                        let result = glue
                            .execute("SELECT * FROM users WHERE age = 25")
                            .await
                            .unwrap();
                        black_box(result);
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(feature = "sql")]
fn bench_sql_transaction(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_transaction");
    group.sample_size(10);

    for size in &[10, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            b.iter_batched(
                || runtime.block_on(setup_database(100)),
                |(mut glue, _temp_dir)| {
                    runtime.block_on(async {
                        // Begin transaction
                        glue.execute("BEGIN").await.unwrap();

                        // Multiple operations in transaction
                        for i in 0..size {
                            let sql = format!("UPDATE users SET age = age + 1 WHERE id = {}", i);
                            glue.execute(&sql).await.unwrap();
                        }

                        // Commit transaction
                        glue.execute("COMMIT").await.unwrap();
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(feature = "sql")]
fn bench_sql_complex_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_complex");
    group.sample_size(10);

    for size in &[100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            b.iter_batched(
                || runtime.block_on(setup_database(size)),
                |(mut glue, _temp_dir)| {
                    runtime.block_on(async {
                        // Complex query with subqueries
                        let result = glue
                            .execute(
                                "SELECT 
                                u.city,
                                COUNT(DISTINCT u.id) as user_count,
                                (SELECT COUNT(*) 
                                 FROM users u2 
                                 WHERE u2.city = u.city AND u2.age > 40) as senior_count,
                                AVG(u.age) as avg_age
                             FROM users u
                             WHERE u.id IN (
                                SELECT id FROM users 
                                WHERE age BETWEEN 25 AND 45
                             )
                             GROUP BY u.city
                             ORDER BY user_count DESC, avg_age ASC",
                            )
                            .await
                            .unwrap();
                        black_box(result);
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(feature = "sql")]
criterion_group!(
    sql_benches,
    bench_sql_insert,
    bench_sql_select,
    bench_sql_join,
    bench_sql_aggregation,
    bench_sql_update,
    bench_sql_delete,
    bench_sql_index_operations,
    bench_sql_transaction,
    bench_sql_complex_query
);

#[cfg(feature = "sql")]
criterion_main!(sql_benches);

#[cfg(not(feature = "sql"))]
fn main() {
    println!("SQL benchmarks require the 'sql' feature to be enabled.");
    println!("Run with: cargo bench --features sql");
}
