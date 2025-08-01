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

//! GlueSQL custom storage implementation using ProllyTree
//!
//! This module implements the GlueSQL Store and StoreMut traits to provide
//! SQL query capabilities over ProllyTree's versioned key-value store.

#[cfg(feature = "sql")]
use std::collections::HashMap;

#[cfg(feature = "sql")]
use async_trait::async_trait;
#[cfg(feature = "sql")]
use futures::stream::iter;
#[cfg(feature = "sql")]
use gluesql_core::{
    data::{Key, Schema},
    error::{Error, Result},
    store::{
        AlterTable, CustomFunction, CustomFunctionMut, DataRow, Index, IndexMut, Metadata, RowIter,
        Store, StoreMut, Transaction,
    },
};

#[cfg(feature = "sql")]
use crate::git::versioned_store::GitVersionedKvStore;

/// GlueSQL storage backend using ProllyTree
#[cfg(feature = "sql")]
pub struct ProllyStorage<const D: usize> {
    store: GitVersionedKvStore<D>,
    schemas: HashMap<String, Schema>,
}

#[cfg(feature = "sql")]
impl<const D: usize> ProllyStorage<D> {
    /// Create a new ProllyStorage instance
    pub fn new(store: GitVersionedKvStore<D>) -> Self {
        Self {
            store,
            schemas: HashMap::new(),
        }
    }

    /// Initialize with a path
    #[allow(clippy::result_large_err)]
    pub fn init(path: &std::path::Path) -> Result<Self> {
        let dir = path.to_path_buf();
        let dir_string = dir.to_string_lossy().to_string();
        let store = GitVersionedKvStore::init(path).map_err(|e| {
            Error::StorageMsg(format!("Failed to initialize store: {e} from {dir_string}"))
        })?;
        Ok(Self::new(store))
    }

    /// Open an existing storage
    #[allow(clippy::result_large_err)]
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let store = GitVersionedKvStore::open(path)
            .map_err(|e| Error::StorageMsg(format!("Failed to open store: {e}")))?;
        Ok(Self::new(store))
    }

    // returns the underlying store
    pub fn store(&self) -> &GitVersionedKvStore<D> {
        &self.store
    }

    /// Convert table name and row key to storage key
    fn make_storage_key(table_name: &str, key: &Key) -> Vec<u8> {
        match key {
            Key::I64(id) => format!("{table_name}:{id}").into_bytes(),
            Key::Str(id) => format!("{table_name}:{id}").into_bytes(),
            Key::None => format!("{table_name}:__schema__").into_bytes(),
            _ => format!("{table_name}:{key:?}").into_bytes(),
        }
    }

    /// Get schema key for a table
    fn schema_key(table_name: &str) -> Vec<u8> {
        Self::make_storage_key(table_name, &Key::None)
    }

    /// Parse key from storage key string
    fn parse_key_from_storage_key(storage_key: &[u8], table_prefix: &str) -> Key {
        let key_str = String::from_utf8_lossy(storage_key);
        let key_part = key_str
            .strip_prefix(&format!("{table_prefix}:"))
            .unwrap_or("");

        if let Ok(id) = key_part.parse::<i64>() {
            Key::I64(id)
        } else {
            Key::Str(key_part.to_string())
        }
    }

    /// Commit with a custom message
    pub async fn commit_with_message(&mut self, message: &str) -> Result<()> {
        // Commit changes to the git repository with custom message
        self.store
            .commit(message)
            .map_err(|e| Error::StorageMsg(format!("Failed to commit: {e}")))?;
        Ok(())
    }
}

// Implement all the required traits
#[cfg(feature = "sql")]
impl<const D: usize> AlterTable for ProllyStorage<D> {}
#[cfg(feature = "sql")]
impl<const D: usize> Index for ProllyStorage<D> {}
#[cfg(feature = "sql")]
impl<const D: usize> IndexMut for ProllyStorage<D> {}
#[cfg(feature = "sql")]
impl<const D: usize> Metadata for ProllyStorage<D> {}
#[cfg(feature = "sql")]
impl<const D: usize> CustomFunction for ProllyStorage<D> {}
#[cfg(feature = "sql")]
impl<const D: usize> CustomFunctionMut for ProllyStorage<D> {}

