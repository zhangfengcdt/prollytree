use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::openai;
use serde_json::json;
use uuid::Uuid;

use crate::memory::{Memory, MemoryContext, MemoryType, VersionedMemoryStore};

pub struct VersionedAgent {
    openai_client: openai::Client,
    memory_store: VersionedMemoryStore,
    session_id: String,
    model: String,
}

impl VersionedAgent {
    pub async fn new(api_key: String, memory_path: &str) -> Result<Self> {
        let client = openai::Client::new(&api_key);
        let memory_store = VersionedMemoryStore::new(memory_path).await?;

        Ok(Self {
            openai_client: client,
            memory_store,
            session_id: Uuid::new_v4().to_string(),
            model: "gpt-4o-mini".to_string(), // Using a more accessible model
        })
    }

    pub async fn process_message(&mut self, input: &str) -> Result<(String, String)> {
        // 1. Store user input in versioned memory
        let input_memory = Memory::with_id(
            format!("input_{}", Utc::now().timestamp()),
            input.to_string(),
            json!({
                "role": "user",
                "session_id": self.session_id
            }),
        );

        self.memory_store
            .store_memory(MemoryType::ShortTerm, &input_memory)
            .await?;

        // 2. Retrieve relevant context from memory
        let context = self.build_context(input).await?;

        // 3. Build prompt with context
        let prompt_text = self.build_prompt(input, &context);

        // 4. Generate response using Rig
        let agent = self.openai_client.agent(&self.model).build();

        let response = agent.prompt(&prompt_text).await?;

        // 5. Store response and commit version
        let response_memory = Memory::with_id(
            format!("response_{}", Utc::now().timestamp()),
            response.clone(),
            json!({
                "role": "assistant",
                "session_id": self.session_id,
                "context_used": context.total_memories()
            }),
        );

        let version = self
            .memory_store
            .store_memory(MemoryType::ShortTerm, &response_memory)
            .await?;

        println!(
            "💾 {}",
            format!("Memory committed to version: {version}").cyan()
        );

        Ok((response, version))
    }

    async fn build_context(&self, input: &str) -> Result<MemoryContext> {
        let mut context = MemoryContext::new();

        // Retrieve relevant long-term memories
        context.long_term_memories = self
            .memory_store
            .recall_memories(input, MemoryType::LongTerm, 3)
            .await?;

        // Retrieve recent conversation
        context.recent_memories = self
            .memory_store
            .recall_memories("", MemoryType::ShortTerm, 5)
            .await?;

        Ok(context)
    }

    fn build_prompt(&self, input: &str, context: &MemoryContext) -> String {
        let context_text = context.build_context_text();

        if context_text.is_empty() {
            format!("User: {input}\nAssistant:")
        } else {
            format!(
                "Context from memory:\n{context_text}\n\nUser: {input}\nAssistant:"
            )
        }
    }

    pub async fn learn_fact(&mut self, concept: &str, fact: &str) -> Result<String> {
        let memory = Memory::with_id(
            format!("fact_{}", Utc::now().timestamp()),
            fact.to_string(),
            json!({
                "concept": concept,
                "learned_from": self.session_id
            }),
        );

        let version = self
            .memory_store
            .store_memory(MemoryType::LongTerm, &memory)
            .await?;

        Ok(version)
    }

    pub async fn record_episode(
        &mut self,
        action: &str,
        outcome: &str,
        reward: f64,
    ) -> Result<String> {
        let memory = Memory::with_id(
            format!("episode_{}", Utc::now().timestamp()),
            action.to_string(),
            json!({
                "episode_id": self.session_id,
                "outcome": outcome,
                "reward": reward
            }),
        );

        let version = self
            .memory_store
            .store_memory(MemoryType::Episodic, &memory)
            .await?;

        Ok(version)
    }

    // Memory versioning methods
    pub async fn create_memory_branch(&self, name: &str) -> Result<()> {
        self.memory_store.create_branch(name).await
    }

    pub async fn rollback_to_version(&mut self, version: &str) -> Result<()> {
        self.memory_store.rollback_to_version(version).await
    }

    pub async fn get_current_version(&self) -> String {
        self.memory_store.get_current_version().await
    }

    pub fn new_session(&mut self) {
        self.session_id = Uuid::new_v4().to_string();
        self.memory_store.new_session();
    }

    pub async fn clear_session(&mut self) -> Result<()> {
        self.memory_store.clear_session_memories().await?;
        self.new_session();
        Ok(())
    }
}
