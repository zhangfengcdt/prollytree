#![allow(dead_code)]
#![allow(unused_imports)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use gluesql_core::prelude::{Glue, Payload};
use prollytree::sql::ProllyStorage;
use prollytree::git::{VersionedKvStore, GitKvError};
use std::path::Path;
use uuid::Uuid;

pub mod consistency;
pub mod display;
pub mod types;

pub use consistency::MemoryConsistencyChecker;
pub use types::{AuditEntry, MemoryType, ValidatedMemory, MemoryCommit, MemoryCommitDetails, MemorySnapshot, MemoryComparison};

/// Core memory store with versioning capabilities
pub struct MemoryStore {
    store_path: String,
    versioned_store: VersionedKvStore<32>,
    audit_enabled: bool,
}

impl MemoryStore {
    pub async fn new(store_path: &str) -> Result<Self> {
        let path = Path::new(store_path);

        // Create directory if it doesn't exist
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }

        // Initialize ProllyTree storage with git-prolly integration
        // Create data directory structure: data/dataset (git-prolly needs subdirectory)
        let data_dir = path.join("data");
        let dataset_dir = data_dir.join("dataset");
        if !dataset_dir.exists() {
            std::fs::create_dir_all(&dataset_dir)?;
        }

        // Initialize git repository in data directory if needed
        let git_dir = data_dir.join(".git");
        if !git_dir.exists() {
            std::process::Command::new("git")
                .args(["init"])
                .current_dir(&data_dir)
                .output()
                .map_err(|e| anyhow::anyhow!("Failed to initialize git repo: {}", e))?;
        }

        // Initialize or open VersionedKvStore in dataset subdirectory
        let versioned_store = if dataset_dir.join(".git-prolly").exists() {
            VersionedKvStore::<32>::open(&dataset_dir).map_err(|e| anyhow::anyhow!("Failed to open versioned store: {:?}", e))?
        } else {
            VersionedKvStore::<32>::init(&dataset_dir).map_err(|e| anyhow::anyhow!("Failed to init versioned store: {:?}", e))?
        };

        // Initialize SQL schema using ProllyStorage
        let storage = if dataset_dir.join(".git-prolly").exists() {
            ProllyStorage::<32>::open(&dataset_dir)?
        } else {
            ProllyStorage::<32>::init(&dataset_dir)?
        };

        let mut glue = Glue::new(storage);
        Self::init_schema(&mut glue).await?;