#[cfg(feature = "sql")]
#[async_trait(?Send)]
impl<const D: usize> Store for ProllyStorage<D> {
    async fn fetch_all_schemas(&self) -> Result<Vec<Schema>> {
        let all_keys = self.store.list_keys();
        let mut schemas = Vec::new();

        for storage_key in all_keys {
            if storage_key.ends_with(b":__schema__") {
                if let Some(schema_data) = self.store.get(&storage_key) {
                    let schema: Schema = serde_json::from_slice(&schema_data).map_err(|e| {
                        Error::StorageMsg(format!("Failed to deserialize schema: {e}"))
                    })?;
                    schemas.push(schema);
                }
            }
        }

        schemas.sort_by(|a, b| a.table_name.cmp(&b.table_name));
        Ok(schemas)
    }

    async fn fetch_schema(&self, table_name: &str) -> Result<Option<Schema>> {
        let key = Self::schema_key(table_name);

        if let Some(schema_data) = self.store.get(&key) {
            let schema: Schema = serde_json::from_slice(&schema_data)
                .map_err(|e| Error::StorageMsg(format!("Failed to deserialize schema: {e}")))?;
            Ok(Some(schema))
        } else {
            Ok(None)
        }
    }

    async fn fetch_data(&self, table_name: &str, key: &Key) -> Result<Option<DataRow>> {
        let storage_key = Self::make_storage_key(table_name, key);

        if let Some(row_data) = self.store.get(&storage_key) {
            let row: DataRow = serde_json::from_slice(&row_data)
                .map_err(|e| Error::StorageMsg(format!("Failed to deserialize row: {e}")))?;
            Ok(Some(row))
        } else {
            Ok(None)
        }
    }

    async fn scan_data<'a>(&'a self, table_name: &str) -> Result<RowIter> {
        let prefix = format!("{table_name}:");
        let prefix_bytes = prefix.as_bytes();
        let table_name = table_name.to_string();

        // Get all keys that start with the table prefix
        let all_keys = self.store.list_keys();
        let mut rows = Vec::new();

        for storage_key in all_keys {
            if storage_key.starts_with(prefix_bytes) {
                // Skip schema entries
                if storage_key.ends_with(b":__schema__") {
                    continue;
                }

                if let Some(row_data) = self.store.get(&storage_key) {
                    let row: DataRow = serde_json::from_slice(&row_data).map_err(|e| {
                        Error::StorageMsg(format!("Failed to deserialize row: {e}"))
                    })?;

                    let key = Self::parse_key_from_storage_key(&storage_key, &table_name);
                    rows.push(Ok((key, row)));
                }
            }
        }

        rows.sort_by(|a, b| match (a, b) {
            (Ok((key_a, _)), Ok((key_b, _))) => key_a.cmp(key_b),
            _ => std::cmp::Ordering::Equal,
        });

        Ok(Box::pin(iter(rows)))
    }
}

#[cfg(feature = "sql")]
#[async_trait(?Send)]
impl<const D: usize> StoreMut for ProllyStorage<D> {
    async fn insert_schema(&mut self, schema: &Schema) -> Result<()> {
        let key = Self::schema_key(&schema.table_name);
        let schema_data = serde_json::to_vec(schema)
            .map_err(|e| Error::StorageMsg(format!("Failed to serialize schema: {e}")))?;

        self.store
            .insert(key, schema_data)
            .map_err(|e| Error::StorageMsg(format!("Failed to insert schema: {e}")))?;

        // Cache the schema
        self.schemas
            .insert(schema.table_name.clone(), schema.clone());

        Ok(())
    }

    async fn delete_schema(&mut self, table_name: &str) -> Result<()> {
        let key = Self::schema_key(table_name);

        let _ = self
            .store
            .delete(&key)
            .map_err(|e| Error::StorageMsg(format!("Failed to delete schema: {e}")))?;

        // Remove from cache
        self.schemas.remove(table_name);

        Ok(())
    }

