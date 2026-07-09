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

// PyO3 0.22 `#[pymethods]` expansion inserts `.into()` on PyErr-returning results,
// which clippy flags as `useless_conversion`. The generated code is not under our
// control, so suppress the lint at the module level.
#![allow(clippy::useless_conversion)]

use parking_lot::Mutex;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyBytesMethods, PyDict, PyList};
use pyo3::IntoPyObjectExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    config::TreeConfig,
    git::{
        types::{DiffOperation, StorageBackend},
        versioned_store::{
            FileVersionedKvStore, GitNamespacedKvStore, GitVersionedKvStore, HistoricalAccess,
            HistoricalCommitAccess, InMemoryVersionedKvStore, ThreadSafeGitVersionedKvStore,
        },
    },
    proof::Proof,
    storage::{FileNodeStorage, InMemoryNodeStorage},
    tree::{ProllyTree, Tree},
};

#[cfg(feature = "rocksdb_storage")]
use crate::git::versioned_store::RocksDBVersionedKvStore;

#[cfg(feature = "sql")]
use crate::sql::ProllyStorage;
#[cfg(feature = "sql")]
use gluesql_core::{data::Value as SqlValue, executor::Payload, prelude::Glue};

// Maximum number of keys that can be retrieved in a single operation
const MAX_KEYS_LIMIT: usize = 1024;

#[pyclass(name = "TreeConfig")]
struct PyTreeConfig {
    base: u64,
    modulus: u64,
    min_chunk_size: usize,
    max_chunk_size: usize,
    pattern: u64,
}

#[pymethods]
impl PyTreeConfig {
    #[new]
    #[pyo3(signature = (base=4, modulus=64, min_chunk_size=1, max_chunk_size=4096, pattern=0))]
    fn new(
        base: u64,
        modulus: u64,
        min_chunk_size: usize,
        max_chunk_size: usize,
        pattern: u64,
    ) -> Self {
        PyTreeConfig {
            base,
            modulus,
            min_chunk_size,
            max_chunk_size,
            pattern,
        }
    }
}

enum ProllyTreeWrapper {
    Memory(ProllyTree<32, InMemoryNodeStorage<32>>),
    File(ProllyTree<32, FileNodeStorage<32>>),
}

macro_rules! with_tree {
    ($self:expr, $tree:ident, $body:expr) => {
        match &*$self {
            ProllyTreeWrapper::Memory($tree) => $body,
            ProllyTreeWrapper::File($tree) => $body,
        }
    };
}

macro_rules! with_tree_mut {
    ($self:expr, $tree:ident, $body:expr) => {
        match &mut *$self {
            ProllyTreeWrapper::Memory($tree) => $body,
            ProllyTreeWrapper::File($tree) => $body,
        }
    };
}

#[pyclass(name = "ProllyTree")]
struct PyProllyTree {
    tree: Arc<Mutex<ProllyTreeWrapper>>,
}

#[pymethods]
impl PyProllyTree {
    #[new]
    #[pyo3(signature = (storage_type="memory", path=None, config=None))]
    fn new(
        storage_type: &str,
        path: Option<String>,
        config: Option<&PyTreeConfig>,
    ) -> PyResult<Self> {
        let tree_config = if let Some(py_config) = config {
            TreeConfig::<32> {
                base: py_config.base,
                modulus: py_config.modulus,
                min_chunk_size: py_config.min_chunk_size,
                max_chunk_size: py_config.max_chunk_size,
                pattern: py_config.pattern,
                root_hash: None,
                key_schema: None,
                value_schema: None,
                encode_types: vec![],
            }
        } else {
            TreeConfig::<32>::default()
        };

        let tree_wrapper = match storage_type {
            "memory" => {
                let storage = InMemoryNodeStorage::<32>::new();
                let tree = ProllyTree::<32, _>::new(storage, tree_config);
                ProllyTreeWrapper::Memory(tree)
            }
            "file" => {
                let path =
                    path.ok_or_else(|| PyValueError::new_err("File storage requires a path"))?;
                let storage = FileNodeStorage::<32>::new(PathBuf::from(path)).map_err(|e| {
                    PyValueError::new_err(format!("Failed to create file storage: {e}"))
                })?;
                let tree = ProllyTree::<32, _>::new(storage, tree_config);
                ProllyTreeWrapper::File(tree)
            }
            _ => {
                return Err(PyValueError::new_err(
                    "Invalid storage type. Use 'memory' or 'file'",
                ))
            }
        };

        Ok(PyProllyTree {
            tree: Arc::new(Mutex::new(tree_wrapper)),
        })
    }

    fn insert(
        &mut self,
        py: Python,
        key: &Bound<'_, PyBytes>,
        value: &Bound<'_, PyBytes>,
    ) -> PyResult<()> {
        let key_vec = key.as_bytes().to_vec();
        let value_vec = value.as_bytes().to_vec();

        py.detach(|| {
            let mut tree_wrapper = self.tree.lock();
            with_tree_mut!(tree_wrapper, tree, {
                tree.insert(key_vec, value_vec);
                Ok(())
            })
        })
    }

    fn insert_batch(&mut self, py: Python, items: Vec<(Vec<u8>, Vec<u8>)>) -> PyResult<()> {
        let keys: Vec<Vec<u8>> = items.iter().map(|(k, _)| k.clone()).collect();
        let values: Vec<Vec<u8>> = items.iter().map(|(_, v)| v.clone()).collect();

        py.detach(|| {
            let mut tree_wrapper = self.tree.lock();
            with_tree_mut!(tree_wrapper, tree, {
                tree.insert_batch(&keys, &values);
                Ok(())
            })
        })
    }

