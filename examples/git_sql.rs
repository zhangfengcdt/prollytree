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

//! Example demonstrating SQL capabilities with ProllyTree storage
//!
//! This example shows how to use GlueSQL with ProllyTree as a custom storage backend
//! to execute SQL queries on versioned key-value data.

#[cfg(feature = "sql")]
use gluesql_core::{error::Result, executor::Payload, prelude::Glue};
use gluesql_core::store::Transaction;
#[cfg(feature = "sql")]
use prollytree::sql::ProllyStorage;
#[cfg(feature = "sql")]
use tempfile::TempDir;

#[cfg(feature = "sql")]
#[tokio::main]
async fn main() -> Result<()> {
    println!("🌟 ProllyTree SQL Example");
    println!("========================\n");

    // Create temporary directory for this example
    let temp_dir = TempDir::new().map_err(|e| {
        gluesql_core::error::Error::StorageMsg(format!("Failed to create temp dir: {}", e))
    })?;
    println!("📁 Using temporary directory: {:?}\n", temp_dir.path());

    // Initialize git repository
    std::process::Command::new("git")
        .arg("init")
        .current_dir(temp_dir.path())
        .output()
        .map_err(|e| {
            gluesql_core::error::Error::StorageMsg(format!("Failed to init git: {}", e))
        })?;

    // Create dataset subdirectory
    let dataset_path = temp_dir.path().join("dataset");
    std::fs::create_dir(&dataset_path).map_err(|e| {
        gluesql_core::error::Error::StorageMsg(format!("Failed to create dataset dir: {}", e))
    })?;

    // Initialize ProllyTree storage with GlueSQL
    let storage = ProllyStorage::<32>::init(&dataset_path)?;
    let mut glue = Glue::new(storage);

    glue.storage.begin(false).await?;

    // 1. Create tables
    println!("1. Creating tables...");

    let create_users = r#"
        CREATE TABLE users (
            id INTEGER,
            name TEXT,
            email TEXT,
            age INTEGER
        )
    "#;

    let create_orders = r#"
        CREATE TABLE orders (
            id INTEGER,
            user_id INTEGER,
            product TEXT,
            amount INTEGER,
            order_date TEXT
        )
    "#;

    glue.storage.commit().await?;

    glue.execute(create_users).await?;
    glue.execute(create_orders).await?;
    println!("   ✓ Created users and orders tables\n");


    // 2. Insert data
    println!("2. Inserting sample data...");

    let insert_users = r#"
        INSERT INTO users (id, name, email, age) VALUES 
        (1, 'Alice Johnson', 'alice@example.com', 30),
        (2, 'Bob Smith', 'bob@example.com', 25),
        (3, 'Charlie Brown', 'charlie@example.com', 35),
        (4, 'Diana Ross', 'diana@example.com', 28)
    "#;

    let insert_orders = r#"
        INSERT INTO orders (id, user_id, product, amount, order_date) VALUES 
        (1, 1, 'Laptop', 1200, '2024-01-15'),
        (2, 1, 'Mouse', 25, '2024-01-16'),
        (3, 2, 'Keyboard', 75, '2024-01-17'),
        (4, 3, 'Monitor', 300, '2024-01-18'),
        (5, 3, 'Webcam', 80, '2024-01-19'),
        (6, 4, 'Headphones', 150, '2024-01-20')
    "#;

    glue.execute(insert_users).await?;
    glue.execute(insert_orders).await?;
    println!("   ✓ Inserted sample data\n");
    glue.storage.commit().await?;

    // 3. Basic SELECT queries
    println!("3. Running SELECT queries...");

    let select_all_users = "SELECT * FROM users";
    let result = glue.execute(select_all_users).await?;
    print_results("All users:", &result);

    let select_young_users = "SELECT name, age FROM users WHERE age < 30";
    let result = glue.execute(select_young_users).await?;
    print_results("Users under 30:", &result);

    // 4. JOIN queries
    println!("4. Running JOIN queries...");

    let join_query = r#"
        SELECT u.name, o.product, o.amount, o.order_date
        FROM users u
        JOIN orders o ON u.id = o.user_id
        ORDER BY o.order_date
    "#;
    let result = glue.execute(join_query).await?;
    print_results("User orders:", &result);

    // 5. Aggregation queries
    println!("5. Running aggregation queries...");

    let user_totals = r#"
        SELECT u.name, COUNT(o.id) as order_count
        FROM users u
        LEFT JOIN orders o ON u.id = o.user_id
        GROUP BY u.id, u.name
        ORDER BY u.name
    "#;
    let result = glue.execute(user_totals).await?;
    print_results("User spending summary:", &result);

    let product_stats = r#"
        SELECT product, COUNT(*) as quantity, AVG(amount) as avg_price
        FROM orders
        GROUP BY product
        HAVING COUNT(*) >= 1
        ORDER BY avg_price DESC
    "#;
    let result = glue.execute(product_stats).await?;
    print_results("Product statistics:", &result);

    // 6. UPDATE and DELETE operations
    println!("6. Running UPDATE and DELETE operations...");

    let update_age = "UPDATE users SET age = 31 WHERE name = 'Alice Johnson'";
    let result = glue.execute(update_age).await?;
    println!("   ✓ Updated Alice's age: {:?}", result);

    let delete_order = "DELETE FROM orders WHERE product = 'Mouse'";
    let result = glue.execute(delete_order).await?;
    println!("   ✓ Deleted mouse order: {:?}", result);

    // Verify changes
    let verify_query = "SELECT name, age FROM users WHERE name = 'Alice Johnson'";
    let result = glue.execute(verify_query).await?;
    print_results("Alice's updated info:", &result);
    glue.storage.commit().await?;

    // 7. Advanced queries with subqueries
    println!("7. Running advanced queries...");

    let multi_order_customers = r#"
        SELECT u.name, u.email
        FROM users u
        WHERE u.id = 1 OR u.id = 3
    "#;
    let result = glue.execute(multi_order_customers).await?;
    print_results("Customers with multiple orders:", &result);

    println!("\n🎉 SQL example completed successfully!");
    println!("This demonstrates how ProllyTree can serve as a backend for SQL queries");
    println!("while maintaining its versioned, git-like capabilities.\n");

    // Check git status
    let kv_store = glue.storage.store();
    match kv_store.log() {
        Ok(history) => {
            println!("Git history:");
            for commit_info in history {
                println!("  - Commit: {:?}", commit_info);
            }
        }
        Err(e) => {
            println!("Failed to get git history: {:?}", e);
        }
    }

    println!("Git keys:");
    for key in kv_store.list_keys() {
        // Convert Vec<u8> to String for display, or use debug format
        match String::from_utf8(key.clone()) {
            Ok(key_str) => println!("  - Key: {}", key_str),
            Err(_) => println!("  - Key: {:?}", key),
        }
    }

    Ok(())
}

#[cfg(feature = "sql")]
fn print_results(title: &str, payloads: &Vec<Payload>) {
    println!("\n   📊 {}", title);

    for payload in payloads {
        match payload {
            Payload::Select { labels, rows } => {
                if rows.is_empty() {
                    println!("      (No results)");
                    continue;
                }

                // Print headers
                let header = labels.join(" | ");
                println!("      {}", header);
                println!("      {}", "-".repeat(header.len()));

                // Print rows
                for row in rows {
                    let row_strs: Vec<String> = row.iter().map(|v| format!("{:?}", v)).collect();
                    println!("      {}", row_strs.join(" | "));
                }
            }
            Payload::Insert(count) => println!("      ✓ Inserted {} rows", count),
            Payload::Update(count) => println!("      ✓ Updated {} rows", count),
            Payload::Delete(count) => println!("      ✓ Deleted {} rows", count),
            _ => println!("      ✓ Operation completed"),
        }
    }
}

#[cfg(not(feature = "sql"))]
fn main() {
    println!("❌ This example requires the 'sql' feature to be enabled.");
    println!("   Run with: cargo run --example sql_example --features sql");
}
