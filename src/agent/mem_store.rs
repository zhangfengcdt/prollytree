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
use chrono::{DateTime, Utc};
use serde_json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::persistence::InMemoryPersistence;
use super::traits::{EmbeddingGenerator, MemoryError, MemoryPersistence, MemoryStore};
use super::types::*;
use super::versioned_persistence::ThreadSafeVersionedPersistence;
use crate::git::{
    ThreadSafeFileVersionedKvStore, ThreadSafeGitVersionedKvStore,
    ThreadSafeInMemoryVersionedKvStore,
};

/// Enum for different persistence backends
pub enum PersistenceBackend {
    Simple(InMemoryPersistence),
    ThreadSafeGit(Arc<ThreadSafeGitVersionedKvStore<32>>),
    ThreadSafeInMemory(Arc<ThreadSafeInMemoryVersionedKvStore<32>>),
    ThreadSafeFile(Arc<ThreadSafeFileVersionedKvStore<32>>),
    // Legacy alias for git-backed persistence (maintained for compatibility)
    ThreadSafe(ThreadSafeVersionedPersistence),
}

#[async_trait::async_trait]
impl MemoryPersistence for PersistenceBackend {
    async fn save(&mut self, key: &str, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            PersistenceBackend::Simple(persistence) => persistence.save(key, data).await,
            PersistenceBackend::ThreadSafeGit(store) => {
                store.insert(key.as_bytes().to_vec(), data.to_vec())?;
                Ok(())
            }
            PersistenceBackend::ThreadSafeInMemory(store) => {
                store.insert(key.as_bytes().to_vec(), data.to_vec())?;
                Ok(())
            }
            PersistenceBackend::ThreadSafeFile(store) => {
                store.insert(key.as_bytes().to_vec(), data.to_vec())?;
                Ok(())
            }
            PersistenceBackend::ThreadSafe(persistence) => persistence.save(key, data).await,
        }
    }

    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        match self {
            PersistenceBackend::Simple(persistence) => persistence.load(key).await,
            PersistenceBackend::ThreadSafeGit(store) => Ok(store.get(key.as_bytes())),
            PersistenceBackend::ThreadSafeInMemory(store) => Ok(store.get(key.as_bytes())),
            PersistenceBackend::ThreadSafeFile(store) => Ok(store.get(key.as_bytes())),
            PersistenceBackend::ThreadSafe(persistence) => persistence.load(key).await,
        }
    }

    async fn delete(&mut self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            PersistenceBackend::Simple(persistence) => persistence.delete(key).await,
            PersistenceBackend::ThreadSafeGit(store) => {
                store.delete(key.as_bytes())?;
                Ok(())
            }
            PersistenceBackend::ThreadSafeInMemory(store) => {
                store.delete(key.as_bytes())?;
                Ok(())
            }
            PersistenceBackend::ThreadSafeFile(store) => {
                store.delete(key.as_bytes())?;
                Ok(())
            }
            PersistenceBackend::ThreadSafe(persistence) => persistence.delete(key).await,
        }
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        match self {
            PersistenceBackend::Simple(persistence) => persistence.list_keys(prefix).await,
            PersistenceBackend::ThreadSafeGit(store) => {
                let all_keys = store.list_keys()?;
                let prefix_bytes = prefix.as_bytes();
                let filtered_keys: Vec<String> = all_keys
                    .into_iter()
                    .filter_map(|key_bytes| {
                        if key_bytes.starts_with(prefix_bytes) {
                            String::from_utf8(key_bytes).ok()
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(filtered_keys)
            }
            PersistenceBackend::ThreadSafeInMemory(store) => {
                let all_keys = store.list_keys()?;
                let prefix_bytes = prefix.as_bytes();
                let filtered_keys: Vec<String> = all_keys
                    .into_iter()
                    .filter_map(|key_bytes| {
                        if key_bytes.starts_with(prefix_bytes) {
                            String::from_utf8(key_bytes).ok()
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(filtered_keys)
            }
            PersistenceBackend::ThreadSafeFile(store) => {
                let all_keys = store.list_keys()?;
                let prefix_bytes = prefix.as_bytes();
                let filtered_keys: Vec<String> = all_keys
                    .into_iter()
                    .filter_map(|key_bytes| {
                        if key_bytes.starts_with(prefix_bytes) {
                            String::from_utf8(key_bytes).ok()
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(filtered_keys)
            }
            PersistenceBackend::ThreadSafe(persistence) => persistence.list_keys(prefix).await,
        }
    }

    async fn checkpoint(&mut self, message: &str) -> Result<String, Box<dyn std::error::Error>> {
        match self {
            PersistenceBackend::Simple(persistence) => persistence.checkpoint(message).await,
            PersistenceBackend::ThreadSafeGit(store) => {
                let commit_id = store.commit(message)?;
                Ok(format!("{commit_id}"))
            }
            PersistenceBackend::ThreadSafeInMemory(store) => {
                let commit_id = store.commit(message)?;
                Ok(format!("{commit_id}"))
            }
            PersistenceBackend::ThreadSafeFile(store) => {
                let commit_id = store.commit(message)?;
                Ok(format!("{commit_id}"))
            }
            PersistenceBackend::ThreadSafe(persistence) => persistence.checkpoint(message).await,
        }
    }
}

impl PersistenceBackend {
    /// Create a new branch (git-specific operation)
    pub async fn create_branch(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            PersistenceBackend::Simple(_) => {
                Err("Branch operations not supported with Simple persistence backend".into())
            }
            PersistenceBackend::ThreadSafeGit(store) => {
                store.create_branch(name)?;
                Ok(())
            }
            PersistenceBackend::ThreadSafeInMemory(_) => {
                Err("Branch operations not supported with InMemory persistence backend".into())
            }
            PersistenceBackend::ThreadSafeFile(_) => {
                Err("Branch operations not supported with File persistence backend".into())
            }
            PersistenceBackend::ThreadSafe(persistence) => persistence.create_branch(name).await,
        }
    }

    /// Switch to a different branch (git-specific operation)
    pub async fn checkout(
        &mut self,
        branch_or_commit: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            PersistenceBackend::Simple(_) => {
                Err("Branch operations not supported with Simple persistence backend".into())
            }
            PersistenceBackend::ThreadSafeGit(store) => {
                store.checkout(branch_or_commit)?;
                Ok(())
            }
            PersistenceBackend::ThreadSafeInMemory(_) => {
                Err("Branch operations not supported with InMemory persistence backend".into())
            }
            PersistenceBackend::ThreadSafeFile(_) => {
                Err("Branch operations not supported with File persistence backend".into())
            }
            PersistenceBackend::ThreadSafe(persistence) => {
                persistence.checkout_branch(branch_or_commit).await
            }
        }
    }
}

/// Base implementation of the memory store supporting multiple persistence backends
#[derive(Clone)]
pub struct BaseMemoryStore {
    persistence: Arc<RwLock<PersistenceBackend>>,
    embedding_generator: Option<Arc<dyn EmbeddingGenerator>>,
    agent_id: String,
    current_branch: String,
}

impl BaseMemoryStore {
    /// Get the agent ID
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Initialize a new memory store with Simple persistence backend
    pub fn init<P: AsRef<Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let persistence = InMemoryPersistence::init(path, &format!("agent_memory_{agent_id}"))?;
        Ok(Self {
            persistence: Arc::new(RwLock::new(PersistenceBackend::Simple(persistence))),
            embedding_generator: embedding_generator
                .map(|gen| Arc::from(gen) as Arc<dyn EmbeddingGenerator>),
            agent_id,
            current_branch: "main".to_string(),
        })
    }

    /// Initialize a new memory store with thread-safe Git persistence backend
    pub fn init_with_thread_safe_git<P: AsRef<Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let store = ThreadSafeGitVersionedKvStore::init(path)?;
        Ok(Self {
            persistence: Arc::new(RwLock::new(PersistenceBackend::ThreadSafeGit(Arc::new(
                store,
            )))),
            embedding_generator: embedding_generator
                .map(|gen| Arc::from(gen) as Arc<dyn EmbeddingGenerator>),
            agent_id,
            current_branch: "main".to_string(),
        })
    }

    /// Initialize a new memory store with thread-safe InMemory persistence backend
    pub fn init_with_thread_safe_inmemory<P: AsRef<Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let store = ThreadSafeInMemoryVersionedKvStore::init(path)?;
        Ok(Self {
            persistence: Arc::new(RwLock::new(PersistenceBackend::ThreadSafeInMemory(
                Arc::new(store),
            ))),
            embedding_generator: embedding_generator
                .map(|gen| Arc::from(gen) as Arc<dyn EmbeddingGenerator>),
            agent_id,
            current_branch: "main".to_string(),
        })
    }

    /// Initialize a new memory store with thread-safe File persistence backend
    pub fn init_with_thread_safe_file<P: AsRef<Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let store = ThreadSafeFileVersionedKvStore::<32>::init(path)?;
        Ok(Self {
            persistence: Arc::new(RwLock::new(PersistenceBackend::ThreadSafeFile(Arc::new(
                store,
            )))),
            embedding_generator: embedding_generator
                .map(|gen| Arc::from(gen) as Arc<dyn EmbeddingGenerator>),
            agent_id,
            current_branch: "main".to_string(),
        })
    }

    /// Initialize a new memory store with thread-safe Prolly persistence backend (git-backed)
    pub fn init_with_thread_safe_prolly<P: AsRef<Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let persistence =
            ThreadSafeVersionedPersistence::init(path, &format!("agent_memory_{agent_id}"))?;
        Ok(Self {
            persistence: Arc::new(RwLock::new(PersistenceBackend::ThreadSafe(persistence))),
            embedding_generator: embedding_generator
                .map(|gen| Arc::from(gen) as Arc<dyn EmbeddingGenerator>),
            agent_id,
            current_branch: "main".to_string(),
        })
    }

    /// Open an existing memory store with Simple persistence backend
    pub fn open<P: AsRef<Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let persistence = InMemoryPersistence::open(path, &format!("agent_memory_{agent_id}"))?;
        Ok(Self {
            persistence: Arc::new(RwLock::new(PersistenceBackend::Simple(persistence))),
            embedding_generator: embedding_generator
                .map(|gen| Arc::from(gen) as Arc<dyn EmbeddingGenerator>),
            agent_id,
            current_branch: "main".to_string(),
        })
    }

    /// Open an existing memory store with thread-safe Git persistence backend
    pub fn open_with_thread_safe_git<P: AsRef<Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let store = ThreadSafeGitVersionedKvStore::open(path)?;
        Ok(Self {
            persistence: Arc::new(RwLock::new(PersistenceBackend::ThreadSafeGit(Arc::new(
                store,
            )))),
            embedding_generator: embedding_generator
                .map(|gen| Arc::from(gen) as Arc<dyn EmbeddingGenerator>),
            agent_id,
            current_branch: "main".to_string(),
        })
    }

    /// Open an existing memory store with thread-safe InMemory persistence backend
    pub fn open_with_thread_safe_inmemory<P: AsRef<Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let store = ThreadSafeInMemoryVersionedKvStore::open(path)?;
        Ok(Self {
            persistence: Arc::new(RwLock::new(PersistenceBackend::ThreadSafeInMemory(
                Arc::new(store),
            ))),
            embedding_generator: embedding_generator
                .map(|gen| Arc::from(gen) as Arc<dyn EmbeddingGenerator>),
            agent_id,
            current_branch: "main".to_string(),
        })
    }

    /// Open an existing memory store with thread-safe File persistence backend
    pub fn open_with_thread_safe_file<P: AsRef<Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let store = ThreadSafeFileVersionedKvStore::open(path)?;
        Ok(Self {
            persistence: Arc::new(RwLock::new(PersistenceBackend::ThreadSafeFile(Arc::new(
                store,
            )))),
            embedding_generator: embedding_generator
                .map(|gen| Arc::from(gen) as Arc<dyn EmbeddingGenerator>),
            agent_id,
            current_branch: "main".to_string(),
        })
    }

    /// Open an existing memory store with thread-safe Prolly persistence backend
    pub fn open_with_thread_safe_prolly<P: AsRef<Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let persistence =
            ThreadSafeVersionedPersistence::open(path, &format!("agent_memory_{agent_id}"))?;
        Ok(Self {
            persistence: Arc::new(RwLock::new(PersistenceBackend::ThreadSafe(persistence))),
            embedding_generator: embedding_generator
                .map(|gen| Arc::from(gen) as Arc<dyn EmbeddingGenerator>),
            agent_id,
            current_branch: "main".to_string(),
        })
    }

    // /// Open an existing memory store with Prolly persistence backend (git-backed)
    // /// Complete implementation available but disabled due to thread safety limitations.
    // pub fn open_with_prolly<P: AsRef<Path>>(
    //     path: P,
    //     agent_id: String,
    //     embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    // ) -> Result<Self, Box<dyn std::error::Error>> {
    //     let persistence = ProllyMemoryPersistence::open(path, &format!("agent_memory_{agent_id}"))?;
    //     Ok(Self {
    //         persistence: Arc::new(RwLock::new(PersistenceBackend::Prolly(persistence))),
    //         embedding_generator: embedding_generator
    //             .map(|gen| Arc::from(gen) as Arc<dyn EmbeddingGenerator>),
    //         agent_id,
    //         current_branch: "main".to_string(),
    //     })
    // }

    // /// Get access to git logs (only available with Prolly backend)
    // /// Complete implementation available but disabled due to thread safety limitations.
    // pub async fn get_git_logs(&self) -> Result<Vec<crate::git::CommitInfo>, Box<dyn std::error::Error>> {
    //     let persistence = self.persistence.read().await;
    //     match &*persistence {
    //         PersistenceBackend::Prolly(prolly) => {
    //             prolly.get_git_log().await.map_err(|e| e.into())
    //         }
    //         PersistenceBackend::Simple(_) => {
    //             Err("Git logs not available with Simple persistence backend".into())
    //         }
    //     }
    // }

    /// Generate key for memory document
    fn memory_key(&self, namespace: &MemoryNamespace, id: &str) -> String {
        format!("{}/{}", namespace.to_path(), id)
    }

    /// Generate embeddings if generator is available
    async fn generate_embeddings(&self, content: &serde_json::Value) -> Option<Vec<f32>> {
        if let Some(ref generator) = self.embedding_generator {
            // Extract text content for embedding generation
            let text = self.extract_text_content(content);
            if !text.is_empty() {
                match generator.generate(&text).await {
                    Ok(embeddings) => Some(embeddings),
                    Err(e) => {
                        eprintln!("Failed to generate embeddings: {e}");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Extract text content from JSON for embedding generation
    fn extract_text_content(&self, content: &serde_json::Value) -> String {
        match content {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Object(map) => {
                let mut text_parts = Vec::new();

                // Common text fields to extract
                let text_fields = ["text", "content", "message", "description", "summary"];

                for field in &text_fields {
                    if let Some(value) = map.get(*field) {
                        if let Some(text) = value.as_str() {
                            text_parts.push(text);
                        }
                    }
                }

                // If no specific text fields, join all string values
                if text_parts.is_empty() {
                    for value in map.values() {
                        if let Some(text) = value.as_str() {
                            text_parts.push(text);
                        }
                    }
                }

                text_parts.join(" ")
            }
            _ => content.to_string(),
        }
    }

    /// Validate memory document
    fn validate_memory(&self, memory: &MemoryDocument) -> Result<(), MemoryError> {
        // Check if agent ID matches
        if memory.metadata.agent_id != *self.agent_id() {
            return Err(MemoryError::PermissionDenied(format!(
                "Memory belongs to different agent: {}",
                memory.metadata.agent_id
            )));
        }

        // Check if memory is expired
        if memory.metadata.is_expired() {
            return Err(MemoryError::Expired(format!(
                "Memory {} has expired",
                memory.id
            )));
        }

        Ok(())
    }

    /// Serialize memory document for storage
    fn serialize_memory(&self, memory: &MemoryDocument) -> Result<Vec<u8>, MemoryError> {
        serde_json::to_vec(memory)
            .map_err(|e| MemoryError::SerializationError(format!("Failed to serialize: {e}")))
    }

    /// Deserialize memory document from storage
    fn deserialize_memory(&self, data: &[u8]) -> Result<MemoryDocument, MemoryError> {
        serde_json::from_slice(data)
            .map_err(|e| MemoryError::SerializationError(format!("Failed to deserialize: {e}")))
    }
}

#[async_trait]
impl MemoryStore for BaseMemoryStore {
    async fn store(&mut self, mut memory: MemoryDocument) -> Result<String, MemoryError> {
        // Validate the memory
        self.validate_memory(&memory)?;

        // Generate ID if not provided
        if memory.id.is_empty() {
            memory.id = Uuid::new_v4().to_string();
        }

        // Generate embeddings if available
        if memory.embeddings.is_none() {
            memory.embeddings = self.generate_embeddings(&memory.content).await;
        }

        // Update metadata
        memory.metadata.updated_at = Utc::now();

        // Store the memory
        let key = self.memory_key(&memory.namespace, &memory.id);
        let data = self.serialize_memory(&memory)?;

        {
            let mut persistence = self.persistence.write().await;
            (*persistence)
                .save(&key, &data)
                .await
                .map_err(|e| MemoryError::StorageError(format!("Failed to store: {e}")))?;
        }

        Ok(memory.id)
    }

    async fn update(&mut self, id: &str, mut memory: MemoryDocument) -> Result<(), MemoryError> {
        // Ensure the ID matches
        memory.id = id.to_string();

        // Validate the memory
        self.validate_memory(&memory)?;

        // Generate embeddings if content changed
        if memory.embeddings.is_none() {
            memory.embeddings = self.generate_embeddings(&memory.content).await;
        }

        // Update metadata
        memory.metadata.updated_at = Utc::now();

        // Store the updated memory
        let key = self.memory_key(&memory.namespace, id);
        let data = self.serialize_memory(&memory)?;

        {
            let mut persistence = self.persistence.write().await;
            (*persistence)
                .save(&key, &data)
                .await
                .map_err(|e| MemoryError::StorageError(format!("Failed to update: {e}")))?;
        }

        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryDocument>, MemoryError> {
        // We need to search across all namespaces for this ID
        // This is a simplified implementation - in practice, you might want to index by ID
        let persistence = self.persistence.read().await;

        // Try different memory types and namespaces
        for memory_type in [
            MemoryType::ShortTerm,
            MemoryType::Semantic,
            MemoryType::Episodic,
            MemoryType::Procedural,
        ] {
            let namespace = MemoryNamespace::new(self.agent_id().to_string(), memory_type);
            let key = self.memory_key(&namespace, id);

            let data_result = (*persistence)
                .load(&key)
                .await
                .map_err(|e| MemoryError::StorageError(format!("Failed to load: {e}")))?;

            if let Some(data) = data_result {
                let memory = self.deserialize_memory(&data)?;
                return Ok(Some(memory));
            }
        }

        Ok(None)
    }

    async fn delete(&mut self, id: &str) -> Result<(), MemoryError> {
        // Similar to get, we need to find the memory first
        if let Some(memory) = self.get(id).await? {
            let key = self.memory_key(&memory.namespace, id);
            let mut persistence = self.persistence.write().await;
            (*persistence)
                .delete(&key)
                .await
                .map_err(|e| MemoryError::StorageError(format!("Failed to delete: {e}")))?;
            Ok(())
        } else {
            Err(MemoryError::NotFound(format!(
                "Memory with ID {id} not found"
            )))
        }
    }

    async fn query(&self, query: MemoryQuery) -> Result<Vec<MemoryDocument>, MemoryError> {
        let mut results = Vec::new();
        let persistence = self.persistence.read().await;

        // Determine which namespaces to search
        let namespaces = if let Some(ns) = &query.namespace {
            vec![ns.clone()]
        } else {
            // Search all memory types for this agent
            let memory_types = query.memory_types.clone().unwrap_or_else(|| {
                vec![
                    MemoryType::ShortTerm,
                    MemoryType::Semantic,
                    MemoryType::Episodic,
                    MemoryType::Procedural,
                ]
            });

            memory_types
                .into_iter()
                .map(|mt| MemoryNamespace::new(self.agent_id().to_string(), mt))
                .collect()
        };

        // Search each namespace
        for namespace in namespaces {
            let prefix = namespace.to_path();
            let keys = (*persistence)
                .list_keys(&prefix)
                .await
                .map_err(|e| MemoryError::StorageError(format!("Failed to list keys: {e}")))?;

            for key in keys {
                let data_result = (*persistence)
                    .load(&key)
                    .await
                    .map_err(|e| MemoryError::StorageError(format!("Failed to load: {e}")))?;

                if let Some(data) = data_result {
                    if let Ok(memory) = self.deserialize_memory(&data) {
                        // Apply filters
                        if self.matches_query(&memory, &query) {
                            results.push(memory);
                        }
                    }
                }
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    async fn get_by_namespace(
        &self,
        namespace: &MemoryNamespace,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        let mut results = Vec::new();
        let persistence = self.persistence.read().await;

        let prefix = namespace.to_path();
        let keys = (*persistence)
            .list_keys(&prefix)
            .await
            .map_err(|e| MemoryError::StorageError(format!("Failed to list keys: {e}")))?;

        for key in keys {
            let data_result = (*persistence)
                .load(&key)
                .await
                .map_err(|e| MemoryError::StorageError(format!("Failed to load: {e}")))?;

            if let Some(data) = data_result {
                if let Ok(memory) = self.deserialize_memory(&data) {
                    if !memory.metadata.is_expired() {
                        results.push(memory);
                    }
                }
            }
        }

        Ok(results)
    }

    async fn commit(&mut self, message: &str) -> Result<String, MemoryError> {
        let mut persistence = self.persistence.write().await;
        (*persistence)
            .checkpoint(message)
            .await
            .map_err(|e| MemoryError::StorageError(format!("Failed to commit: {e}")))
    }

    async fn create_branch(&mut self, name: &str) -> Result<(), MemoryError> {
        let mut persistence = self.persistence.write().await;
        persistence.create_branch(name).await?;
        Ok(())
    }

    async fn checkout(&mut self, branch_or_commit: &str) -> Result<(), MemoryError> {
        let mut persistence = self.persistence.write().await;
        persistence.checkout(branch_or_commit).await?;
        self.current_branch = branch_or_commit.to_string();
        Ok(())
    }

    fn current_branch(&self) -> &str {
        &self.current_branch
    }

    async fn get_stats(&self) -> Result<MemoryStats, MemoryError> {
        let mut by_type = HashMap::new();
        let mut by_namespace = HashMap::new();
        let mut total_memories = 0;
        let mut total_size_bytes = 0;
        let mut access_counts = Vec::new();
        let mut oldest: Option<DateTime<Utc>> = None;
        let mut newest: Option<DateTime<Utc>> = None;
        let mut expired_count = 0;

        // Query all memories to build stats
        let query = MemoryQuery {
            namespace: None,
            memory_types: None,
            tags: None,
            time_range: None,
            text_query: None,
            semantic_query: None,
            limit: None,
            include_expired: true,
        };

        let memories = self.query(query).await?;

        for memory in memories {
            total_memories += 1;

            // Count by type
            *by_type.entry(memory.memory_type).or_insert(0) += 1;

            // Count by namespace
            let ns_key = memory.namespace.to_path();
            *by_namespace.entry(ns_key).or_insert(0) += 1;

            // Size estimation (rough)
            total_size_bytes += std::mem::size_of_val(&memory);

            // Access count
            access_counts.push(memory.metadata.access_count as f64);

            // Time tracking
            if oldest.is_none_or(|o| memory.metadata.created_at < o) {
                oldest = Some(memory.metadata.created_at);
            }
            if newest.is_none_or(|n| memory.metadata.created_at > n) {
                newest = Some(memory.metadata.created_at);
            }

            // Expired count
            if memory.metadata.is_expired() {
                expired_count += 1;
            }
        }

        let avg_access_count = if access_counts.is_empty() {
            0.0
        } else {
            access_counts.iter().sum::<f64>() / access_counts.len() as f64
        };

        Ok(MemoryStats {
            total_memories,
            by_type,
            by_namespace,
            total_size_bytes,
            avg_access_count,
            oldest_memory: oldest,
            newest_memory: newest,
            expired_count,
        })
    }

    async fn cleanup_expired(&mut self) -> Result<usize, MemoryError> {
        let query = MemoryQuery {
            namespace: None,
            memory_types: None,
            tags: None,
            time_range: None,
            text_query: None,
            semantic_query: None,
            limit: None,
            include_expired: true,
        };

        let memories = self.query(query).await?;
        let mut cleaned_count = 0;

        for memory in memories {
            if memory.metadata.is_expired() {
                self.delete(&memory.id).await?;
                cleaned_count += 1;
            }
        }

        Ok(cleaned_count)
    }
}

impl BaseMemoryStore {
    /// Check if a memory matches the query criteria
    fn matches_query(&self, memory: &MemoryDocument, query: &MemoryQuery) -> bool {
        // Check expiry
        if !query.include_expired && memory.metadata.is_expired() {
            return false;
        }

        // Check tags
        if let Some(required_tags) = &query.tags {
            if !required_tags
                .iter()
                .all(|tag| memory.metadata.tags.contains(tag))
            {
                return false;
            }
        }

        // Check time range
        if let Some(time_range) = &query.time_range {
            if let Some(start) = time_range.start {
                if memory.metadata.created_at < start {
                    return false;
                }
            }
            if let Some(end) = time_range.end {
                if memory.metadata.created_at > end {
                    return false;
                }
            }
        }

        // Check text query (simple substring search)
        if let Some(text_query) = &query.text_query {
            let content_str = memory.content.to_string().to_lowercase();
            if !content_str.contains(&text_query.to_lowercase()) {
                return false;
            }
        }

        true
    }
}
