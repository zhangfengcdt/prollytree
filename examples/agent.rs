use chrono::Duration;
use prollytree::agent::*;
use rig::{completion::Prompt, providers::openai::Client};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use tempfile::TempDir;

/// A Rig-powered agent that uses the prolly tree memory system
pub struct IntelligentAgent {
    memory_system: AgentMemorySystem,
    rig_client: Option<Client>,
    agent_id: String,
    conversation_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub role: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentResponse {
    pub content: String,
    pub reasoning: Option<String>,
    pub mode: ResponseMode,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseMode {
    AIPowered,
    MemoryBased,
    Hybrid,
}

impl IntelligentAgent {
    /// Create a new intelligent agent with Rig integration and prolly tree memory
    pub async fn new(
        memory_path: &std::path::Path,
        agent_id: String,
        openai_api_key: Option<String>,
    ) -> Result<Self, Box<dyn Error>> {
        // Initialize the memory system with prolly tree persistence
        let memory_system = AgentMemorySystem::init(
            memory_path,
            agent_id.clone(),
            Some(Box::new(MockEmbeddingGenerator)), // Use mock embeddings for demo
        )?;

        // Initialize Rig client if API key provided
        let rig_client = openai_api_key.map(|key| Client::new(&key));

        let conversation_id = format!("conversation_{}", chrono::Utc::now().timestamp());

        Ok(Self {
            memory_system,
            rig_client,
            agent_id,
            conversation_id,
        })
    }

    /// Process a user message using memory and optionally AI
    pub async fn process_message(
        &mut self,
        user_message: &str,
    ) -> Result<AgentResponse, Box<dyn Error>> {
        println!("🧠 Processing message with memory and AI...");

        // 1. Store user message in short-term memory
        self.memory_system
            .short_term
            .store_conversation_turn(&self.conversation_id, "user", user_message, None)
            .await?;

        // 2. Retrieve relevant context from memory
        let context = self.retrieve_relevant_context(user_message).await?;

        // 3. Generate response using Rig if available, otherwise use memory-based response
        let response = if let Some(ref client) = self.rig_client {
            self.generate_ai_response(user_message, &context, client)
                .await?
        } else {
            self.generate_memory_response(user_message, &context)
                .await?
        };

        // 4. Store assistant response in memory
        self.memory_system
            .short_term
            .store_conversation_turn(&self.conversation_id, "assistant", &response.content, None)
            .await?;

        // 5. Learn from the interaction (episodic memory)
        self.store_interaction_episode(user_message, &response)
            .await?;

        // 6. Update procedural knowledge if applicable
        self.update_procedural_knowledge(user_message, &response)
            .await?;

        Ok(response)
    }

    /// Retrieve relevant context from all memory types
    async fn retrieve_relevant_context(&self, message: &str) -> Result<String, Box<dyn Error>> {
        let mut context_parts = Vec::new();

        // Get recent conversation history
        let recent_history = self
            .memory_system
            .short_term
            .get_conversation_history(&self.conversation_id, Some(5))
            .await?;

        if !recent_history.is_empty() {
            context_parts.push(format!(
                "Recent conversation ({} turns):\n{}",
                recent_history.len(),
                recent_history
                    .iter()
                    .map(|turn| {
                        format!(
                            "{}: {}",
                            turn.content
                                .get("role")
                                .and_then(|r| r.as_str())
                                .unwrap_or("unknown"),
                            turn.content
                                .get("content")
                                .and_then(|c| c.as_str())
                                .unwrap_or("")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        // Search semantic memory for relevant facts
        let semantic_query = MemoryQuery {
            namespace: None,
            memory_types: Some(vec![MemoryType::Semantic]),
            tags: None,
            time_range: None,
            text_query: Some(message.to_string()),
            semantic_query: None,
            limit: Some(3),
            include_expired: false,
        };

        let semantic_results = self.memory_system.semantic.query(semantic_query).await?;
        if !semantic_results.is_empty() {
            context_parts.push(format!(
                "Relevant facts ({} items):\n{}",
                semantic_results.len(),
                semantic_results
                    .iter()
                    .map(|mem| format!("- {}", mem.content.to_string()))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        // Get relevant past episodes
        let episodic_query = MemoryQuery {
            namespace: None,
            memory_types: Some(vec![MemoryType::Episodic]),
            tags: None,
            time_range: Some(TimeRange {
                start: Some(chrono::Utc::now() - Duration::days(7)),
                end: Some(chrono::Utc::now()),
            }),
            text_query: Some(message.to_string()),
            semantic_query: None,
            limit: Some(2),
            include_expired: false,
        };

        let episodic_results = self.memory_system.episodic.query(episodic_query).await?;
        if !episodic_results.is_empty() {
            context_parts.push(format!(
                "Past experiences ({} items):\n{}",
                episodic_results.len(),
                episodic_results
                    .iter()
                    .map(|mem| {
                        mem.content
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("No description")
                    })
                    .collect::<Vec<_>>()
                    .join("\n- ")
            ));
        }

        Ok(if context_parts.is_empty() {
            "No relevant context found.".to_string()
        } else {
            context_parts.join("\n\n")
        })
    }

    /// Generate AI-powered response using Rig
    async fn generate_ai_response(
        &self,
        message: &str,
        context: &str,
        client: &Client,
    ) -> Result<AgentResponse, Box<dyn Error>> {
        let prompt = format!(
            "You are an intelligent assistant with access to conversation memory and knowledge base.\n\n\
             CONTEXT:\n{}\n\n\
             USER MESSAGE: {}\n\n\
             Provide a helpful, contextual response based on the available information.",
            context, message
        );

        println!("🤖 Generating AI response with Rig...");

        let agent = client
            .agent("gpt-3.5-turbo")
            .preamble("You are a helpful, knowledgeable assistant that uses context from previous conversations and stored knowledge to provide relevant responses.")
            .max_tokens(300)
            .temperature(0.7)
            .build();

        match agent.prompt(&prompt).await {
            Ok(response) => Ok(AgentResponse {
                content: response.trim().to_string(),
                reasoning: Some("Generated using AI with memory context".to_string()),
                mode: ResponseMode::AIPowered,
            }),
            Err(e) => {
                println!(
                    "⚠️ AI generation failed: {}, falling back to memory-based response",
                    e
                );
                self.generate_memory_response(message, context).await
            }
        }
    }

    /// Generate memory-based response as fallback
    async fn generate_memory_response(
        &self,
        message: &str,
        context: &str,
    ) -> Result<AgentResponse, Box<dyn Error>> {
        let response = if context.contains("No relevant context") {
            format!(
                "I understand you're asking about '{}'. While I don't have specific context about this topic yet, \
                 I'm ready to learn and help. Could you provide more details?",
                message
            )
        } else {
            format!(
                "Based on our previous interactions and what I know:\n\n{}\n\n\
                 Regarding your question about '{}', I can help you with this based on the context above.",
                context, message
            )
        };

        Ok(AgentResponse {
            content: response,
            reasoning: Some("Generated using memory and rules".to_string()),
            mode: ResponseMode::MemoryBased,
        })
    }

    /// Store the interaction as an episode
    async fn store_interaction_episode(
        &mut self,
        user_message: &str,
        response: &AgentResponse,
    ) -> Result<(), Box<dyn Error>> {
        self.memory_system
            .episodic
            .store_interaction(
                "conversation",
                vec![self.agent_id.clone(), "user".to_string()],
                &format!("User: {} | Agent: {}", user_message, response.content),
                json!({
                    "user_message": user_message,
                    "response_content": response.content,
                    "response_mode": response.mode,
                    "conversation_id": self.conversation_id
                }),
                Some(0.8), // Positive interaction
            )
            .await?;

        Ok(())
    }

    /// Update procedural knowledge based on interactions
    async fn update_procedural_knowledge(
        &mut self,
        user_message: &str,
        response: &AgentResponse,
    ) -> Result<(), Box<dyn Error>> {
        // Store a procedure for handling similar questions
        if user_message.to_lowercase().contains("help") {
            self.memory_system
                .procedural
                .store_procedure(
                    "assistance",
                    "help_request_handler",
                    "How to handle user help requests effectively",
                    vec![
                        json!({"step": 1, "action": "Analyze the help request for specific topics"}),
                        json!({"step": 2, "action": "Search memory for relevant context"}),
                        json!({"step": 3, "action": "Provide contextual assistance"}),
                        json!({"step": 4, "action": "Store the interaction for future reference"}),
                    ],
                    Some(json!({
                        "triggers": ["help", "assist", "support"],
                        "effectiveness": match response.mode {
                            ResponseMode::AIPowered => "high",
                            ResponseMode::MemoryBased => "medium",
                            ResponseMode::Hybrid => "high",
                        }
                    })),
                    7, // Medium-high priority
                )
                .await?;
        }

        Ok(())
    }

    /// Get agent statistics
    pub async fn get_stats(&self) -> Result<serde_json::Value, Box<dyn Error>> {
        let system_stats = self.memory_system.get_system_stats().await?;

        Ok(json!({
            "agent_id": self.agent_id,
            "conversation_id": self.conversation_id,
            "memory_stats": system_stats,
            "ai_enabled": self.rig_client.is_some()
        }))
    }

    /// Create a checkpoint of the agent's memory state
    pub async fn checkpoint(&mut self, message: &str) -> Result<String, Box<dyn Error>> {
        self.memory_system
            .checkpoint(message)
            .await
            .map_err(|e| e.into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🤖 Intelligent Agent with Rig + Prolly Tree Memory Demo");
    println!("======================================================");

    // Create a temporary directory for this demo
    let temp_dir = TempDir::new()?;
    let memory_path = temp_dir.path();

    println!("📁 Initializing agent at: {:?}", memory_path);

    // Initialize agent (without OpenAI API key for demo)
    let mut agent = IntelligentAgent::new(
        memory_path,
        "smart_agent_001".to_string(),
        std::env::var("OPENAI_API_KEY").ok(), // Will use memory-based responses if not set
    )
    .await?;

    println!("✅ Agent initialized successfully!");
    println!();

    // Demo conversation sequence
    let demo_messages = vec![
        "Hello! I'm learning about Rust programming. Can you help me?",
        "What are the key benefits of using Rust for system programming?",
        "I'm having trouble with ownership and borrowing. Any tips?",
        "Can you recommend some resources for learning more about memory management in Rust?",
        "Thank you for all the help! This has been very useful.",
    ];

    for (i, message) in demo_messages.iter().enumerate() {
        println!("💬 User Message {}: {}", i + 1, message);
        println!("{}", "─".repeat(80));

        let response = agent.process_message(message).await?;

        println!("🤖 Agent Response ({:?}):", response.mode);
        println!("{}", response.content);

        if let Some(reasoning) = response.reasoning {
            println!("🧠 Reasoning: {}", reasoning);
        }

        println!();

        // Add some delay between messages to make it more realistic
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // Demonstrate memory capabilities
    println!("📊 Agent Memory Statistics:");
    println!("{}", "═".repeat(50));

    let stats = agent.get_stats().await?;
    println!("{}", serde_json::to_string_pretty(&stats)?);

    println!();

    // Create a checkpoint
    let checkpoint_id = agent.checkpoint("Demo conversation completed").await?;
    println!("💾 Created memory checkpoint: {}", checkpoint_id);

    println!();
    println!("🎉 Demo completed successfully!");
    println!();
    println!("Key Features Demonstrated:");
    println!("• 🧠 Prolly tree-based memory persistence");
    println!("• 💬 Conversation history tracking");
    println!("• 📚 Episodic memory of interactions");
    println!("• ⚙️ Procedural knowledge updating");
    println!("• 🤖 Rig framework integration (with fallback)");
    println!("• 📊 Memory statistics and checkpoints");

    if std::env::var("OPENAI_API_KEY").is_err() {
        println!();
        println!("💡 To enable AI-powered responses, set OPENAI_API_KEY environment variable:");
        println!("   export OPENAI_API_KEY=your_api_key_here");
        println!("   cargo run --example agent_rig_demo");
    }

    Ok(())
}
