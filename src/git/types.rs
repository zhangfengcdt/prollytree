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

use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitKvError {
    #[error("Git open error: {0}")]
    GitOpenError(#[from] Box<gix::open::Error>),

    #[error("Git init error: {0}")]
    GitInitError(#[from] Box<gix::init::Error>),

    #[error("Git object error: {0}")]
    GitObjectError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Repository not found at path: {0}")]
    RepositoryNotFound(String),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Merge conflict: {0}")]
    MergeConflict(String),

    #[error("Invalid commit: {0}")]
    InvalidCommit(String),

    #[error("Branch not found: {0}")]
    BranchNotFound(String),
}

#[derive(Debug, Clone)]
pub enum MergeResult {
    FastForward(gix::ObjectId),
    ThreeWay(gix::ObjectId),
    Conflict(Vec<KvConflict>),
}

#[derive(Debug, Clone)]
pub struct KvConflict {
    pub key: Vec<u8>,
    pub base_value: Option<Vec<u8>>,
    pub our_value: Option<Vec<u8>>,
    pub their_value: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct KvDiff {
    pub key: Vec<u8>,
    pub operation: DiffOperation,
}

#[derive(Debug, Clone)]
pub enum DiffOperation {
    Added(Vec<u8>),
    Removed(Vec<u8>),
    Modified { old: Vec<u8>, new: Vec<u8> },
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub id: gix::ObjectId,
    pub author: String,
    pub committer: String,
    pub message: String,
    pub timestamp: i64,
}

impl fmt::Display for CommitInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Commit {} by {} (timestamp: {})\nMessage: {}",
            self.id, self.author, self.timestamp, self.message
        )
    }
}

#[derive(Debug, Clone)]
pub struct CommitDetails {
    pub info: CommitInfo,
    pub changes: Vec<KvDiff>,
    pub parent_ids: Vec<gix::ObjectId>,
}

#[derive(Debug, Clone)]
pub struct KvStorageMetadata {
    pub total_keys: usize,
    pub tree_depth: usize,
    pub node_count: usize,
    pub root_hash: Option<gix::ObjectId>,
    pub last_commit: Option<gix::ObjectId>,
}

/// Storage backend types supported by VersionedKvStore
#[derive(Debug, Clone, PartialEq, Default)]
pub enum StorageBackend {
    /// In-memory storage (volatile, fastest)
    InMemory,
    /// File-based storage (persistent, simple)
    File,
    /// RocksDB storage (persistent, high-performance)
    #[cfg(feature = "rocksdb_storage")]
    RocksDB,
    /// Git object storage (development only, default)
    #[default]
    Git,
}

impl std::fmt::Display for StorageBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageBackend::InMemory => write!(f, "InMemory"),
            StorageBackend::File => write!(f, "File"),
            #[cfg(feature = "rocksdb_storage")]
            StorageBackend::RocksDB => write!(f, "RocksDB"),
            StorageBackend::Git => write!(f, "Git"),
        }
    }
}

impl fmt::Display for DiffOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffOperation::Added(value) => write!(f, "Added: {:?}", String::from_utf8_lossy(value)),
            DiffOperation::Removed(value) => {
                write!(f, "Removed: {:?}", String::from_utf8_lossy(value))
            }
            DiffOperation::Modified { old, new } => write!(
                f,
                "Modified: {:?} -> {:?}",
                String::from_utf8_lossy(old),
                String::from_utf8_lossy(new)
            ),
        }
    }
}

impl fmt::Display for KvConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Conflict on key: {:?}",
            String::from_utf8_lossy(&self.key)
        )?;
        if let Some(base) = &self.base_value {
            write!(f, "\n  Base: {:?}", String::from_utf8_lossy(base))?;
        }
        if let Some(ours) = &self.our_value {
            write!(f, "\n  Ours: {:?}", String::from_utf8_lossy(ours))?;
        }
        if let Some(theirs) = &self.their_value {
            write!(f, "\n  Theirs: {:?}", String::from_utf8_lossy(theirs))?;
        }
        Ok(())
    }
}
