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

use async_trait::async_trait;
use std::error::Error;
use std::fmt;

use super::types::*;

/// Error types for memory operations
#[derive(Debug)]
pub enum MemoryError {
    NotFound(String),
    InvalidNamespace(String),
    StorageError(String),
    SerializationError(String),
    PermissionDenied(String),
    Expired(String),
    ConflictError(String),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::NotFound(msg) => write!(f, "Memory not found: {msg}"),
            MemoryError::InvalidNamespace(msg) => write!(f, "Invalid namespace: {msg}"),
            MemoryError::StorageError(msg) => write!(f, "Storage error: {msg}"),
            MemoryError::SerializationError(msg) => write!(f, "Serialization error: {msg}"),
            MemoryError::PermissionDenied(msg) => write!(f, "Permission denied: {msg}"),
            MemoryError::Expired(msg) => write!(f, "Memory expired: {msg}"),
            MemoryError::ConflictError(msg) => write!(f, "Conflict error: {msg}"),
        }
    }
}

impl Error for MemoryError {}

impl From<Box<dyn Error>> for MemoryError {
    fn from(error: Box<dyn Error>) -> Self {
        MemoryError::StorageError(error.to_string())
    }
}

/// Core trait for memory store implementations
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Store a new memory document
    async fn store(&mut self, memory: MemoryDocument) -> Result<String, MemoryError>;

    /// Update an existing memory
    async fn update(&mut self, id: &str, memory: MemoryDocument) -> Result<(), MemoryError>;

    /// Retrieve a memory by ID
    async fn get(&self, id: &str) -> Result<Option<MemoryDocument>, MemoryError>;

    /// Delete a memory by ID
    async fn delete(&mut self, id: &str) -> Result<(), MemoryError>;

    /// Query memories based on criteria
    async fn query(&self, query: MemoryQuery) -> Result<Vec<MemoryDocument>, MemoryError>;

    /// Get memories by namespace
    async fn get_by_namespace(
        &self,
        namespace: &MemoryNamespace,
    ) -> Result<Vec<MemoryDocument>, MemoryError>;

    /// Commit current changes (for version control)
    async fn commit(&mut self, message: &str) -> Result<String, MemoryError>;

    /// Create a new branch
    async fn create_branch(&mut self, name: &str) -> Result<(), MemoryError>;

    /// Switch to a different branch
    async fn checkout(&mut self, branch_or_commit: &str) -> Result<(), MemoryError>;

    /// Get current branch name
    fn current_branch(&self) -> &str;

    /// Get memory statistics
    async fn get_stats(&self) -> Result<MemoryStats, MemoryError>;

    /// Cleanup expired memories
    async fn cleanup_expired(&mut self) -> Result<usize, MemoryError>;
}

/// Trait for memory stores with search capabilities
#[async_trait]
pub trait SearchableMemoryStore: MemoryStore {
    /// Perform semantic search using embeddings
    async fn semantic_search(
        &self,
        query: SemanticQuery,
        namespace: Option<&MemoryNamespace>,
    ) -> Result<Vec<(MemoryDocument, f64)>, MemoryError>;

    /// Full-text search across memories
    async fn text_search(
        &self,
        query: &str,
        namespace: Option<&MemoryNamespace>,
    ) -> Result<Vec<MemoryDocument>, MemoryError>;

    /// Find related memories based on a given memory
    async fn find_related(
        &self,
        memory_id: &str,
        limit: usize,
    ) -> Result<Vec<MemoryDocument>, MemoryError>;
}

/// Trait for memory lifecycle management
#[async_trait]
pub trait MemoryLifecycle: MemoryStore {
    /// Apply consolidation strategy
    async fn consolidate(&mut self, strategy: ConsolidationStrategy) -> Result<usize, MemoryError>;

    /// Archive old memories
    async fn archive(
        &mut self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<usize, MemoryError>;

    /// Subscribe to memory events
    async fn subscribe_events<F>(&mut self, callback: F) -> Result<(), MemoryError>
    where
        F: Fn(MemoryEvent) + Send + Sync + 'static;

    /// Get memory history for a specific ID
    async fn get_history(&self, memory_id: &str) -> Result<Vec<MemoryDocument>, MemoryError>;
}

/// Trait for embedding generation
#[async_trait]
pub trait EmbeddingGenerator: Send + Sync {
    /// Generate embeddings for text content
    async fn generate(&self, text: &str) -> Result<Vec<f32>, Box<dyn Error>>;

    /// Batch generate embeddings
    async fn generate_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Box<dyn Error>>;
}

/// Trait for memory persistence backend
#[async_trait]
pub trait MemoryPersistence: Send + Sync {
    /// Save memory data
    async fn save(&mut self, key: &str, data: &[u8]) -> Result<(), Box<dyn Error>>;

    /// Load memory data
    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn Error>>;

    /// Delete memory data
    async fn delete(&mut self, key: &str) -> Result<(), Box<dyn Error>>;

    /// List keys with prefix
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, Box<dyn Error>>;

    /// Create checkpoint
    async fn checkpoint(&mut self, message: &str) -> Result<String, Box<dyn Error>>;
}