    fn find(&self, py: Python, key: &Bound<'_, PyBytes>) -> PyResult<Option<Py<PyBytes>>> {
        let key_vec = key.as_bytes().to_vec();

        let result = py.detach(|| {
            let tree_wrapper = self.tree.lock();
            with_tree!(tree_wrapper, tree, { tree.find(&key_vec) })
        });

        match result {
            Some(node) => {
                // Find the key in the node and return the corresponding value
                if let Some(key_index) = node.keys.iter().position(|k| k == &key_vec) {
                    if key_index < node.values.len() {
                        Ok(Some(PyBytes::new(py, &node.values[key_index]).into()))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    fn update(
        &mut self,
        py: Python,
        key: &Bound<'_, PyBytes>,
        value: &Bound<'_, PyBytes>,
    ) -> PyResult<()> {
        let key_vec = key.as_bytes().to_vec();
        let value_vec = value.as_bytes().to_vec();

        py.detach(|| {
            let mut tree_wrapper = self.tree.lock();
            with_tree_mut!(tree_wrapper, tree, {
                tree.update(key_vec, value_vec);
                Ok(())
            })
        })
    }

    fn delete(&mut self, py: Python, key: &Bound<'_, PyBytes>) -> PyResult<()> {
        let key_vec = key.as_bytes().to_vec();

        py.detach(|| {
            let mut tree_wrapper = self.tree.lock();
            with_tree_mut!(tree_wrapper, tree, {
                tree.delete(&key_vec);
                Ok(())
            })
        })
    }

    fn delete_batch(&mut self, py: Python, keys: Vec<Vec<u8>>) -> PyResult<()> {
        let key_vecs: Vec<Vec<u8>> = keys;

        py.detach(|| {
            let mut tree_wrapper = self.tree.lock();
            with_tree_mut!(tree_wrapper, tree, {
                tree.delete_batch(&key_vecs);
                Ok(())
            })
        })
    }

    fn size(&self) -> PyResult<usize> {
        let tree_wrapper = self.tree.lock();
        Ok(with_tree!(tree_wrapper, tree, tree.size()))
    }

    fn depth(&self) -> PyResult<usize> {
        let tree_wrapper = self.tree.lock();
        Ok(with_tree!(tree_wrapper, tree, tree.depth()))
    }

    fn get_root_hash(&self, py: Python) -> PyResult<Py<PyBytes>> {
        let tree_wrapper = self.tree.lock();
        let hash_opt = with_tree!(tree_wrapper, tree, tree.get_root_hash());
        match hash_opt {
            Some(hash) => Ok(PyBytes::new(py, hash.as_ref()).into()),
            None => Ok(PyBytes::new(py, &[0u8; 32]).into()),
        }
    }

    fn stats(&self) -> PyResult<HashMap<String, usize>> {
        let tree_wrapper = self.tree.lock();
        let stats = with_tree!(tree_wrapper, tree, tree.stats());
        let mut map = HashMap::new();
        map.insert("num_nodes".to_string(), stats.num_nodes);
        map.insert("num_leaves".to_string(), stats.num_leaves);
        map.insert("num_internal_nodes".to_string(), stats.num_internal_nodes);
        map.insert("avg_node_size".to_string(), stats.avg_node_size as usize);
        map.insert(
            "total_key_value_pairs".to_string(),
            stats.total_key_value_pairs,
        );
        Ok(map)
    }

    fn generate_proof(&self, py: Python, key: &Bound<'_, PyBytes>) -> PyResult<Py<PyBytes>> {
        let key_vec = key.as_bytes().to_vec();

        let proof_bytes = py.detach(|| {
            let tree_wrapper = self.tree.lock();
            let proof = with_tree!(tree_wrapper, tree, tree.generate_proof(&key_vec));

            crate::serde_bincode::serialize(&proof)
                .map_err(|e| PyValueError::new_err(format!("Proof serialization failed: {}", e)))
        })?;

        Ok(PyBytes::new(py, &proof_bytes).into())
    }

    #[pyo3(signature = (proof_bytes, key, expected_value=None))]
    fn verify_proof(
        &self,
        py: Python,
        proof_bytes: &Bound<'_, PyBytes>,
        key: &Bound<'_, PyBytes>,
        expected_value: Option<&Bound<'_, PyBytes>>,
    ) -> PyResult<bool> {
        let key_vec = key.as_bytes().to_vec();
        let proof_vec = proof_bytes.as_bytes().to_vec();
        let value_option = expected_value.map(|v| v.as_bytes().to_vec());

        py.detach(|| {
            let proof: Proof<32> = crate::serde_bincode::deserialize(&proof_vec).map_err(|e| {
                PyValueError::new_err(format!("Proof deserialization failed: {}", e))
            })?;

            let tree_wrapper = self.tree.lock();
            Ok(with_tree!(
                tree_wrapper,
                tree,
                tree.verify(proof, &key_vec, value_option.as_deref())
            ))
        })
    }

    fn diff(&self, py: Python, _other: &PyProllyTree) -> PyResult<Py<PyDict>> {
        // Implement a key-value level diff by comparing actual data
        // This approach works regardless of tree structure differences

        let dict = PyDict::new(py);
        let added = PyDict::new(py);
        let removed = PyDict::new(py);
        let modified = PyDict::new(py);

        // We'll need to collect all keys from both trees and compare values
        // For simplicity, we'll implement this by getting all key-value pairs
        // This is not the most efficient approach, but it works correctly

        // Note: This is a simplified implementation. A proper implementation
        // would traverse both trees simultaneously, but that requires more
        // complex logic to handle different tree structures.

        // For now, let's disable the diff functionality and return empty results
        // until we can implement a proper key-value level diff

        dict.set_item("added", added)?;
        dict.set_item("removed", removed)?;
        dict.set_item("modified", modified)?;

        Ok(dict.into())
    }

    fn traverse(&self) -> PyResult<String> {
        let tree_wrapper = self.tree.lock();
        Ok(with_tree!(tree_wrapper, tree, tree.traverse()))
    }

    fn save_config(&self, py: Python) -> PyResult<()> {
        py.detach(|| {
            let tree_wrapper = self.tree.lock();
            with_tree!(tree_wrapper, tree, {
                let _ = tree.save_config();
                Ok(())
            })
        })
    }
}

#[pyclass(name = "StorageBackend", eq, eq_int, from_py_object)]
#[derive(Clone, PartialEq)]
enum PyStorageBackend {
    InMemory,
    File,
    Git,
    RocksDB,
}

#[pymethods]
impl PyStorageBackend {
    fn __str__(&self) -> &str {
        match self {
            PyStorageBackend::InMemory => "InMemory",
            PyStorageBackend::File => "File",
            PyStorageBackend::Git => "Git",
            PyStorageBackend::RocksDB => "RocksDB",
        }
    }
}

// Note: We don't implement From<PyStorageBackend> for StorageBackend because
// the RocksDB case requires error handling when the feature is disabled.
// Instead, the conversion is handled directly in PyVersionedKvStore::new() and open().

impl From<StorageBackend> for PyStorageBackend {
    fn from(backend: StorageBackend) -> Self {
        match backend {
            StorageBackend::InMemory => PyStorageBackend::InMemory,
            StorageBackend::File => PyStorageBackend::File,
            StorageBackend::Git => PyStorageBackend::Git,
            #[cfg(feature = "rocksdb_storage")]
            StorageBackend::RocksDB => PyStorageBackend::RocksDB,
        }
    }
}

/// Wrapper enum for different VersionedKvStore storage backends
enum VersionedKvStoreWrapper {
    Git(GitVersionedKvStore<32>),
    File(FileVersionedKvStore<32>),
    InMemory(InMemoryVersionedKvStore<32>),
    #[cfg(feature = "rocksdb_storage")]
    RocksDB(RocksDBVersionedKvStore<32>),
}

/// Macro for dispatching operations to the correct storage backend
macro_rules! with_versioned_store {
    ($self:expr, $store:ident, $body:expr) => {
        match &*$self {
            VersionedKvStoreWrapper::Git($store) => $body,
            VersionedKvStoreWrapper::File($store) => $body,
            VersionedKvStoreWrapper::InMemory($store) => $body,
            #[cfg(feature = "rocksdb_storage")]
            VersionedKvStoreWrapper::RocksDB($store) => $body,
        }
    };
}

/// Macro for dispatching mutable operations to the correct storage backend
macro_rules! with_versioned_store_mut {
    ($self:expr, $store:ident, $body:expr) => {
        match &mut *$self {
            VersionedKvStoreWrapper::Git($store) => $body,
            VersionedKvStoreWrapper::File($store) => $body,
            VersionedKvStoreWrapper::InMemory($store) => $body,
            #[cfg(feature = "rocksdb_storage")]
            VersionedKvStoreWrapper::RocksDB($store) => $body,
        }
    };
}

/// Python wrapper for MergeConflict
#[pyclass(name = "MergeConflict", from_py_object)]
#[derive(Clone)]
struct PyMergeConflict {
    key: Vec<u8>,
    base_value: Option<Vec<u8>>,
    source_value: Option<Vec<u8>>,
    destination_value: Option<Vec<u8>>,
}

#[pymethods]
impl PyMergeConflict {
    #[getter]
    fn key(&self, py: Python) -> PyResult<Py<PyBytes>> {
        Ok(PyBytes::new(py, &self.key).into())
    }

    #[getter]
    fn base_value(&self, py: Python) -> PyResult<Option<Py<PyBytes>>> {
        Ok(self.base_value.as_ref().map(|v| PyBytes::new(py, v).into()))
    }

    #[getter]
    fn source_value(&self, py: Python) -> PyResult<Option<Py<PyBytes>>> {
        Ok(self
            .source_value
            .as_ref()
            .map(|v| PyBytes::new(py, v).into()))
    }

    #[getter]
    fn destination_value(&self, py: Python) -> PyResult<Option<Py<PyBytes>>> {
        Ok(self
            .destination_value
            .as_ref()
            .map(|v| PyBytes::new(py, v).into()))
    }

    fn __repr__(&self) -> String {
        format!(
            "MergeConflict(key={:?}, base={:?}, source={:?}, dest={:?})",
            String::from_utf8_lossy(&self.key),
            self.base_value
                .as_ref()
                .map(|v| String::from_utf8_lossy(v).to_string()),
            self.source_value
                .as_ref()
                .map(|v| String::from_utf8_lossy(v).to_string()),
            self.destination_value
                .as_ref()
                .map(|v| String::from_utf8_lossy(v).to_string()),
        )
    }
}

/// Python enum for conflict resolution strategies
#[pyclass(name = "ConflictResolution", eq, eq_int, from_py_object)]
#[derive(Clone, PartialEq)]
enum PyConflictResolution {
    IgnoreAll,
    TakeSource,
    TakeDestination,
}

/// Python wrapper for DiffOperation
#[pyclass(name = "DiffOperation", from_py_object)]
#[derive(Clone)]
struct PyDiffOperation {
    operation_type: String,
    value: Option<Vec<u8>>,
    old_value: Option<Vec<u8>>,
    new_value: Option<Vec<u8>>,
}

#[pymethods]
impl PyDiffOperation {
    #[getter]
    fn operation_type(&self) -> String {
        self.operation_type.clone()
    }

    #[getter]
    fn value(&self, py: Python) -> PyResult<Option<Py<PyBytes>>> {
        Ok(self.value.as_ref().map(|v| PyBytes::new(py, v).into()))
    }

    #[getter]
    fn old_value(&self, py: Python) -> PyResult<Option<Py<PyBytes>>> {
        Ok(self.old_value.as_ref().map(|v| PyBytes::new(py, v).into()))
    }

    #[getter]
    fn new_value(&self, py: Python) -> PyResult<Option<Py<PyBytes>>> {
        Ok(self.new_value.as_ref().map(|v| PyBytes::new(py, v).into()))
    }

    fn __repr__(&self) -> String {
        match self.operation_type.as_str() {
            "Added" => format!(
                "DiffOperation.Added(value_size={})",
                self.value.as_ref().map_or(0, |v| v.len())
            ),
            "Removed" => format!(
                "DiffOperation.Removed(value_size={})",
                self.value.as_ref().map_or(0, |v| v.len())
            ),
            "Modified" => format!(
                "DiffOperation.Modified(old_size={}, new_size={})",
                self.old_value.as_ref().map_or(0, |v| v.len()),
                self.new_value.as_ref().map_or(0, |v| v.len())
            ),
            _ => "DiffOperation.Unknown".to_string(),
        }
    }
}

/// Python wrapper for KvDiff
#[pyclass(name = "KvDiff", from_py_object)]
#[derive(Clone)]
struct PyKvDiff {
    key: Vec<u8>,
    operation: PyDiffOperation,
}

#[pymethods]
impl PyKvDiff {
    #[getter]
    fn key(&self, py: Python) -> PyResult<Py<PyBytes>> {
        Ok(PyBytes::new(py, &self.key).into())
    }

    #[getter]
    fn operation(&self) -> PyDiffOperation {
        self.operation.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "KvDiff(key={:?}, operation={})",
            String::from_utf8_lossy(&self.key),
            self.operation.__repr__()
        )
    }
}

#[pyclass(name = "VersionedKvStore")]
struct PyVersionedKvStore {
    inner: Arc<Mutex<VersionedKvStoreWrapper>>,
}

#[pymethods]
impl PyVersionedKvStore {
    #[new]
    #[pyo3(signature = (path, storage_backend=None))]
    fn new(path: String, storage_backend: Option<PyStorageBackend>) -> PyResult<Self> {
        let backend = storage_backend.unwrap_or(PyStorageBackend::Git);
        let wrapper = match backend {
            PyStorageBackend::Git => {
                let store = GitVersionedKvStore::<32>::init(&path).map_err(|e| {
                    PyValueError::new_err(format!("Failed to initialize Git store: {}", e))
                })?;
                VersionedKvStoreWrapper::Git(store)
            }
            PyStorageBackend::File => {
                let store = FileVersionedKvStore::<32>::init(&path).map_err(|e| {
                    PyValueError::new_err(format!("Failed to initialize File store: {}", e))
                })?;
                VersionedKvStoreWrapper::File(store)
            }
            PyStorageBackend::InMemory => {
                let store = InMemoryVersionedKvStore::<32>::init(&path).map_err(|e| {
                    PyValueError::new_err(format!("Failed to initialize InMemory store: {}", e))
                })?;
                VersionedKvStoreWrapper::InMemory(store)
            }
            #[cfg(feature = "rocksdb_storage")]
            PyStorageBackend::RocksDB => {
                let store = RocksDBVersionedKvStore::<32>::init(&path).map_err(|e| {
                    PyValueError::new_err(format!("Failed to initialize RocksDB store: {}", e))
                })?;
                VersionedKvStoreWrapper::RocksDB(store)
            }
            #[cfg(not(feature = "rocksdb_storage"))]
            PyStorageBackend::RocksDB => {
                return Err(PyValueError::new_err(
                    "RocksDB storage backend requires 'rocksdb_storage' feature to be enabled",
                ));
            }
        };

        Ok(PyVersionedKvStore {
            inner: Arc::new(Mutex::new(wrapper)),
        })
    }

    #[staticmethod]
    #[pyo3(signature = (path, storage_backend=None))]
    fn open(path: String, storage_backend: Option<PyStorageBackend>) -> PyResult<Self> {
        let backend = storage_backend.unwrap_or(PyStorageBackend::Git);
        let wrapper = match backend {
            PyStorageBackend::Git => {
                let store = GitVersionedKvStore::<32>::open(&path).map_err(|e| {
                    PyValueError::new_err(format!("Failed to open Git store: {}", e))
                })?;
                VersionedKvStoreWrapper::Git(store)
            }
            PyStorageBackend::File => {
                let store = FileVersionedKvStore::<32>::open(&path).map_err(|e| {
                    PyValueError::new_err(format!("Failed to open File store: {}", e))
                })?;
                VersionedKvStoreWrapper::File(store)
            }
            PyStorageBackend::InMemory => {
                let store = InMemoryVersionedKvStore::<32>::open(&path).map_err(|e| {
                    PyValueError::new_err(format!("Failed to open InMemory store: {}", e))
                })?;
                VersionedKvStoreWrapper::InMemory(store)
            }
            #[cfg(feature = "rocksdb_storage")]
            PyStorageBackend::RocksDB => {
                let store = RocksDBVersionedKvStore::<32>::open(&path).map_err(|e| {
                    PyValueError::new_err(format!("Failed to open RocksDB store: {}", e))
                })?;
                VersionedKvStoreWrapper::RocksDB(store)
            }
            #[cfg(not(feature = "rocksdb_storage"))]
            PyStorageBackend::RocksDB => {
                return Err(PyValueError::new_err(
                    "RocksDB storage backend requires 'rocksdb_storage' feature to be enabled",
                ));
            }
        };

        Ok(PyVersionedKvStore {
            inner: Arc::new(Mutex::new(wrapper)),
        })
    }

    fn insert(&self, key: &Bound<'_, PyBytes>, value: &Bound<'_, PyBytes>) -> PyResult<()> {
        let key_vec = key.as_bytes().to_vec();
        let value_vec = value.as_bytes().to_vec();

        let mut guard = self.inner.lock();
        with_versioned_store_mut!(guard, store, {
            store
                .insert(key_vec, value_vec)
                .map_err(|e| PyValueError::new_err(format!("Failed to insert: {}", e)))?;
            Ok(())
        })
    }

    fn get(&self, py: Python, key: &Bound<'_, PyBytes>) -> PyResult<Option<Py<PyBytes>>> {
        let key_vec = key.as_bytes().to_vec();

        let guard = self.inner.lock();
        with_versioned_store!(guard, store, {
            match store.get(&key_vec) {
                Some(value) => Ok(Some(PyBytes::new(py, &value).into())),
                None => Ok(None),
            }
        })
    }

    fn update(&self, key: &Bound<'_, PyBytes>, value: &Bound<'_, PyBytes>) -> PyResult<bool> {
        let key_vec = key.as_bytes().to_vec();
        let value_vec = value.as_bytes().to_vec();

        let mut guard = self.inner.lock();
        with_versioned_store_mut!(guard, store, {
            store
                .update(key_vec, value_vec)
                .map_err(|e| PyValueError::new_err(format!("Failed to update: {}", e)))
        })
    }

    fn delete(&self, key: &Bound<'_, PyBytes>) -> PyResult<bool> {
        let key_vec = key.as_bytes().to_vec();

        let mut guard = self.inner.lock();
        with_versioned_store_mut!(guard, store, {
            store
                .delete(&key_vec)
                .map_err(|e| PyValueError::new_err(format!("Failed to delete: {}", e)))
        })
    }

    fn list_keys(&self, py: Python) -> PyResult<Vec<Py<PyBytes>>> {
        let guard = self.inner.lock();
        with_versioned_store!(guard, store, {
            let keys = store.list_keys();

            let total_keys = keys.len();
            if total_keys > MAX_KEYS_LIMIT {
                eprintln!(
                    "Warning: Tree contains {} keys, but only returning first {} keys due to limit. \
                    Consider using more specific queries or implementing pagination.",
                    total_keys, MAX_KEYS_LIMIT
                );
            }

            let py_keys: Vec<Py<PyBytes>> = keys
                .iter()
                .take(MAX_KEYS_LIMIT)
                .map(|key| PyBytes::new(py, key).into())
                .collect();

            Ok(py_keys)
        })
    }

    fn status(&self, py: Python) -> PyResult<Vec<(Py<PyBytes>, String)>> {
        let guard = self.inner.lock();
        with_versioned_store!(guard, store, {
            let status = store.status();

            let py_status: Vec<(Py<PyBytes>, String)> = status
                .iter()
                .map(|(key, status_str)| (PyBytes::new(py, key).into(), status_str.clone()))
                .collect();

            Ok(py_status)
        })
    }

    fn commit(&self, message: String) -> PyResult<String> {
        let mut guard = self.inner.lock();
        with_versioned_store_mut!(guard, store, {
            let commit_id = store
                .commit(&message)
                .map_err(|e| PyValueError::new_err(format!("Failed to commit: {}", e)))?;

            Ok(commit_id.to_hex().to_string())
        })
    }

    fn branch(&self, name: String) -> PyResult<()> {
        let mut guard = self.inner.lock();
        with_versioned_store_mut!(guard, store, {
            store
                .branch(&name)
                .map_err(|e| PyValueError::new_err(format!("Failed to create branch: {}", e)))?;

            Ok(())
        })
    }

    fn create_branch(&self, name: String) -> PyResult<()> {
        let mut guard = self.inner.lock();
        with_versioned_store_mut!(guard, store, {
            store.create_branch(&name).map_err(|e| {
                PyValueError::new_err(format!("Failed to create and switch branch: {}", e))
            })?;

            Ok(())
        })
    }

    fn checkout(&self, branch_or_commit: String) -> PyResult<()> {
        let mut guard = self.inner.lock();
        // All backends support checkout because they all use git for version control
        with_versioned_store_mut!(guard, store, {
            store
                .checkout_generic(&branch_or_commit)
                .map_err(|e| PyValueError::new_err(format!("Failed to checkout: {}", e)))?;
            Ok(())
        })
    }

    fn current_branch(&self) -> PyResult<String> {
        let guard = self.inner.lock();
        with_versioned_store!(guard, store, { Ok(store.current_branch().to_string()) })
    }

    fn list_branches(&self) -> PyResult<Vec<String>> {
        let guard = self.inner.lock();
        with_versioned_store!(guard, store, {
            store
                .list_branches()
                .map_err(|e| PyValueError::new_err(format!("Failed to list branches: {}", e)))
        })
    }

    fn log(&self) -> PyResult<Vec<HashMap<String, Py<PyAny>>>> {
        // Collect commit data under lock, then release before Python GIL operations
        // to avoid potential deadlock between mutex and GIL
        let commits_data: Vec<(String, String, String, String, i64)> = {
            let guard = self.inner.lock();
            with_versioned_store!(guard, store, {
                let history = store
                    .log()
                    .map_err(|e| PyValueError::new_err(format!("Failed to get log: {}", e)))?;

                let data: Vec<_> = history
                    .iter()
                    .map(|commit| {
                        (
                            commit.id.to_hex().to_string(),
                            commit.author.clone(),
                            commit.committer.clone(),
                            commit.message.clone(),
                            commit.timestamp,
                        )
                    })
                    .collect();
                Ok::<_, PyErr>(data)
            })?
        };

        // Now convert to Python objects without holding the store lock
        Python::attach(|py| {
            let results: PyResult<Vec<HashMap<String, Py<PyAny>>>> = commits_data
                .into_iter()
                .map(|(id, author, committer, message, timestamp)| {
                    let mut map = HashMap::new();
                    map.insert("id".to_string(), id.into_py_any(py).unwrap());
                    map.insert("author".to_string(), author.into_py_any(py).unwrap());
                    map.insert("committer".to_string(), committer.into_py_any(py).unwrap());
                    map.insert("message".to_string(), message.into_py_any(py).unwrap());
                    map.insert("timestamp".to_string(), timestamp.into_py_any(py).unwrap());
                    Ok(map)
                })
                .collect();
            results
        })
    }

    fn get_commits_for_key(
        &self,
        key: &Bound<'_, PyBytes>,
    ) -> PyResult<Vec<HashMap<String, Py<PyAny>>>> {
        let key_vec = key.as_bytes().to_vec();

        // All backends now support get_commits_for_key because they all write config to dataset_dir
        // which is tracked in git commits
        let commits_data: Vec<(String, String, String, String, i64)> = {
            let guard = self.inner.lock();
            with_versioned_store!(guard, store, {
                let commits = store.get_commits_for_key(&key_vec).map_err(|e| {
                    PyValueError::new_err(format!("Failed to get commits for key: {}", e))
                })?;

                let data: Vec<_> = commits
                    .iter()
                    .map(|commit| {
                        (
                            commit.id.to_hex().to_string(),
                            commit.author.clone(),
                            commit.committer.clone(),
                            commit.message.clone(),
                            commit.timestamp,
                        )
                    })
                    .collect();
                Ok::<_, PyErr>(data)
            })?
        };

        // Now convert to Python objects without holding the store lock
        Python::attach(|py| {
            let results: PyResult<Vec<HashMap<String, Py<PyAny>>>> = commits_data
                .into_iter()
                .map(|(id, author, committer, message, timestamp)| {
                    let mut map = HashMap::new();
                    map.insert("id".to_string(), id.into_py_any(py).unwrap());
                    map.insert("author".to_string(), author.into_py_any(py).unwrap());
                    map.insert("committer".to_string(), committer.into_py_any(py).unwrap());
                    map.insert("message".to_string(), message.into_py_any(py).unwrap());
                    map.insert("timestamp".to_string(), timestamp.into_py_any(py).unwrap());
                    Ok(map)
                })
                .collect();
            results
        })
    }

    fn get_commit_history(&self) -> PyResult<Vec<HashMap<String, Py<PyAny>>>> {
        // Collect commit data under lock, then release before Python GIL operations
        // to avoid potential deadlock between mutex and GIL
        let commits_data: Vec<(String, String, String, String, i64)> = {
            let guard = self.inner.lock();
            with_versioned_store!(guard, store, {
                let commits = store.get_commit_history().map_err(|e| {
                    PyValueError::new_err(format!("Failed to get commit history: {}", e))
                })?;

                let data: Vec<_> = commits
                    .iter()
                    .map(|commit| {
                        (
                            commit.id.to_hex().to_string(),
                            commit.author.clone(),
                            commit.committer.clone(),
                            commit.message.clone(),
                            commit.timestamp,
                        )
                    })
                    .collect();
                Ok::<_, PyErr>(data)
            })?
        };

        // Now convert to Python objects without holding the store lock
        Python::attach(|py| {
            let results: PyResult<Vec<HashMap<String, Py<PyAny>>>> = commits_data
                .into_iter()
                .map(|(id, author, committer, message, timestamp)| {
                    let mut map = HashMap::new();
                    map.insert("id".to_string(), id.into_py_any(py).unwrap());
                    map.insert("author".to_string(), author.into_py_any(py).unwrap());
                    map.insert("committer".to_string(), committer.into_py_any(py).unwrap());
                    map.insert("message".to_string(), message.into_py_any(py).unwrap());
                    map.insert("timestamp".to_string(), timestamp.into_py_any(py).unwrap());
                    Ok(map)
                })
                .collect();
            results
        })
    }

    /// Merge another branch into the current branch
    ///
    /// Args:
    ///     source_branch: Name of the branch to merge from
    ///     conflict_resolution: Strategy for resolving conflicts (default: IgnoreAll)
    ///
    /// Returns:
    ///     str: The commit ID of the merge commit
    ///
    /// Raises:
    ///     ValueError: If merge fails, has unresolved conflicts, or storage backend doesn't support merge
    #[pyo3(signature = (source_branch, conflict_resolution=None))]
    fn merge(
        &self,
        source_branch: String,
        conflict_resolution: Option<PyConflictResolution>,
    ) -> PyResult<String> {
        let mut guard = self.inner.lock();
        let resolution = conflict_resolution.unwrap_or(PyConflictResolution::IgnoreAll);

        // All backends support merge because they all use git for version control
        with_versioned_store_mut!(guard, store, {
            let commit_id = match resolution {
                PyConflictResolution::IgnoreAll => {
                    let resolver = crate::diff::IgnoreConflictsResolver;
                    store
                        .merge_generic(&source_branch, &resolver)
                        .map_err(|e| PyValueError::new_err(format!("Merge failed: {}", e)))?
                }
                PyConflictResolution::TakeSource => {
                    let resolver = crate::diff::TakeSourceResolver;
                    store
                        .merge_generic(&source_branch, &resolver)
                        .map_err(|e| PyValueError::new_err(format!("Merge failed: {}", e)))?
                }
                PyConflictResolution::TakeDestination => {
                    let resolver = crate::diff::TakeDestinationResolver;
                    store
                        .merge_generic(&source_branch, &resolver)
                        .map_err(|e| PyValueError::new_err(format!("Merge failed: {}", e)))?
                }
            };
            Ok(commit_id.to_hex().to_string())
        })
    }

    /// Attempt to merge another branch and return any conflicts
    ///
    /// Args:
    ///     source_branch: Name of the branch to merge from
    ///
    /// Returns:
    ///     tuple: (success: bool, conflicts: List[MergeConflict])
    ///            If success is True, conflicts will be empty and merge was applied
    ///            If success is False, conflicts contains unresolved conflicts and merge was not applied
    fn try_merge(&self, source_branch: String) -> PyResult<(bool, Vec<PyMergeConflict>)> {
        let mut guard = self.inner.lock();

        // All backends support try_merge because they all use git for version control
        with_versioned_store_mut!(guard, store, {
            match store.try_merge_generic(&source_branch) {
                Ok(_commit_id) => {
                    // Merge succeeded with no conflicts
                    Ok((true, Vec::new()))
                }
                Err(crate::git::types::GitKvError::MergeConflictError(conflicts)) => {
                    // Convert conflicts to Python format
                    let py_conflicts: Vec<PyMergeConflict> = conflicts
                        .into_iter()
                        .map(|c| PyMergeConflict {
                            key: c.key,
                            base_value: c.base_value,
                            source_value: c.source_value,
                            destination_value: c.destination_value,
                        })
                        .collect();
                    Ok((false, py_conflicts))
                }
                Err(e) => Err(PyValueError::new_err(format!("Merge failed: {}", e))),
            }
        })
    }

    fn storage_backend(&self) -> PyResult<PyStorageBackend> {
        let guard = self.inner.lock();
        with_versioned_store!(guard, store, { Ok(store.storage_backend().clone().into()) })
    }

    fn generate_proof(&self, py: Python, key: &Bound<'_, PyBytes>) -> PyResult<Py<PyBytes>> {
        let key_vec = key.as_bytes().to_vec();

        let proof_bytes = py.detach(|| {
            let guard = self.inner.lock();
            with_versioned_store!(guard, store, {
                let proof = store.generate_proof(&key_vec);
                crate::serde_bincode::serialize(&proof).map_err(|e| {
                    PyValueError::new_err(format!("Proof serialization failed: {}", e))
                })
            })
        })?;

        Ok(PyBytes::new(py, &proof_bytes).into())
    }

    #[pyo3(signature = (proof_bytes, key, expected_value=None))]
    fn verify_proof(
        &self,
        py: Python,
        proof_bytes: &Bound<'_, PyBytes>,
        key: &Bound<'_, PyBytes>,
        expected_value: Option<&Bound<'_, PyBytes>>,
    ) -> PyResult<bool> {
        let key_vec = key.as_bytes().to_vec();
        let proof_vec = proof_bytes.as_bytes().to_vec();
        let value_option = expected_value.map(|v| v.as_bytes().to_vec());

        py.detach(|| {
            let proof: crate::proof::Proof<32> = crate::serde_bincode::deserialize(&proof_vec)
                .map_err(|e| {
                    PyValueError::new_err(format!("Proof deserialization failed: {}", e))
                })?;

            let guard = self.inner.lock();
            with_versioned_store!(guard, store, {
                Ok(store.verify(proof, &key_vec, value_option.as_deref()))
            })
        })
    }

    fn get_keys_at_ref(
        &self,
        py: Python,
        reference: String,
    ) -> PyResult<Vec<(Py<PyBytes>, Py<PyBytes>)>> {
        let guard = self.inner.lock();

        // All backends now support get_keys_at_ref because they all write config to dataset_dir
        // which is committed to git history.
        with_versioned_store!(guard, store, {
            let keys_map = HistoricalAccess::get_keys_at_ref(store, &reference)
                .map_err(|e| PyValueError::new_err(format!("Failed to get keys at ref: {}", e)))?;

            let total_keys = keys_map.len();
            if total_keys > MAX_KEYS_LIMIT {
                eprintln!(
                    "Warning: Tree contains {} keys, but only returning first {} keys due to limit. \
                    Consider using more specific queries or implementing pagination.",
                    total_keys, MAX_KEYS_LIMIT
                );
            }

            let py_pairs: Vec<(Py<PyBytes>, Py<PyBytes>)> = keys_map
                .into_iter()
                .take(MAX_KEYS_LIMIT)
                .map(|(key, value): (Vec<u8>, Vec<u8>)| {
                    (
                        PyBytes::new(py, &key).into(),
                        PyBytes::new(py, &value).into(),
                    )
                })
                .collect();

            Ok(py_pairs)
        })
    }

    /// Compare two commits or branches and return all keys that are added, updated or deleted
    ///
    /// Args:
    ///     from_ref: Reference (branch or commit) to compare from
    ///     to_ref: Reference (branch or commit) to compare to
    ///
    /// Returns:
    ///     List[KvDiff]: List of differences between the two references
    fn diff(&self, from_ref: String, to_ref: String) -> PyResult<Vec<PyKvDiff>> {
        let guard = self.inner.lock();

        // All backends support diff because they all implement HistoricalAccess
        // which provides get_keys_at_ref, and diff is built on top of that.
        with_versioned_store!(guard, store, {
            let diffs = store
                .diff(&from_ref, &to_ref)
                .map_err(|e| PyValueError::new_err(format!("Failed to compute diff: {}", e)))?;

            let py_diffs: Vec<PyKvDiff> = diffs
                .into_iter()
                .map(|diff| {
                    let operation = match diff.operation {
                        DiffOperation::Added(value) => PyDiffOperation {
                            operation_type: "Added".to_string(),
                            value: Some(value),
                            old_value: None,
                            new_value: None,
                        },
                        DiffOperation::Removed(value) => PyDiffOperation {
                            operation_type: "Removed".to_string(),
                            value: Some(value),
                            old_value: None,
                            new_value: None,
                        },
                        DiffOperation::Modified { old, new } => PyDiffOperation {
                            operation_type: "Modified".to_string(),
                            value: None,
                            old_value: Some(old),
                            new_value: Some(new),
                        },
                    };

                    PyKvDiff {
                        key: diff.key,
                        operation,
                    }
                })
                .collect();

            Ok(py_diffs)
        })
    }

    /// Get the current commit's object ID
    ///
    /// Returns:
    ///     str: The hexadecimal string representation of the current commit ID
    fn current_commit(&self) -> PyResult<String> {
        let guard = self.inner.lock();

        with_versioned_store!(guard, store, {
            let commit_id = store.current_commit().map_err(|e| {
                PyValueError::new_err(format!("Failed to get current commit: {}", e))
            })?;

            Ok(commit_id.to_hex().to_string())
        })
    }
}

#[cfg(feature = "git")]
#[pyclass(name = "WorktreeManager")]
struct PyWorktreeManager {
    inner: Arc<Mutex<crate::git::worktree::WorktreeManager>>,
}

#[cfg(feature = "git")]
#[pymethods]
impl PyWorktreeManager {
    #[new]
    fn new(repo_path: String) -> PyResult<Self> {
        let manager = crate::git::worktree::WorktreeManager::new(repo_path).map_err(|e| {
            PyValueError::new_err(format!("Failed to create worktree manager: {}", e))
        })?;

        Ok(PyWorktreeManager {
            inner: Arc::new(Mutex::new(manager)),
        })
    }

    fn add_worktree(
        &self,
        path: String,
        branch: String,
        create_branch: bool,
    ) -> PyResult<HashMap<String, Py<PyAny>>> {
        let mut manager = self.inner.lock();
        let info = manager
            .add_worktree(path, &branch, create_branch)
            .map_err(|e| PyValueError::new_err(format!("Failed to add worktree: {}", e)))?;

        Python::attach(|py| {
            let mut map = HashMap::new();
            map.insert("id".to_string(), info.id.into_py_any(py).unwrap());
            map.insert(
                "path".to_string(),
                info.path.to_string_lossy().into_py_any(py).unwrap(),
            );
            map.insert("branch".to_string(), info.branch.into_py_any(py).unwrap());
            map.insert(
                "is_linked".to_string(),
                info.is_linked.into_py_any(py).unwrap(),
            );
            Ok(map)
        })
    }

    fn remove_worktree(&self, worktree_id: String) -> PyResult<()> {
        let mut manager = self.inner.lock();
        manager
            .remove_worktree(&worktree_id)
            .map_err(|e| PyValueError::new_err(format!("Failed to remove worktree: {}", e)))?;
        Ok(())
    }

    fn lock_worktree(&self, worktree_id: String, reason: String) -> PyResult<()> {
        let mut manager = self.inner.lock();
        manager
            .lock_worktree(&worktree_id, &reason)
            .map_err(|e| PyValueError::new_err(format!("Failed to lock worktree: {}", e)))?;
        Ok(())
    }

    fn unlock_worktree(&self, worktree_id: String) -> PyResult<()> {
        let mut manager = self.inner.lock();
        manager
            .unlock_worktree(&worktree_id)
            .map_err(|e| PyValueError::new_err(format!("Failed to unlock worktree: {}", e)))?;
        Ok(())
    }

    fn list_worktrees(&self) -> PyResult<Vec<HashMap<String, Py<PyAny>>>> {
        let manager = self.inner.lock();
        let worktrees = manager.list_worktrees();

        Python::attach(|py| {
            let results: Vec<HashMap<String, Py<PyAny>>> = worktrees
                .iter()
                .map(|info| {
                    let mut map = HashMap::new();
                    map.insert("id".to_string(), info.id.clone().into_py_any(py).unwrap());
                    map.insert(
                        "path".to_string(),
                        info.path.to_string_lossy().into_py_any(py).unwrap(),
                    );
                    map.insert(
                        "branch".to_string(),
                        info.branch.clone().into_py_any(py).unwrap(),
                    );
                    map.insert(
                        "is_linked".to_string(),
                        info.is_linked.into_py_any(py).unwrap(),
                    );
                    map
                })
                .collect();
            Ok(results)
        })
    }

    fn is_locked(&self, worktree_id: String) -> PyResult<bool> {
        let manager = self.inner.lock();
        Ok(manager.is_locked(&worktree_id))
    }

    /// Merge a worktree branch back to main branch
    fn merge_to_main(&self, worktree_id: String, commit_message: String) -> PyResult<String> {
        let mut manager = self.inner.lock();
        manager
            .merge_to_main(&worktree_id, &commit_message)
            .map_err(|e| PyValueError::new_err(format!("Failed to merge to main: {}", e)))
    }

    /// Merge a worktree branch to another target branch
    fn merge_branch(
        &self,
        source_worktree_id: String,
        target_branch: String,
        commit_message: String,
    ) -> PyResult<String> {
        let mut manager = self.inner.lock();
        manager
            .merge_branch(&source_worktree_id, &target_branch, &commit_message)
            .map_err(|e| PyValueError::new_err(format!("Failed to merge branch: {}", e)))
    }

    /// Get the current commit hash of a branch
    fn get_branch_commit(&self, branch: String) -> PyResult<String> {
        let manager = self.inner.lock();
        manager
            .get_branch_commit(&branch)
            .map_err(|e| PyValueError::new_err(format!("Failed to get branch commit: {}", e)))
    }

    /// List all branches in the repository
    fn list_branches(&self) -> PyResult<Vec<String>> {
        let manager = self.inner.lock();
        manager
            .list_branches()
            .map_err(|e| PyValueError::new_err(format!("Failed to list branches: {}", e)))
    }
}

#[cfg(feature = "git")]
#[pyclass(name = "WorktreeVersionedKvStore")]
struct PyWorktreeVersionedKvStore {
    inner: Arc<Mutex<crate::git::worktree::WorktreeVersionedKvStore<32>>>,
}

#[cfg(feature = "git")]
#[pymethods]
impl PyWorktreeVersionedKvStore {
    #[staticmethod]
    fn from_worktree(
        worktree_path: String,
        worktree_id: String,
        branch: String,
        manager: &PyWorktreeManager,
    ) -> PyResult<Self> {
        use std::path::PathBuf;

        let worktree_info = crate::git::worktree::WorktreeInfo {
            id: worktree_id,
            path: PathBuf::from(worktree_path),
            branch,
            is_linked: true,
            lock_file: None,
        };

        let store = crate::git::worktree::WorktreeVersionedKvStore::from_worktree(
            worktree_info,
            Arc::clone(&manager.inner),
        )
        .map_err(|e| PyValueError::new_err(format!("Failed to create worktree store: {}", e)))?;

        Ok(PyWorktreeVersionedKvStore {
            inner: Arc::new(Mutex::new(store)),
        })
    }

    fn worktree_id(&self) -> PyResult<String> {
        let store = self.inner.lock();
        Ok(store.worktree_id().to_string())
    }

    fn current_branch(&self) -> PyResult<String> {
        let store = self.inner.lock();
        Ok(store.current_branch().to_string())
    }

    fn is_locked(&self) -> PyResult<bool> {
        let store = self.inner.lock();
        Ok(store.is_locked())
    }

    fn lock(&self, reason: String) -> PyResult<()> {
        let store = self.inner.lock();
        store
            .lock(&reason)
            .map_err(|e| PyValueError::new_err(format!("Failed to lock worktree: {}", e)))?;
        Ok(())
    }

    fn unlock(&self) -> PyResult<()> {
        let store = self.inner.lock();
        store
            .unlock()
            .map_err(|e| PyValueError::new_err(format!("Failed to unlock worktree: {}", e)))?;
        Ok(())
    }

    // Delegate key-value operations to the underlying store
    fn insert(&self, key: Vec<u8>, value: Vec<u8>) -> PyResult<()> {
        let mut store = self.inner.lock();
        store
            .store_mut()
            .insert(key, value)
            .map_err(|e| PyValueError::new_err(format!("Failed to insert: {}", e)))?;
        Ok(())
    }

    fn get(&self, key: Vec<u8>) -> PyResult<Option<Vec<u8>>> {
        let store = self.inner.lock();
        Ok(store.store().get(&key))
    }

    fn delete(&self, key: Vec<u8>) -> PyResult<bool> {
        let mut store = self.inner.lock();
        let result = store
            .store_mut()
            .delete(&key)
            .map_err(|e| PyValueError::new_err(format!("Failed to delete: {}", e)))?;
        Ok(result)
    }

    fn commit(&self, message: String) -> PyResult<String> {
        let mut store = self.inner.lock();
        let commit_id = store
            .store_mut()
            .commit(&message)
            .map_err(|e| PyValueError::new_err(format!("Failed to commit: {}", e)))?;
        Ok(commit_id.to_hex().to_string())
    }

    fn list_keys(&self) -> PyResult<Vec<Vec<u8>>> {
        let store = self.inner.lock();
        let keys = store.store().list_keys();

        let total_keys = keys.len();
        if total_keys > MAX_KEYS_LIMIT {
            eprintln!(
                "Warning: Tree contains {} keys, but only returning first {} keys due to limit. \
                Consider using more specific queries or implementing pagination.",
                total_keys, MAX_KEYS_LIMIT
            );
        }

        Ok(keys.into_iter().take(MAX_KEYS_LIMIT).collect())
    }
}

/// Python-exposed SQL store backed by ProllyTree.
///
/// # Async/Sync Bridge (Python ↔ Rust ↔ GlueSQL)
///
/// Python is single-threaded (GIL), Rust storage is synchronous, and GlueSQL
/// requires an async runtime. The bridging pattern used here is:
///
/// 1. `py.detach()` — releases the Python GIL so other Python threads
///    can run while Rust executes.
/// 2. A **per-call** `tokio::runtime::Runtime` is created inside the
///    GIL-free closure. A per-call runtime avoids lifetime issues with
///    Python's GIL management and is acceptable because Python-to-Rust calls
///    are already coarse-grained.
/// 3. `runtime.block_on(async { ... })` drives the GlueSQL async execution,
///    which internally uses `spawn_blocking` for the underlying sync store
///    operations.
/// 4. `Python::attach()` re-acquires the GIL to construct Python return
///    values.
///
/// ```text
/// Python call (GIL held)
///   └─ py.detach (GIL released)
///        └─ tokio::runtime::Runtime::new()
///             └─ runtime.block_on(async { glue.execute(...).await })
///                  └─ GlueSQL → ProllyStorage → spawn_blocking → sync store
///                       └─ Python::attach() → return PyObject
/// ```
#[cfg(feature = "sql")]
#[pyclass(name = "ProllySQLStore")]
struct PyProllySQLStore {
    inner: Arc<Mutex<Glue<ProllyStorage<32>>>>,
}

#[cfg(feature = "sql")]
#[pymethods]
impl PyProllySQLStore {
    #[new]
    fn new(path: String) -> PyResult<Self> {
        let store = ThreadSafeGitVersionedKvStore::<32>::init(path)
            .map_err(|e| PyValueError::new_err(format!("Failed to initialize store: {}", e)))?;

        let storage = ProllyStorage::<32>::new(store);
        let glue = Glue::new(storage);

        Ok(PyProllySQLStore {
            inner: Arc::new(Mutex::new(glue)),
        })
    }

    #[staticmethod]
    fn open(path: String) -> PyResult<Self> {
        let store = ThreadSafeGitVersionedKvStore::<32>::open(path)
            .map_err(|e| PyValueError::new_err(format!("Failed to open store: {}", e)))?;

        let storage = ProllyStorage::<32>::new(store);
        let glue = Glue::new(storage);

        Ok(PyProllySQLStore {
            inner: Arc::new(Mutex::new(glue)),
        })
    }

    #[pyo3(signature = (query, format="dict"))]
    fn execute(&self, py: Python, query: String, format: &str) -> PyResult<Py<PyAny>> {
        py.detach(|| {
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|e| PyValueError::new_err(format!("Failed to create runtime: {}", e)))?;

            let mut glue = self.inner.lock();

            runtime.block_on(async {
                let results = glue
                    .execute(&query)
                    .await
                    .map_err(|e| PyValueError::new_err(format!("SQL execution failed: {}", e)))?;

                // GlueSQL returns a Vec<Payload>, we'll handle the first result
                let result = results
                    .into_iter()
                    .next()
                    .ok_or_else(|| PyValueError::new_err("No result from SQL query"))?;

                Python::attach(|py| {
                    match result {
                        Payload::Select { labels, rows } => {
                            match format {
                                "dict" | "dicts" => {
                                    // Return list of dictionaries
                                    let py_list = PyList::empty(py);
                                    for row in rows {
                                        let dict = PyDict::new(py);
                                        for (i, value) in row.iter().enumerate() {
                                            if i < labels.len() {
                                                let py_value = sql_value_to_python(py, value)?;
                                                dict.set_item(&labels[i], py_value)?;
                                            }
                                        }
                                        py_list.append(dict)?;
                                    }
                                    Ok(py_list.into())
                                }
                                "tuples" => {
                                    // Return (labels, rows) tuple
                                    let py_labels = PyList::empty(py);
                                    for label in &labels {
                                        py_labels.append(label)?;
                                    }

                                    let py_rows = PyList::empty(py);
                                    for row in rows {
                                        let py_row = PyList::empty(py);
                                        for value in row {
                                            let py_value = sql_value_to_python(py, &value)?;
                                            py_row.append(py_value)?;
                                        }
                                        py_rows.append(py_row)?;
                                    }

                                    Ok((py_labels, py_rows).into_py_any(py).unwrap())
                                }
                                "json" => {
                                    // Return JSON string
                                    let mut json_rows = Vec::new();
                                    for row in rows {
                                        let mut json_row = serde_json::Map::new();
                                        for (i, value) in row.iter().enumerate() {
                                            if i < labels.len() {
                                                let json_value = sql_value_to_json(value);
                                                json_row.insert(labels[i].clone(), json_value);
                                            }
                                        }
                                        json_rows.push(serde_json::Value::Object(json_row));
                                    }
                                    let json_str = serde_json::to_string_pretty(&json_rows)
                                        .map_err(|e| {
                                            PyValueError::new_err(format!(
                                                "JSON serialization failed: {}",
                                                e
                                            ))
                                        })?;
                                    Ok(json_str.into_py_any(py).unwrap())
                                }
                                "csv" => {
                                    // Return CSV string
                                    let mut csv_str = labels.join(",") + "\n";
                                    for row in rows {
                                        let row_strs: Vec<String> = row
                                            .iter()
                                            .map(|v| {
                                                let s = format!("{:?}", v);
                                                if s.contains(',') {
                                                    format!("\"{}\"", s.replace('"', "\"\""))
                                                } else {
                                                    s
                                                }
                                            })
                                            .collect();
                                        csv_str.push_str(&row_strs.join(","));
                                        csv_str.push('\n');
                                    }
                                    Ok(csv_str.into_py_any(py).unwrap())
                                }
                                _ => Err(PyValueError::new_err(format!(
                                    "Unknown format: {}. Use 'dict', 'tuples', 'json', or 'csv'",
                                    format
                                ))),
                            }
                        }
                        Payload::Insert(count) => {
                            let dict = PyDict::new(py);
                            dict.set_item("type", "insert")?;
                            dict.set_item("count", count)?;
                            Ok(dict.into())
                        }
                        Payload::Update(count) => {
                            let dict = PyDict::new(py);
                            dict.set_item("type", "update")?;
                            dict.set_item("count", count)?;
                            Ok(dict.into())
                        }
                        Payload::Delete(count) => {
                            let dict = PyDict::new(py);
                            dict.set_item("type", "delete")?;
                            dict.set_item("count", count)?;
                            Ok(dict.into())
                        }
                        Payload::Create => {
                            let dict = PyDict::new(py);
                            dict.set_item("type", "create")?;
                            dict.set_item("success", true)?;
                            Ok(dict.into())
                        }
                        Payload::DropTable(_) => {
                            let dict = PyDict::new(py);
                            dict.set_item("type", "drop_table")?;
                            dict.set_item("success", true)?;
                            Ok(dict.into())
                        }
                        Payload::AlterTable => {
                            let dict = PyDict::new(py);
                            dict.set_item("type", "alter_table")?;
                            dict.set_item("success", true)?;
                            Ok(dict.into())
                        }
                        _ => {
                            let dict = PyDict::new(py);
                            dict.set_item("type", "success")?;
                            dict.set_item("success", true)?;
                            Ok(dict.into())
                        }
                    }
                })
            })
        })
    }

    fn execute_many(&self, py: Python, queries: Vec<String>) -> PyResult<Vec<Py<PyAny>>> {
        let mut results = Vec::new();
        for query in queries {
            let result = self.execute(py, query, "dict")?;
            results.push(result);
        }
        Ok(results)
    }

    fn commit(&self, py: Python, _message: String) -> PyResult<String> {
        py.detach(|| {
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|e| PyValueError::new_err(format!("Failed to create runtime: {}", e)))?;

            let mut glue = self.inner.lock();

            runtime.block_on(async {
                // Execute the COMMIT command which will trigger the underlying storage commit
                glue.execute("COMMIT")
                    .await
                    .map_err(|e| PyValueError::new_err(format!("Failed to commit: {}", e)))?;

                // Return a placeholder commit ID for now
                // In a real implementation, we'd need to expose a way to get the commit ID
                // from the underlying storage through the SQL layer
                Ok("committed".to_string())
            })
        })
    }

    fn create_table(
        &self,
        py: Python,
        table_name: String,
        columns: Vec<(String, String)>,
    ) -> PyResult<Py<PyAny>> {
        let mut column_defs = Vec::new();
        for (name, dtype) in columns {
            column_defs.push(format!("{} {}", name, dtype));
        }
        let query = format!("CREATE TABLE {} ({})", table_name, column_defs.join(", "));
        self.execute(py, query, "dict")
    }

    fn insert(
        &self,
        py: Python,
        table_name: String,
        values: Vec<Vec<Py<PyAny>>>,
    ) -> PyResult<Py<PyAny>> {
        if values.is_empty() {
            return Err(PyValueError::new_err("No values to insert"));
        }

        let mut value_strings = Vec::new();
        for row in values {
            let mut row_values = Vec::new();
            for value in row {
                let value_str = Python::attach(|py| -> PyResult<String> {
                    if let Ok(s) = value.extract::<String>(py) {
                        Ok(format!("'{}'", s.replace('\'', "''")))
                    } else if let Ok(i) = value.extract::<i64>(py) {
                        Ok(i.to_string())
                    } else if let Ok(f) = value.extract::<f64>(py) {
                        Ok(f.to_string())
                    } else if let Ok(b) = value.extract::<bool>(py) {
                        Ok(b.to_string())
                    } else if value.is_none(py) {
                        Ok("NULL".to_string())
                    } else {
                        Ok(format!("'{}'", value))
                    }
                })?;
                row_values.push(value_str);
            }
            value_strings.push(format!("({})", row_values.join(", ")));
        }

        let query = format!(
            "INSERT INTO {} VALUES {}",
            table_name,
            value_strings.join(", ")
        );
        self.execute(py, query, "dict")
    }

    #[pyo3(signature = (table_name, columns=None, where_clause=None))]
    fn select(
        &self,
        py: Python,
        table_name: String,
        columns: Option<Vec<String>>,
        where_clause: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let columns_str = columns
            .map(|c| c.join(", "))
            .unwrap_or_else(|| "*".to_string());
        let mut query = format!("SELECT {} FROM {}", columns_str, table_name);

        if let Some(where_str) = where_clause {
            query.push_str(&format!(" WHERE {}", where_str));
        }

        self.execute(py, query, "dict")
    }
}

#[cfg(feature = "sql")]
fn sql_value_to_python(py: Python, value: &SqlValue) -> PyResult<Py<PyAny>> {
    match value {
        SqlValue::Null => Ok(py.None()),
        SqlValue::Bool(b) => Ok(b.into_py_any(py).unwrap()),
        SqlValue::I8(i) => Ok(i.into_py_any(py).unwrap()),
        SqlValue::I16(i) => Ok(i.into_py_any(py).unwrap()),
        SqlValue::I32(i) => Ok(i.into_py_any(py).unwrap()),
        SqlValue::I64(i) => Ok(i.into_py_any(py).unwrap()),
        SqlValue::I128(i) => Ok(i.into_py_any(py).unwrap()),
        SqlValue::U8(i) => Ok(i.into_py_any(py).unwrap()),
        SqlValue::U16(i) => Ok(i.into_py_any(py).unwrap()),
        SqlValue::U32(i) => Ok(i.into_py_any(py).unwrap()),
        SqlValue::U64(i) => Ok(i.into_py_any(py).unwrap()),
        SqlValue::U128(i) => Ok(i.to_string().into_py_any(py).unwrap()),
        SqlValue::F32(f) => Ok(f.into_py_any(py).unwrap()),
        SqlValue::F64(f) => Ok(f.into_py_any(py).unwrap()),
        SqlValue::Str(s) => Ok(s.into_py_any(py).unwrap()),
        SqlValue::Bytea(b) => Ok(PyBytes::new(py, b).into()),
        SqlValue::Date(d) => Ok(d.to_string().into_py_any(py).unwrap()),
        SqlValue::Time(t) => Ok(t.to_string().into_py_any(py).unwrap()),
        SqlValue::Timestamp(ts) => Ok(ts.to_string().into_py_any(py).unwrap()),
        SqlValue::Interval(i) => Ok(format!("{:?}", i).into_py_any(py).unwrap()),
        SqlValue::Uuid(u) => Ok(u.to_string().into_py_any(py).unwrap()),
        SqlValue::Map(m) => {
            let dict = PyDict::new(py);
            for (k, v) in m.iter() {
                let py_value = sql_value_to_python(py, v)?;
                dict.set_item(k, py_value)?;
            }
            Ok(dict.into())
        }
        SqlValue::List(l) => {
            let py_list = PyList::empty(py);
            for item in l.iter() {
                let py_value = sql_value_to_python(py, item)?;
                py_list.append(py_value)?;
            }
            Ok(py_list.into())
        }
        SqlValue::Decimal(d) => Ok(d.to_string().into_py_any(py).unwrap()),
        SqlValue::Point(p) => Ok(format!("POINT({} {})", p.x, p.y).into_py_any(py).unwrap()),
        SqlValue::Inet(ip) => Ok(ip.to_string().into_py_any(py).unwrap()),
    }
}

#[cfg(feature = "sql")]
fn sql_value_to_json(value: &SqlValue) -> serde_json::Value {
    match value {
        SqlValue::Null => serde_json::Value::Null,
        SqlValue::Bool(b) => serde_json::Value::Bool(*b),
        SqlValue::I8(i) => serde_json::Value::Number((*i).into()),
        SqlValue::I16(i) => serde_json::Value::Number((*i).into()),
        SqlValue::I32(i) => serde_json::Value::Number((*i).into()),
        SqlValue::I64(i) => serde_json::Value::Number((*i).into()),
        SqlValue::I128(i) => serde_json::Value::String(i.to_string()),
        SqlValue::U8(i) => serde_json::Value::Number((*i).into()),
        SqlValue::U16(i) => serde_json::Value::Number((*i).into()),
        SqlValue::U32(i) => serde_json::Value::Number((*i).into()),
        SqlValue::U64(i) => serde_json::Value::Number((*i).into()),
        SqlValue::U128(i) => serde_json::Value::String(i.to_string()),
        SqlValue::F32(f) => serde_json::json!(f),
        SqlValue::F64(f) => serde_json::json!(f),
        SqlValue::Str(s) => serde_json::Value::String(s.clone()),
        SqlValue::Bytea(b) => {
            use base64::Engine;
            serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(b))
        }
        SqlValue::Date(d) => serde_json::Value::String(d.to_string()),
        SqlValue::Time(t) => serde_json::Value::String(t.to_string()),
        SqlValue::Timestamp(ts) => serde_json::Value::String(ts.to_string()),
        SqlValue::Interval(i) => serde_json::Value::String(format!("{:?}", i)),
        SqlValue::Uuid(u) => serde_json::Value::String(u.to_string()),
        SqlValue::Map(m) => {
            let mut map = serde_json::Map::new();
            for (k, v) in m.iter() {
                map.insert(k.clone(), sql_value_to_json(v));
            }
            serde_json::Value::Object(map)
        }
        SqlValue::List(l) => {
            let list: Vec<serde_json::Value> = l.iter().map(sql_value_to_json).collect();
            serde_json::Value::Array(list)
        }
        SqlValue::Decimal(d) => serde_json::Value::String(d.to_string()),
        SqlValue::Point(p) => serde_json::json!({"x": p.x, "y": p.y}),
        SqlValue::Inet(ip) => serde_json::Value::String(ip.to_string()),
    }
}

// ---------------------------------------------------------------------------
// NamespacedKvStore Python bindings
// ---------------------------------------------------------------------------

/// Python wrapper for the namespace-aware versioned key-value store.
///
/// Each namespace is backed by its own ProllyTree subtree, enabling O(1) change
/// detection, efficient namespace-scoped operations, and clean isolation.
#[cfg(feature = "proximity")]
type PyTextConfigMap = HashMap<(String, String), (Arc<dyn Embedder>, Arc<dyn Chunker>)>;

#[pyclass(name = "NamespacedKvStore")]
struct PyNamespacedKvStore {
    inner: Arc<Mutex<GitNamespacedKvStore<32>>>,
    // Python-side cache of `(ns, idx) -> (embedder, chunker)` so subsequent
    // text-index calls can rebuild the generic `TextIndexConfig<E>` without
    // forcing the caller to re-pass the embedder every time.
    #[cfg(feature = "proximity")]
    text_configs: Arc<Mutex<PyTextConfigMap>>,
}

#[pymethods]
impl PyNamespacedKvStore {
    /// Create a new NamespacedKvStore (initializes a new Git repository dataset).
    #[new]
    fn new(path: String) -> PyResult<Self> {
        let store = GitNamespacedKvStore::<32>::init(&path).map_err(|e| {
            PyValueError::new_err(format!("Failed to initialize NamespacedKvStore: {e}"))
        })?;
        Ok(Self {
            inner: Arc::new(Mutex::new(store)),
            #[cfg(feature = "proximity")]
            text_configs: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Open an existing NamespacedKvStore (auto-detects V1/V2 format).
    #[staticmethod]
    fn open(path: String) -> PyResult<Self> {
        let store = GitNamespacedKvStore::<32>::open(&path)
            .map_err(|e| PyValueError::new_err(format!("Failed to open NamespacedKvStore: {e}")))?;
        Ok(Self {
            inner: Arc::new(Mutex::new(store)),
            #[cfg(feature = "proximity")]
            text_configs: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    // -- Namespace-scoped operations --

    /// Insert a key-value pair into a specific namespace.
    fn ns_insert(
        &self,
        namespace: &str,
        key: &Bound<'_, PyBytes>,
        value: &Bound<'_, PyBytes>,
    ) -> PyResult<()> {
        let mut store = self.inner.lock();
        store
            .namespace(namespace)
            .insert(key.as_bytes().to_vec(), value.as_bytes().to_vec())
            .map_err(|e| PyValueError::new_err(format!("Insert failed: {e}")))
    }

    /// Get a value by key from a specific namespace.
    fn ns_get<'py>(
        &self,
        py: Python<'py>,
        namespace: &str,
        key: &Bound<'py, PyBytes>,
    ) -> PyResult<Option<Py<PyBytes>>> {
        let mut store = self.inner.lock();
        Ok(store
            .namespace(namespace)
            .get(key.as_bytes())
            .map(|v| PyBytes::new(py, &v).into()))
    }

    /// Delete a key from a specific namespace.
    fn ns_delete(&self, namespace: &str, key: &Bound<'_, PyBytes>) -> PyResult<bool> {
        let mut store = self.inner.lock();
        store
            .namespace(namespace)
            .delete(key.as_bytes())
            .map_err(|e| PyValueError::new_err(format!("Delete failed: {e}")))
    }

    /// List all keys in a specific namespace.
    fn ns_list_keys<'py>(&self, py: Python<'py>, namespace: &str) -> PyResult<Py<PyList>> {
        let mut store = self.inner.lock();
        let keys = store.namespace(namespace).list_keys();
        let py_keys: Vec<Py<PyBytes>> = keys.iter().map(|k| PyBytes::new(py, k).into()).collect();
        Ok(PyList::new(py, &py_keys)?.into())
    }

    // -- Registry operations --

    /// List all namespace names.
    fn list_namespaces(&self) -> Vec<String> {
        self.inner.lock().list_namespaces()
    }

    /// Delete an entire namespace.
    fn delete_namespace(&self, namespace: &str) -> PyResult<bool> {
        self.inner
            .lock()
            .delete_namespace(namespace)
            .map_err(|e| PyValueError::new_err(format!("Delete namespace failed: {e}")))
    }

    /// Get the root hash for a namespace (O(1) lookup).
    fn get_namespace_root_hash<'py>(
        &self,
        py: Python<'py>,
        namespace: &str,
    ) -> Option<Py<PyBytes>> {
        self.inner
            .lock()
            .get_namespace_root_hash(namespace)
            .map(|h| PyBytes::new(py, h.as_bytes()).into())
    }

    /// Check if a namespace changed between two commits.
    fn namespace_changed(&self, namespace: &str, commit_a: &str, commit_b: &str) -> PyResult<bool> {
        self.inner
            .lock()
            .namespace_changed(namespace, commit_a, commit_b)
            .map_err(|e| PyValueError::new_err(format!("namespace_changed failed: {e}")))
    }

    // -- Backward-compatible flat API (default namespace) --

    /// Insert into the default namespace.
    fn insert(&self, key: &Bound<'_, PyBytes>, value: &Bound<'_, PyBytes>) -> PyResult<()> {
        self.inner
            .lock()
            .insert(key.as_bytes().to_vec(), value.as_bytes().to_vec())
            .map_err(|e| PyValueError::new_err(format!("Insert failed: {e}")))
    }

    /// Get from the default namespace.
    fn get<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyBytes>,
    ) -> PyResult<Option<Py<PyBytes>>> {
        Ok(self
            .inner
            .lock()
            .get(key.as_bytes())
            .map(|v| PyBytes::new(py, &v).into()))
    }

    /// Delete from the default namespace.
    fn delete(&self, key: &Bound<'_, PyBytes>) -> PyResult<bool> {
        self.inner
            .lock()
            .delete(key.as_bytes())
            .map_err(|e| PyValueError::new_err(format!("Delete failed: {e}")))
    }

    /// List keys in the default namespace.
    fn list_keys<'py>(&self, py: Python<'py>) -> PyResult<Py<PyList>> {
        let keys = self.inner.lock().list_keys();
        let py_keys: Vec<Py<PyBytes>> = keys.iter().map(|k| PyBytes::new(py, k).into()).collect();
        Ok(PyList::new(py, &py_keys)?.into())
    }

    // -- Git operations --

    /// Commit all staged changes across all namespaces.
    #[pyo3(signature = (message=None))]
    fn commit(&self, message: Option<&str>) -> PyResult<String> {
        let msg = message.unwrap_or("commit");
        self.inner
            .lock()
            .commit(msg)
            .map(|id| id.to_hex().to_string())
            .map_err(|e| PyValueError::new_err(format!("Commit failed: {e}")))
    }

    /// Checkout a branch or commit.
    fn checkout(&self, branch_or_commit: &str) -> PyResult<()> {
        self.inner
            .lock()
            .checkout(branch_or_commit)
            .map_err(|e| PyValueError::new_err(format!("Checkout failed: {e}")))
    }

    /// Create a new branch and switch to it.
    fn branch(&self, name: &str) -> PyResult<()> {
        self.inner
            .lock()
            .create_branch(name)
            .map_err(|e| PyValueError::new_err(format!("Branch failed: {e}")))
    }

    /// Get current branch name.
    #[getter]
    fn current_branch(&self) -> String {
        self.inner.lock().current_branch().to_string()
    }

    /// Get commit history.
    fn log(&self) -> PyResult<Vec<HashMap<String, String>>> {
        let history = self
            .inner
            .lock()
            .log()
            .map_err(|e| PyValueError::new_err(format!("Log failed: {e}")))?;
        Ok(history
            .into_iter()
            .map(|c| {
                let mut m = HashMap::new();
                m.insert("id".to_string(), c.id.to_hex().to_string());
                m.insert("message".to_string(), c.message);
                m.insert("author".to_string(), c.author);
                m.insert("timestamp".to_string(), c.timestamp.to_string());
                m
            })
            .collect())
    }

    /// Migrate a V1 (flat) store to V2 (namespaced) format.
    fn migrate_v1_to_v2(&self) -> PyResult<HashMap<String, String>> {
        let report = self
            .inner
            .lock()
            .migrate_v1_to_v2()
            .map_err(|e| PyValueError::new_err(format!("Migration failed: {e}")))?;
        let mut m = HashMap::new();
        m.insert(
            "keys_migrated".to_string(),
            report.keys_migrated.to_string(),
        );
        m.insert(
            "namespaces_created".to_string(),
            report.namespaces_created.join(", "),
        );
        Ok(m)
    }

    fn __repr__(&self) -> String {
        let store = self.inner.lock();
        format!(
            "NamespacedKvStore(namespaces={}, branch='{}')",
            store.list_namespaces().len(),
            store.current_branch()
        )
    }

    // ----- Proximity / text-search (feature `proximity`) ----------------
    // All methods below are additive — no existing surface area changes.

    /// Open or create a named text sub-index inside a namespace. Caches the
    /// embedder + chunker on the Python wrapper so subsequent text-index
    /// methods don't require re-passing them.
    ///
    /// `chunker` is one of `"identity"` (default — one chunk per doc) or
    /// `"line"` (split on `\n`).
    #[cfg(feature = "proximity")]
    #[pyo3(signature = (namespace, idx_name, embedder, chunker=None))]
    fn text_index_open(
        &self,
        namespace: &str,
        idx_name: &str,
        embedder: &Bound<'_, PyAny>,
        chunker: Option<&str>,
    ) -> PyResult<()> {
        let arc_embedder = extract_embedder(embedder)?;
        let arc_chunker = build_chunker(chunker)?;
        {
            let mut store = self.inner.lock();
            let mut ns = store.namespace(namespace);
            let cfg = Self::make_text_cfg(arc_embedder.clone(), arc_chunker.clone());
            ns.text_index(idx_name, cfg)
                .map(|_| ())
                .map_err(|e| PyValueError::new_err(format!("text_index_open failed: {e}")))?;
        }
        self.text_configs.lock().insert(
            (namespace.to_string(), idx_name.to_string()),
            (arc_embedder, arc_chunker),
        );
        Ok(())
    }

    /// Insert a `(id, text)` pair into a text sub-index.
    #[cfg(feature = "proximity")]
    fn text_index_insert(
        &self,
        namespace: &str,
        idx_name: &str,
        id: &Bound<'_, PyBytes>,
        text: &str,
    ) -> PyResult<()> {
        let (e, c) = self.lookup_text_cfg(namespace, idx_name)?;
        let id_bytes = id.as_bytes().to_vec();
        let mut store = self.inner.lock();
        let mut ns = store.namespace(namespace);
        let mut handle = ns
            .text_index(idx_name, Self::make_text_cfg(e, c))
            .map_err(|e| PyValueError::new_err(format!("text_index re-open failed: {e}")))?;
        handle
            .insert(&id_bytes, text)
            .map_err(|e| PyValueError::new_err(format!("text_index_insert failed: {e}")))
    }

    /// Search a text sub-index. Returns a list of `(id_bytes, score)`
    /// tuples — top-k documents (dedupped across chunks).
    #[cfg(feature = "proximity")]
    fn text_index_search<'py>(
        &self,
        py: Python<'py>,
        namespace: &str,
        idx_name: &str,
        query: &str,
        k: usize,
    ) -> PyResult<Py<PyList>> {
        let (e, c) = self.lookup_text_cfg(namespace, idx_name)?;
        let hits = {
            let mut store = self.inner.lock();
            let mut ns = store.namespace(namespace);
            let mut handle = ns
                .text_index(idx_name, Self::make_text_cfg(e, c))
                .map_err(|e| PyValueError::new_err(format!("text_index re-open failed: {e}")))?;
            handle
                .search(query, k)
                .map_err(|e| PyValueError::new_err(format!("text_index_search failed: {e}")))?
        };
        let tuples: Vec<Py<PyAny>> = hits
            .into_iter()
            .map(|h| -> PyResult<Py<PyAny>> {
                let t = pyo3::types::PyTuple::new(
                    py,
                    &[
                        PyBytes::new(py, &h.id).into_py_any(py)?,
                        h.score.into_py_any(py)?,
                    ],
                )?;
                Ok(t.into())
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok(PyList::new(py, &tuples)?.into())
    }

    /// Delete every chunk for `id` from a text sub-index.
    #[cfg(feature = "proximity")]
    fn text_index_delete(
        &self,
        namespace: &str,
        idx_name: &str,
        id: &Bound<'_, PyBytes>,
    ) -> PyResult<bool> {
        let (e, c) = self.lookup_text_cfg(namespace, idx_name)?;
        let id_bytes = id.as_bytes().to_vec();
        let mut store = self.inner.lock();
        let mut ns = store.namespace(namespace);
        let mut handle = ns
            .text_index(idx_name, Self::make_text_cfg(e, c))
            .map_err(|e| PyValueError::new_err(format!("text_index re-open failed: {e}")))?;
        Ok(handle.delete(&id_bytes))
    }

    /// Number of distinct documents in a text sub-index.
    #[cfg(feature = "proximity")]
    fn text_index_len(&self, namespace: &str, idx_name: &str) -> PyResult<usize> {
        let (e, c) = self.lookup_text_cfg(namespace, idx_name)?;
        let mut store = self.inner.lock();
        let mut ns = store.namespace(namespace);
        let handle = ns
            .text_index(idx_name, Self::make_text_cfg(e, c))
            .map_err(|e| PyValueError::new_err(format!("text_index re-open failed: {e}")))?;
        Ok(handle.len())
    }

    /// Raw chunk count for a text sub-index (>= len() under non-identity chunkers).
    #[cfg(feature = "proximity")]
    fn text_index_chunk_count(&self, namespace: &str, idx_name: &str) -> PyResult<usize> {
        let (e, c) = self.lookup_text_cfg(namespace, idx_name)?;
        let mut store = self.inner.lock();
        let mut ns = store.namespace(namespace);
        let handle = ns
            .text_index(idx_name, Self::make_text_cfg(e, c))
            .map_err(|e| PyValueError::new_err(format!("text_index re-open failed: {e}")))?;
        Ok(handle.chunk_count())
    }

    /// Drop a text sub-index from the in-memory cache.
    #[cfg(feature = "proximity")]
    fn text_index_drop(&self, namespace: &str, idx_name: &str) -> bool {
        let dropped = {
            let mut store = self.inner.lock();
            let mut ns = store.namespace(namespace);
            ns.drop_text_index(idx_name)
        };
        self.text_configs
            .lock()
            .remove(&(namespace.to_string(), idx_name.to_string()));
        dropped
    }

    /// Configure auto-cascade — every primary insert/delete in this
    /// namespace mirrors into the listed text sub-indexes.
    #[cfg(feature = "proximity")]
    fn set_cascade(&self, namespace: &str, idx_names: Vec<String>) {
        self.inner.lock().set_cascade(namespace, idx_names);
    }

    /// Disable auto-cascade for a namespace.
    #[cfg(feature = "proximity")]
    fn clear_cascade(&self, namespace: &str) {
        self.inner.lock().clear_cascade(namespace);
    }

    /// Current cascade list for a namespace, or None if not configured.
    #[cfg(feature = "proximity")]
    fn cascade_for_namespace(&self, namespace: &str) -> Option<Vec<String>> {
        self.inner
            .lock()
            .cascade_for_namespace(namespace)
            .map(|s| s.to_vec())
    }

    /// Audit a text sub-index against the primary KV tree.
    /// Returns a dict with `orphans_in_index`, `missing_from_index`,
    /// and `is_in_sync` keys.
    #[cfg(feature = "proximity")]
    fn audit_text_index<'py>(
        &self,
        py: Python<'py>,
        namespace: &str,
        idx_name: &str,
    ) -> PyResult<Py<PyDict>> {
        let report = self
            .inner
            .lock()
            .audit_text_index(namespace, idx_name)
            .map_err(|e| PyValueError::new_err(format!("audit_text_index failed: {e}")))?;
        let d = PyDict::new(py);
        let orphans: Vec<Py<PyBytes>> = report
            .orphans_in_index
            .iter()
            .map(|b| PyBytes::new(py, b).into())
            .collect();
        let missing: Vec<Py<PyBytes>> = report
            .missing_from_index
            .iter()
            .map(|b| PyBytes::new(py, b).into())
            .collect();
        d.set_item("orphans_in_index", PyList::new(py, &orphans)?)?;
        d.set_item("missing_from_index", PyList::new(py, &missing)?)?;
        d.set_item("is_in_sync", report.is_in_sync())?;
        Ok(d.into())
    }

    /// Delete every orphan id from a text sub-index. Returns the count.
    #[cfg(feature = "proximity")]
    fn purge_text_index_orphans(&self, namespace: &str, idx_name: &str) -> PyResult<usize> {
        self.inner
            .lock()
            .purge_text_index_orphans(namespace, idx_name)
            .map_err(|e| PyValueError::new_err(format!("purge_text_index_orphans failed: {e}")))
    }

    /// Set the externalisation threshold (in bytes). None disables.
    #[cfg(feature = "proximity")]
    #[pyo3(signature = (threshold=None))]
    fn set_externalize_threshold(&self, threshold: Option<usize>) {
        self.inner.lock().set_externalize_threshold(threshold);
    }

    /// Get the externalisation threshold, or None when disabled.
    #[cfg(feature = "proximity")]
    fn externalize_threshold(&self) -> Option<usize> {
        self.inner.lock().externalize_threshold()
    }

    /// Run blob garbage collection. Returns a dict with `total`,
    /// `referenced`, `removed`, and `errors` keys.
    #[cfg(feature = "proximity")]
    fn gc_blobs<'py>(&self, py: Python<'py>) -> PyResult<Py<PyDict>> {
        let report = self
            .inner
            .lock()
            .gc_blobs()
            .map_err(|e| PyValueError::new_err(format!("gc_blobs failed: {e}")))?;
        let d = PyDict::new(py);
        d.set_item("total", report.total)?;
        d.set_item("referenced", report.referenced)?;
        d.set_item("removed", report.removed)?;
        d.set_item("errors", report.errors)?;
        Ok(d.into())
    }
}

// ---------------------------------------------------------------------------
// Proximity / text-search bindings
// ---------------------------------------------------------------------------
//
// Backward compatibility: every existing class and method above is unchanged.
// All new surface area is additive — gated by `#[cfg(feature = "proximity")]`
// (with `proximity_text` for the MiniLM embedder).

#[cfg(feature = "proximity")]
use crate::proximity::{Chunker, Embedder, IdentityChunker, LineChunker};

/// Python wrapper around the built-in deterministic `HashEmbedder` — pure
/// Rust, no ML deps. Useful for tests and demos; NOT a semantic embedder.
#[cfg(feature = "proximity")]
#[pyclass(name = "HashEmbedder")]
struct PyHashEmbedder {
    inner: Arc<dyn Embedder>,
}

#[cfg(feature = "proximity")]
#[pymethods]
impl PyHashEmbedder {
    /// Build a new hash embedder of the given dimensionality.
    #[new]
    fn new(dim: u16, seed: u64) -> Self {
        Self {
            inner: Arc::new(crate::proximity::HashEmbedder::new(dim, seed)),
        }
    }

