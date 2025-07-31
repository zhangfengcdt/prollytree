#![allow(dead_code)]
#![allow(unused_imports)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use gluesql_core::prelude::{Glue, Payload};
use gluesql_core::store::Transaction;
use prollytree::git::{GitKvError, GitVersionedKvStore};
use prollytree::sql::ProllyStorage;
use std::any::Any;
use std::path::Path;
use uuid::Uuid;

pub mod consistency;
pub mod display;
pub mod enhanced_types;
pub mod types;

use crate::memory::MemoryType::Recommendation;
pub use consistency::MemoryConsistencyChecker;
pub use enhanced_types::*;
pub use types::{
    AuditEntry, MemoryCommit, MemoryCommitDetails, MemoryComparison, MemorySnapshot, MemoryStatus,
    MemoryType, SourceStatus, ValidatedMemory, ValidationSource,
};

const PROLLY_CONFIG_FILE: &str = "prolly_config_tree_config";

/// Trait for types that can be stored in versioned memory
pub trait Storable: serde::Serialize + serde::de::DeserializeOwned + Clone {
    /// Get the table name for this type
    fn table_name() -> &'static str;

    /// Get the unique identifier for this instance
    fn get_id(&self) -> String;

    /// Store this instance to the database
    fn store_to_db(
        &self,
        glue: &mut Glue<ProllyStorage<32>>,
        memory: &ValidatedMemory,
    ) -> impl std::future::Future<Output = Result<()>>;

    /// Load instances from the database
    fn load_from_db(
        glue: &mut Glue<ProllyStorage<32>>,
        limit: Option<usize>,
    ) -> impl std::future::Future<Output = Result<Vec<ValidatedMemory>>>;

    /// Handle updates (default: delete and insert)
    fn update_in_db(
        &self,
        glue: &mut Glue<ProllyStorage<32>>,
        memory: &ValidatedMemory,
    ) -> impl std::future::Future<Output = Result<()>> {
        async move {
            // Default implementation: delete existing and insert new
            let delete_sql = format!(
                "DELETE FROM {} WHERE id = '{}'",
                Self::table_name(),
                self.get_id()
            );
            let _ = glue.execute(&delete_sql).await; // Ignore error if doesn't exist

            self.store_to_db(glue, memory).await
        }
    }
}

/// Core memory store with versioning capabilities
pub struct MemoryStore {
    store_path: String,
    versioned_store: GitVersionedKvStore<32>,
    audit_enabled: bool,
}

impl MemoryStore {
    pub async fn new(store_path: &str) -> Result<Self> {
        let path = Path::new(store_path);

        // Create directory if it doesn't exist
        if !path.exists() {
            println!("Creating {store_path}");
            std::fs::create_dir_all(path)?;
        }

        // change current directory to store path
        std::env::set_current_dir(path)?;

        // get current directory
        let current_dir = std::env::current_dir()?;

        // Initialize VersionedKvStore in dataset subdirectory
        // Check if prolly tree config exists to determine if we should init or open
        let versioned_store = if current_dir.join(PROLLY_CONFIG_FILE).exists() {
            GitVersionedKvStore::<32>::open(&current_dir)
                .map_err(|e| anyhow::anyhow!("Failed to open versioned store: {:?}", e))?
        } else {
            GitVersionedKvStore::<32>::init(&current_dir)
                .map_err(|e| anyhow::anyhow!("Failed to init versioned store: {:?}", e))?
        };

        // Initialize ProllyStorage - it will create its own VersionedKvStore accessing the same prolly tree files
        let storage = if current_dir.join(PROLLY_CONFIG_FILE).exists() {
            println!("Opening existing storage: {PROLLY_CONFIG_FILE}");
            ProllyStorage::<32>::open(&current_dir)?
        } else {
            println!("Creating new storage: {PROLLY_CONFIG_FILE}");
            ProllyStorage::<32>::init(&current_dir)?
        };

        let mut glue = Glue::new(storage);
        Self::init_schema(&mut glue).await?;

        Ok(Self {
            store_path: store_path.to_string(),
            versioned_store,
            audit_enabled: false,
        })
    }

    async fn init_schema(glue: &mut Glue<ProllyStorage<32>>) -> Result<()> {
        Self::ensure_table_exists(
            glue,
            "market_data",
            r#"CREATE TABLE market_data (
                id TEXT PRIMARY KEY,
                symbol TEXT,
                content TEXT,
                validation_hash TEXT,
                sources TEXT,
                confidence FLOAT,
                timestamp INTEGER
            )"#,
        )
        .await?;

