use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::collections::HashMap;
use tokio::sync::broadcast;

use super::embedding_search::DistanceCalculator;
use super::traits::{MemoryError, MemoryLifecycle, MemoryStore};
use super::types::*;

/// Memory lifecycle manager for consolidation, archival, and cleanup
pub struct MemoryLifecycleManager<T: MemoryStore> {
    store: T,
    event_sender: Option<broadcast::Sender<MemoryEvent>>,
    consolidation_rules: Vec<ConsolidationRule>,
}

impl<T: MemoryStore> MemoryLifecycleManager<T> {
    pub fn new(store: T) -> Self {
        Self {
            store,
            event_sender: None,
            consolidation_rules: Vec::new(),
        }
    }

    /// Enable event broadcasting
    pub fn enable_events(&mut self) -> broadcast::Receiver<MemoryEvent> {
        let (sender, receiver) = broadcast::channel(1000);
        self.event_sender = Some(sender);
        receiver
    }

    /// Add a consolidation rule
    pub fn add_consolidation_rule(&mut self, rule: ConsolidationRule) {
        self.consolidation_rules.push(rule);
    }

    /// Emit a memory event
    fn emit_event(&self, event: MemoryEvent) {
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(event); // Ignore send errors
        }
    }

    /// Merge similar memories based on content similarity
    async fn merge_similar_memories(
        &mut self,
        memories: Vec<MemoryDocument>,
        similarity_threshold: f64,
    ) -> Result<usize, MemoryError> {
        let mut merged_count = 0;
        let mut to_delete = Vec::new();
        let mut clusters: Vec<Vec<MemoryDocument>> = Vec::new();

        // Simple clustering based on embedding similarity
        for memory in memories {
            let mut target_cluster_idx = None;

            for (idx, cluster) in clusters.iter().enumerate() {
                if let (Some(ref embeddings1), Some(ref embeddings2)) =
                    (&memory.embeddings, &cluster[0].embeddings)
                {
                    let similarity =
                        DistanceCalculator::cosine_similarity(embeddings1, embeddings2);

                    if similarity >= similarity_threshold {
                        target_cluster_idx = Some(idx);
                        break;
                    }
                }
            }

            if let Some(idx) = target_cluster_idx {
                clusters[idx].push(memory);
            } else {
                clusters.push(vec![memory]);
            }
        }

        // Merge clusters with multiple memories
        for cluster in clusters {
            if cluster.len() > 1 {
                // Mark originals for deletion (except the first one which becomes the merged one)
                for memory in &cluster[1..] {
                    to_delete.push(memory.id.clone());
                }

                let cluster_size = cluster.len();
                let namespace = cluster[0].namespace.clone();
                let merged_memory = self.merge_cluster(cluster).await?;

                // Store the merged memory
                let merged_id = self.store.store(merged_memory).await?;

                merged_count += cluster_size - 1;

                self.emit_event(MemoryEvent::Updated {
                    memory_id: merged_id,
                    namespace,
                    timestamp: Utc::now(),
                    changes: vec!["merged_similar_memories".to_string()],
                });
            }
        }

        // Delete original memories
        for id in to_delete {
            self.store.delete(&id).await?;
        }

        Ok(merged_count)
    }

    /// Merge a cluster of similar memories into one
    async fn merge_cluster(
        &self,
        cluster: Vec<MemoryDocument>,
    ) -> Result<MemoryDocument, MemoryError> {
        if cluster.is_empty() {
            return Err(MemoryError::InvalidNamespace("Empty cluster".to_string()));
        }

        let first = &cluster[0];
        let mut merged_content = serde_json::Map::new();

        // Combine content from all memories
        merged_content.insert(
            "merged_from".to_string(),
            serde_json::Value::Array(
                cluster
                    .iter()
                    .map(|m| serde_json::Value::String(m.id.clone()))
                    .collect(),
            ),
        );

        merged_content.insert(
            "merged_at".to_string(),
            serde_json::Value::String(Utc::now().to_rfc3339()),
        );

        // Collect all content
        let mut all_content = Vec::new();
        for memory in &cluster {
            all_content.push(memory.content.clone());
        }
        merged_content.insert(
            "contents".to_string(),
            serde_json::Value::Array(all_content),
        );

        // Merge metadata
        let mut merged_metadata = first.metadata.clone();
        merged_metadata.updated_at = Utc::now();
        merged_metadata.source = "memory_consolidation".to_string();

        // Combine tags from all memories
        let mut all_tags = std::collections::HashSet::new();
        for memory in &cluster {
            for tag in &memory.metadata.tags {
                all_tags.insert(tag.clone());
            }
        }
        merged_metadata.tags = all_tags.into_iter().collect();
        merged_metadata.tags.push("consolidated".to_string());

        // Average confidence
        let avg_confidence =
            cluster.iter().map(|m| m.metadata.confidence).sum::<f64>() / cluster.len() as f64;
        merged_metadata.confidence = avg_confidence;

        // Collect related memories
        let mut all_related = std::collections::HashSet::new();
        for memory in &cluster {
            for related in &memory.metadata.related_memories {
                all_related.insert(related.clone());
            }
        }
        merged_metadata.related_memories = all_related.into_iter().collect();

        Ok(MemoryDocument {
            id: first.id.clone(),
            namespace: first.namespace.clone(),
            memory_type: first.memory_type,
            content: serde_json::Value::Object(merged_content),
            metadata: merged_metadata,
            embeddings: first.embeddings.clone(),
        })
    }

    /// Summarize memories into higher-level concepts
    async fn summarize_memories(
        &mut self,
        memories: Vec<MemoryDocument>,
        max_memories: usize,
    ) -> Result<usize, MemoryError> {
        if memories.len() <= max_memories {
            return Ok(0);
        }

        // Group memories by namespace and type
        let mut groups: HashMap<(MemoryNamespace, Vec<String>), Vec<MemoryDocument>> =
            HashMap::new();

        for memory in memories {
            let key = (memory.namespace.clone(), memory.metadata.tags.clone());
            groups.entry(key).or_default().push(memory);
        }

        let mut summarized_count = 0;

        for ((namespace, tags), mut group_memories) in groups {
            if group_memories.len() > max_memories {
                // Sort by importance/confidence
                group_memories.sort_by(|a, b| {
                    b.metadata
                        .confidence
                        .partial_cmp(&a.metadata.confidence)
                        .unwrap()
                });

                // Keep the most important ones
                let _to_keep = group_memories.split_off(max_memories);
                let to_summarize = group_memories;

                // Create summary
                let summary = self
                    .create_summary(&to_summarize, &namespace, &tags)
                    .await?;

                // Store summary
                let summary_id = self.store.store(summary).await?;

                // Delete summarized memories
                for memory in &to_summarize {
                    self.store.delete(&memory.id).await?;
                }

                summarized_count += to_summarize.len();

                self.emit_event(MemoryEvent::Created {
                    memory_id: summary_id,
                    namespace: namespace.clone(),
                    timestamp: Utc::now(),
                });
            }
        }

        Ok(summarized_count)
    }

    /// Create a summary memory from a group of memories
    async fn create_summary(
        &self,
        memories: &[MemoryDocument],
        namespace: &MemoryNamespace,
        tags: &[String],
    ) -> Result<MemoryDocument, MemoryError> {
        let mut summary_content = serde_json::Map::new();

        summary_content.insert(
            "summary_type".to_string(),
            serde_json::Value::String("automatic_consolidation".to_string()),
        );

        summary_content.insert(
            "summarized_count".to_string(),
            serde_json::Value::Number(memories.len().into()),
        );

        summary_content.insert(
            "time_range".to_string(),
            serde_json::json!({
                "start": memories.iter().map(|m| m.metadata.created_at).min(),
                "end": memories.iter().map(|m| m.metadata.created_at).max()
            }),
        );

        // Extract key themes/patterns
        let mut all_text = String::new();
        for memory in memories {
            all_text.push_str(&memory.content.to_string());
            all_text.push(' ');
        }

        summary_content.insert(
            "content_summary".to_string(),
            serde_json::Value::String(self.extract_summary(&all_text)),
        );

        summary_content.insert(
            "original_ids".to_string(),
            serde_json::Value::Array(
                memories
                    .iter()
                    .map(|m| serde_json::Value::String(m.id.clone()))
                    .collect(),
            ),
        );

        let mut summary_metadata = MemoryMetadata::new(
            namespace.agent_id.clone(),
            "memory_summarization".to_string(),
        );

        summary_metadata.tags = tags.to_vec();
        summary_metadata.tags.push("summary".to_string());
        summary_metadata.confidence =
            memories.iter().map(|m| m.metadata.confidence).sum::<f64>() / memories.len() as f64;

        Ok(MemoryDocument {
            id: format!("summary_{}", uuid::Uuid::new_v4()),
            namespace: namespace.clone(),
            memory_type: memories[0].memory_type,
            content: serde_json::Value::Object(summary_content),
            metadata: summary_metadata,
            embeddings: None,
        })
    }

    /// Extract a simple summary from text (placeholder implementation)
    fn extract_summary(&self, text: &str) -> String {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() <= 50 {
            text.to_string()
        } else {
            format!(
                "Summary of {} words: {}...",
                words.len(),
                words[..50].join(" ")
            )
        }
    }

    /// Archive old memories to a different namespace
    async fn archive_old_memories(
        &mut self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<usize, MemoryError> {
        let query = MemoryQuery {
            namespace: None,
            memory_types: None,
            tags: None,
            time_range: Some(TimeRange {
                start: None,
                end: Some(before),
            }),
            text_query: None,
            semantic_query: None,
            limit: None,
            include_expired: false,
        };

        let old_memories = self.store.query(query).await?;
        let mut archived_count = 0;

        for mut memory in old_memories {
            // Move to archive namespace
            memory.namespace = MemoryNamespace::with_sub(
                memory.namespace.agent_id.clone(),
                memory.namespace.memory_type,
                format!(
                    "archive/{}",
                    memory.namespace.sub_namespace.unwrap_or_default()
                ),
            );

            memory.metadata.tags.push("archived".to_string());
            memory.metadata.updated_at = Utc::now();

            // Update the memory
            self.store.update(&memory.id, memory.clone()).await?;
            archived_count += 1;

            self.emit_event(MemoryEvent::Updated {
                memory_id: memory.id,
                namespace: memory.namespace,
                timestamp: Utc::now(),
                changes: vec!["archived".to_string()],
            });
        }

        Ok(archived_count)
    }

    /// Prune low-value memories
    async fn prune_memories(
        &mut self,
        confidence_threshold: f64,
        access_threshold: u32,
    ) -> Result<usize, MemoryError> {
        let query = MemoryQuery {
            namespace: None,
            memory_types: None,
            tags: None,
            time_range: None,
            text_query: None,
            semantic_query: None,
            limit: None,
            include_expired: false,
        };

        let all_memories = self.store.query(query).await?;
        let mut pruned_count = 0;

        for memory in all_memories {
            // Check if memory should be pruned
            if memory.metadata.confidence < confidence_threshold
                || memory.metadata.access_count < access_threshold
            {
                self.store.delete(&memory.id).await?;
                pruned_count += 1;

                self.emit_event(MemoryEvent::Deleted {
                    memory_id: memory.id,
                    namespace: memory.namespace,
                    timestamp: Utc::now(),
                    reason: "low_value_pruning".to_string(),
                });
            }
        }

        Ok(pruned_count)
    }
}