    /// Stable id of the embedder family.
    #[getter]
    fn id(&self) -> String {
        self.inner.id().to_string()
    }

    /// Version string (changes if dim or seed change).
    #[getter]
    fn version(&self) -> String {
        self.inner.version().to_string()
    }

    /// Vector dimensionality this embedder produces.
    #[getter]
    fn dim(&self) -> u16 {
        self.inner.dim()
    }

    /// Embed `text` into a `dim`-length list of floats.
    fn embed(&self, text: &str) -> PyResult<Vec<f32>> {
        self.inner
            .embed(text)
            .map_err(|e| PyValueError::new_err(format!("embed failed: {e}")))
    }

    fn __repr__(&self) -> String {
        format!(
            "HashEmbedder(id='{}', version='{}', dim={})",
            self.inner.id(),
            self.inner.version(),
            self.inner.dim()
        )
    }
}

/// Python wrapper around the Candle + all-MiniLM-L6-v2 sentence embedder.
/// First call downloads ~90 MB of weights into the local cache (override the
/// path via `PROLLYTREE_EMBEDDER_CACHE`).
#[cfg(feature = "proximity_text")]
#[pyclass(name = "MiniLmEmbedder")]
struct PyMiniLmEmbedder {
    inner: Arc<dyn Embedder>,
}

#[cfg(feature = "proximity_text")]
#[pymethods]
impl PyMiniLmEmbedder {
    /// Construct with default `sentence-transformers/all-MiniLM-L6-v2@main`.
    #[new]
    #[pyo3(signature = (model_id=None, revision=None))]
    fn new(model_id: Option<&str>, revision: Option<&str>) -> Self {
        let model_id = model_id.unwrap_or(crate::proximity::DEFAULT_MODEL_ID);
        let revision = revision.unwrap_or(crate::proximity::DEFAULT_REVISION);
        Self {
            inner: Arc::new(crate::proximity::MiniLmEmbedder::new(model_id, revision)),
        }
    }

