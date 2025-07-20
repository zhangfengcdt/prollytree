use anyhow::Result;
use chrono::Utc;
use gluesql_core::prelude::{Glue, Payload};
use prollytree::sql::ProllyStorage;
use std::path::Path;
use uuid::Uuid;

use super::schema::setup_schema;
use super::types::{Memory, MemoryType};

pub struct VersionedMemoryStore {
    store_path: String,
    current_session: String,
}

impl VersionedMemoryStore {
    pub async fn new(store_path: &str) -> Result<Self> {
        let path = Path::new(store_path);
        
        // Create directory if it doesn't exist
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }

        // Initialize git repository if it doesn't exist
        let git_dir = path.join(".git");
        if !git_dir.exists() {
            use std::process::Command;
            
            // Initialize git repository
            let output = Command::new("git")
                .args(&["init"])
                .current_dir(path)
                .output()?;
                
            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to initialize git repository: {}", 
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            
            // Set up initial git config if needed
            Command::new("git")
                .args(&["config", "user.name", "AI Agent"])
                .current_dir(path)
                .output()
                .ok(); // Ignore errors, might already be set globally
                
            Command::new("git")
                .args(&["config", "user.email", "agent@example.com"])
                .current_dir(path)
                .output()
                .ok(); // Ignore errors, might already be set globally
        }

        // Initialize ProllyStorage in a subdirectory to avoid git root conflict
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
        setup_schema(&mut glue).await?;