#[async_trait]
impl<T: MemoryStore> MemoryLifecycle for MemoryLifecycleManager<T> {
    async fn consolidate(&mut self, strategy: ConsolidationStrategy) -> Result<usize, MemoryError> {
        match strategy {
            ConsolidationStrategy::MergeSimilar {
                similarity_threshold,
            } => {
                // Get all memories for merging
                let query = MemoryQuery {
                    namespace: None,
                    memory_types: None,
                    tags: None,
                    time_range: None,
                    text_query: None,
                    semantic_query: None,
                    limit: None,
                    include_expired: false,
                };

                let memories = self.store.query(query).await?;
                self.merge_similar_memories(memories, similarity_threshold)
                    .await
            }
            ConsolidationStrategy::Summarize { max_memories } => {
                let query = MemoryQuery {
                    namespace: None,
                    memory_types: None,
                    tags: None,
                    time_range: None,
                    text_query: None,
                    semantic_query: None,
                    limit: None,
                    include_expired: false,
                };

                let memories = self.store.query(query).await?;
                self.summarize_memories(memories, max_memories).await
            }
            ConsolidationStrategy::Archive { age_threshold } => {
                let cutoff = Utc::now() - age_threshold;
                self.archive_old_memories(cutoff).await
            }
            ConsolidationStrategy::Prune {
                confidence_threshold,
                access_threshold,
            } => {
                self.prune_memories(confidence_threshold, access_threshold)
                    .await
            }
        }
    }

