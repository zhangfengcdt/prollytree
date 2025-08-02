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
use chrono::{Duration, Utc};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use super::mem_store::BaseMemoryStore;
use super::traits::{MemoryError, MemoryStore};
use super::types::*;

/// Short-term memory store for session/thread-scoped memories
pub struct ShortTermMemoryStore {
    base_store: BaseMemoryStore,
    default_ttl: Duration,
    max_memories_per_thread: usize,
}

impl ShortTermMemoryStore {
    /// Create a new short-term memory store
    pub fn new(
        base_store: BaseMemoryStore,
        default_ttl: Duration,
        max_memories_per_thread: usize,
    ) -> Self {
        Self {
            base_store,
            default_ttl,
            max_memories_per_thread,
        }
    }

    /// Store a conversation turn
    pub async fn store_conversation_turn(
        &mut self,
        thread_id: &str,
        role: &str,
        content: &str,
        metadata: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<String, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::ShortTerm,
            thread_id.to_string(),
        );

        let mut memory_metadata = MemoryMetadata::new(
            self.base_store.agent_id().to_string(),
            "conversation".to_string(),
        );
        memory_metadata.thread_id = Some(thread_id.to_string());
        memory_metadata.ttl = Some(self.default_ttl);
        memory_metadata.tags = vec!["conversation".to_string(), role.to_string()];

        let mut content_json = json!({
            "role": role,
            "content": content,
            "timestamp": Utc::now()
        });

        // Add additional metadata if provided
        if let Some(meta) = metadata {
            if let serde_json::Value::Object(ref mut map) = content_json {
                for (key, value) in meta {
                    map.insert(key, value);
                }
            }
        }

        let memory = MemoryDocument {
            id: Uuid::new_v4().to_string(),
            namespace,
            memory_type: MemoryType::ShortTerm,
            content: content_json,
            metadata: memory_metadata,
            embeddings: None,
        };

        // Check thread memory limit
        self.enforce_thread_limit(thread_id).await?;

        self.base_store.store(memory).await
    }

    /// Get conversation history for a thread
    pub async fn get_conversation_history(
        &self,
        thread_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::ShortTerm,
            thread_id.to_string(),
        );

        let query = MemoryQuery {
            namespace: Some(namespace),
            memory_types: Some(vec![MemoryType::ShortTerm]),
            tags: Some(vec!["conversation".to_string()]),
            time_range: None,
            text_query: None,
            semantic_query: None,
            limit,
            include_expired: false,
        };

        let mut memories = self.base_store.query(query).await?;

        // Sort by creation time
        memories.sort_by(|a, b| a.metadata.created_at.cmp(&b.metadata.created_at));

        Ok(memories)
    }

    /// Store working memory (temporary state, calculations, etc.)
    pub async fn store_working_memory(
        &mut self,
        thread_id: &str,
        key: &str,
        data: serde_json::Value,
        ttl: Option<Duration>,
    ) -> Result<String, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::ShortTerm,
            format!("{thread_id}/working"),
        );

        let mut memory_metadata = MemoryMetadata::new(
            self.base_store.agent_id().to_string(),
            "working_memory".to_string(),
        );
        memory_metadata.thread_id = Some(thread_id.to_string());
        memory_metadata.ttl = ttl.or(Some(self.default_ttl));
        memory_metadata.tags = vec!["working_memory".to_string(), key.to_string()];

        let content = json!({
            "key": key,
            "data": data,
            "timestamp": Utc::now()
        });

        let memory = MemoryDocument {
            id: format!("working_{thread_id}_{key}"),
            namespace,
            memory_type: MemoryType::ShortTerm,
            content,
            metadata: memory_metadata,
            embeddings: None,
        };

        self.base_store.store(memory).await
    }

    /// Get working memory by key
    pub async fn get_working_memory(
        &self,
        thread_id: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>, MemoryError> {
        let memory_id = format!("working_{thread_id}_{key}");

        if let Some(memory) = self.base_store.get(&memory_id).await? {
            if let Some(data) = memory.content.get("data") {
                Ok(Some(data.clone()))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Store context information for the current session
    pub async fn store_session_context(
        &mut self,
        thread_id: &str,
        context_type: &str,
        context_data: serde_json::Value,
    ) -> Result<String, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::ShortTerm,
            format!("{thread_id}/context"),
        );

        let mut memory_metadata = MemoryMetadata::new(
            self.base_store.agent_id().to_string(),
            "session_context".to_string(),
        );
        memory_metadata.thread_id = Some(thread_id.to_string());
        memory_metadata.ttl = Some(self.default_ttl);
        memory_metadata.tags = vec!["context".to_string(), context_type.to_string()];

        let content = json!({
            "context_type": context_type,
            "context_data": context_data,
            "timestamp": Utc::now()
        });

        let memory = MemoryDocument {
            id: format!("context_{thread_id}_{context_type}"),
            namespace,
            memory_type: MemoryType::ShortTerm,
            content,
            metadata: memory_metadata,
            embeddings: None,
        };

        self.base_store.store(memory).await
    }

    /// Get all session context
    pub async fn get_session_context(
        &self,
        thread_id: &str,
    ) -> Result<HashMap<String, serde_json::Value>, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::ShortTerm,
            format!("{thread_id}/context"),
        );

        let memories = self.base_store.get_by_namespace(&namespace).await?;
        let mut context = HashMap::new();

        for memory in memories {
            if let Some(context_type) = memory.content.get("context_type").and_then(|v| v.as_str())
            {
                if let Some(context_data) = memory.content.get("context_data") {
                    context.insert(context_type.to_string(), context_data.clone());
                }
            }
        }

        Ok(context)
    }

    /// Clear all memories for a thread
    pub async fn clear_thread(&mut self, thread_id: &str) -> Result<usize, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::ShortTerm,
            thread_id.to_string(),
        );

        let memories = self.base_store.get_by_namespace(&namespace).await?;
        let count = memories.len();

        for memory in memories {
            self.base_store.delete(&memory.id).await?;
        }

        Ok(count)
    }

    /// Enforce memory limit per thread
    async fn enforce_thread_limit(&mut self, thread_id: &str) -> Result<(), MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::ShortTerm,
            thread_id.to_string(),
        );

        let mut memories = self.base_store.get_by_namespace(&namespace).await?;

        if memories.len() >= self.max_memories_per_thread {
            // Sort by creation time and remove oldest
            memories.sort_by(|a, b| a.metadata.created_at.cmp(&b.metadata.created_at));

            let to_remove = memories.len() - self.max_memories_per_thread + 1;
            for memory in memories.iter().take(to_remove) {
                self.base_store.delete(&memory.id).await?;
            }
        }

        Ok(())
    }

    /// Get memory statistics for short-term store
    pub async fn get_short_term_stats(&self) -> Result<ShortTermStats, MemoryError> {
        let query = MemoryQuery {
            namespace: None,
            memory_types: Some(vec![MemoryType::ShortTerm]),
            tags: None,
            time_range: None,
            text_query: None,
            semantic_query: None,
            limit: None,
            include_expired: true,
        };

        let memories = self.base_store.query(query).await?;

        let mut thread_counts = HashMap::new();
        let mut active_threads = std::collections::HashSet::new();
        let mut total_conversations = 0;
        let mut total_working_memory = 0;
        let mut expired_count = 0;

        for memory in memories {
            if let Some(thread_id) = &memory.metadata.thread_id {
                *thread_counts.entry(thread_id.clone()).or_insert(0) += 1;

                if !memory.metadata.is_expired() {
                    active_threads.insert(thread_id.clone());
                }
            }

            if memory.metadata.tags.contains(&"conversation".to_string()) {
                total_conversations += 1;
            }

            if memory.metadata.tags.contains(&"working_memory".to_string()) {
                total_working_memory += 1;
            }

            if memory.metadata.is_expired() {
                expired_count += 1;
            }
        }

        Ok(ShortTermStats {
            total_threads: thread_counts.len(),
            active_threads: active_threads.len(),
            total_conversations,
            total_working_memory,
            thread_memory_counts: thread_counts,
            expired_count,
        })
    }
}

