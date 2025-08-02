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

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Different types of memory in the agent system
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryType {
    /// Short-term memory - thread/session scoped
    ShortTerm,
    /// Long-term semantic memory - facts and concepts
    Semantic,
    /// Long-term episodic memory - past experiences
    Episodic,
    /// Long-term procedural memory - rules and instructions
    Procedural,
}

impl fmt::Display for MemoryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryType::ShortTerm => write!(f, "ShortTerm"),
            MemoryType::Semantic => write!(f, "Semantic"),
            MemoryType::Episodic => write!(f, "Episodic"),
            MemoryType::Procedural => write!(f, "Procedural"),
        }
    }
}

/// Memory namespace for organizing memories hierarchically
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MemoryNamespace {
    /// Agent identifier
    pub agent_id: String,
    /// Memory type
    pub memory_type: MemoryType,
    /// Optional sub-namespace (e.g., thread_id for short-term, entity_type for semantic)
    pub sub_namespace: Option<String>,
}

impl MemoryNamespace {
    /// Create a new namespace
    pub fn new(agent_id: String, memory_type: MemoryType) -> Self {
        Self {
            agent_id,
            memory_type,
            sub_namespace: None,
        }
    }

    /// Create namespace with sub-namespace
    pub fn with_sub(agent_id: String, memory_type: MemoryType, sub: String) -> Self {
        Self {
            agent_id,
            memory_type,
            sub_namespace: Some(sub),
        }
    }

    /// Convert to path representation for storage
    pub fn to_path(&self) -> String {
        let base = format!("/memory/agents/{}/{}", self.agent_id, self.memory_type);
        match &self.sub_namespace {
            Some(sub) => format!("{base}/{sub}"),
            None => base,
        }
    }
}

/// Memory document structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDocument {
    /// Unique identifier
    pub id: String,
    /// Namespace this memory belongs to
    pub namespace: MemoryNamespace,
    /// Memory type
    pub memory_type: MemoryType,
    /// The actual content/data
    pub content: serde_json::Value,
    /// Metadata about the memory
    pub metadata: MemoryMetadata,
    /// Optional embeddings for semantic search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embeddings: Option<Vec<f32>>,
}

/// Metadata associated with a memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
    /// Agent that created this memory
    pub agent_id: String,
    /// Thread/session ID for short-term memories
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Time-to-live for automatic expiration (mainly for short-term)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<Duration>,
    /// Access pattern tracking
    pub access_count: u32,
    pub last_accessed: Option<DateTime<Utc>>,
    /// Source of the memory (user input, inference, external API, etc.)
    pub source: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Related memory IDs for cross-referencing
    #[serde(default)]
    pub related_memories: Vec<String>,
}

impl MemoryMetadata {
    /// Create new metadata with defaults
    pub fn new(agent_id: String, source: String) -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            updated_at: now,
            agent_id,
            thread_id: None,
            tags: Vec::new(),
            ttl: None,
            access_count: 0,
            last_accessed: None,
            source,
            confidence: 1.0,
            related_memories: Vec::new(),
        }
    }

    /// Mark memory as accessed
    pub fn mark_accessed(&mut self) {
        self.access_count += 1;
        self.last_accessed = Some(Utc::now());
    }

    /// Check if memory has expired
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            Utc::now() > self.created_at + ttl
        } else {
            false
        }
    }
}

/// Query parameters for memory retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuery {
    /// Namespace to search in
    pub namespace: Option<MemoryNamespace>,
    /// Memory types to include
    pub memory_types: Option<Vec<MemoryType>>,
    /// Tags to filter by (AND operation)
    pub tags: Option<Vec<String>>,
    /// Time range filter
    pub time_range: Option<TimeRange>,
    /// Full-text search query
    pub text_query: Option<String>,
    /// Semantic search with embeddings
    pub semantic_query: Option<SemanticQuery>,
    /// Maximum number of results
    pub limit: Option<usize>,
    /// Include expired memories
    pub include_expired: bool,
}

/// Time range for filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

/// Semantic search parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticQuery {
    /// Query embeddings
    pub embeddings: Vec<f32>,
    /// Similarity threshold (0.0 to 1.0)
    pub threshold: f64,
    /// Distance metric to use
    pub metric: DistanceMetric,
}

/// Distance metrics for semantic search
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

/// Memory operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryResult<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl<T> MemoryResult<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            metadata: HashMap::new(),
        }
    }

    pub fn failure(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            metadata: HashMap::new(),
        }
    }
}

/// Memory lifecycle events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryEvent {
    Created {
        memory_id: String,
        namespace: MemoryNamespace,
        timestamp: DateTime<Utc>,
    },
    Updated {
        memory_id: String,
        namespace: MemoryNamespace,
        timestamp: DateTime<Utc>,
        changes: Vec<String>,
    },
    Accessed {
        memory_id: String,
        namespace: MemoryNamespace,
        timestamp: DateTime<Utc>,
        access_count: u32,
    },
    Expired {
        memory_id: String,
        namespace: MemoryNamespace,
        timestamp: DateTime<Utc>,
        ttl: Duration,
    },
    Deleted {
        memory_id: String,
        namespace: MemoryNamespace,
        timestamp: DateTime<Utc>,
        reason: String,
    },
}

/// Memory consolidation strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsolidationStrategy {
    /// Merge similar memories into a single entry
    MergeSimilar { similarity_threshold: f64 },
    /// Summarize multiple memories into abstract concepts
    Summarize { max_memories: usize },
    /// Archive old memories to cold storage
    Archive { age_threshold: Duration },
    /// Remove low-value memories
    Prune {
        confidence_threshold: f64,
        access_threshold: u32,
    },
}

/// Memory statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_memories: usize,
    pub by_type: HashMap<MemoryType, usize>,
    pub by_namespace: HashMap<String, usize>,
    pub total_size_bytes: usize,
    pub avg_access_count: f64,
    pub oldest_memory: Option<DateTime<Utc>>,
    pub newest_memory: Option<DateTime<Utc>>,
    pub expired_count: usize,
}
