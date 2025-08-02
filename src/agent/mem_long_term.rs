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
use chrono::{Datelike, Utc};
use serde_json::json;
use uuid::Uuid;

use super::mem_store::BaseMemoryStore;
use super::traits::{MemoryError, MemoryStore, SearchableMemoryStore};
use super::types::*;

/// Semantic memory store for facts, concepts, and knowledge
pub struct SemanticMemoryStore {
    base_store: BaseMemoryStore,
}

impl SemanticMemoryStore {
    pub fn new(base_store: BaseMemoryStore) -> Self {
        Self { base_store }
    }

    /// Store a fact or concept
    pub async fn store_fact(
        &mut self,
        entity_type: &str,
        entity_id: &str,
        fact: serde_json::Value,
        confidence: f64,
        source: &str,
    ) -> Result<String, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::Semantic,
            format!("{entity_type}/{entity_id}"),
        );

        let mut metadata =
            MemoryMetadata::new(self.base_store.agent_id().to_string(), source.to_string());
        metadata.confidence = confidence;
        metadata.tags = vec!["fact".to_string(), entity_type.to_string()];

        let content = json!({
            "entity_type": entity_type,
            "entity_id": entity_id,
            "fact": fact,
            "confidence": confidence,
            "timestamp": Utc::now()
        });

        let memory = MemoryDocument {
            id: Uuid::new_v4().to_string(),
            namespace,
            memory_type: MemoryType::Semantic,
            content,
            metadata,
            embeddings: None,
        };

        self.base_store.store(memory).await
    }

    /// Store a relationship between entities
    pub async fn store_relationship(
        &mut self,
        from_entity: (&str, &str), // (type, id)
        to_entity: (&str, &str),   // (type, id)
        relationship_type: &str,
        properties: Option<serde_json::Value>,
        confidence: f64,
    ) -> Result<String, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::Semantic,
            "relationships".to_string(),
        );

        let mut metadata = MemoryMetadata::new(
            self.base_store.agent_id().to_string(),
            "relationship_inference".to_string(),
        );
        metadata.confidence = confidence;
        metadata.tags = vec!["relationship".to_string(), relationship_type.to_string()];

        let content = json!({
            "from_entity": {
                "type": from_entity.0,
                "id": from_entity.1
            },
            "to_entity": {
                "type": to_entity.0,
                "id": to_entity.1
            },
            "relationship_type": relationship_type,
            "properties": properties,
            "confidence": confidence,
            "timestamp": Utc::now()
        });

        let memory = MemoryDocument {
            id: Uuid::new_v4().to_string(),
            namespace,
            memory_type: MemoryType::Semantic,
            content,
            metadata,
            embeddings: None,
        };

        self.base_store.store(memory).await
    }

    /// Get all facts about an entity
    pub async fn get_entity_facts(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::Semantic,
            format!("{entity_type}/{entity_id}"),
        );

        self.base_store.get_by_namespace(&namespace).await
    }

    /// Get relationships involving an entity
    pub async fn get_entity_relationships(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        let query = MemoryQuery {
            namespace: Some(MemoryNamespace::with_sub(
                self.base_store.agent_id().to_string(),
                MemoryType::Semantic,
                "relationships".to_string(),
            )),
            memory_types: Some(vec![MemoryType::Semantic]),
            tags: Some(vec!["relationship".to_string()]),
            time_range: None,
            text_query: Some(format!("{entity_type}:{entity_id}")),
            semantic_query: None,
            limit: None,
            include_expired: false,
        };

        self.base_store.query(query).await
    }
}

/// Episodic memory store for experiences and events
pub struct EpisodicMemoryStore {
    base_store: BaseMemoryStore,
}

impl EpisodicMemoryStore {
    pub fn new(base_store: BaseMemoryStore) -> Self {
        Self { base_store }
    }

    /// Store an episode/experience
    pub async fn store_episode(
        &mut self,
        episode_type: &str,
        description: &str,
        context: serde_json::Value,
        outcome: Option<serde_json::Value>,
        importance: f64,
    ) -> Result<String, MemoryError> {
        let timestamp = Utc::now();
        let time_bucket = format!("{}-{:02}", timestamp.year(), timestamp.month());

        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::Episodic,
            time_bucket,
        );

        let mut metadata = MemoryMetadata::new(
            self.base_store.agent_id().to_string(),
            "experience".to_string(),
        );
        metadata.confidence = importance;
        metadata.tags = vec!["episode".to_string(), episode_type.to_string()];

        let content = json!({
            "episode_type": episode_type,
            "description": description,
            "context": context,
            "outcome": outcome,
            "importance": importance,
            "timestamp": timestamp
        });

