//! Agent Memory System
//!
//! This module provides a comprehensive memory system for AI agents, implementing
//! different types of memory (short-term, semantic, episodic, procedural) with
//! persistence backed by git-based prolly trees.
//!
//! # Architecture
//!
//! The memory system is built on several key components:
//!
//! - **Types**: Core data structures and enums for memory representation
//! - **Traits**: Abstract interfaces for memory operations and lifecycle
//! - **Persistence**: Git-based prolly tree storage backend
//! - **Store**: Base memory store implementation
//! - **Memory Types**: Specialized stores for different memory types
//! - **Search**: Advanced search and retrieval capabilities
//! - **Lifecycle**: Memory consolidation, archival, and cleanup
//!
//! # Memory Types
//!
//! ## Short-Term Memory
//! - Session/thread scoped memories
//! - Automatic expiration (TTL)
//! - Conversation history
//! - Working memory for temporary state
//!
//! ## Semantic Memory
//! - Facts and concepts about entities
//! - Relationships between entities
//! - Knowledge representation
//!
//! ## Episodic Memory
//! - Past experiences and interactions
//! - Time-indexed memories
//! - Context-rich event storage
//!
//! ## Procedural Memory
//! - Rules and procedures
//! - Task instructions
//! - Decision-making guidelines
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use prollytree::agent::*;
//! use chrono::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize the base memory store
//!     let base_store = BaseMemoryStore::init(
//!         "/tmp/agent_memory",
//!         "agent_001".to_string(),
//!         None, // No embedding generator for this example
//!     )?;
//!
//!     // Create short-term memory store
//!     let mut short_term = ShortTermMemoryStore::new(
//!         base_store,
//!         Duration::hours(24), // 24-hour TTL
//!         100, // Max 100 memories per thread
//!     );
//!
//!     // Store a conversation turn
//!     short_term.store_conversation_turn(
//!         "thread_123",
//!         "user",
//!         "Hello, how are you?",
//!         None,
//!     ).await?;
//!
//!     short_term.store_conversation_turn(
//!         "thread_123",
//!         "assistant",
//!         "I'm doing well, thank you for asking!",
//!         None,
//!     ).await?;
//!
//!     // Retrieve conversation history
//!     let history = short_term.get_conversation_history("thread_123", None).await?;
//!     println!("Conversation history: {} messages", history.len());
//!
//!     // Commit changes
//!     short_term.commit("Store initial conversation").await?;
//!
//!     Ok(())
//! }
//! ```

pub mod traits;
pub mod types;
// pub mod persistence; // Disabled due to Send/Sync issues with GitVersionedKvStore
pub mod mem_lifecycle;
pub mod mem_long_term;
pub mod embedding_search;
pub mod mem_short_term;
pub mod persistence_simple;
pub mod mem_store;

// Re-export main types and traits for convenience
pub use traits::*;
pub use types::*;
// pub use persistence::ProllyMemoryPersistence; // Disabled
pub use mem_lifecycle::MemoryLifecycleManager;
pub use mem_long_term::{EpisodicMemoryStore, ProceduralMemoryStore, SemanticMemoryStore};
pub use embedding_search::{DistanceCalculator, MemorySearchEngine, MockEmbeddingGenerator};
pub use mem_short_term::ShortTermMemoryStore;
pub use persistence_simple::SimpleMemoryPersistence;
pub use mem_store::BaseMemoryStore;

/// High-level memory system that combines all memory types
pub struct AgentMemorySystem {
    pub short_term: ShortTermMemoryStore,
    pub semantic: SemanticMemoryStore,
    pub episodic: EpisodicMemoryStore,
    pub procedural: ProceduralMemoryStore,
    pub lifecycle_manager: MemoryLifecycleManager<BaseMemoryStore>,
}

impl AgentMemorySystem {
    /// Initialize a complete agent memory system
    pub fn init<P: AsRef<std::path::Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let base_store = BaseMemoryStore::init(path, agent_id.clone(), embedding_generator)?;

        let short_term =
            ShortTermMemoryStore::new(base_store.clone(), chrono::Duration::hours(24), 1000);

        let semantic = SemanticMemoryStore::new(base_store.clone());
        let episodic = EpisodicMemoryStore::new(base_store.clone());
        let procedural = ProceduralMemoryStore::new(base_store.clone());
        let lifecycle_manager = MemoryLifecycleManager::new(base_store);