        Ok(Self {
            store_path: store_path.to_string(),
            current_session: Uuid::new_v4().to_string(),
        })
    }

    pub async fn store_memory(
        &self,
        memory_type: MemoryType,
        memory: &Memory,
    ) -> Result<String> {
        let path = Path::new(&self.store_path).join("data");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);

        match memory_type {
            MemoryType::ShortTerm => {
                let sql = format!(
                    r#"
                    INSERT INTO short_term_memory 
                    (id, session_id, timestamp, role, content, metadata)
                    VALUES ('{}', '{}', {}, '{}', '{}', '{}')"#,
                    memory.id,
                    self.current_session,
                    memory.timestamp.timestamp(),
                    memory
                        .metadata
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("user"),
                    memory.content.replace('\'', "''"), // Escape single quotes
                    memory.metadata.to_string().replace('\'', "''")
                );
                glue.execute(&sql).await?;
            }
            MemoryType::LongTerm => {
                let sql = format!(
                    r#"
                    INSERT INTO long_term_memory 
                    (id, concept, facts, confidence, created_at, access_count)
                    VALUES ('{}', '{}', '{}', 0.8, {}, 1)"#,
                    memory.id,
                    memory
                        .metadata
                        .get("concept")
                        .and_then(|v| v.as_str())
                        .unwrap_or("general"),
                    memory.content.replace('\'', "''"),
                    memory.timestamp.timestamp()
                );
                glue.execute(&sql).await?;
            }
            MemoryType::Episodic => {
                let sql = format!(
                    r#"
                    INSERT INTO episodic_memory 
                    (id, episode_id, timestamp, context, action_taken, outcome, reward)
                    VALUES ('{}', '{}', {}, '{}', '{}', '{}', {})"#,
                    memory.id,
                    memory
                        .metadata
                        .get("episode_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&self.current_session),
                    memory.timestamp.timestamp(),
                    memory.metadata.to_string().replace('\'', "''"),
                    memory.content.replace('\'', "''"),
                    memory
                        .metadata
                        .get("outcome")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    memory
                        .metadata
                        .get("reward")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0)
                );
                glue.execute(&sql).await?;
            }
        }

        // Create a version (commit to git)
        let version = self.create_version(&format!("Store {} memory", memory_type)).await?;
        Ok(version)
    }

    pub async fn recall_memories(
        &self,
        query: &str,
        memory_type: MemoryType,
        limit: usize,
    ) -> Result<Vec<Memory>> {
        let path = Path::new(&self.store_path).join("data");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);

        let sql = match memory_type {
            MemoryType::ShortTerm => {
                if query.is_empty() {
                    format!(
                        r#"
                        SELECT id, content, timestamp, metadata 
                        FROM short_term_memory 
                        WHERE session_id = '{}'
                        ORDER BY timestamp DESC 
                        LIMIT {}"#,
                        self.current_session, limit
                    )
                } else {
                    format!(
                        r#"
                        SELECT id, content, timestamp, metadata 
                        FROM short_term_memory 
                        WHERE content LIKE '%{}%' 
                        ORDER BY timestamp DESC 
                        LIMIT {}"#,
                        query.replace('\'', "''"),
                        limit
                    )
                }
            }
            MemoryType::LongTerm => format!(
                r#"
                SELECT id, facts as content, created_at as timestamp, concept
                FROM long_term_memory 
                WHERE facts LIKE '%{}%' 
                ORDER BY access_count DESC 
                LIMIT {}"#,
                query.replace('\'', "''"),
                limit
            ),
            MemoryType::Episodic => format!(
                r#"
                SELECT id, action_taken as content, timestamp, context
                FROM episodic_memory 
                WHERE action_taken LIKE '%{}%' OR context LIKE '%{}%'
                ORDER BY timestamp DESC 
                LIMIT {}"#,
                query.replace('\'', "''"),
                query.replace('\'', "''"),
                limit
            ),
        };

        let results = glue.execute(&sql).await?;
        let memories = self.parse_query_results(results, memory_type)?;
        Ok(memories)
    }

    fn parse_query_results(
        &self,
        results: Vec<Payload>,
        memory_type: MemoryType,
    ) -> Result<Vec<Memory>> {
        use gluesql_core::data::Value;
        let mut memories = Vec::new();

        for payload in results {
            if let Payload::Select { labels: _, rows } = payload {
                for row in rows {
                    if row.len() >= 4 {
                        let id = match &row[0] {
                            Value::Str(s) => s.clone(),
                            _ => continue,
                        };
                        
                        let content = match &row[1] {
                            Value::Str(s) => s.clone(),
                            _ => continue,
                        };
                        
                        let timestamp = match &row[2] {
                            Value::I64(ts) => *ts,
                            Value::I32(ts) => *ts as i64,
                            Value::I16(ts) => *ts as i64,
                            _ => Utc::now().timestamp(),
                        };

                        let metadata = match memory_type {
                            MemoryType::ShortTerm => {
                                match &row[3] {
                                    Value::Str(s) => serde_json::from_str(s).unwrap_or(serde_json::json!({})),
                                    _ => serde_json::json!({}),
                                }
                            }
                            MemoryType::LongTerm => {
                                match &row[3] {
                                    Value::Str(concept) => serde_json::json!({ "concept": concept }),
                                    _ => serde_json::json!({ "concept": "general" }),
                                }
                            }
                            MemoryType::Episodic => {
                                match &row[3] {
                                    Value::Str(s) => serde_json::from_str(s).unwrap_or(serde_json::json!({})),
                                    _ => serde_json::json!({}),
                                }
                            }
                        };

                        let memory = Memory {
                            id,
                            content,
                            timestamp: chrono::DateTime::from_timestamp(timestamp, 0)
                                .unwrap_or_else(Utc::now),
                            metadata,
                            embedding: None,
                        };

                        memories.push(memory);
                    }
                }
            }
        }

        Ok(memories)
    }

    pub async fn create_branch(&self, name: &str) -> Result<()> {
        // In a real implementation, we would create a git branch using VersionedKvStore
        println!("🌿 Created memory branch: {}", name);
        Ok(())
    }

    pub async fn rollback_to_version(&self, version: &str) -> Result<()> {
        // In a real implementation, we would checkout a specific git commit
        println!("⏪ Rolled back to version: {}", version);
        Ok(())
    }

    pub async fn get_current_version(&self) -> String {
        format!("v_{}", Utc::now().timestamp())
    }

    async fn create_version(&self, _message: &str) -> Result<String> {
        // In a real implementation, this would create a git commit
        let version = format!("v_{}", Utc::now().timestamp());
        Ok(version)
    }

    pub async fn clear_session_memories(&self) -> Result<()> {
        let path = Path::new(&self.store_path).join("data");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);
        
        glue.execute(&format!(
            "DELETE FROM short_term_memory WHERE session_id = '{}'",
            self.current_session
        ))
        .await?;
        
        Ok(())
    }

    pub fn new_session(&mut self) {
        self.current_session = Uuid::new_v4().to_string();
    }

    pub async fn update_access_count(&self, memory_id: &str) -> Result<()> {
        let path = Path::new(&self.store_path).join("data");
        let storage = ProllyStorage::<32>::open(&path)?;
        let mut glue = Glue::new(storage);
        
        glue.execute(&format!(
            "UPDATE long_term_memory SET access_count = access_count + 1 WHERE id = '{}'",
            memory_id
        ))
        .await?;
        
        Ok(())
    }
}