#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
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

/// Memory commit information following rig_versioned_memory pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCommit {
    pub hash: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub memory_type: MemoryType,
}

/// Detailed commit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCommitDetails {
    pub hash: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub author: String,
    pub changed_files: Vec<String>,
    pub memory_impact: String,
}

/// Memory snapshot at a specific point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub commit_hash: String,
    pub timestamp: DateTime<Utc>,
    pub recommendations: Vec<ValidatedMemory>,
    pub market_data: Vec<ValidatedMemory>,
    pub audit_entries: Vec<AuditEntry>,
    pub total_memories: usize,
}

/// Comparison between two memory states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryComparison {
    pub from_commit: String,
    pub to_commit: String,
    pub from_time: DateTime<Utc>,
    pub to_time: DateTime<Utc>,
    pub recommendation_changes: i64,
    pub market_data_changes: i64,
    pub total_memory_change: i64,
    pub summary: String,
}

/// Real-time memory system status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatus {
    pub validation_active: bool,
    pub audit_enabled: bool,
    pub security_monitoring: bool,
    pub current_branch: String,
    pub current_commit: String,
    pub total_branches: usize,
    pub total_commits: usize,
    pub recommendation_count: usize,
    pub market_data_count: usize,
    pub audit_count: usize,
    pub storage_healthy: bool,
    pub git_healthy: bool,
}

/// Validation source status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSource {
    pub name: String,
    pub trust_level: f64,
    pub status: SourceStatus,
    pub last_checked: Option<DateTime<Utc>>,
    pub response_time_ms: Option<u32>,
}

/// Source health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceStatus {
    Active,
    Inactive,
    Error,
    Unknown,
}
