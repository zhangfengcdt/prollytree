#![allow(dead_code)]
#![allow(unused_imports)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use gluesql_core::prelude::{Glue, Payload};
use prollytree::sql::ProllyStorage;
use std::path::Path;
use uuid::Uuid;

pub mod consistency;
pub mod display;
pub mod types;

pub use consistency::MemoryConsistencyChecker;
pub use types::{AuditEntry, MemoryType, ValidatedMemory};

/// Core memory store with versioning capabilities
pub struct MemoryStore {
    store_path: String,
    current_branch: String,
    audit_enabled: bool,
}

impl MemoryStore {
    pub async fn new(store_path: &str) -> Result<Self> {
        let path = Path::new(store_path);

        // Create directory if it doesn't exist
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }

        // Initialize git repository if needed
        let git_dir = path.join(".git");
        if !git_dir.exists() {
            std::process::Command::new("git")
                .args(["init"])
                .current_dir(path)
                .output()?;
        }

        // Initialize ProllyTree storage
        let data_dir = path.join("data");
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)?;
        }

        let storage = if data_dir.join(".git-prolly").exists() {
            ProllyStorage::<32>::open(&data_dir)?
        } else {
            ProllyStorage::<32>::init(&data_dir)?
        };

        let mut glue = Glue::new(storage);

        // Initialize schema
        Self::init_schema(&mut glue).await?;

        Ok(Self {
            store_path: store_path.to_string(),
            current_branch: "main".to_string(),
            audit_enabled: true,
        })
    }

    async fn init_schema(glue: &mut Glue<ProllyStorage<32>>) -> Result<()> {
        // Market data table
        glue.execute(
            r#"
            CREATE TABLE IF NOT EXISTS market_data (
                id TEXT PRIMARY KEY,
                symbol TEXT,
                content TEXT,
                validation_hash TEXT,
                sources TEXT,
                confidence FLOAT,
                timestamp INTEGER
            )
        "#,
        )
        .await?;

        // Recommendations table
        glue.execute(
            r#"
            CREATE TABLE IF NOT EXISTS recommendations (
                id TEXT PRIMARY KEY,
                client_id TEXT,
                symbol TEXT,
                recommendation_type TEXT,
                reasoning TEXT,
                confidence FLOAT,
                validation_hash TEXT,
                memory_version TEXT,
                timestamp INTEGER
            )
        "#,
        )
        .await?;

        // Audit log table
        glue.execute(
            r#"
            CREATE TABLE IF NOT EXISTS audit_log (
                id TEXT PRIMARY KEY,
                action TEXT,
                memory_type TEXT,
                memory_id TEXT,
                branch TEXT,
                timestamp INTEGER,
                details TEXT
            )
        "#,
        )
        .await?;

        // Memory cross-references table
        glue.execute(
            r#"
            CREATE TABLE IF NOT EXISTS cross_references (
                source_id TEXT,
                target_id TEXT,
                reference_type TEXT,
                confidence FLOAT,
                PRIMARY KEY (source_id, target_id)
            )
        "#,
        )
        .await?;

        Ok(())
    }

    pub async fn store(
        &mut self,
        memory_type: MemoryType,
        memory: &ValidatedMemory,
    ) -> Result<String> {
        let path = Path::new(&self.store_path).join("data");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);

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
            let sql = format!(
                r#"INSERT OR REPLACE INTO cross_references 
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
        let path = Path::new(&self.store_path).join("data");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);

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
        // In a real implementation, use git-prolly branch commands
        let branch_id = format!("{}-{}", name, Uuid::new_v4());
        self.current_branch = branch_id.clone();

        if self.audit_enabled {
            self.log_audit(
                &format!("Created branch: {name}"),
                MemoryType::System,
                &branch_id,
            )
            .await?;
        }

        Ok(branch_id)
    }

    pub async fn commit(&self, message: &str) -> Result<String> {
        // In a real implementation, use git-prolly commit
        let version = format!("v-{}", Utc::now().timestamp());

        if self.audit_enabled {
            self.log_audit(&format!("Commit: {message}"), MemoryType::System, &version)
                .await?;
        }

        Ok(version)
    }

    pub async fn rollback(&mut self, version: &str) -> Result<()> {
        // In a real implementation, use git-prolly checkout
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
        let path = Path::new(&self.store_path).join("data");
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
        let path = Path::new(&self.store_path).join("data");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);

        let audit_entry = AuditEntry {
            id: Uuid::new_v4().to_string(),
            action: action.to_string(),
            memory_type: format!("{memory_type:?}"),
            memory_id: memory_id.to_string(),
            branch: self.current_branch.clone(),
            timestamp: Utc::now(),
            details: serde_json::json!({
                "branch": self.current_branch,
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

    async fn create_version(&self, message: &str) -> String {
        // In production, this would create a git commit
        format!("v-{}-{}", Utc::now().timestamp(), &message[..8])
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
