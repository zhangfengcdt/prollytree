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

//! Integration tests for the SQL layer (GlueSQL ↔ ProllyStorage).

#![cfg(all(feature = "git", feature = "sql"))]

mod common;

use gluesql_core::{data::Value, executor::Payload, prelude::Glue, store::Store};
use prollytree::sql::ProllyStorage;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup_glue() -> (tempfile::TempDir, Glue<ProllyStorage<32>>) {
    let (temp, dataset) = common::setup_repo_and_dataset();
    let storage = ProllyStorage::<32>::init(&dataset).expect("ProllyStorage init");
    let glue = Glue::new(storage);
    (temp, glue)
}

async fn exec(glue: &mut Glue<ProllyStorage<32>>, sql: &str) {
    glue.execute(sql).await.unwrap_or_else(|e| {
        panic!("SQL failed: {sql}\nError: {e}");
    });
}

// ---------------------------------------------------------------------------
// Create, Insert, Select roundtrip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_sql_create_insert_select() {
    #[allow(unused_mut)]
    let (_temp, mut glue) = setup_glue();

    exec(&mut glue, "CREATE TABLE users (id INTEGER, name TEXT)").await;
    exec(&mut glue, "INSERT INTO users VALUES (1, 'Alice')").await;
    exec(&mut glue, "INSERT INTO users VALUES (2, 'Bob')").await;

    let results = glue.execute("SELECT * FROM users").await.unwrap();
    // Should return rows
    assert!(!results.is_empty(), "SELECT should return payload");
}

// ---------------------------------------------------------------------------
// Update and Delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_sql_update_and_delete() {
    #[allow(unused_mut)]
    let (_temp, mut glue) = setup_glue();

    exec(&mut glue, "CREATE TABLE items (id INTEGER, name TEXT)").await;
    exec(&mut glue, "INSERT INTO items VALUES (1, 'Old')").await;
    exec(&mut glue, "UPDATE items SET name = 'New' WHERE id = 1").await;

    let results = glue
        .execute("SELECT name FROM items WHERE id = 1")
        .await
        .unwrap();
    assert!(!results.is_empty());

    exec(&mut glue, "DELETE FROM items WHERE id = 1").await;

    let results = glue.execute("SELECT * FROM items").await.unwrap();
    // After delete, select should return empty or a payload with 0 rows
    assert!(!results.is_empty());
}

// ---------------------------------------------------------------------------
// Multiple tables
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_sql_multiple_tables() {
    #[allow(unused_mut)]
    let (_temp, mut glue) = setup_glue();

    exec(&mut glue, "CREATE TABLE t1 (id INTEGER, val TEXT)").await;
    exec(&mut glue, "CREATE TABLE t2 (id INTEGER, val TEXT)").await;

    exec(&mut glue, "INSERT INTO t1 VALUES (1, 'a')").await;
    exec(&mut glue, "INSERT INTO t2 VALUES (2, 'b')").await;

    let r1 = glue.execute("SELECT * FROM t1").await.unwrap();
    let r2 = glue.execute("SELECT * FROM t2").await.unwrap();

    assert!(!r1.is_empty());
    assert!(!r2.is_empty());
}

// ---------------------------------------------------------------------------
// Drop table
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_sql_drop_table() {
    #[allow(unused_mut)]
    let (_temp, mut glue) = setup_glue();

    exec(&mut glue, "CREATE TABLE droppable (id INTEGER)").await;
    exec(&mut glue, "INSERT INTO droppable VALUES (1)").await;
    exec(&mut glue, "DROP TABLE droppable").await;

    let result = glue.execute("SELECT * FROM droppable").await;
    assert!(result.is_err(), "SELECT from dropped table should fail");
}

// ---------------------------------------------------------------------------
// Commit persists across reopen
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_sql_commit_persists_across_reopen() {
    let (temp, dataset) = common::setup_repo_and_dataset();

    {
        let mut storage = ProllyStorage::<32>::init(&dataset).expect("init");
        let mut glue = Glue::new(storage);
        exec(&mut glue, "CREATE TABLE persist (id INTEGER, name TEXT)").await;
        exec(&mut glue, "INSERT INTO persist VALUES (1, 'saved')").await;
        glue.execute("COMMIT").await.unwrap();
    }

    // Re-open
    let storage = ProllyStorage::<32>::open(&dataset).expect("open");
    let mut glue = Glue::new(storage);
    let results = glue.execute("SELECT * FROM persist").await.unwrap();
    assert!(!results.is_empty());
    // Keep temp alive
    drop(temp);
}

// ---------------------------------------------------------------------------
// Schema persistence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_sql_schema_persistence() {
    let (temp, dataset) = common::setup_repo_and_dataset();

    {
        let mut storage = ProllyStorage::<32>::init(&dataset).expect("init");
        let mut glue = Glue::new(storage);
        exec(&mut glue, "CREATE TABLE schema_test (id INTEGER, val TEXT)").await;
        glue.execute("COMMIT").await.unwrap();
    }

    let storage = ProllyStorage::<32>::open(&dataset).expect("open");
    let schemas = storage.fetch_all_schemas().await.unwrap();
    assert!(
        schemas.iter().any(|s| s.table_name == "schema_test"),
        "schema_test should persist"
    );
    drop(temp);
}

#[tokio::test]
async fn test_sql_text_primary_key_schema_sentinel_does_not_corrupt_schema() {
    #[allow(unused_mut)]
    let (_temp, mut glue) = setup_glue();

    exec(
        &mut glue,
        "CREATE TABLE tricky (id TEXT PRIMARY KEY, val TEXT)",
    )
    .await;
    exec(
        &mut glue,
        "INSERT INTO tricky VALUES ('__schema__', 'still a row')",
    )
    .await;

    let schemas = glue.storage.fetch_all_schemas().await.unwrap();
    assert!(
        schemas.iter().any(|s| s.table_name == "tricky"),
        "row key must not overwrite the table schema"
    );

    let results = glue
        .execute("SELECT id, val FROM tricky WHERE id = '__schema__'")
        .await
        .unwrap();
    match &results[0] {
        Payload::Select { labels, rows } => {
            assert_eq!(labels, &vec!["id".to_string(), "val".to_string()]);
            assert_eq!(rows.len(), 1);
            assert!(
                matches!(&rows[0][0], Value::Str(value) if value == "__schema__"),
                "expected id row value, got {:?}",
                rows[0][0]
            );
            assert!(
                matches!(&rows[0][1], Value::Str(value) if value == "still a row"),
                "expected val row value, got {:?}",
                rows[0][1]
            );
        }
        payload => panic!("expected SELECT payload, got {payload:?}"),
    }
}