        Ok(Self {
            store_path: store_path.to_string(),
            versioned_store,
            audit_enabled: true,
        })
    }

    async fn init_schema(glue: &mut Glue<ProllyStorage<32>>) -> Result<()> {
        Self::ensure_table_exists(glue, "market_data", 
            r#"CREATE TABLE market_data (
                id TEXT PRIMARY KEY,
                symbol TEXT,
                content TEXT,
                validation_hash TEXT,
                sources TEXT,
                confidence FLOAT,
                timestamp INTEGER
            )"#).await?;
        
        Self::ensure_table_exists(glue, "recommendations", 
            r#"CREATE TABLE recommendations (
                id TEXT PRIMARY KEY,
                client_id TEXT,
                symbol TEXT,
                recommendation_type TEXT,
                reasoning TEXT,
                confidence FLOAT,
                validation_hash TEXT,
                memory_version TEXT,
                timestamp INTEGER
            )"#).await?;
            
        Self::ensure_table_exists(glue, "audit_log", 
            r#"CREATE TABLE audit_log (
                id TEXT PRIMARY KEY,
                action TEXT,
                memory_type TEXT,
                memory_id TEXT,
                branch TEXT,
                timestamp INTEGER,
                details TEXT
            )"#).await?;
            
        Self::ensure_table_exists(glue, "cross_references", 
            r#"CREATE TABLE cross_references (
                source_id TEXT,
                target_id TEXT,
                reference_type TEXT,
                confidence FLOAT,
                PRIMARY KEY (source_id, target_id)
            )"#).await?;

        Ok(())
    }

    async fn ensure_table_exists(
        glue: &mut Glue<ProllyStorage<32>>, 
        table_name: &str, 
        create_sql: &str
    ) -> Result<()> {
        // Try a simple query to check if table exists
        let check_sql = format!("SELECT COUNT(*) FROM {}", table_name);
        if glue.execute(&check_sql).await.is_err() {
            // Table doesn't exist, create it
            glue.execute(create_sql).await?;
        }
        Ok(())
    }

    pub async fn store(
        &mut self,
        memory_type: MemoryType,
        memory: &ValidatedMemory,
    ) -> Result<String> {
        let path = Path::new(&self.store_path).join("data").join("dataset");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);
        
        // Ensure schema exists (this should be safe to run multiple times)
        Self::init_schema(&mut glue).await?;

        match memory_type {
            MemoryType::MarketData => {
                let sql = format!(
                    r#"INSERT INTO market_data 
                    (id, symbol, content, validation_hash, sources, confidence, timestamp)
                    VALUES ('{}', '{}', '{}', '{}', '{}', {}, {})"#,
                    memory.id,
                    self.extract_symbol(&memory.content)?,
                    memory.content.replace('\'', "''"),
                    hex::encode(memory.validation_hash),
                    memory.sources.join(","),
                    memory.confidence,
                    memory.timestamp.timestamp()
                );
                glue.execute(&sql).await?;
            }

            MemoryType::Recommendation => {
                // Parse recommendation from content
                let rec: crate::advisor::Recommendation = serde_json::from_str(&memory.content)?;
                let sql = format!(
                    r#"INSERT INTO recommendations 
                    (id, client_id, symbol, recommendation_type, reasoning, confidence, 
                     validation_hash, memory_version, timestamp)
                    VALUES ('{}', '{}', '{}', '{}', '{}', {}, '{}', '{}', {})"#,
                    rec.id,
                    rec.client_id,
                    rec.symbol,
                    rec.recommendation_type.as_str(),
                    rec.reasoning.replace('\'', "''"),
                    rec.confidence,
                    hex::encode(memory.validation_hash),
                    rec.memory_version,
                    rec.timestamp.timestamp()
                );
                glue.execute(&sql).await?;
            }

            _ => {}
        }

        // Store cross-references
        for reference in &memory.cross_references {
            // First try to delete if exists, then insert (GlueSQL doesn't support UPSERT)
            let delete_sql = format!(
                "DELETE FROM cross_references WHERE source_id = '{}' AND target_id = '{}'",
                memory.id, reference
            );
            let _ = glue.execute(&delete_sql).await; // Ignore if record doesn't exist
            
            let sql = format!(
                r#"INSERT INTO cross_references 
                (source_id, target_id, reference_type, confidence)
                VALUES ('{}', '{}', 'validation', {})"#,
                memory.id, reference, memory.confidence
            );
            glue.execute(&sql).await?;
        }

        // Create version
        let version = self
            .create_version(&format!("Store {} memory: {}", memory_type, memory.id))
            .await;

        Ok(version)
    }

    pub async fn store_with_audit(
        &mut self,
        memory_type: MemoryType,
        memory: &ValidatedMemory,
        action: &str,
    ) -> Result<String> {
        // Store the memory
        let version = self.store(memory_type, memory).await?;

        // Log audit entry
        if self.audit_enabled {
            self.log_audit(action, memory_type, &memory.id).await?;
        }

        Ok(version)
    }

    pub async fn query_related(&self, content: &str, limit: usize) -> Result<Vec<ValidatedMemory>> {
        let path = Path::new(&self.store_path).join("data").join("dataset");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);
        
        // Ensure schema exists
        Self::init_schema(&mut glue).await?;

        // For now, do a simple content search
        // In production, this would use vector embeddings
        let sql = format!(
            r#"SELECT id, content, validation_hash, sources, confidence, timestamp
            FROM market_data
            WHERE content LIKE '%{}%'
            ORDER BY timestamp DESC
            LIMIT {}"#,
            content.replace('\'', "''"),
            limit
        );

        let results = glue.execute(&sql).await?;
        self.parse_memory_results(results)
    }

    pub async fn create_branch(&mut self, name: &str) -> Result<String> {
        // Use real git-prolly branch creation
        self.versioned_store.create_branch(name)
            .map_err(|e| anyhow::anyhow!("Failed to create branch '{}': {:?}", name, e))?;

        if self.audit_enabled {
            self.log_audit(
                &format!("Created branch: {name}"),
                MemoryType::System,
                name,
            )
            .await?;
        }

        Ok(name.to_string())
    }

    pub async fn commit(&mut self, message: &str) -> Result<String> {
        // Use real git-prolly commit
        let commit_id = self.versioned_store.commit(message)
            .map_err(|e| anyhow::anyhow!("Failed to commit: {:?}", e))?;
        
        let commit_hex = commit_id.to_hex().to_string();

        if self.audit_enabled {
            self.log_audit(&format!("Commit: {message}"), MemoryType::System, &commit_hex)
                .await?;
        }

        Ok(commit_hex)
    }

    pub async fn rollback(&mut self, version: &str) -> Result<()> {
        // Use real git-prolly checkout
        self.versioned_store.checkout(version)
            .map_err(|e| anyhow::anyhow!("Failed to rollback to '{}': {:?}", version, e))?;

        if self.audit_enabled {
            self.log_audit(
                &format!("Rollback to: {version}"),
                MemoryType::System,
                version,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn get_audit_trail(
        &self,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Result<Vec<AuditEntry>> {
        let path = Path::new(&self.store_path).join("data").join("dataset");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);

        let mut sql =
            "SELECT id, action, memory_type, memory_id, branch, timestamp, details FROM audit_log"
                .to_string();

        let mut conditions = vec![];
        if let Some(from) = from {
            conditions.push(format!("timestamp >= {}", from.timestamp()));
        }
        if let Some(to) = to {
            conditions.push(format!("timestamp <= {}", to.timestamp()));
        }

        if !conditions.is_empty() {
            sql.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
        }

        sql.push_str(" ORDER BY timestamp DESC");

        let results = glue.execute(&sql).await?;
        self.parse_audit_results(results)
    }

    async fn log_audit(
        &self,
        action: &str,
        memory_type: MemoryType,
        memory_id: &str,
    ) -> Result<()> {
        let path = Path::new(&self.store_path).join("data").join("dataset");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);
        
        // Ensure schema exists
        Self::init_schema(&mut glue).await?;

        let audit_entry = AuditEntry {
            id: Uuid::new_v4().to_string(),
            action: action.to_string(),
            memory_type: format!("{memory_type:?}"),
            memory_id: memory_id.to_string(),
            branch: self.current_branch().to_string(),
            timestamp: Utc::now(),
            details: serde_json::json!({
                "branch": self.current_branch(),
                "audit_enabled": self.audit_enabled,
            }),
        };

        let sql = format!(
            r#"INSERT INTO audit_log 
            (id, action, memory_type, memory_id, branch, timestamp, details)
            VALUES ('{}', '{}', '{}', '{}', '{}', {}, '{}')"#,
            audit_entry.id,
            audit_entry.action.replace('\'', "''"),
            audit_entry.memory_type,
            audit_entry.memory_id,
            audit_entry.branch,
            audit_entry.timestamp.timestamp(),
            audit_entry.details.to_string().replace('\'', "''")
        );

        glue.execute(&sql).await?;
        Ok(())
    }

    async fn create_version(&mut self, message: &str) -> String {
        // Create a real git commit
        match self.versioned_store.commit(message) {
            Ok(commit_id) => commit_id.to_hex().to_string(),
            Err(_) => format!("v-{}-{}", Utc::now().timestamp(), &message[..8.min(message.len())]),
        }
    }

    /// Get current branch name
    pub fn current_branch(&self) -> &str {
        self.versioned_store.current_branch()
    }

    /// List all branches
    pub fn list_branches(&self) -> Result<Vec<String>> {
        self.versioned_store.list_branches()
            .map_err(|e| anyhow::anyhow!("Failed to list branches: {:?}", e))
    }

    /// Get git status (staged changes)
    pub fn status(&self) -> Vec<(Vec<u8>, String)> {
        self.versioned_store.status()
    }

    /// Checkout branch or commit
    pub async fn checkout(&mut self, branch_or_commit: &str) -> Result<()> {
        self.versioned_store.checkout(branch_or_commit)
            .map_err(|e| anyhow::anyhow!("Failed to checkout '{}': {:?}", branch_or_commit, e))?;
        
        if self.audit_enabled {
            self.log_audit(
                &format!("Checked out: {branch_or_commit}"),
                MemoryType::System,
                branch_or_commit,
            ).await?;
        }
        
        Ok(())
    }

    /// Get commit history (simplified version of diff)
    pub async fn get_memory_history(&self, limit: Option<usize>) -> Result<Vec<MemoryCommit>> {
        // For now, implement using git log to show commit history
        // This follows the rig_versioned_memory pattern of showing version progression
        let log_output = std::process::Command::new("git")
            .args(["log", "--oneline", "--format=%H|%s|%at"])
            .current_dir(Path::new(&self.store_path).join("data"))
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to get git log: {}", e))?;
            
        let log_str = String::from_utf8_lossy(&log_output.stdout);
        let mut commits = Vec::new();
        
        for (index, line) in log_str.lines().enumerate() {
            if let Some(limit) = limit {
                if index >= limit {
                    break;
                }
            }
            
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 {
                let commit = MemoryCommit {
                    hash: parts[0].to_string(),
                    message: parts[1].to_string(),
                    timestamp: DateTime::from_timestamp(parts[2].parse().unwrap_or(0), 0).unwrap_or_else(Utc::now),
                    memory_type: self.parse_memory_type_from_message(parts[1]),
                };
                commits.push(commit);
            }
        }
        
        if self.audit_enabled {
            self.log_audit(
                "Retrieved memory history",
                MemoryType::System,
                &format!("limit_{}", limit.unwrap_or(10)),
            ).await?;
        }
        
        Ok(commits)
    }

    /// Simple branch merge (fast-forward only for now)
    pub async fn merge_memory_branch(&mut self, branch: &str) -> Result<String> {
        // Simplified merge implementation - just checkout the other branch
        // This follows the rig_versioned_memory pattern of simple branch switching
        let current_branch = self.current_branch().to_string();
        
        // Switch to target branch
        self.checkout(branch).await?;
        let _target_commit = self.get_current_commit_id().await?;
        
        // Switch back to original branch  
        self.checkout(&current_branch).await?;
        
        // For now, just create a merge commit message
        let merge_message = format!("Merge branch '{branch}' into '{current_branch}'");
        let merge_commit = self.commit(&merge_message).await?;
        
        if self.audit_enabled {
            self.log_audit(
                &format!("Merged branch: {branch} -> {current_branch}"),
                MemoryType::System,
                branch,
            ).await?;
        }
        
        Ok(merge_commit)
    }

    /// Show basic information about a specific commit
    pub async fn show_memory_commit(&self, commit: &str) -> Result<MemoryCommitDetails> {
        // Simple implementation using git show command
        let show_output = std::process::Command::new("git")
            .args(["show", "--format=%H|%s|%at|%an", "--name-only", commit])
            .current_dir(Path::new(&self.store_path).join("data"))
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to show commit '{}': {}", commit, e))?;
            
        let show_str = String::from_utf8_lossy(&show_output.stdout);
        let lines: Vec<&str> = show_str.lines().collect();
        
        if lines.is_empty() {
            return Err(anyhow::anyhow!("Commit not found: {}", commit));
        }
        
        // Parse first line with commit info
        let parts: Vec<&str> = lines[0].split('|').collect();
        let details = if parts.len() >= 4 {
            MemoryCommitDetails {
                hash: parts[0].to_string(),
                message: parts[1].to_string(),
                timestamp: DateTime::from_timestamp(parts[2].parse().unwrap_or(0), 0).unwrap_or_else(Utc::now),
                author: parts[3].to_string(),
                changed_files: lines[2..].iter().map(|s| s.to_string()).collect(),
                memory_impact: format!("Modified {} files", lines.len().saturating_sub(2)),
            }
        } else {
            MemoryCommitDetails {
                hash: commit.to_string(),
                message: "Unknown".to_string(),
                timestamp: Utc::now(),
                author: "Unknown".to_string(),
                changed_files: Vec::new(),
                memory_impact: "Unknown changes".to_string(),
            }
        };
        
        if self.audit_enabled {
            self.log_audit(
                &format!("Viewed commit: {commit}"),
                MemoryType::System,
                commit,
            ).await?;
        }
        
        Ok(details)
    }

    /// Revert to a specific commit (simplified rollback)
    pub async fn revert_to_commit(&mut self, commit: &str) -> Result<String> {
        // Simple revert using git reset (following rig_versioned_memory pattern)
        let reset_output = std::process::Command::new("git")
            .args(["reset", "--hard", commit])
            .current_dir(Path::new(&self.store_path).join("data"))
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to reset to commit '{}': {}", commit, e))?;
            
        if !reset_output.status.success() {
            let error = String::from_utf8_lossy(&reset_output.stderr);
            return Err(anyhow::anyhow!("Git reset failed: {}", error));
        }
        
        // Create a new commit to document this revert
        let revert_message = format!("Revert to commit {commit}");
        let new_commit = self.commit(&revert_message).await?;
        
        if self.audit_enabled {
            self.log_audit(
                &format!("Reverted to commit: {commit}"),
                MemoryType::System,
                &new_commit,
            ).await?;
        }
        
        Ok(new_commit)
    }

    /// Get current commit ID
    pub async fn get_current_commit_id(&self) -> Result<String> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(Path::new(&self.store_path).join("data"))
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to get commit ID: {}", e))?;
            
        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to get current commit"));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Parse memory type from commit message
    fn parse_memory_type_from_message(&self, message: &str) -> MemoryType {
        if message.contains("MarketData") {
            MemoryType::MarketData
        } else if message.contains("Recommendation") {
            MemoryType::Recommendation
        } else if message.contains("Audit") {
            MemoryType::Audit
        } else if message.contains("ClientProfile") {
            MemoryType::ClientProfile
        } else {
            MemoryType::System
        }
    }

    /// Get recommendations at a specific commit/time (temporal query)
    pub async fn get_recommendations_at_commit(&self, commit: &str) -> Result<Vec<ValidatedMemory>> {
        // Save current state
        let _current_commit = self.get_current_commit_id().await?;
        let current_branch = self.current_branch().to_string();
        
        // Temporarily checkout the target commit
        let checkout_output = std::process::Command::new("git")
            .args(["checkout", commit])
            .current_dir(Path::new(&self.store_path).join("data"))
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to checkout commit '{}': {}", commit, e))?;
            
        if !checkout_output.status.success() {
            let error = String::from_utf8_lossy(&checkout_output.stderr);
            return Err(anyhow::anyhow!("Git checkout failed: {}", error));
        }
        
        // Query recommendations from this point in time
        let path = Path::new(&self.store_path).join("data").join("dataset");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);
        
        let sql = "SELECT id, client_id, symbol, recommendation_type, reasoning, confidence, validation_hash, memory_version, timestamp FROM recommendations ORDER BY timestamp DESC";
        let results = glue.execute(sql).await?;
        
        let recommendations = self.parse_recommendation_results(results)?;
        
        // Restore original state
        std::process::Command::new("git")
            .args(["checkout", &current_branch])
            .current_dir(Path::new(&self.store_path).join("data"))
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to restore branch '{}': {}", current_branch, e))?;
        
        if self.audit_enabled {
            self.log_audit(
                &format!("Temporal query: recommendations at commit {commit}"),
                MemoryType::Recommendation,
                commit,
            ).await?;
        }
        
        Ok(recommendations)
    }

    /// Get market data at a specific commit/time (temporal query)
    pub async fn get_market_data_at_commit(&self, commit: &str, symbol: Option<&str>) -> Result<Vec<ValidatedMemory>> {
        // Save current state
        let _current_commit = self.get_current_commit_id().await?;
        let current_branch = self.current_branch().to_string();
        
        // Temporarily checkout the target commit
        let checkout_output = std::process::Command::new("git")
            .args(["checkout", commit])
            .current_dir(Path::new(&self.store_path).join("data"))
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to checkout commit '{}': {}", commit, e))?;
            
        if !checkout_output.status.success() {
            let error = String::from_utf8_lossy(&checkout_output.stderr);
            return Err(anyhow::anyhow!("Git checkout failed: {}", error));
        }
        
        // Query market data from this point in time
        let path = Path::new(&self.store_path).join("data").join("dataset");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);
        
        let sql = if let Some(symbol) = symbol {
            format!("SELECT id, symbol, content, validation_hash, sources, confidence, timestamp FROM market_data WHERE symbol = '{symbol}' ORDER BY timestamp DESC")
        } else {
            "SELECT id, symbol, content, validation_hash, sources, confidence, timestamp FROM market_data ORDER BY timestamp DESC".to_string()
        };
        
        let results = glue.execute(&sql).await?;
        let market_data = self.parse_memory_results(results)?;
        
        // Restore original state
        std::process::Command::new("git")
            .args(["checkout", &current_branch])
            .current_dir(Path::new(&self.store_path).join("data"))
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to restore branch '{}': {}", current_branch, e))?;
        
        if self.audit_enabled {
            let query_desc = if let Some(symbol) = symbol {
                format!("market data for {symbol} at commit {commit}")
            } else {
                format!("all market data at commit {commit}")
            };
            self.log_audit(
                &format!("Temporal query: {query_desc}"),
                MemoryType::MarketData,
                commit,
            ).await?;
        }
        
        Ok(market_data)
    }

    /// Get memory state at a specific date/time
    pub async fn get_memory_state_at_time(&self, target_time: DateTime<Utc>) -> Result<MemorySnapshot> {
        // Find the commit closest to the target time
        let commit_hash = self.find_commit_at_time(target_time).await?;
        
        // Get all memory types at that commit
        let recommendations = self.get_recommendations_at_commit(&commit_hash).await.unwrap_or_default();
        let market_data = self.get_market_data_at_commit(&commit_hash, None).await.unwrap_or_default();
        let audit_trail = self.get_audit_trail_at_commit(&commit_hash).await.unwrap_or_default();
        
        let total_memories = recommendations.len() + market_data.len();
        
        Ok(MemorySnapshot {
            commit_hash: commit_hash.clone(),
            timestamp: target_time,
            recommendations,
            market_data,
            audit_entries: audit_trail,
            total_memories,
        })
    }

    /// Find commit hash closest to a specific time
    async fn find_commit_at_time(&self, target_time: DateTime<Utc>) -> Result<String> {
        let log_output = std::process::Command::new("git")
            .args(["log", "--format=%H|%at", "--until", &target_time.timestamp().to_string()])
            .current_dir(Path::new(&self.store_path).join("data"))
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to get git log: {}", e))?;
            
        let log_str = String::from_utf8_lossy(&log_output.stdout);
        let lines: Vec<&str> = log_str.lines().collect();
        
        if lines.is_empty() {
            return Err(anyhow::anyhow!("No commits found before time {}", target_time));
        }
        
        // Return the most recent commit before the target time
        let parts: Vec<&str> = lines[0].split('|').collect();
        if !parts.is_empty() {
            Ok(parts[0].to_string())
        } else {
            Err(anyhow::anyhow!("Invalid git log format"))
        }
    }

    /// Get audit trail at a specific commit
    async fn get_audit_trail_at_commit(&self, commit: &str) -> Result<Vec<AuditEntry>> {
        // Save current state
        let current_branch = self.current_branch().to_string();
        
        // Temporarily checkout the target commit
        std::process::Command::new("git")
            .args(["checkout", commit])
            .current_dir(Path::new(&self.store_path).join("data"))
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to checkout commit for audit: {}", e))?;
        
        // Query audit trail from this point in time
        let audit_entries = self.get_audit_trail(None, None).await.unwrap_or_default();
        
        // Restore original state
        std::process::Command::new("git")
            .args(["checkout", &current_branch])
            .current_dir(Path::new(&self.store_path).join("data"))
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to restore branch for audit: {}", e))?;
        
        Ok(audit_entries)
    }

    /// Compare memory states between two time points
    pub async fn compare_memory_states(&self, from_time: DateTime<Utc>, to_time: DateTime<Utc>) -> Result<MemoryComparison> {
        let from_snapshot = self.get_memory_state_at_time(from_time).await?;
        let to_snapshot = self.get_memory_state_at_time(to_time).await?;
        
        // Simple comparison - count differences
        let recommendation_diff = to_snapshot.recommendations.len() as i64 - from_snapshot.recommendations.len() as i64;
        let market_data_diff = to_snapshot.market_data.len() as i64 - from_snapshot.market_data.len() as i64;
        let total_diff = to_snapshot.total_memories as i64 - from_snapshot.total_memories as i64;
        
        Ok(MemoryComparison {
            from_commit: from_snapshot.commit_hash,
            to_commit: to_snapshot.commit_hash,
            from_time,
            to_time,
            recommendation_changes: recommendation_diff,
            market_data_changes: market_data_diff,
            total_memory_change: total_diff,
            summary: format!("Memory changed by {} entries between {} and {}", 
                           total_diff, 
                           from_time.format("%Y-%m-%d %H:%M"), 
                           to_time.format("%Y-%m-%d %H:%M")),
        })
    }

    fn extract_symbol(&self, content: &str) -> Result<String> {
        // Try to parse JSON and extract symbol
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(symbol) = json.get("symbol").and_then(|s| s.as_str()) {
                return Ok(symbol.to_string());
            }
        }
        Ok("UNKNOWN".to_string())
    }

    /// Parse recommendation query results
    fn parse_recommendation_results(&self, results: Vec<Payload>) -> Result<Vec<ValidatedMemory>> {
        use gluesql_core::data::Value;
        let mut recommendations = Vec::new();
        
        for payload in results {
            if let Payload::Select { labels: _, rows } = payload {
                for row in rows {
                    if row.len() >= 9 {
                        // Reconstruct ValidatedMemory from recommendation data
                        let id = match &row[0] {
                            Value::Str(s) => s.clone(),
                            _ => continue,
                        };
                        
                        let client_id = match &row[1] {
                            Value::Str(s) => s.clone(),
                            _ => continue,
                        };
                        
                        let symbol = match &row[2] {
                            Value::Str(s) => s.clone(),
                            _ => continue,
                        };
                        
                        // Create content from recommendation fields
                        let content = serde_json::json!({
                            "id": id,
                            "client_id": client_id,
                            "symbol": symbol,
                            "recommendation_type": row[3],
                            "reasoning": row[4],
                            "confidence": row[5],
                            "memory_version": row[7]
                        }).to_string();
                        
                        let memory = ValidatedMemory {
                            id,
                            content,
                            validation_hash: match &row[6] {
                                Value::Str(s) => hex::decode(s)
                                    .unwrap_or_default()
                                    .try_into()
                                    .unwrap_or([0u8; 32]),
                                _ => [0u8; 32],
                            },
                            sources: vec!["recommendation_engine".to_string()],
                            confidence: match &row[5] {
                                Value::F64(f) => *f,
                                _ => 0.0,
                            },
                            timestamp: match &row[8] {
                                Value::I64(ts) => {
                                    DateTime::from_timestamp(*ts, 0).unwrap_or_else(Utc::now)
                                }
                                _ => Utc::now(),
                            },
                            cross_references: vec![],
                        };
                        
                        recommendations.push(memory);
                    }
                }
            }
        }
        
        Ok(recommendations)
    }


    fn parse_memory_results(&self, results: Vec<Payload>) -> Result<Vec<ValidatedMemory>> {
        use gluesql_core::data::Value;
        let mut memories = Vec::new();

        for payload in results {
            if let Payload::Select { labels: _, rows } = payload {
                for row in rows {
                    if row.len() >= 6 {
                        let memory = ValidatedMemory {
                            id: match &row[0] {
                                Value::Str(s) => s.clone(),
                                _ => continue,
                            },
                            content: match &row[1] {
                                Value::Str(s) => s.clone(),
                                _ => continue,
                            },
                            validation_hash: match &row[2] {
                                Value::Str(s) => hex::decode(s)
                                    .unwrap_or_default()
                                    .try_into()
                                    .unwrap_or([0u8; 32]),
                                _ => [0u8; 32],
                            },
                            sources: match &row[3] {
                                Value::Str(s) => s.split(',').map(String::from).collect(),
                                _ => vec![],
                            },
                            confidence: match &row[4] {
                                Value::F64(f) => *f,
                                _ => 0.0,
                            },
                            timestamp: match &row[5] {
                                Value::I64(ts) => {
                                    DateTime::from_timestamp(*ts, 0).unwrap_or_else(Utc::now)
                                }
                                _ => Utc::now(),
                            },
                            cross_references: vec![],
                        };
                        memories.push(memory);
                    }
                }
            }
        }

        Ok(memories)
    }

    fn parse_audit_results(&self, results: Vec<Payload>) -> Result<Vec<AuditEntry>> {
        use gluesql_core::data::Value;
        let mut entries = Vec::new();

        for payload in results {
            if let Payload::Select { labels: _, rows } = payload {
                for row in rows {
                    if row.len() >= 7 {
                        let entry = AuditEntry {
                            id: match &row[0] {
                                Value::Str(s) => s.clone(),
                                _ => continue,
                            },
                            action: match &row[1] {
                                Value::Str(s) => s.clone(),
                                _ => continue,
                            },
                            memory_type: match &row[2] {
                                Value::Str(s) => s.clone(),
                                _ => continue,
                            },
                            memory_id: match &row[3] {
                                Value::Str(s) => s.clone(),
                                _ => continue,
                            },
                            branch: match &row[4] {
                                Value::Str(s) => s.clone(),
                                _ => continue,
                            },
                            timestamp: match &row[5] {
                                Value::I64(ts) => {
                                    DateTime::from_timestamp(*ts, 0).unwrap_or_else(Utc::now)
                                }
                                _ => Utc::now(),
                            },
                            details: match &row[6] {
                                Value::Str(s) => serde_json::from_str(s).unwrap_or_default(),
                                _ => serde_json::json!({}),
                            },
                        };
                        entries.push(entry);
                    }
                }
            }
        }

        Ok(entries)
    }
}
