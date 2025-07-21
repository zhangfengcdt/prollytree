use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MemoryType {
    MarketData,
    Recommendation,
    ClientProfile,
    Audit,
    System,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::MarketData => write!(f, "MarketData"),
            MemoryType::Recommendation => write!(f, "Recommendation"),
            MemoryType::ClientProfile => write!(f, "ClientProfile"),
            MemoryType::Audit => write!(f, "Audit"),
            MemoryType::System => write!(f, "System"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedMemory {
    pub id: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub validation_hash: [u8; 32],
    pub sources: Vec<String>,
    pub confidence: f64,
    pub cross_references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub action: String,
    pub memory_type: String,
    pub memory_id: String,
    pub branch: String,
    pub timestamp: DateTime<Utc>,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBranch {
    pub id: String,
    pub name: String,
    pub parent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryVersion {
    pub hash: String,
    pub timestamp: DateTime<Utc>,
    pub message: String,
    pub author: String,
}