    async fn append_data(&mut self, table_name: &str, rows: Vec<DataRow>) -> Result<()> {
        for row in rows {
            // Generate a key for the row (using a simple counter approach)
            let mut counter = 0i64;
            let storage_key = loop {
                let key = Key::I64(counter);
                let storage_key = Self::make_storage_key(table_name, &key);

                if self.store.get(&storage_key).is_none() {
                    break storage_key;
                }
                counter += 1;
            };

            let row_data = serde_json::to_vec(&row)
                .map_err(|e| Error::StorageMsg(format!("Failed to serialize row: {e}")))?;

            self.store
                .insert(storage_key, row_data)
                .map_err(|e| Error::StorageMsg(format!("Failed to insert row: {e}")))?;
        }

        Ok(())
    }

    async fn insert_data(&mut self, table_name: &str, rows: Vec<(Key, DataRow)>) -> Result<()> {
        for (key, row) in rows {
            let storage_key = Self::make_storage_key(table_name, &key);
            let row_data = serde_json::to_vec(&row)
                .map_err(|e| Error::StorageMsg(format!("Failed to serialize row: {e}")))?;

            self.store
                .insert(storage_key, row_data)
                .map_err(|e| Error::StorageMsg(format!("Failed to insert row: {e}")))?;
        }

        Ok(())
    }

    async fn delete_data(&mut self, table_name: &str, keys: Vec<Key>) -> Result<()> {
        for key in keys {
            let storage_key = Self::make_storage_key(table_name, &key);

            let _ = self
                .store
                .delete(&storage_key)
                .map_err(|e| Error::StorageMsg(format!("Failed to delete row: {e}")))?;
        }

        Ok(())
    }
}

#[cfg(feature = "sql")]
#[async_trait(?Send)]
impl<const D: usize> Transaction for ProllyStorage<D> {
    async fn begin(&mut self, autocommit: bool) -> Result<bool> {
        if autocommit {
            return Ok(false);
        }

        // ProllyTree with git backend doesn't support nested transactions
        // Always return false to indicate no transaction was started
        Ok(false)
    }

    async fn rollback(&mut self) -> Result<()> {
        // Since we don't support transactions, rollback is a no-op
        // In a real implementation, you might want to reset to the last commit
        Ok(())
    }

    async fn commit(&mut self) -> Result<()> {
        // Commit changes to the git repository
        self.store
            .commit("Transaction commit")
            .map_err(|e| Error::StorageMsg(format!("Failed to commit transaction: {e}")))?;
        Ok(())
    }
}

#[cfg(all(test, feature = "sql"))]
mod tests {
    use super::*;
    use gluesql_core::{
        ast::{ColumnDef, DataType},
        data::{Key, Schema, Value},
        store::DataRow,
    };
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_basic_operations() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repository in temp directory
        std::process::Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to initialize git repository");

        // Create a subdirectory for the dataset
        let dataset_path = temp_dir.path().join("dataset");
        std::fs::create_dir(&dataset_path).unwrap();

        let mut storage = ProllyStorage::<32>::init(&dataset_path).unwrap();

        // Create a simple schema
        let schema = Schema {
            table_name: "users".to_string(),
            column_defs: Some(vec![
                ColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Int,
                    nullable: false,
                    default: None,
                    unique: None,
                },
                ColumnDef {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                    nullable: false,
                    default: None,
                    unique: None,
                },
            ]),
            indexes: vec![],
            engine: None,
        };

        // Insert schema
        storage.insert_schema(&schema).await.unwrap();

        // Verify schema
        let fetched = storage.fetch_schema("users").await.unwrap();
        assert!(fetched.is_some());

        // Insert some data
        let row = DataRow::Vec(vec![Value::I64(1), Value::Str("Alice".to_string())]);
        let key = Key::I64(1);
        storage
            .insert_data("users", vec![(key.clone(), row.clone())])
            .await
            .unwrap();

        // Fetch data
        let fetched_row = storage.fetch_data("users", &key).await.unwrap();
        assert!(fetched_row.is_some());

        // Scan data
        use futures::StreamExt;
        let mut iter = storage.scan_data("users").await.unwrap();
        let first = iter.next().await.unwrap().unwrap();
        assert_eq!(first.0, key);
    }
}