    #[getter]
    fn id(&self) -> String {
        self.inner.id().to_string()
    }

    #[getter]
    fn version(&self) -> String {
        self.inner.version().to_string()
    }

    #[getter]
    fn dim(&self) -> u16 {
        self.inner.dim()
    }

    fn embed(&self, text: &str) -> PyResult<Vec<f32>> {
        self.inner
            .embed(text)
            .map_err(|e| PyValueError::new_err(format!("embed failed: {e}")))
    }

    fn __repr__(&self) -> String {
        format!(
            "MiniLmEmbedder(id='{}', version='{}', dim={})",
            self.inner.id(),
            self.inner.version(),
            self.inner.dim()
        )
    }
}

/// Wraps a Python-side embedder (any callable returning `list[float]`) as an
/// `Arc<dyn Embedder>` so users can plug in their own embedding pipelines
/// without writing Rust. The wrapped callable is invoked under the GIL; the
/// `id`, `version`, and `dim` metadata is captured at construction so the
/// `Embedder` trait's `&str` accessors stay borrow-safe.
#[cfg(feature = "proximity")]
#[pyclass(name = "CallableEmbedder")]
struct PyCallableEmbedder {
    inner: Arc<CallableEmbedderImpl>,
}

#[cfg(feature = "proximity")]
struct CallableEmbedderImpl {
    id: String,
    version: String,
    dim: u16,
    callable: Py<PyAny>,
}

#[cfg(feature = "proximity")]
impl Embedder for CallableEmbedderImpl {
    fn id(&self) -> &str {
        &self.id
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn dim(&self) -> u16 {
        self.dim
    }
    fn embed(&self, text: &str) -> Result<Vec<f32>, crate::proximity::EmbedError> {
        Python::attach(|py| {
            let result = self.callable.bind(py).call1((text,)).map_err(|e| {
                crate::proximity::EmbedError::Failure(format!("python callback raised: {e}"))
            })?;
            let vec: Vec<f32> = result.extract().map_err(|e| {
                crate::proximity::EmbedError::Failure(format!(
                    "python callback returned non-list-of-floats: {e}"
                ))
            })?;
            if vec.len() != self.dim as usize {
                return Err(crate::proximity::EmbedError::DimensionMismatch {
                    expected: self.dim,
                    got: vec.len(),
                });
            }
            Ok(vec)
        })
    }
}

#[cfg(feature = "proximity")]
#[pymethods]
impl PyCallableEmbedder {
    /// Wrap a Python callable as an Embedder. `embed_fn(text)` must return a
    /// list of `dim` floats and must be deterministic for a given input —
    /// changing what it returns for the same text breaks index identity.
    ///
    /// `id` and `version` are persisted in the text-index registry on first
    /// open and re-checked on every reopen. Pick values that change whenever
    /// the embedding distribution changes (e.g. model upgrade, new tokenizer)
    /// so reopens correctly surface as `EmbedderMismatch`.
    #[new]
    fn new(id: String, version: String, dim: u16, embed_fn: Py<PyAny>) -> Self {
        Self {
            inner: Arc::new(CallableEmbedderImpl {
                id,
                version,
                dim,
                callable: embed_fn,
            }),
        }
    }

    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    #[getter]
    fn version(&self) -> String {
        self.inner.version.clone()
    }