// Delegate most MemoryStore methods to the base store
#[async_trait]
impl MemoryStore for ShortTermMemoryStore {
    async fn store(&mut self, memory: MemoryDocument) -> Result<String, MemoryError> {
        // Ensure it's short-term memory
        if memory.memory_type != MemoryType::ShortTerm {
            return Err(MemoryError::InvalidNamespace(
                "Memory type must be ShortTerm".to_string(),
            ));
        }
        self.base_store.store(memory).await
    }

    async fn update(&mut self, id: &str, memory: MemoryDocument) -> Result<(), MemoryError> {
        self.base_store.update(id, memory).await
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryDocument>, MemoryError> {
        self.base_store.get(id).await
    }

    async fn delete(&mut self, id: &str) -> Result<(), MemoryError> {
        self.base_store.delete(id).await
    }

    async fn query(&self, query: MemoryQuery) -> Result<Vec<MemoryDocument>, MemoryError> {
        self.base_store.query(query).await
    }

    async fn get_by_namespace(
        &self,
        namespace: &MemoryNamespace,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        self.base_store.get_by_namespace(namespace).await
    }

    async fn commit(&mut self, message: &str) -> Result<String, MemoryError> {
        self.base_store.commit(message).await
    }

    async fn create_branch(&mut self, name: &str) -> Result<(), MemoryError> {
        self.base_store.create_branch(name).await
    }

    async fn checkout(&mut self, branch_or_commit: &str) -> Result<(), MemoryError> {
        self.base_store.checkout(branch_or_commit).await
    }

    fn current_branch(&self) -> &str {
        self.base_store.current_branch()
    }

    async fn get_stats(&self) -> Result<MemoryStats, MemoryError> {
        self.base_store.get_stats().await
    }

    async fn cleanup_expired(&mut self) -> Result<usize, MemoryError> {
        self.base_store.cleanup_expired().await
    }
}

/// Statistics specific to short-term memory
#[derive(Debug, Clone, serde::Serialize)]
pub struct ShortTermStats {
    pub total_threads: usize,
    pub active_threads: usize,
    pub total_conversations: usize,
    pub total_working_memory: usize,
    pub thread_memory_counts: HashMap<String, usize>,
    pub expired_count: usize,
}