        Self::ensure_table_exists(
            glue,
            "recommendations",
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
            )"#,
        )
        .await?;

        Self::ensure_table_exists(
            glue,
            "memories",
            r#"CREATE TABLE memories (
                id TEXT PRIMARY KEY,
                content TEXT,
                timestamp INTEGER,
                validation_hash TEXT,
                sources TEXT,
                confidence FLOAT,
                cross_references TEXT
            )"#,
        )
        .await?;

        Self::ensure_table_exists(
            glue,
            "audit_log",
            r#"CREATE TABLE audit_log (
                id TEXT PRIMARY KEY,
                action TEXT,
                memory_type TEXT,
                memory_id TEXT,
                branch TEXT,
                timestamp INTEGER,
                details TEXT
            )"#,
        )
        .await?;

        Self::ensure_table_exists(
            glue,
            "cross_references",
            r#"CREATE TABLE cross_references (
                source_id TEXT,
                target_id TEXT,
                reference_type TEXT,
                confidence FLOAT,
                PRIMARY KEY (source_id, target_id)
            )"#,
        )
        .await?;

        Self::ensure_table_exists(
            glue,
            "client_profiles",
            r#"CREATE TABLE client_profiles (
                id TEXT PRIMARY KEY,
                content TEXT,
                timestamp INTEGER,
                validation_hash TEXT,
                sources TEXT,
                confidence FLOAT
            )"#,
        )
        .await?;

        Ok(())
    }

    async fn ensure_table_exists(
        glue: &mut Glue<ProllyStorage<32>>,
        table_name: &str,
        create_sql: &str,
    ) -> Result<()> {
        // Try a simple query to check if table exists
        let check_sql = format!("SELECT COUNT(*) FROM {table_name}");
        if glue.execute(&check_sql).await.is_err() {
            // Table doesn't exist, create it
            glue.execute(create_sql).await?;
            glue.storage
                .commit_with_message(&format!("Create table: {table_name}"))
                .await?;
        }
        Ok(())
    }

    /// Generic store method that uses the Storable trait
    pub async fn store_typed<T: Storable>(
        &mut self,
        item: &T,
        memory: &ValidatedMemory,
    ) -> Result<String> {
        let path = Path::new(&self.store_path);

        // Debug: Check for branch mismatch (external git operations)
        let cached_branch = self.current_branch();
        let actual_branch = self.get_actual_current_branch();

        if cached_branch != actual_branch {
            println!("DEBUG: ⚠️ Branch mismatch: cached='{cached_branch}', actual='{actual_branch}' (external git operation?)");
        }

        let storage = if path.join(PROLLY_CONFIG_FILE).exists() {
            ProllyStorage::<32>::open(path)?
        } else {
            ProllyStorage::<32>::init(path)?
        };
        let mut glue = Glue::new(storage);

        // Ensure schema exists
        Self::init_schema(&mut glue).await?;

        // Use the item's update method to handle storage
        item.update_in_db(&mut glue, memory).await?;

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

        let version = memory.clone().id;
        glue.storage
            .commit_with_message(&format!("Store memory: {}", memory.id))
            .await?;

        Ok(version)
    }

    /// Legacy store method for backward compatibility
    pub async fn store(&mut self, memory: &ValidatedMemory) -> Result<String> {
        // Try to determine type and delegate to typed methods
        if let Ok(rec) = serde_json::from_str::<crate::advisor::Recommendation>(&memory.content) {
            self.store_typed(&rec, memory).await
        } else if let Ok(profile) =
            serde_json::from_str::<crate::advisor::ClientProfile>(&memory.content)
        {
            self.store_typed(&profile, memory).await
        } else {
            // Fallback to market data storage
            self.store_as_market_data(memory).await
        }
    }

    /// Fallback method for market data and unknown types
    async fn store_as_market_data(&mut self, memory: &ValidatedMemory) -> Result<String> {
        let path = Path::new(&self.store_path);
        let storage = if path.join(PROLLY_CONFIG_FILE).exists() {
            ProllyStorage::<32>::open(path)?
        } else {
            ProllyStorage::<32>::init(path)?
        };
        let mut glue = Glue::new(storage);

        Self::init_schema(&mut glue).await?;

        // Determine symbol from JSON if possible
        let symbol =
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&memory.content) {
                json_value
                    .get("symbol")
                    .and_then(|s| s.as_str())
                    .unwrap_or("UNKNOWN")
                    .to_string()
            } else {
                "UNKNOWN".to_string()
            };

        let sql = format!(
            r#"INSERT INTO market_data
            (id, symbol, content, validation_hash, sources, confidence, timestamp)
            VALUES ('{}', '{}', '{}', '{}', '{}', {}, {})"#,
            memory.id,
            &symbol,
            memory.content.replace('\'', "''"),
            hex::encode(memory.validation_hash),
            memory.sources.join(","),
            memory.confidence,
            memory.timestamp.timestamp()
        );
        glue.execute(&sql).await?;

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

        let version = memory.clone().id;
        glue.storage
            .commit_with_message(&format!("Store memory: {}", memory.id))
            .await?;

        Ok(version)
    }

    pub async fn store_with_commit(
        &mut self,
        memory: &ValidatedMemory,
        commit_message: &str,
    ) -> Result<String> {
        let path = Path::new(&self.store_path);
        let storage = if path.join(PROLLY_CONFIG_FILE).exists() {
            ProllyStorage::<32>::open(path)?
        } else {
            ProllyStorage::<32>::init(path)?
        };
        let mut glue = Glue::new(storage);

        // Ensure schema exists
        Self::init_schema(&mut glue).await?;

        // Store memory in the memories table
        let memory_sql = format!(
            r#"INSERT INTO memories
            (id, content, timestamp, validation_hash, sources, confidence, cross_references)
            VALUES ('{}', '{}', {}, '{}', '{}', {}, '{}')"#,
            memory.id,
            memory.content.replace('\'', "''"),
            memory.timestamp.timestamp(),
            hex::encode(memory.validation_hash),
            memory.sources.join(","),
            memory.confidence,
            memory.cross_references.join(",")
        );

        glue.execute(&memory_sql).await?;

        // Store cross-references
        for reference in &memory.cross_references {
            // Remove existing cross-reference to avoid duplicates
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

        let version = memory.clone().id;
        glue.storage.commit_with_message(commit_message).await?;

        Ok(version)
    }

    pub async fn store_typed_with_commit<T: Storable>(
        &mut self,
        item: &T,
        memory: &ValidatedMemory,
        commit_message: &str,
    ) -> Result<String> {
        let path = Path::new(&self.store_path);
        let storage = if path.join(PROLLY_CONFIG_FILE).exists() {
            ProllyStorage::<32>::open(path)?
        } else {
            ProllyStorage::<32>::init(path)?
        };
        let mut glue = Glue::new(storage);

        // Ensure schema exists
        Self::init_schema(&mut glue).await?;

        // Use the item's store method to handle storage
        item.store_to_db(&mut glue, memory).await?;

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

        let version = memory.clone().id;
        glue.storage.commit_with_message(commit_message).await?;

        Ok(version)
    }

    pub async fn store_with_audit(
        &mut self,
        memory_type: MemoryType,
        memory: &ValidatedMemory,
        action: &str,
    ) -> Result<String> {
        // Store the memory
        let version = self.store(memory).await?;

        // Log audit entry
        if self.audit_enabled {
            self.log_audit(action, memory_type, &memory.id).await?;
        }

        Ok(version)
    }

    pub async fn store_with_audit_and_commit(
        &mut self,
        memory_type: MemoryType,
        memory: &ValidatedMemory,
        action: &str,
        commit_message: &str,
    ) -> Result<String> {
        // Store the memory with custom commit message
        let version = self.store_with_commit(memory, commit_message).await?;

        // Log audit entry
        if self.audit_enabled {
            self.log_audit(action, memory_type, &memory.id).await?;
        }

        Ok(version)
    }

    pub async fn store_typed_with_audit_and_commit<T: Storable>(
        &mut self,
        item: &T,
        memory_type: MemoryType,
        memory: &ValidatedMemory,
        action: &str,
        commit_message: &str,
    ) -> Result<String> {
        // Store using typed storage with custom commit message
        let version = self
            .store_typed_with_commit(item, memory, commit_message)
            .await?;

        // Log audit entry
        if self.audit_enabled {
            self.log_audit(action, memory_type, &memory.id).await?;
        }

        Ok(version)
    }

    pub async fn query_related(&self, content: &str, limit: usize) -> Result<Vec<ValidatedMemory>> {
        let path = Path::new(&self.store_path);
        let storage = if path.join(PROLLY_CONFIG_FILE).exists() {
            ProllyStorage::<32>::open(path)?
        } else {
            ProllyStorage::<32>::init(path)?
        };
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
        self.versioned_store
            .create_branch(name)
            .map_err(|e| anyhow::anyhow!("Failed to create branch '{}': {:?}", name, e))?;

        // Important: We need to ensure any ProllyStorage instances created after this
        // will see the branch. The versioned_store has created the branch but hasn't switched to it.

        if self.audit_enabled {
            self.log_audit(&format!("Created branch: {name}"), MemoryType::System, name)
                .await?;
        }

        Ok(name.to_string())
    }

    pub async fn commit(&mut self, message: &str) -> Result<String> {
        // Use real git-prolly commit
        let commit_id = self
            .versioned_store
            .commit(message)
            .map_err(|e| anyhow::anyhow!("Failed to commit: {:?}", e))?;

        let commit_hex = commit_id.to_hex().to_string();

        Ok(commit_hex)
    }

    pub async fn rollback(&mut self, version: &str) -> Result<()> {
        // Use real git-prolly checkout
        self.versioned_store
            .checkout(version)
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
        let path = std::env::current_dir()?;
        let storage = if path.join(PROLLY_CONFIG_FILE).exists() {
            ProllyStorage::<32>::open(&path)?
        } else {
            ProllyStorage::<32>::init(&path)?
        };
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
        let path = std::env::current_dir()?;
        let storage = if path.join(PROLLY_CONFIG_FILE).exists() {
            ProllyStorage::<32>::open(&path)?
        } else {
            ProllyStorage::<32>::init(&path)?
        };
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
        glue.storage
            .commit_with_message(&format!("Commit Audit log: {action}"))
            .await?;
        Ok(())
    }

    async fn commit_version(&mut self, message: &str) -> String {
        // Create a real git commit
        match self.versioned_store.commit(message) {
            Ok(commit_id) => commit_id.to_hex().to_string(),
            Err(_) => format!(
                "v-{}-{}",
                Utc::now().timestamp(),
                &message[..8.min(message.len())]
            ),
        }
    }

    /// Get current branch name
    pub fn current_branch(&self) -> &str {
        self.versioned_store.current_branch()
    }

    /// Get the actual current branch from git HEAD (not cached)
    pub fn get_actual_current_branch(&self) -> String {
        let store_path = Path::new(&self.store_path);

        // Try multiple possible git directory locations
        // Git repository is typically in the parent directory of the store path
        let possible_git_dirs = vec![
            store_path.parent().unwrap().join(".git"), // /tmp/advisor/.git
            store_path.join(".git"),                   // /tmp/advisor/data7/.git
            std::env::current_dir().unwrap().join(".git"), // current working directory
        ];

        for git_dir in possible_git_dirs {
            let head_file = git_dir.join("HEAD");

            if head_file.exists() {
                // Read the HEAD file
                if let Ok(head_content) = std::fs::read_to_string(&head_file) {
                    let head_content = head_content.trim();

                    // Check if HEAD points to a branch (ref: refs/heads/branch_name)
                    if let Some(branch_ref) = head_content.strip_prefix("ref: refs/heads/") {
                        return branch_ref.to_string();
                    }

                    // If HEAD contains a commit hash (detached HEAD), show first 8 chars
                    if head_content.len() >= 8
                        && head_content.chars().all(|c| c.is_ascii_hexdigit())
                    {
                        return format!("detached@{}", &head_content[..8]);
                    }
                }
            }
        }

        // Fallback to cached branch name if git read fails
        self.versioned_store.current_branch().to_string()
    }

    /// List all branches
    pub fn list_branches(&self) -> Result<Vec<String>> {
        self.versioned_store
            .list_branches()
            .map_err(|e| anyhow::anyhow!("Failed to list branches: {:?}", e))
    }

    /// Get git status (staged changes)
    pub fn status(&self) -> Vec<(Vec<u8>, String)> {
        self.versioned_store.status()
    }

    /// Checkout branch or commit
    pub async fn checkout(&mut self, branch_or_commit: &str) -> Result<()> {
        self.versioned_store
            .checkout(branch_or_commit)
            .map_err(|e| anyhow::anyhow!("Failed to checkout '{}': {:?}", branch_or_commit, e))?;

        if self.audit_enabled {
            self.log_audit(
                &format!("Checked out: {branch_or_commit}"),
                MemoryType::System,
                branch_or_commit,
            )
            .await?;
        }

        Ok(())
    }

    /// Get commit history (simplified version of diff)
    pub async fn get_memory_history(&self, limit: Option<usize>) -> Result<Vec<MemoryCommit>> {
        // The git repository is in the parent directory (data), not dataset
        let git_dir = Path::new(&self.store_path);

        // First, check if git repository exists
        if !git_dir.join(".git").exists() {
            return Ok(vec![]); // Return empty history if no git repo
        }

        // Build git log command with proper limit
        let mut args = vec!["log", "--format=%H|%s|%at|%an"];

        // Add limit if specified
        let limit_str;
        if let Some(limit) = limit {
            limit_str = format!("-{limit}");
            args.insert(1, &limit_str);
        }

        let log_output = std::process::Command::new("git")
            .args(&args)
            .current_dir(git_dir)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to get git log: {}", e))?;

        if !log_output.status.success() {
            let error = String::from_utf8_lossy(&log_output.stderr);
            // If no commits yet, return empty history
            if error.contains("does not have any commits") || error.contains("bad revision") {
                return Ok(vec![]);
            }
            return Err(anyhow::anyhow!("Git log failed: {}", error));
        }

        let log_str = String::from_utf8_lossy(&log_output.stdout);
        let mut commits = Vec::new();

        for line in log_str.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 4 {
                let commit = MemoryCommit {
                    hash: parts[0].to_string(),
                    message: parts[1].to_string(),
                    timestamp: DateTime::from_timestamp(parts[2].parse().unwrap_or(0), 0)
                        .unwrap_or_else(Utc::now),
                    memory_type: self.parse_memory_type_from_message(parts[1]),
                };
                commits.push(commit);
            }
        }

        if self.audit_enabled {
            self.log_audit(
                "Retrieved memory history",
                MemoryType::System,
                &format!("limit_{}", limit.unwrap_or_default()),
            )
            .await?;
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
            )
            .await?;
        }

        Ok(merge_commit)
    }

    /// Show basic information about a specific commit
    pub async fn show_memory_commit(&self, commit: &str) -> Result<MemoryCommitDetails> {
        // Use the git directory (data), not dataset
        let current_dir = std::env::current_dir()?;
        let git_dir = current_dir.parent().unwrap();

        let show_output = std::process::Command::new("git")
            .args(["show", "--format=%H|%s|%at|%an", "--name-only", commit])
            .current_dir(git_dir)
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
                timestamp: DateTime::from_timestamp(parts[2].parse().unwrap_or(0), 0)
                    .unwrap_or_else(Utc::now),
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
            )
            .await?;
        }

        Ok(details)
    }

    /// Revert to a specific commit (simplified rollback)
    pub async fn revert_to_commit(&mut self, commit: &str) -> Result<String> {
        // Simple revert using git reset (following rig_versioned_memory pattern)
        let git_dir = Path::new(&self.store_path);
        let reset_output = std::process::Command::new("git")
            .args(["reset", "--hard", commit])
            .current_dir(git_dir)
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
            )
            .await?;
        }

        Ok(new_commit)
    }

    /// Get current commit ID
    pub async fn get_current_commit_id(&self) -> Result<String> {
        let git_dir = Path::new(&self.store_path);
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(git_dir)
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
    pub async fn get_recommendations_at_commit(
        &self,
        commit: &str,
    ) -> Result<Vec<ValidatedMemory>> {
        // Save current state
        let _current_commit = self.get_current_commit_id().await?;
        let current_branch = self.current_branch().to_string();
        let git_dir = Path::new(&self.store_path);

        // Temporarily checkout the target commit
        let checkout_output = std::process::Command::new("git")
            .args(["checkout", commit])
            .current_dir(git_dir)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to checkout commit '{}': {}", commit, e))?;

        if !checkout_output.status.success() {
            let error = String::from_utf8_lossy(&checkout_output.stderr);
            return Err(anyhow::anyhow!("Git checkout failed: {}", error));
        }

        // Query recommendations from this point in time
        let path = Path::new(&self.store_path);
        let storage = if path.join(PROLLY_CONFIG_FILE).exists() {
            ProllyStorage::<32>::open(path)?
        } else {
            ProllyStorage::<32>::init(path)?
        };
        let mut glue = Glue::new(storage);

        let sql = "SELECT id, client_id, symbol, recommendation_type, reasoning, confidence, validation_hash, memory_version, timestamp FROM recommendations ORDER BY timestamp DESC";
        let results = glue.execute(sql).await?;

        let recommendations = self.parse_recommendation_results(results)?;

        // Restore original state
        std::process::Command::new("git")
            .args(["checkout", &current_branch])
            .current_dir(git_dir)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to restore branch '{}': {}", current_branch, e))?;

        if self.audit_enabled {
            self.log_audit(
                &format!("Temporal query: recommendations at commit {commit}"),
                MemoryType::Recommendation,
                commit,
            )
            .await?;
        }

        Ok(recommendations)
    }

    /// Get market data at a specific commit/time (temporal query)
    pub async fn get_market_data_at_commit(
        &self,
        commit: &str,
        symbol: Option<&str>,
    ) -> Result<Vec<ValidatedMemory>> {
        // Save current state
        let _current_commit = self.get_current_commit_id().await?;
        let current_branch = self.current_branch().to_string();
        let git_dir = Path::new(&self.store_path);

        // Temporarily checkout the target commit
        let checkout_output = std::process::Command::new("git")
            .args(["checkout", commit])
            .current_dir(git_dir)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to checkout commit '{}': {}", commit, e))?;

        if !checkout_output.status.success() {
            let error = String::from_utf8_lossy(&checkout_output.stderr);
            return Err(anyhow::anyhow!("Git checkout failed: {}", error));
        }

        // Query market data from this point in time
        let path = Path::new(&self.store_path);
        let storage = if path.join(PROLLY_CONFIG_FILE).exists() {
            ProllyStorage::<32>::open(path)?
        } else {
            ProllyStorage::<32>::init(path)?
        };
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
            .current_dir(git_dir)
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
            )
            .await?;
        }

        Ok(market_data)
    }

    /// Get memory state at a specific date/time
    pub async fn get_memory_state_at_time(
        &self,
        target_time: DateTime<Utc>,
    ) -> Result<MemorySnapshot> {
        // Find the commit closest to the target time
        let commit_hash = self.find_commit_at_time(target_time).await?;

        // Get all memory types at that commit
        let recommendations = self
            .get_recommendations_at_commit(&commit_hash)
            .await
            .unwrap_or_default();
        let market_data = self
            .get_market_data_at_commit(&commit_hash, None)
            .await
            .unwrap_or_default();
        let audit_trail = self
            .get_audit_trail_at_commit(&commit_hash)
            .await
            .unwrap_or_default();

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
        let datadir = std::env::current_dir()?;
        let log_output = std::process::Command::new("git")
            .args([
                "log",
                "--format=%H|%at",
                "--until",
                &target_time.timestamp().to_string(),
            ])
            .current_dir(&datadir)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to get git log: {}", e))?;

        let log_str = String::from_utf8_lossy(&log_output.stdout);
        let lines: Vec<&str> = log_str.lines().collect();

        if lines.is_empty() {
            return Err(anyhow::anyhow!(
                "No commits found before time {}",
                target_time
            ));
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
        let git_dir = Path::new(&self.store_path);

        // Temporarily checkout the target commit
        std::process::Command::new("git")
            .args(["checkout", commit])
            .current_dir(git_dir)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to checkout commit for audit: {}", e))?;

        // Query audit trail from this point in time
        let audit_entries = self.get_audit_trail(None, None).await.unwrap_or_default();

        // Restore original state
        std::process::Command::new("git")
            .args(["checkout", &current_branch])
            .current_dir(git_dir)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to restore branch for audit: {}", e))?;

        Ok(audit_entries)
    }

    /// Compare memory states between two time points
    pub async fn compare_memory_states(
        &self,
        from_time: DateTime<Utc>,
        to_time: DateTime<Utc>,
    ) -> Result<MemoryComparison> {
        let from_snapshot = self.get_memory_state_at_time(from_time).await?;
        let to_snapshot = self.get_memory_state_at_time(to_time).await?;

        // Simple comparison - count differences
        let recommendation_diff =
            to_snapshot.recommendations.len() as i64 - from_snapshot.recommendations.len() as i64;
        let market_data_diff =
            to_snapshot.market_data.len() as i64 - from_snapshot.market_data.len() as i64;
        let total_diff = to_snapshot.total_memories as i64 - from_snapshot.total_memories as i64;

        Ok(MemoryComparison {
            from_commit: from_snapshot.commit_hash,
            to_commit: to_snapshot.commit_hash,
            from_time,
            to_time,
            recommendation_changes: recommendation_diff,
            market_data_changes: market_data_diff,
            total_memory_change: total_diff,
            summary: format!(
                "Memory changed by {} entries between {} and {}",
                total_diff,
                from_time.format("%Y-%m-%d %H:%M"),
                to_time.format("%Y-%m-%d %H:%M")
            ),
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
                        let recommendation_type = match &row[3] {
                            Value::Str(s) => s.clone(),
                            _ => "UNKNOWN".to_string(),
                        };

                        let reasoning = match &row[4] {
                            Value::Str(s) => s.clone(),
                            _ => "".to_string(),
                        };

                        let confidence = match &row[5] {
                            Value::F64(f) => *f,
                            _ => 0.0,
                        };

                        let memory_version = match &row[7] {
                            Value::Str(s) => s.clone(),
                            _ => "".to_string(),
                        };

                        let timestamp = match &row[8] {
                            Value::I64(ts) => DateTime::from_timestamp(*ts, 0)
                                .unwrap_or_else(Utc::now)
                                .to_rfc3339(),
                            _ => Utc::now().to_rfc3339(),
                        };

                        let validation_hash = match &row[6] {
                            Value::Str(s) => hex::decode(s)
                                .unwrap_or_default()
                                .try_into()
                                .unwrap_or([0u8; 32]),
                            _ => [0u8; 32],
                        };

                        let content = serde_json::json!({
                            "id": id,
                            "client_id": client_id,
                            "symbol": symbol,
                            "recommendation_type": recommendation_type,
                            "reasoning": reasoning,
                            "confidence": confidence,
                            "memory_version": memory_version,
                            "timestamp": timestamp,
                            "validation_result": {
                                "is_valid": true,
                                "confidence": confidence,
                                "hash": validation_hash.to_vec(),
                                "cross_references": [],
                                "issues": []
                            }
                        })
                        .to_string();

                        let memory = ValidatedMemory {
                            id,
                            content,
                            validation_hash,
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

    /// Get recent recommendations from versioned storage
    pub async fn get_recent_recommendations(
        &self,
        limit: usize,
    ) -> Result<Vec<crate::advisor::Recommendation>> {
        self.get_recommendations(None, None, Some(limit)).await
    }

    /// Get recommendations with optional branch/commit and limit
    pub async fn get_recommendations(
        &self,
        branch: Option<&str>,
        commit: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<crate::advisor::Recommendation>> {
        if let Some(commit_hash) = commit {
            // Use the existing temporal query method
            let memories = self.get_recommendations_at_commit(commit_hash).await?;

            let mut recommendations = Vec::new();
            for memory in memories {
                match serde_json::from_str::<crate::advisor::Recommendation>(&memory.content) {
                    Ok(rec) => recommendations.push(rec),
                    Err(e) => eprintln!("Warning: Failed to parse recommendation: {e}"),
                }
            }

            // Apply limit if specified
            if let Some(limit) = limit {
                recommendations.truncate(limit);
            }

            return Ok(recommendations);
        }

        // Handle branch-specific queries
        if let Some(branch_name) = branch {
            let current_branch = self.current_branch().to_string();
            let git_dir = Path::new(&self.store_path);

            // Temporarily checkout the target branch
            let checkout_output = std::process::Command::new("git")
                .args(["checkout", branch_name])
                .current_dir(git_dir)
                .output()
                .map_err(|e| {
                    anyhow::anyhow!("Failed to checkout branch '{}': {}", branch_name, e)
                })?;

            if !checkout_output.status.success() {
                let error = String::from_utf8_lossy(&checkout_output.stderr);
                return Err(anyhow::anyhow!("Git checkout failed: {}", error));
            }

            // Query recommendations on this branch (call internal method to avoid recursion)
            let result = self.get_recommendations_internal(limit).await;

            // Restore original branch
            std::process::Command::new("git")
                .args(["checkout", &current_branch])
                .current_dir(git_dir)
                .output()
                .map_err(|e| {
                    anyhow::anyhow!("Failed to restore branch '{}': {}", current_branch, e)
                })?;

            return result;
        }

        // Query current branch for recommendations
        self.get_recommendations_internal(limit).await
    }

    /// Internal method to get recommendations from current branch (no branch/commit switching)
    async fn get_recommendations_internal(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<crate::advisor::Recommendation>> {
        let path = Path::new(&self.store_path);
        if !path.exists() {
            return Ok(vec![]);
        }

        let storage = if path.join(PROLLY_CONFIG_FILE).exists() {
            ProllyStorage::<32>::open(path)?
        } else {
            return Ok(vec![]);
        };

        let mut glue = Glue::new(storage);

        // Use the Storable trait method
        let memories = crate::advisor::Recommendation::load_from_db(&mut glue, limit).await?;

        let mut recommendations = Vec::new();
        for memory in memories {
            match serde_json::from_str::<crate::advisor::Recommendation>(&memory.content) {
                Ok(rec) => recommendations.push(rec),
                Err(e) => eprintln!("Warning: Failed to parse recommendation: {e}"),
            }
        }

        Ok(recommendations)
    }

    /// Get memory system status information
    pub async fn get_memory_status(&self) -> Result<MemoryStatus> {
        let current_commit = self
            .get_current_commit_id()
            .await
            .unwrap_or_else(|_| "unknown".to_string());
        let branches = self.list_branches().unwrap_or_default();
        let current_branch = self.current_branch();

        // Get memory stats
        let history = self.get_memory_history(Some(100)).await.unwrap_or_default();
        let total_commits = history.len();

        // Count by type
        let mut recommendation_count = 0;
        let mut market_data_count = 0;
        let mut audit_count = 0;

        for commit in &history {
            match commit.memory_type {
                MemoryType::Recommendation => recommendation_count += 1,
                MemoryType::MarketData => market_data_count += 1,
                MemoryType::Audit => audit_count += 1,
                _ => {}
            }
        }

        // Check if git repo exists and is healthy
        let git_dir = Path::new(&self.store_path);
        let git_healthy = git_dir.join(".git").exists();

        // Check if dataset directory exists
        let dataset_dir = Path::new(&self.store_path);
        let storage_healthy = dataset_dir.exists() && dataset_dir.join(PROLLY_CONFIG_FILE).exists();

        Ok(MemoryStatus {
            validation_active: storage_healthy && git_healthy,
            audit_enabled: self.audit_enabled,
            security_monitoring: true, // Always enabled for this demo
            current_branch: current_branch.to_string(),
            current_commit: current_commit[..8.min(current_commit.len())].to_string(),
            total_branches: branches.len(),
            total_commits,
            recommendation_count,
            market_data_count,
            audit_count,
            storage_healthy,
            git_healthy,
        })
    }

    /// Get validation sources with their current status
    pub async fn get_validation_sources(&self) -> Result<Vec<ValidationSource>> {
        // In a real implementation, we would check the actual health of each source
        // For now, simulate checking each source
        Ok(vec![
            ValidationSource {
                name: "Bloomberg".to_string(),
                trust_level: 0.95,
                status: SourceStatus::Active,
                last_checked: Some(Utc::now()),
                response_time_ms: Some(45),
            },
            ValidationSource {
                name: "Yahoo Finance".to_string(),
                trust_level: 0.85,
                status: SourceStatus::Active,
                last_checked: Some(Utc::now()),
                response_time_ms: Some(120),
            },
            ValidationSource {
                name: "Alpha Vantage".to_string(),
                trust_level: 0.80,
                status: SourceStatus::Active,
                last_checked: Some(Utc::now()),
                response_time_ms: Some(200),
            },
        ])
    }

    /// Store client profile in versioned memory using the Storable trait
    pub async fn store_client_profile(
        &mut self,
        profile: &crate::advisor::ClientProfile,
    ) -> Result<()> {
        let validated_memory = ValidatedMemory {
            id: profile.id.clone(),
            content: serde_json::to_string(profile)?,
            timestamp: Utc::now(),
            validation_hash: self.hash_content(&serde_json::to_string(profile)?),
            sources: vec!["user_input".to_string()],
            confidence: 1.0,
            cross_references: vec![],
        };

        // Store using the typed method
        self.store_typed(profile, &validated_memory).await?;

        // Log audit entry if enabled
        if self.audit_enabled {
            self.log_audit(
                &format!("Updated client profile for {}", profile.id),
                MemoryType::ClientProfile,
                &profile.id,
            )
            .await?;
        }

        Ok(())
    }

    /// Load client profile from versioned memory using the Storable trait
    pub async fn load_client_profile(&self) -> Result<Option<crate::advisor::ClientProfile>> {
        let path = Path::new(&self.store_path);
        if !path.exists() || !path.join(PROLLY_CONFIG_FILE).exists() {
            return Ok(None);
        }

        // Create a fresh storage instance to ensure we see the latest committed state
        let storage = ProllyStorage::<32>::open(path)?;
        let mut glue = Glue::new(storage);

        // Use the Storable trait method
        let memories = crate::advisor::ClientProfile::load_from_db(&mut glue, Some(1)).await?;

        if let Some(memory) = memories.first() {
            match serde_json::from_str::<crate::advisor::ClientProfile>(&memory.content) {
                Ok(profile) => Ok(Some(profile)),
                Err(e) => {
                    eprintln!("Warning: Failed to parse client profile: {e}");
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Simple content hashing for validation
    fn hash_content(&self, content: &str) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hasher.finalize().into()
    }
}