    #[getter]
    fn dim(&self) -> u16 {
        self.inner.dim
    }

    fn embed(&self, text: &str) -> PyResult<Vec<f32>> {
        Embedder::embed(self.inner.as_ref(), text)
            .map_err(|e| PyValueError::new_err(format!("embed failed: {e}")))
    }

    fn __repr__(&self) -> String {
        format!(
            "CallableEmbedder(id='{}', version='{}', dim={})",
            self.inner.id, self.inner.version, self.inner.dim
        )
    }
}

/// Internal helper: extract the inner `Arc<dyn Embedder>` from any of the
/// known Python embedder wrappers.
#[cfg(feature = "proximity")]
fn extract_embedder(obj: &Bound<'_, PyAny>) -> PyResult<Arc<dyn Embedder>> {
    if let Ok(h) = obj.cast::<PyHashEmbedder>() {
        return Ok(h.borrow().inner.clone());
    }
    if let Ok(c) = obj.cast::<PyCallableEmbedder>() {
        return Ok(c.borrow().inner.clone());
    }
    #[cfg(feature = "proximity_text")]
    if let Ok(m) = obj.cast::<PyMiniLmEmbedder>() {
        return Ok(m.borrow().inner.clone());
    }
    Err(PyValueError::new_err(
        "expected an Embedder (HashEmbedder, CallableEmbedder, or MiniLmEmbedder)",
    ))
}

/// Internal helper: build a chunker from a string name. None or "identity"
/// → IdentityChunker. "line" → LineChunker.
#[cfg(feature = "proximity")]
fn build_chunker(name: Option<&str>) -> PyResult<Arc<dyn Chunker>> {
    match name {
        None | Some("identity") => Ok(Arc::new(IdentityChunker)),
        Some("line") => Ok(Arc::new(LineChunker)),
        Some(other) => Err(PyValueError::new_err(format!(
            "unknown chunker '{other}'; expected 'identity' or 'line'"
        ))),
    }
}

#[cfg(feature = "proximity")]
impl PyNamespacedKvStore {
    /// Look up `(embedder, chunker)` for a previously-opened text index.
    fn lookup_text_cfg(
        &self,
        namespace: &str,
        idx_name: &str,
    ) -> PyResult<(Arc<dyn Embedder>, Arc<dyn Chunker>)> {
        let guard = self.text_configs.lock();
        guard
            .get(&(namespace.to_string(), idx_name.to_string()))
            .cloned()
            .ok_or_else(|| {
                PyValueError::new_err(format!(
                    "text index '{idx_name}' not opened in namespace '{namespace}' — call text_index_open first"
                ))
            })
    }

