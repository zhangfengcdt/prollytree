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
mod integration_tests {
    use gluesql_core::{error::Result, executor::Payload, prelude::Glue};
    use prollytree::sql::ProllyStorage;
    use tempfile::TempDir;

    async fn setup_test_db() -> Result<(TempDir, Glue<ProllyStorage<32>>)> {
        let temp_dir = TempDir::new().map_err(|e| {
            gluesql_core::error::Error::StorageMsg(format!("Failed to create temp dir: {}", e))
        })?;

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

        let storage = ProllyStorage::<32>::init(&dataset_path)?;
        let glue = Glue::new(storage);

        Ok((temp_dir, glue))
    }

    #[tokio::test]
    async fn test_create_table_and_insert() -> Result<()> {
        let (_temp_dir, mut glue) = setup_test_db().await?;

        // Create table
        let create_sql = r#"
            CREATE TABLE test_table (
                id INTEGER,
                name TEXT,
                value INTEGER
            )
        "#;

        let result = glue.execute(create_sql).await?;
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], Payload::Create));

        // Insert data
        let insert_sql = r#"
            INSERT INTO test_table (id, name, value) VALUES
            (1, 'first', 100),
            (2, 'second', 200),
            (3, 'third', 300)
        "#;

        let result = glue.execute(insert_sql).await?;
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], Payload::Insert(3)));

        Ok(())
    }

    #[tokio::test]
    async fn test_select_operations() -> Result<()> {
        let (_temp_dir, mut glue) = setup_test_db().await?;

        // Setup data
        glue.execute(
            r#"
            CREATE TABLE products (
                id INTEGER,
                name TEXT,
                price INTEGER,
                category TEXT
            )
        "#,
        )
        .await?;

        glue.execute(
            r#"
            INSERT INTO products (id, name, price, category) VALUES
            (1, 'Laptop', 1000, 'Electronics'),
            (2, 'Book', 20, 'Education'),
            (3, 'Phone', 800, 'Electronics'),
            (4, 'Notebook', 5, 'Education')
        "#,
        )
        .await?;

        // Test SELECT *
        let result = glue.execute("SELECT * FROM products").await?;
        if let Payload::Select { labels, rows } = &result[0] {
            assert_eq!(labels.len(), 4);
            assert_eq!(rows.len(), 4);
            assert_eq!(labels, &vec!["id", "name", "price", "category"]);
        } else {
            panic!("Expected Select payload");
        }

        // Test SELECT with WHERE
        let result = glue
            .execute("SELECT name, price FROM products WHERE category = 'Electronics'")
            .await?;
        if let Payload::Select { labels, rows } = &result[0] {
            assert_eq!(labels, &vec!["name", "price"]);
            assert_eq!(rows.len(), 2);
        } else {
            panic!("Expected Select payload");
        }

        // Test ORDER BY
        let result = glue
            .execute("SELECT name FROM products ORDER BY price")
            .await?;
        if let Payload::Select { labels, rows } = &result[0] {
            assert_eq!(labels, &vec!["name"]);
            assert_eq!(rows.len(), 4);
        } else {
            panic!("Expected Select payload");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_join_operations() -> Result<()> {
        let (_temp_dir, mut glue) = setup_test_db().await?;

        // Setup tables
        glue.execute(
            r#"
            CREATE TABLE customers (
                id INTEGER,
                name TEXT,
                email TEXT
            )
        "#,
        )
        .await?;

        glue.execute(
            r#"
            CREATE TABLE orders (
                id INTEGER,
                customer_id INTEGER,
                product TEXT,
                amount INTEGER
            )
        "#,
        )
        .await?;

        // Insert data
        glue.execute(
            r#"
            INSERT INTO customers (id, name, email) VALUES
            (1, 'Alice', 'alice@example.com'),
            (2, 'Bob', 'bob@example.com')
        "#,
        )
        .await?;

        glue.execute(
            r#"
            INSERT INTO orders (id, customer_id, product, amount) VALUES
            (1, 1, 'Laptop', 1000),
            (2, 1, 'Mouse', 50),
            (3, 2, 'Keyboard', 100)
        "#,
        )
        .await?;

        // Test JOIN
        let result = glue
            .execute(
                r#"
            SELECT c.name, o.product, o.amount
            FROM customers c
            JOIN orders o ON c.id = o.customer_id
            ORDER BY c.name, o.product
        "#,
            )
            .await?;

        if let Payload::Select { labels, rows } = &result[0] {
            assert_eq!(labels, &vec!["name", "product", "amount"]);
            assert_eq!(rows.len(), 3);
        } else {
            panic!("Expected Select payload");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_update_delete_operations() -> Result<()> {
        let (_temp_dir, mut glue) = setup_test_db().await?;

        // Setup data
        glue.execute(
            r#"
            CREATE TABLE items (
                id INTEGER,
                name TEXT,
                quantity INTEGER
            )
        "#,
        )
        .await?;

        glue.execute(
            r#"
            INSERT INTO items (id, name, quantity) VALUES
            (1, 'Item1', 10),
            (2, 'Item2', 20),
            (3, 'Item3', 30)
        "#,
        )
        .await?;

        // Test UPDATE
        let result = glue
            .execute("UPDATE items SET quantity = 15 WHERE id = 1")
            .await?;
        assert!(matches!(result[0], Payload::Update(1)));

        // Verify update
        let result = glue
            .execute("SELECT quantity FROM items WHERE id = 1")
            .await?;
        if let Payload::Select { rows, .. } = &result[0] {
            assert_eq!(rows.len(), 1);
        }

        // Test DELETE
        let result = glue.execute("DELETE FROM items WHERE id = 3").await?;
        assert!(matches!(result[0], Payload::Delete(1)));

        // Verify delete
        let result = glue.execute("SELECT * FROM items").await?;
        if let Payload::Select { rows, .. } = &result[0] {
            assert_eq!(rows.len(), 2);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_aggregation_operations() -> Result<()> {
        let (_temp_dir, mut glue) = setup_test_db().await?;

        // Setup data
        glue.execute(
            r#"
            CREATE TABLE sales (
                id INTEGER,
                product TEXT,
                quantity INTEGER,
                price INTEGER
            )
        "#,
        )
        .await?;

        glue.execute(
            r#"
            INSERT INTO sales (id, product, quantity, price) VALUES
            (1, 'A', 2, 100),
            (2, 'B', 1, 200),
            (3, 'A', 3, 150),
            (4, 'C', 1, 300)
        "#,
        )
        .await?;

        // Test COUNT
        let result = glue.execute("SELECT COUNT(id) FROM sales").await?;
        if let Payload::Select { rows, .. } = &result[0] {
            assert_eq!(rows.len(), 1);
        }

        // Test GROUP BY with COUNT
        let result = glue
            .execute("SELECT product, COUNT(id) FROM sales GROUP BY product ORDER BY product")
            .await?;
        if let Payload::Select { labels, rows } = &result[0] {
            assert_eq!(labels, &vec!["product", "COUNT(id)"]);
            assert_eq!(rows.len(), 3); // A, B, C
        }

        // Test AVG
        let result = glue.execute("SELECT AVG(price) FROM sales").await?;
        if let Payload::Select { rows, .. } = &result[0] {
            assert_eq!(rows.len(), 1);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_schema_operations() -> Result<()> {
        let (_temp_dir, mut glue) = setup_test_db().await?;

        // Create multiple tables
        glue.execute(
            r#"
            CREATE TABLE table1 (
                id INTEGER,
                name TEXT
            )
        "#,
        )
        .await?;

        glue.execute(
            r#"
            CREATE TABLE table2 (
                id INTEGER,
                value INTEGER
            )
        "#,
        )
        .await?;

        // Test that we can query both tables
        let result = glue.execute("SELECT * FROM table1").await?;
        assert!(matches!(result[0], Payload::Select { .. }));

        let result = glue.execute("SELECT * FROM table2").await?;
        assert!(matches!(result[0], Payload::Select { .. }));

        Ok(())
    }
}