        let memory = MemoryDocument {
            id: Uuid::new_v4().to_string(),
            namespace,
            memory_type: MemoryType::Episodic,
            content,
            metadata,
            embeddings: None,
        };

        self.base_store.store(memory).await
    }

    /// Store an interaction
    pub async fn store_interaction(
        &mut self,
        interaction_type: &str,
        participants: Vec<String>,
        summary: &str,
        details: serde_json::Value,
        sentiment: Option<f64>,
    ) -> Result<String, MemoryError> {
        let timestamp = Utc::now();
        let time_bucket = format!("{}-{:02}", timestamp.year(), timestamp.month());

        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::Episodic,
            time_bucket,
        );

        let mut metadata = MemoryMetadata::new(
            self.base_store.agent_id().to_string(),
            "interaction".to_string(),
        );
        metadata.tags = vec!["interaction".to_string(), interaction_type.to_string()];

        let content = json!({
            "interaction_type": interaction_type,
            "participants": participants,
            "summary": summary,
            "details": details,
            "sentiment": sentiment,
            "timestamp": timestamp
        });

        let memory = MemoryDocument {
            id: Uuid::new_v4().to_string(),
            namespace,
            memory_type: MemoryType::Episodic,
            content,
            metadata,
            embeddings: None,
        };

        self.base_store.store(memory).await
    }

    /// Get episodes from a time period
    pub async fn get_episodes_in_period(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        let query = MemoryQuery {
            namespace: None,
            memory_types: Some(vec![MemoryType::Episodic]),
            tags: Some(vec!["episode".to_string()]),
            time_range: Some(TimeRange {
                start: Some(start),
                end: Some(end),
            }),
            text_query: None,
            semantic_query: None,
            limit: None,
            include_expired: false,
        };

        self.base_store.query(query).await
    }
}

/// Procedural memory store for rules, instructions, and procedures
pub struct ProceduralMemoryStore {
    base_store: BaseMemoryStore,
}

impl ProceduralMemoryStore {
    pub fn new(base_store: BaseMemoryStore) -> Self {
        Self { base_store }
    }

    /// Store a rule or procedure
    pub async fn store_procedure(
        &mut self,
        category: &str,
        name: &str,
        description: &str,
        steps: Vec<serde_json::Value>,
        conditions: Option<serde_json::Value>,
        priority: u32,
    ) -> Result<String, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::Procedural,
            category.to_string(),
        );

        let mut metadata = MemoryMetadata::new(
            self.base_store.agent_id().to_string(),
            "procedure_definition".to_string(),
        );
        metadata.tags = vec![
            "procedure".to_string(),
            category.to_string(),
            name.to_string(),
        ];

        let content = json!({
            "category": category,
            "name": name,
            "description": description,
            "steps": steps,
            "conditions": conditions,
            "priority": priority,
            "timestamp": Utc::now()
        });

        let memory = MemoryDocument {
            id: format!("procedure_{category}_{name}"),
            namespace,
            memory_type: MemoryType::Procedural,
            content,
            metadata,
            embeddings: None,
        };

        self.base_store.store(memory).await
    }

    /// Store a rule
    pub async fn store_rule(
        &mut self,
        category: &str,
        rule_name: &str,
        condition: serde_json::Value,
        action: serde_json::Value,
        priority: u32,
        enabled: bool,
    ) -> Result<String, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::Procedural,
            format!("{category}/rules"),
        );

        let mut metadata = MemoryMetadata::new(
            self.base_store.agent_id().to_string(),
            "rule_definition".to_string(),
        );
        metadata.tags = vec![
            "rule".to_string(),
            category.to_string(),
            rule_name.to_string(),
        ];

        let content = json!({
            "category": category,
            "rule_name": rule_name,
            "condition": condition,
            "action": action,
            "priority": priority,
            "enabled": enabled,
            "timestamp": Utc::now()
        });

        let memory = MemoryDocument {
            id: format!("rule_{category}_{rule_name}"),
            namespace,
            memory_type: MemoryType::Procedural,
            content,
            metadata,
            embeddings: None,
        };

        self.base_store.store(memory).await
    }

    /// Get procedures by category
    pub async fn get_procedures_by_category(
        &self,
        category: &str,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::Procedural,
            category.to_string(),
        );

        let mut memories = self.base_store.get_by_namespace(&namespace).await?;

        // Filter for procedures only
        memories.retain(|m| m.metadata.tags.contains(&"procedure".to_string()));

        // Sort by priority
        memories.sort_by(|a, b| {
            let priority_a = a
                .content
                .get("priority")
                .and_then(|p| p.as_u64())
                .unwrap_or(0);
            let priority_b = b
                .content
                .get("priority")
                .and_then(|p| p.as_u64())
                .unwrap_or(0);
            priority_b.cmp(&priority_a) // Higher priority first
        });

        Ok(memories)
    }

    /// Get active rules by category
    pub async fn get_active_rules_by_category(
        &self,
        category: &str,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        let namespace = MemoryNamespace::with_sub(
            self.base_store.agent_id().to_string(),
            MemoryType::Procedural,
            format!("{category}/rules"),
        );

        let mut memories = self.base_store.get_by_namespace(&namespace).await?;

        // Filter for active rules only
        memories.retain(|m| {
            m.metadata.tags.contains(&"rule".to_string())
                && m.content
                    .get("enabled")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false)
        });

        // Sort by priority
        memories.sort_by(|a, b| {
            let priority_a = a
                .content
                .get("priority")
                .and_then(|p| p.as_u64())
                .unwrap_or(0);
            let priority_b = b
                .content
                .get("priority")
                .and_then(|p| p.as_u64())
                .unwrap_or(0);
            priority_b.cmp(&priority_a) // Higher priority first
        });

        Ok(memories)
    }

    /// Update rule status
    pub async fn update_rule_status(
        &mut self,
        category: &str,
        rule_name: &str,
        enabled: bool,
    ) -> Result<(), MemoryError> {
        let rule_id = format!("rule_{category}_{rule_name}");

        if let Some(mut memory) = self.base_store.get(&rule_id).await? {
            // Update the enabled status
            if let serde_json::Value::Object(ref mut map) = memory.content {
                map.insert("enabled".to_string(), json!(enabled));
                map.insert("last_modified".to_string(), json!(Utc::now()));
            }

            self.base_store.update(&rule_id, memory).await?;
        }

        Ok(())
    }
}

