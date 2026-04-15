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

use crate::storage::StorageError;
use thiserror::Error;

/// Unified error type that bridges all error domains in the crate.
///
/// Use this at API boundaries where errors from different subsystems
/// (tree operations, storage backends, git versioning) may need to be
/// returned through a single `Result` type.
#[derive(Error, Debug)]
pub enum ProllyError {
    /// An error from core tree operations (encoding, schema, etc.).
    #[error(transparent)]
    Tree(#[from] ProllyTreeError),

    /// An error from a storage backend (I/O, serialization, etc.).
    #[error(transparent)]
    Storage(#[from] StorageError),

    /// An error from the git versioning layer.
    #[cfg(feature = "git")]
    #[error(transparent)]
    Git(#[from] crate::git::types::GitKvError),
}

/// Error type for core tree operations (encoding, schema, etc.).
#[derive(Error, Debug)]
pub enum ProllyTreeError {
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unsupported Value Type")]
    UnsupportedValueType,

    #[error("Unsupported Key Type")]
    UnsupportedKeyType,

    #[error("Unsupported Chunking Strategy")]
    UnsupportedChunkingStrategy,

    #[error("Unknown Codec")]
    UnknownCodec,

    #[error("Serde Error")]
    Serde,

    #[error("Schema not found")]
    SchemaNotFound,

    #[error("Invalid JSON value")]
    InvalidJsonValue,

    #[error("Invalid digest length")]
    InvalidDigestLength,
}