    /// Build a `TextIndexConfig<ArcEmbedderShim>` from cached parts.
    fn make_text_cfg(
        embedder: Arc<dyn Embedder>,
        chunker: Arc<dyn Chunker>,
    ) -> crate::proximity::TextIndexConfig<ArcEmbedderShim> {
        let mut cfg = crate::proximity::TextIndexConfig::new(ArcEmbedderShim(embedder));
        cfg.chunker = chunker;
        cfg
    }
}

/// Internal newtype wrapping `Arc<dyn Embedder>` so it satisfies
/// `E: Embedder + 'static` for the generic `text_index<E>` method.
/// Delegates every trait method to the inner Arc.
#[cfg(feature = "proximity")]
struct ArcEmbedderShim(Arc<dyn Embedder>);

#[cfg(feature = "proximity")]
impl Embedder for ArcEmbedderShim {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn version(&self) -> &str {
        self.0.version()
    }
    fn dim(&self) -> u16 {
        self.0.dim()
    }
    fn embed(&self, text: &str) -> Result<Vec<f32>, crate::proximity::EmbedError> {
        self.0.embed(text)
    }
}

#[pymodule]
fn prollytree(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTreeConfig>()?;
    m.add_class::<PyProllyTree>()?;
    m.add_class::<PyStorageBackend>()?;
    m.add_class::<PyMergeConflict>()?;
    m.add_class::<PyConflictResolution>()?;
    m.add_class::<PyDiffOperation>()?;
    m.add_class::<PyKvDiff>()?;
    m.add_class::<PyVersionedKvStore>()?;
    #[cfg(feature = "git")]
    m.add_class::<PyNamespacedKvStore>()?;
    #[cfg(feature = "git")]
    m.add_class::<PyWorktreeManager>()?;
    #[cfg(feature = "git")]
    m.add_class::<PyWorktreeVersionedKvStore>()?;
    #[cfg(feature = "sql")]
    m.add_class::<PyProllySQLStore>()?;
    #[cfg(feature = "proximity")]
    m.add_class::<PyHashEmbedder>()?;
    #[cfg(feature = "proximity")]
    m.add_class::<PyCallableEmbedder>()?;
    #[cfg(feature = "proximity_text")]
    m.add_class::<PyMiniLmEmbedder>()?;
    Ok(())
}