// Implement MemoryStore trait for each long-term store by delegating to base_store
macro_rules! impl_memory_store_delegate {
    ($store_type:ty) => {
        #[async_trait]
        impl MemoryStore for $store_type {
            async fn store(&mut self, memory: MemoryDocument) -> Result<String, MemoryError> {
                self.base_store.store(memory).await
            }

            async fn update(
                &mut self,
                id: &str,
                memory: MemoryDocument,
            ) -> Result<(), MemoryError> {
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
    };
}

impl_memory_store_delegate!(SemanticMemoryStore);
impl_memory_store_delegate!(EpisodicMemoryStore);
impl_memory_store_delegate!(ProceduralMemoryStore);

// Implement SearchableMemoryStore for semantic memory (as it's most suitable for search)
#[async_trait]
impl SearchableMemoryStore for SemanticMemoryStore {
    async fn semantic_search(
        &self,
        _query: SemanticQuery,
        namespace: Option<&MemoryNamespace>,
    ) -> Result<Vec<(MemoryDocument, f64)>, MemoryError> {
        // This would implement actual semantic search using embeddings
        // For now, fall back to text search
        let text_query = ""; // Would extract from semantic query
        let memories = self.text_search(text_query, namespace).await?;

        // Convert to scored results (placeholder scores)
        let scored_results = memories
            .into_iter()
            .map(|m| (m, 0.5)) // Placeholder similarity score
            .collect();

        Ok(scored_results)
    }

    async fn text_search(
        &self,
        query: &str,
        namespace: Option<&MemoryNamespace>,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        let memory_query = MemoryQuery {
            namespace: namespace.cloned(),
            memory_types: Some(vec![MemoryType::Semantic]),
            tags: None,
            time_range: None,
            text_query: Some(query.to_string()),
            semantic_query: None,
            limit: None,
            include_expired: false,
        };

        self.base_store.query(memory_query).await
    }

    async fn find_related(
        &self,
        memory_id: &str,
        limit: usize,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        // Get the source memory
        if let Some(source_memory) = self.base_store.get(memory_id).await? {
            // Find memories with related tags or content
            let query = MemoryQuery {
                namespace: None,
                memory_types: Some(vec![MemoryType::Semantic]),
                tags: Some(source_memory.metadata.tags.clone()),
                time_range: None,
                text_query: None,
                semantic_query: None,
                limit: Some(limit),
                include_expired: false,
            };

            let mut related = self.base_store.query(query).await?;

            // Remove the source memory from results
            related.retain(|m| m.id != memory_id);

            Ok(related)
        } else {
            Err(MemoryError::NotFound(format!(
                "Memory {memory_id} not found"
            )))
        }
    }
}