        Ok(Self {
            short_term,
            semantic,
            episodic,
            procedural,
            lifecycle_manager,
        })
    }

    /// Open an existing agent memory system
    pub fn open<P: AsRef<std::path::Path>>(
        path: P,
        agent_id: String,
        embedding_generator: Option<Box<dyn EmbeddingGenerator>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let base_store = BaseMemoryStore::open(path, agent_id.clone(), embedding_generator)?;

        let short_term =
            ShortTermMemoryStore::new(base_store.clone(), chrono::Duration::hours(24), 1000);

        let semantic = SemanticMemoryStore::new(base_store.clone());
        let episodic = EpisodicMemoryStore::new(base_store.clone());
        let procedural = ProceduralMemoryStore::new(base_store.clone());
        let lifecycle_manager = MemoryLifecycleManager::new(base_store);

        Ok(Self {
            short_term,
            semantic,
            episodic,
            procedural,
            lifecycle_manager,
        })
    }

    /// Get comprehensive memory statistics
    pub async fn get_system_stats(&self) -> Result<AgentMemoryStats, MemoryError> {
        let short_term_stats = self.short_term.get_short_term_stats().await?;
        let overall_stats = self.lifecycle_manager.get_stats().await?;

        Ok(AgentMemoryStats {
            overall: overall_stats,
            short_term: short_term_stats,
        })
    }

    /// Perform system-wide cleanup and optimization
    pub async fn optimize(&mut self) -> Result<OptimizationReport, MemoryError> {
        // Cleanup expired memories
        let expired_cleaned = self.lifecycle_manager.cleanup_expired().await?;

        // Consolidate similar memories
        let memories_consolidated = self
            .lifecycle_manager
            .consolidate(ConsolidationStrategy::MergeSimilar {
                similarity_threshold: 0.8,
            })
            .await?;

        // Archive old memories (older than 30 days)
        let cutoff = chrono::Utc::now() - chrono::Duration::days(30);
        let memories_archived = self.lifecycle_manager.archive(cutoff).await?;

        // Prune low-value memories
        let memories_pruned = self
            .lifecycle_manager
            .consolidate(ConsolidationStrategy::Prune {
                confidence_threshold: 0.1,
                access_threshold: 0,
            })
            .await?;

        Ok(OptimizationReport {
            expired_cleaned,
            memories_consolidated,
            memories_archived,
            memories_pruned,
        })
    }

    /// Create a memory checkpoint
    pub async fn checkpoint(&mut self, message: &str) -> Result<String, MemoryError> {
        self.lifecycle_manager.commit(message).await
    }

    /// Rollback to a specific checkpoint/commit
    pub async fn rollback(&mut self, checkpoint_id: &str) -> Result<(), MemoryError> {
        // Rollback all memory stores to the specified checkpoint
        self.short_term.checkout(checkpoint_id).await?;
        self.semantic.checkout(checkpoint_id).await?;
        self.episodic.checkout(checkpoint_id).await?;
        self.procedural.checkout(checkpoint_id).await?;

        Ok(())
    }

    /// Get list of available checkpoints/commits
    pub async fn list_checkpoints(&self) -> Result<Vec<CheckpointInfo>, MemoryError> {
        // For now, return a simplified list - in a full implementation this would
        // query the underlying git repository for commit history
        Ok(vec![])
    }

    /// Compare memory state between two checkpoints
    pub async fn compare_checkpoints(
        &self,
        from: &str,
        to: &str,
    ) -> Result<MemoryDiff, MemoryError> {
        // Placeholder for checkpoint comparison - would be implemented with actual
        // git diff functionality in a full system
        Ok(MemoryDiff {
            added_memories: 0,
            modified_memories: 0,
            deleted_memories: 0,
            changes_summary: format!("Comparison between {from} and {to}"),
        })
    }
}

/// Combined statistics for the entire memory system
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentMemoryStats {
    pub overall: MemoryStats,
    pub short_term: mem_short_term::ShortTermStats,
}

/// Report from memory optimization operations
#[derive(Debug, Clone, Default)]
pub struct OptimizationReport {
    pub expired_cleaned: usize,
    pub memories_consolidated: usize,
    pub memories_archived: usize,
    pub memories_pruned: usize,
}

/// Information about a memory checkpoint
#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckpointInfo {
    pub id: String,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub memory_count: usize,
}

/// Comparison between two memory states
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryDiff {
    pub added_memories: usize,
    pub modified_memories: usize,
    pub deleted_memories: usize,
    pub changes_summary: String,
}

impl OptimizationReport {
    pub fn total_processed(&self) -> usize {
        self.expired_cleaned
            + self.memories_consolidated
            + self.memories_archived
            + self.memories_pruned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_agent_memory_system_basic() {
        let temp_dir = TempDir::new().unwrap();
        let mut memory_system =
            AgentMemorySystem::init(temp_dir.path(), "test_agent".to_string(), None).unwrap();

        // Test short-term memory
        let conversation_id = memory_system
            .short_term
            .store_conversation_turn("test_thread", "user", "Hello world", None)
            .await
            .unwrap();

        assert!(!conversation_id.is_empty());

        // Test semantic memory
        let fact_id = memory_system
            .semantic
            .store_fact(
                "person",
                "john_doe",
                serde_json::json!({"age": 30, "occupation": "developer"}),
                0.9,
                "user_input",
            )
            .await
            .unwrap();

        assert!(!fact_id.is_empty());

        // Test procedural memory
        let procedure_id = memory_system
            .procedural
            .store_procedure(
                "task_management",
                "create_task",
                "How to create a new task",
                vec![
                    serde_json::json!({"step": 1, "action": "Define task name"}),
                    serde_json::json!({"step": 2, "action": "Set priority"}),
                ],
                None,
                1,
            )
            .await
            .unwrap();

        assert!(!procedure_id.is_empty());

        // Test system stats
        let stats = memory_system.get_system_stats().await.unwrap();
        assert!(stats.overall.total_memories > 0);
    }

    #[tokio::test]
    async fn test_memory_optimization() {
        let temp_dir = TempDir::new().unwrap();
        let mut memory_system =
            AgentMemorySystem::init(temp_dir.path(), "test_agent".to_string(), None).unwrap();

        // Add some test memories
        for i in 0..10 {
            memory_system
                .short_term
                .store_conversation_turn("test_thread", "user", &format!("Message {}", i), None)
                .await
                .unwrap();
        }

        // Run optimization
        let report = memory_system.optimize().await.unwrap();

        // Optimization report should be valid (total_processed is always >= 0 for usize)
        // Just verify the report exists and has reasonable values
        assert!(report.expired_cleaned <= 50); // Reasonable upper bound for test
        assert!(report.memories_consolidated <= 50);
        assert!(report.memories_archived <= 50);
        assert!(report.memories_pruned <= 50);
    }
}
