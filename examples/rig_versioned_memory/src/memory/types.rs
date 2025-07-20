use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: serde_json::Value,
    pub embedding: Option<Vec<f32>>,
}

impl Memory {
    pub fn new(content: String, metadata: serde_json::Value) -> Self {
        Self {
            id: format!("mem_{}", uuid::Uuid::new_v4()),
            content,
            timestamp: Utc::now(),
            metadata,
            embedding: None,
        }
    }

    pub fn with_id(id: String, content: String, metadata: serde_json::Value) -> Self {
        Self {
            id,
            content,
            timestamp: Utc::now(),
            metadata,
            embedding: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    ShortTerm,  // Current conversation context
    LongTerm,   // Learned facts, preferences
    Episodic,   // Past experiences, outcomes
}

impl MemoryType {
    pub fn table_name(&self) -> &'static str {
        match self {
            MemoryType::ShortTerm => "short_term_memory",
            MemoryType::LongTerm => "long_term_memory",
            MemoryType::Episodic => "episodic_memory",
        }
    }
}

impl fmt::Display for MemoryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryType::ShortTerm => write!(f, "short_term"),
            MemoryType::LongTerm => write!(f, "long_term"),
            MemoryType::Episodic => write!(f, "episodic"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DecisionAudit {
    pub timestamp: DateTime<Utc>,
    pub input: String,
    pub memories_accessed: Vec<String>,
    pub reasoning_chain: Vec<String>,
    pub decision: String,
    pub confidence: f64,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct MemoryContext {
    pub long_term_memories: Vec<Memory>,
    pub recent_memories: Vec<Memory>,
}

impl MemoryContext {
    pub fn new() -> Self {
        Self {
            long_term_memories: Vec::new(),
            recent_memories: Vec::new(),
        }
    }

    pub fn total_memories(&self) -> usize {
        self.long_term_memories.len() + self.recent_memories.len()
    }

    pub fn build_context_text(&self) -> String {
        let mut context = String::new();

        if !self.long_term_memories.is_empty() {
            context.push_str("Relevant facts:\n");
            for memory in &self.long_term_memories {
                context.push_str(&format!("- {}\n", memory.content));
            }
        }

        if !self.recent_memories.is_empty() {
            context.push_str("\nRecent conversation:\n");
            for memory in self.recent_memories.iter().rev().take(3) {
                let role = memory
                    .metadata
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                context.push_str(&format!("{}: {}\n", role, memory.content));
            }
        }

        context
    }
}