    async fn archive(
        &mut self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<usize, MemoryError> {
        self.archive_old_memories(before).await
    }

    async fn subscribe_events<F>(&mut self, callback: F) -> Result<(), MemoryError>
    where
        F: Fn(MemoryEvent) + Send + Sync + 'static,
    {
        let mut receiver = self.enable_events();

        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                callback(event);
            }
        });

        Ok(())
    }

    async fn get_history(&self, memory_id: &str) -> Result<Vec<MemoryDocument>, MemoryError> {
        // This would require implementing versioning at the storage level
        // For now, return just the current version
        if let Some(memory) = self.store.get(memory_id).await? {
            Ok(vec![memory])
        } else {
            Ok(vec![])
        }
    }
}

// Delegate MemoryStore methods to the wrapped store
#[async_trait]
impl<T: MemoryStore> MemoryStore for MemoryLifecycleManager<T> {
    async fn store(&mut self, memory: MemoryDocument) -> Result<String, MemoryError> {
        let id = self.store.store(memory.clone()).await?;

        self.emit_event(MemoryEvent::Created {
            memory_id: id.clone(),
            namespace: memory.namespace,
            timestamp: Utc::now(),
        });

        Ok(id)
    }

    async fn update(&mut self, id: &str, memory: MemoryDocument) -> Result<(), MemoryError> {
        self.store.update(id, memory.clone()).await?;

        self.emit_event(MemoryEvent::Updated {
            memory_id: id.to_string(),
            namespace: memory.namespace,
            timestamp: Utc::now(),
            changes: vec!["manual_update".to_string()],
        });

        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryDocument>, MemoryError> {
        let result = self.store.get(id).await?;

        if let Some(ref memory) = result {
            // Update access tracking would go here
            self.emit_event(MemoryEvent::Accessed {
                memory_id: id.to_string(),
                namespace: memory.namespace.clone(),
                timestamp: Utc::now(),
                access_count: memory.metadata.access_count + 1,
            });
        }

        Ok(result)
    }

    async fn delete(&mut self, id: &str) -> Result<(), MemoryError> {
        // Get memory info before deletion for event
        let memory_info = self.store.get(id).await?;

        self.store.delete(id).await?;

        if let Some(memory) = memory_info {
            self.emit_event(MemoryEvent::Deleted {
                memory_id: id.to_string(),
                namespace: memory.namespace,
                timestamp: Utc::now(),
                reason: "manual_deletion".to_string(),
            });
        }

        Ok(())
    }

    async fn query(&self, query: MemoryQuery) -> Result<Vec<MemoryDocument>, MemoryError> {
        self.store.query(query).await
    }

    async fn get_by_namespace(
        &self,
        namespace: &MemoryNamespace,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        self.store.get_by_namespace(namespace).await
    }

    async fn commit(&mut self, message: &str) -> Result<String, MemoryError> {
        self.store.commit(message).await
    }

    async fn create_branch(&mut self, name: &str) -> Result<(), MemoryError> {
        self.store.create_branch(name).await
    }

    async fn checkout(&mut self, branch_or_commit: &str) -> Result<(), MemoryError> {
        self.store.checkout(branch_or_commit).await
    }

    fn current_branch(&self) -> &str {
        self.store.current_branch()
    }

    async fn get_stats(&self) -> Result<MemoryStats, MemoryError> {
        self.store.get_stats().await
    }

    async fn cleanup_expired(&mut self) -> Result<usize, MemoryError> {
        let count = self.store.cleanup_expired().await?;

        // Note: Individual expiry events would be emitted during cleanup
        // This is a summary event for the cleanup operation

        Ok(count)
    }
}

/// Configuration for consolidation rules
#[derive(Debug, Clone)]
pub struct ConsolidationRule {
    pub name: String,
    pub trigger: ConsolidationTrigger,
    pub strategy: ConsolidationStrategy,
    pub enabled: bool,
}

/// Triggers for automatic consolidation
#[derive(Debug, Clone)]
pub enum ConsolidationTrigger {
    MemoryCount(usize),
    TimeInterval(Duration),
    StorageSize(usize),
    Custom(fn(&MemoryStats) -> bool),
}
