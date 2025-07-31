use prollytree::agent::*;
use rig::{completion::Prompt, providers::openai::Client};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use tempfile::TempDir;

/// Tools available to the agent, similar to LangGraph example
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentTool {
    WriteToScratchpad {
        notes: String,
    },
    ReadFromScratchpad,
    WebSearch {
        query: String,
    },
    StoreFact {
        category: String,
        fact: String,
    },
    StoreRule {
        rule_name: String,
        condition: String,
        action: String,
    },
    RecallFacts {
        category: String,
    },
    RecallRules,
}

/// Tool execution result
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: AgentTool,
    pub result: String,
}

/// Agent with context offloading capabilities using AgentMemorySystem
pub struct ContextOffloadingAgent {
    memory_system: AgentMemorySystem,
    rig_client: Option<Client>,
    agent_id: String,
    current_thread_id: String,
    namespace: String,
}

impl ContextOffloadingAgent {
    /// Initialize a new agent with persistent memory across threads
    pub async fn new(
        memory_path: &std::path::Path,
        agent_id: String,
        namespace: String,
        openai_api_key: Option<String>,
    ) -> Result<Self, Box<dyn Error>> {
        // Initialize the memory system for cross-thread persistence
        let memory_system = AgentMemorySystem::init(
            memory_path,
            agent_id.clone(),
            Some(Box::new(MockEmbeddingGenerator)),
        )?;

        let rig_client = openai_api_key.map(|key| Client::new(&key));
        let current_thread_id = format!("thread_{}", chrono::Utc::now().timestamp());

        Ok(Self {
            memory_system,
            rig_client,
            agent_id,
            current_thread_id,
            namespace,
        })
    }

    /// Switch to a different conversation thread
    pub fn switch_thread(&mut self, thread_id: String) {
        self.current_thread_id = thread_id;
        println!("🔄 Switched to thread: {}", self.current_thread_id);
    }

    /// Execute a tool with memory persistence
    pub async fn execute_tool(&mut self, tool: AgentTool) -> Result<ToolResult, Box<dyn Error>> {
        match tool {
            AgentTool::WriteToScratchpad { ref notes } => {
                // Store in semantic memory for cross-thread access
                let memory_id = self
                    .memory_system
                    .semantic
                    .store_fact(
                        "scratchpad",    // entity_type
                        &self.namespace, // entity_id (namespace acts as the specific scratchpad ID)
                        json!({
                            "content": notes,
                            "updated_by": self.current_thread_id,
                            "timestamp": chrono::Utc::now()
                        }),
                        1.0, // max confidence
                        &format!("thread_{}", self.current_thread_id),
                    )
                    .await?;

                println!("📝 Wrote to scratchpad (memory_id: {})", memory_id);

                Ok(ToolResult {
                    tool: tool.clone(),
                    result: format!("Wrote to scratchpad: {}", notes),
                })
            }

            AgentTool::ReadFromScratchpad => {
                // Retrieve from semantic memory across threads
                let facts = self
                    .memory_system
                    .semantic
                    .get_entity_facts("scratchpad", &self.namespace)
                    .await?;

                if !facts.is_empty() {
                    // Get the most recent fact (facts are ordered by timestamp)
                    let latest_fact = facts.last().unwrap();

                    // The content is stored as "fact" field in the semantic memory
                    let content = if let Some(fact_value) = latest_fact.content.get("fact") {
                        // The fact field contains our JSON object with "content"
                        if let Some(fact_obj) = fact_value.as_object() {
                            fact_obj
                                .get("content")
                                .and_then(|c| c.as_str())
                                .unwrap_or("No content found in facts object")
                                .to_string()
                        } else if let Some(fact_str) = fact_value.as_str() {
                            // Try parsing it as JSON string
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(fact_str)
                            {
                                parsed
                                    .get("content")
                                    .and_then(|c| c.as_str())
                                    .unwrap_or("No content found in parsed facts")
                                    .to_string()
                            } else {
                                fact_str.to_string()
                            }
                        } else {
                            "Facts field is not in expected format".to_string()
                        }
                    } else {
                        "No facts field found".to_string()
                    };

                    println!("📖 Read from scratchpad: {}", content);

                    Ok(ToolResult {
                        tool,
                        result: format!("Notes from scratchpad: {}", content),
                    })
                } else {
                    println!("📖 No facts found for namespace: {}", self.namespace);
                    Ok(ToolResult {
                        tool,
                        result: "No notes found in scratchpad".to_string(),
                    })
                }
            }

            AgentTool::WebSearch { ref query } => {
                // Simulate web search and store results
                let search_results = format!(
                    "Search results for '{}': Found relevant information about the topic.",
                    query
                );

                // Store search results in episodic memory
                self.memory_system
                    .episodic
                    .store_episode(
                        "search",
                        &format!("Search for: {}", query),
                        json!({
                            "query": query,
                            "results": search_results.clone(),
                            "thread_id": self.current_thread_id
                        }),
                        Some(json!({"success": true})),
                        0.8,
                    )
                    .await?;

                Ok(ToolResult {
                    tool,
                    result: search_results,
                })
            }

            AgentTool::StoreFact {
                ref category,
                ref fact,
            } => {
                // Store as a semantic fact
                let _memory_id = self
                    .memory_system
                    .semantic
                    .store_fact(
                        "research_fact",
                        &format!("{}_{}", self.namespace, category),
                        json!({
                            "category": category,
                            "fact": fact,
                            "stored_by": self.current_thread_id,
                            "timestamp": chrono::Utc::now()
                        }),
                        0.95,
                        &self.current_thread_id,
                    )
                    .await?;

                println!("📚 Stored fact in category '{}': {}", category, fact);

                Ok(ToolResult {
                    tool: tool.clone(),
                    result: format!("Stored fact in {}: {}", category, fact),
                })
            }

            AgentTool::StoreRule {
                ref rule_name,
                ref condition,
                ref action,
            } => {
                // Store as a procedural rule
                self.memory_system
                    .procedural
                    .store_rule(
                        "climate_analysis",
                        rule_name,
                        json!(condition),
                        json!(action),
                        5,
                        true,
                    )
                    .await?;

                println!(
                    "📏 Stored rule '{}': IF {} THEN {}",
                    rule_name, condition, action
                );

                Ok(ToolResult {
                    tool: tool.clone(),
                    result: format!("Stored rule: {}", rule_name),
                })
            }

            AgentTool::RecallFacts { ref category } => {
                // Retrieve facts from semantic memory
                let facts = self
                    .memory_system
                    .semantic
                    .get_entity_facts("research_fact", &format!("{}_{}", self.namespace, category))
                    .await?;

                if !facts.is_empty() {
                    let mut fact_list = Vec::new();
                    for fact in facts.iter() {
                        if let Some(fact_obj) = fact.content.get("fact") {
                            if let Some(fact_data) = fact_obj.as_object() {
                                if let Some(fact_text) =
                                    fact_data.get("fact").and_then(|f| f.as_str())
                                {
                                    fact_list.push(fact_text.to_string());
                                }
                            }
                        }
                    }

                    println!(
                        "📚 Found {} facts in category '{}'",
                        fact_list.len(),
                        category
                    );

                    Ok(ToolResult {
                        tool: tool.clone(),
                        result: if fact_list.is_empty() {
                            format!("No facts found in category: {}", category)
                        } else {
                            format!("Facts in {}: {}", category, fact_list.join("; "))
                        },
                    })
                } else {
                    Ok(ToolResult {
                        tool: tool.clone(),
                        result: format!("No facts found in category: {}", category),
                    })
                }
            }

            AgentTool::RecallRules => {
                // Retrieve rules from procedural memory
                let rules = self
                    .memory_system
                    .procedural
                    .get_active_rules_by_category("climate_analysis")
                    .await?;

                if !rules.is_empty() {
                    let rule_list: Vec<String> = rules
                        .iter()
                        .map(|r| {
                            format!(
                                "{}: {}",
                                r.content
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("Unknown"),
                                r.content
                                    .get("description")
                                    .and_then(|d| d.as_str())
                                    .unwrap_or("")
                            )
                        })
                        .collect();

                    println!("📏 Found {} rules", rule_list.len());

                    Ok(ToolResult {
                        tool,
                        result: format!("Rules: {}", rule_list.join("; ")),
                    })
                } else {
                    Ok(ToolResult {
                        tool,
                        result: "No rules found".to_string(),
                    })
                }
            }
        }
    }

    /// Process a message with tool execution and memory
    pub async fn process_with_tools(&mut self, message: &str) -> Result<String, Box<dyn Error>> {
        // Store the user message in conversation history
        self.memory_system
            .short_term
            .store_conversation_turn(&self.current_thread_id, "user", message, None)
            .await?;

        // Determine which tools to use based on the message
        let tools_to_execute = self.determine_tools(message).await?;

        let mut tool_results = Vec::new();

        // Execute tools
        for tool in tools_to_execute {
            let result = self.execute_tool(tool).await?;
            tool_results.push(result);
        }

        // Generate response based on tool results
        let response = if let Some(ref client) = self.rig_client {
            self.generate_ai_response_with_tools(message, &tool_results, client)
                .await?
        } else {
            self.generate_memory_response_with_tools(message, &tool_results)
                .await?
        };

        // Store assistant response
        self.memory_system
            .short_term
            .store_conversation_turn(&self.current_thread_id, "assistant", &response, None)
            .await?;

        Ok(response)
    }

    /// Use LLM to determine which tools to use based on the message and context
    async fn determine_tools(&self, message: &str) -> Result<Vec<AgentTool>, Box<dyn Error>> {
        // If no LLM client available, fall back to simple parsing
        if self.rig_client.is_none() {
            return self.determine_tools_fallback(message).await;
        }

        let client = self.rig_client.as_ref().unwrap();

        // Get recent conversation context
        let recent_history = self
            .memory_system
            .short_term
            .get_conversation_history(&self.current_thread_id, Some(3))
            .await?;

        let context = recent_history
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
            .join("\n");

        let tool_selection_prompt = format!(
            r#"You are an AI assistant helping with climate research. Based on the user message and conversation context, select the most appropriate tools from the available options.

AVAILABLE TOOLS:
1. WriteToScratchpad - Store persistent notes accessible across conversation threads
2. ReadFromScratchpad - Retrieve notes from the persistent scratchpad
3. WebSearch - Search for information on the web
4. StoreFact - Store factual information in a specific category (hurricanes, heat_waves, flooding, economic, adaptation)
5. StoreRule - Store conditional rules/policies in IF-THEN format
6. RecallFacts - Retrieve stored facts from a specific category
7. RecallRules - Retrieve all stored rules and policies

RECENT CONVERSATION CONTEXT:
{}

USER MESSAGE: "{}"

INSTRUCTIONS:
- Analyze the user's intent and select 1-3 most appropriate tools
- For StoreFact: extract the fact content and category
- For StoreRule: extract rule name, condition, and action
- For WebSearch: extract the search query
- For RecallFacts: determine the appropriate category
- For WriteToScratchpad: extract the content to remember

Respond ONLY with a JSON object in this exact format:
{{"tools": [{{"name": "ToolName", "params": {{"param1": "value1"}}}}], "reasoning": "brief explanation"}}

Examples:
- "Fact: Miami flood defenses cost $400M category: adaptation" → {{"tools": [{{"name": "StoreFact", "params": {{"category": "adaptation", "fact": "Miami flood defenses cost $400M"}}}}], "reasoning": "User wants to store an adaptation fact"}}
- "What hurricanes facts do we have?" → {{"tools": [{{"name": "RecallFacts", "params": {{"category": "hurricanes"}}}}], "reasoning": "User wants to recall hurricane facts"}}
- "Search for Atlanta heat data" → {{"tools": [{{"name": "WebSearch", "params": {{"query": "Atlanta heat data"}}}}], "reasoning": "User wants to search for information"}}"#,
            context, message
        );

        println!("🤖 Using LLM for tool selection...");

        let agent = client
            .agent("gpt-3.5-turbo")
            .preamble(
                "You are a precise tool selection assistant. Always respond with valid JSON only.",
            )
            .max_tokens(300)
            .temperature(0.1)
            .build();

        match agent.prompt(&tool_selection_prompt).await {
            Ok(response) => {
                println!("📋 LLM Response: {}", response.trim());
                self.parse_llm_tool_response(&response).await
            }
            Err(e) => {
                println!("⚠️ LLM tool selection failed: {}, using fallback", e);
                self.determine_tools_fallback(message).await
            }
        }
    }

    /// Parse LLM response and convert to AgentTool instances
    async fn parse_llm_tool_response(
        &self,
        response: &str,
    ) -> Result<Vec<AgentTool>, Box<dyn Error>> {
        let mut tools = Vec::new();

        // Try to parse JSON response
        match serde_json::from_str::<serde_json::Value>(response.trim()) {
            Ok(json) => {
                if let Some(tool_array) = json.get("tools").and_then(|t| t.as_array()) {
                    for tool_obj in tool_array {
                        if let Some(tool_name) = tool_obj.get("name").and_then(|n| n.as_str()) {
                            let default_params = serde_json::json!({});
                            let params = tool_obj.get("params").unwrap_or(&default_params);

                            match tool_name {
                                "WriteToScratchpad" => {
                                    if let Some(notes) =
                                        params.get("notes").and_then(|n| n.as_str())
                                    {
                                        tools.push(AgentTool::WriteToScratchpad {
                                            notes: notes.to_string(),
                                        });
                                    }
                                }
                                "ReadFromScratchpad" => {
                                    tools.push(AgentTool::ReadFromScratchpad);
                                }
                                "WebSearch" => {
                                    if let Some(query) =
                                        params.get("query").and_then(|q| q.as_str())
                                    {
                                        tools.push(AgentTool::WebSearch {
                                            query: query.to_string(),
                                        });
                                    }
                                }
                                "StoreFact" => {
                                    if let (Some(category), Some(fact)) = (
                                        params.get("category").and_then(|c| c.as_str()),
                                        params.get("fact").and_then(|f| f.as_str()),
                                    ) {
                                        tools.push(AgentTool::StoreFact {
                                            category: category.to_string(),
                                            fact: fact.to_string(),
                                        });
                                    }
                                }
                                "StoreRule" => {
                                    if let (Some(rule_name), Some(condition), Some(action)) = (
                                        params.get("rule_name").and_then(|r| r.as_str()),
                                        params.get("condition").and_then(|c| c.as_str()),
                                        params.get("action").and_then(|a| a.as_str()),
                                    ) {
                                        tools.push(AgentTool::StoreRule {
                                            rule_name: rule_name.to_string(),
                                            condition: condition.to_string(),
                                            action: action.to_string(),
                                        });
                                    }
                                }
                                "RecallFacts" => {
                                    if let Some(category) =
                                        params.get("category").and_then(|c| c.as_str())
                                    {
                                        tools.push(AgentTool::RecallFacts {
                                            category: category.to_string(),
                                        });
                                    }
                                }
                                "RecallRules" => {
                                    tools.push(AgentTool::RecallRules);
                                }
                                _ => {
                                    println!("⚠️ Unknown tool name: {}", tool_name);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("⚠️ Failed to parse LLM JSON response: {}", e);
                // Fall back to simple parsing
                return self.determine_tools_fallback(response).await;
            }
        }

        Ok(tools)
    }

    /// Fallback tool determination when LLM is not available or fails
    async fn determine_tools_fallback(
        &self,
        message: &str,
    ) -> Result<Vec<AgentTool>, Box<dyn Error>> {
        let mut tools = Vec::new();
        let message_lower = message.to_lowercase();

        // Simple fallback logic (original string matching approach)
        if message_lower.contains("recall")
            || message_lower.contains("what did")
            || (message_lower.contains("remember") && message_lower.contains("?"))
        {
            tools.push(AgentTool::ReadFromScratchpad);
            return Ok(tools);
        }

        if message_lower.starts_with("please remember:") || message_lower.starts_with("remember:") {
            let content = if let Some(idx) = message.find(':') {
                message[(idx + 1)..].trim().to_string()
            } else {
                message.to_string()
            };
            tools.push(AgentTool::WriteToScratchpad { notes: content });
        }

        if message_lower.contains("fact:") {
            // Extract fact and category
            let fact_part = message.split("fact:").nth(1).unwrap_or("").trim();
            let (fact, category) = if let Some(cat_idx) = fact_part.to_lowercase().find("category:")
            {
                let fact_text = fact_part[..cat_idx].trim();
                let category_text = fact_part[cat_idx + 9..]
                    .trim()
                    .split_whitespace()
                    .next()
                    .unwrap_or("general");
                (fact_text.to_string(), category_text.to_string())
            } else {
                (fact_part.to_string(), "general".to_string())
            };
            tools.push(AgentTool::StoreFact { category, fact });
        }

        if message_lower.contains("search") {
            let query = message
                .split_whitespace()
                .skip_while(|&w| !w.to_lowercase().contains("search"))
                .skip(1)
                .collect::<Vec<_>>()
                .join(" ");
            if !query.is_empty() {
                tools.push(AgentTool::WebSearch { query });
            }
        }

        Ok(tools)
    }

    /// Generate AI response with tool results
    async fn generate_ai_response_with_tools(
        &self,
        message: &str,
        tool_results: &[ToolResult],
        client: &Client,
    ) -> Result<String, Box<dyn Error>> {
        let tool_context = tool_results
            .iter()
            .map(|tr| format!("Tool: {:?}\nResult: {}", tr.tool, tr.result))
            .collect::<Vec<_>>()
            .join("\n\n");

        let prompt = format!(
            "You are an intelligent assistant with access to various tools and persistent memory.\n\n\
             USER MESSAGE: {}\n\n\
             TOOL EXECUTION RESULTS:\n{}\n\n\
             Based on the tool results, provide a helpful response to the user.",
            message, tool_context
        );

        let agent = client
            .agent("gpt-3.5-turbo")
            .preamble("You are a helpful assistant that uses tools and memory to provide accurate responses.")
            .max_tokens(200)
            .temperature(0.7)
            .build();

        match agent.prompt(&prompt).await {
            Ok(response) => Ok(response.trim().to_string()),
            Err(e) => {
                println!("⚠️ AI generation failed: {}", e);
                self.generate_memory_response_with_tools(message, tool_results)
                    .await
            }
        }
    }

    /// Generate memory-based response with tool results
    async fn generate_memory_response_with_tools(
        &self,
        _message: &str,
        tool_results: &[ToolResult],
    ) -> Result<String, Box<dyn Error>> {
        if tool_results.is_empty() {
            return Ok("I've processed your request. How can I help you further?".to_string());
        }

        let response = tool_results
            .iter()
            .map(|tr| tr.result.clone())
            .collect::<Vec<_>>()
            .join(" ");

        Ok(response)
    }

    /// Get statistics about the agent's memory
    pub async fn get_memory_stats(&self) -> Result<serde_json::Value, Box<dyn Error>> {
        let stats = self.memory_system.get_system_stats().await?;
        Ok(json!({
            "agent_id": self.agent_id,
            "current_thread": self.current_thread_id,
            "namespace": self.namespace,
            "memory_stats": stats
        }))
    }

    /// Demonstrate versioned storage benefits - Time Travel Debugging
    pub async fn demonstrate_time_travel_debugging(&mut self) -> Result<(), Box<dyn Error>> {
        println!("🕰️  VERSIONED STORAGE DEMO: Time Travel Debugging");
        println!("{}", "═".repeat(60));

        // Store initial version of a fact
        println!("📝 Storing initial hurricane data...");
        self.execute_tool(AgentTool::StoreFact {
            category: "hurricanes".to_string(),
            fact: "Hurricane frequency: 10 per year average (1990-2000)".to_string(),
        })
        .await?;

        let checkpoint_v1 = self
            .memory_system
            .checkpoint("Initial hurricane data")
            .await?;
        println!("💾 Checkpoint V1 created: {}", checkpoint_v1);

        // Update the fact (new version)
        println!("\n📝 Updating hurricane data with new research...");
        self.execute_tool(AgentTool::StoreFact {
            category: "hurricanes".to_string(),
            fact: "Hurricane frequency: 15 per year average (2010-2020) - 50% increase".to_string(),
        })
        .await?;

        let checkpoint_v2 = self
            .memory_system
            .checkpoint("Updated hurricane frequency data")
            .await?;
        println!("💾 Checkpoint V2 created: {}", checkpoint_v2);

        // Further update (another version)
        println!("\n📝 Adding more recent data...");
        self.execute_tool(AgentTool::StoreFact {
            category: "hurricanes".to_string(),
            fact: "Hurricane frequency: 18 per year average (2020-2024) - continued acceleration"
                .to_string(),
        })
        .await?;

        let checkpoint_v3 = self
            .memory_system
            .checkpoint("Latest hurricane acceleration data")
            .await?;
        println!("💾 Checkpoint V3 created: {}", checkpoint_v3);

        // Demonstrate current state
        println!("\n🔍 Current memory state:");
        let current_facts = self
            .execute_tool(AgentTool::RecallFacts {
                category: "hurricanes".to_string(),
            })
            .await?;
        println!("   {}", current_facts.result);

        println!("\n🕰️  DEMONSTRATING TIME TRAVEL NAVIGATION:");
        println!("   Let's travel back to V1 and see the difference...");

        // Time travel to V1
        match self.memory_system.rollback(&checkpoint_v1).await {
            Ok(()) => {
                println!("   ✅ Time traveled to checkpoint V1");
                let v1_facts = self
                    .execute_tool(AgentTool::RecallFacts {
                        category: "hurricanes".to_string(),
                    })
                    .await?;
                println!("   📊 V1 State: {}", v1_facts.result);

                // Time travel to V3 (most recent)
                self.memory_system.rollback(&checkpoint_v3).await?;
                println!("   ✅ Time traveled back to checkpoint V3 (latest)");
                let v3_facts = self
                    .execute_tool(AgentTool::RecallFacts {
                        category: "hurricanes".to_string(),
                    })
                    .await?;
                println!("   📊 V3 State: {}", v3_facts.result);

                println!("\n   🎯 TIME TRAVEL COMPLETE: Successfully navigated between versions!");
            }
            Err(e) => {
                println!("   ⚠️  Time travel demonstration limited: {}", e);
            }
        }

        println!("\n✨ BENEFITS: With versioned storage, we can:");
        println!("   • 🕰️  Time Travel: Navigate to any historical version instantly");
        println!("   • 📈 View Evolution: Complete progression of data understanding");
        println!("   • 🐛 Debug Issues: Identify exactly when incorrect data was introduced");
        println!("   • 🔄 Safe Rollback: Restore to any previous version with confidence");
        println!("   • 📋 Audit Trail: Immutable record of data provenance and decisions");

        Ok(())
    }

    /// Demonstrate memory evolution tracking
    pub async fn demonstrate_memory_evolution(&mut self) -> Result<(), Box<dyn Error>> {
        println!("\n🧬 VERSIONED STORAGE DEMO: Memory Evolution Tracking");
        println!("{}", "═".repeat(60));

        // Simulate agent learning process with evolving understanding
        let learning_stages = vec![
            ("Initial Research", "Fact: Sea level rise: 3mm per year globally category: flooding"),
            ("Regional Analysis", "Fact: Sea level rise: 5mm per year in Southeast US coast category: flooding"),
            ("Accelerated Understanding", "Fact: Sea level rise: 8mm per year in Miami area due to land subsidence category: flooding"),
            ("Comprehensive Data", "Fact: Sea level rise: 12mm per year in Norfolk, VA - highest US rate category: flooding"),
        ];

        let mut checkpoints = Vec::new();

        for (stage, fact_data) in learning_stages {
            println!("\n📚 Learning Stage: {}", stage);

            // Parse the fact format
            if let Some(fact_start) = fact_data.find("Fact: ") {
                if let Some(category_start) = fact_data.find(" category: ") {
                    let fact = fact_data[fact_start + 6..category_start].trim();
                    let category = fact_data[category_start + 11..].trim();

                    self.execute_tool(AgentTool::StoreFact {
                        category: category.to_string(),
                        fact: fact.to_string(),
                    })
                    .await?;
                }
            }

            let checkpoint = self
                .memory_system
                .checkpoint(&format!("Learning stage: {}", stage))
                .await?;
            checkpoints.push((stage, checkpoint));
            println!(
                "💾 Checkpoint: {} ({})",
                stage,
                checkpoints.last().unwrap().1
            );
        }

        // Show evolution benefits
        println!("\n🔍 Final memory state - Flooding facts:");
        let final_facts = self
            .execute_tool(AgentTool::RecallFacts {
                category: "flooding".to_string(),
            })
            .await?;
        println!("   {}", final_facts.result);

        println!("\n✨ BENEFITS of Memory Evolution Tracking:");
        println!("   • 📈 Track how agent's understanding becomes more sophisticated");
        println!("   • 🐛 Debug when agent learned incorrect information");
        println!("   • 🔄 Revert to previous knowledge state if corruption occurs");
        println!("   • 📊 Analyze learning patterns to optimize agent training");
        println!("   • 🔍 Audit trail shows knowledge refinement process");

        Ok(())
    }

    /// Demonstrate audit trail and transparency features
    pub async fn demonstrate_audit_trail(&mut self) -> Result<(), Box<dyn Error>> {
        println!("\n📋 VERSIONED STORAGE DEMO: Audit Trail & Transparency");
        println!("{}", "═".repeat(60));

        // Simulate a decision-making process with full audit trail
        println!(
            "🎯 Decision Scenario: Should we recommend evacuation for a Category 2 hurricane?"
        );

        // Store decision-making rule
        println!("\n📏 Storing evacuation policy...");
        self.execute_tool(AgentTool::StoreRule {
            rule_name: "cat2_evacuation".to_string(),
            condition: "hurricane category = 2 AND storm_surge > 6ft".to_string(),
            action: "recommend voluntary evacuation for coastal areas".to_string(),
        })
        .await?;

        let policy_checkpoint = self
            .memory_system
            .checkpoint("Initial Cat 2 evacuation policy")
            .await?;
        println!("💾 Policy Checkpoint: {}", policy_checkpoint);

        // Store supporting fact
        println!("\n📚 Adding supporting hurricane damage data...");
        self.execute_tool(AgentTool::StoreFact {
            category: "hurricanes".to_string(),
            fact: "Category 2 hurricanes with >6ft storm surge caused average $2.3B damage in Southeast US".to_string(),
        }).await?;

        let data_checkpoint = self
            .memory_system
            .checkpoint("Supporting damage data for Cat 2 policy")
            .await?;
        println!("💾 Data Checkpoint: {}", data_checkpoint);

        // Demonstrate rule application
        println!("\n🤔 Applying decision logic...");
        let rules = self.execute_tool(AgentTool::RecallRules).await?;
        println!("📏 Available rules: {}", rules.result);

        let facts = self
            .execute_tool(AgentTool::RecallFacts {
                category: "hurricanes".to_string(),
            })
            .await?;
        println!("📚 Supporting facts: {}", facts.result);

        // Final decision checkpoint
        let decision_checkpoint = self
            .memory_system
            .checkpoint("Cat 2 evacuation decision made with full audit trail")
            .await?;
        println!("\n✅ Decision Checkpoint: {}", decision_checkpoint);

        println!("\n✨ AUDIT TRAIL BENEFITS:");
        println!("   • 🔍 Complete Decision History: Every rule, fact, and checkpoint traced");
        println!("   • ⚖️  Compliance Ready: Immutable record of AI decision process");
        println!("   • 🛡️  Accountability: Can prove what data influenced each decision");
        println!("   • 🔬 Post-Mortem Analysis: Full history available for incident investigation");
        println!("   • 📝 Explainable AI: Can reconstruct reasoning chain for any decision");
        println!(
            "   • 🕵️  Bias Detection: Historical patterns reveal potential biases in data/rules"
        );

        Ok(())
    }

    /// Demonstrate rollback capabilities for error recovery
    pub async fn demonstrate_rollback_recovery(&mut self) -> Result<(), Box<dyn Error>> {
        println!("\n🔄 VERSIONED STORAGE DEMO: Rollback & Error Recovery");
        println!("{}", "═".repeat(60));

        // Simulate good state
        println!("📝 Establishing good baseline state...");
        self.execute_tool(AgentTool::StoreFact {
            category: "economic".to_string(),
            fact: "Climate change costs US economy $150B annually (EPA 2023)".to_string(),
        })
        .await?;

        let good_checkpoint = self
            .memory_system
            .checkpoint("Good economic baseline data")
            .await?;
        println!("💾 Good State Checkpoint: {}", good_checkpoint);

        // Simulate corruption/bad data
        println!("\n⚠️  Simulating data corruption or bad input...");
        self.execute_tool(AgentTool::StoreFact {
            category: "economic".to_string(),
            fact:
                "Climate change saves money!!! Only costs $5 per year total!!! [CLEARLY WRONG DATA]"
                    .to_string(),
        })
        .await?;

        println!("📊 Current corrupted state:");
        let corrupted_facts = self
            .execute_tool(AgentTool::RecallFacts {
                category: "economic".to_string(),
            })
            .await?;
        println!("   {}", corrupted_facts.result);

        // NOW IMPLEMENTING ACTUAL ROLLBACK FUNCTIONALITY!
        println!("\n🔄 PERFORMING ACTUAL ROLLBACK:");
        println!("   1. ✅ Detected bad data: corrupted economic facts");
        println!(
            "   2. ✅ Identified last good checkpoint: {}",
            good_checkpoint
        );
        println!("   3. 🔄 Executing rollback to restore memory state...");

        match self.memory_system.rollback(&good_checkpoint).await {
            Ok(()) => {
                println!("   4. ✅ ROLLBACK SUCCESSFUL! Memory restored to good state");

                // Verify the rollback worked
                println!("\n📊 Verifying rollback - Current economic facts:");
                let restored_facts = self
                    .execute_tool(AgentTool::RecallFacts {
                        category: "economic".to_string(),
                    })
                    .await?;
                println!("   {}", restored_facts.result);

                println!("\n🎉 ROLLBACK VERIFICATION:");
                if restored_facts.result.contains("$150B annually")
                    && !restored_facts.result.contains("$5 per year")
                {
                    println!("   ✅ SUCCESS: Bad data removed, good data restored!");
                    println!("   ✅ Memory integrity fully recovered");
                } else {
                    println!("   ⚠️  Partial success - manual verification may be needed");
                }
            }
            Err(e) => {
                println!("   ❌ Rollback failed: {}", e);
                println!("   🔧 Fallback: Manual data correction would be required");
            }
        }

        println!("\n✨ ERROR RECOVERY BENEFITS:");
        println!(
            "   • 🛡️  Data Integrity: Corrupted memory can be restored to any previous good state"
        );
        println!("   • ⚡ Rapid Recovery: Instant rollback vs. manual data reconstruction");
        println!("   • 🔒 Safety Net: Experiment with confidence knowing you can always revert");
        println!("   • 📊 Impact Analysis: Compare before/after states to assess damage");
        println!("   • 🔍 Root Cause: Version history reveals when/how corruption occurred");

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🧠 Context Offloading Agent Demo (Rig + ProllyTree)");
    println!("==================================================");
    println!();

    // Create a temporary directory for the demo
    let temp_dir = TempDir::new()?;
    let memory_path = temp_dir.path();

    // Initialize agent
    let mut agent = ContextOffloadingAgent::new(
        memory_path,
        "context_agent_001".to_string(),
        "research_project".to_string(),
        std::env::var("OPENAI_API_KEY").ok(),
    )
    .await?;

    println!("✅ Agent initialized with namespace: research_project");
    println!();

    // Thread 1: Initial research
    println!("📋 Thread 1: Starting research project - Initial Data Collection");
    println!("{}", "─".repeat(60));

    let thread1_messages = vec![
        "Please remember: Research project on the impact of extreme weather on southeast US due to climate change. Key areas to track: hurricane intensity trends, flooding patterns, heat wave frequency, economic impacts on agriculture and infrastructure, and adaptation strategies being implemented.",

        "Search for recent data on hurricane damage costs in Florida and Georgia",

        "Fact: Hurricane Ian (2022) caused over $112 billion in damages, making it the costliest natural disaster in Florida's history category: hurricanes",

        "Fact: Category 4 and 5 hurricanes have increased by 25% in the Southeast US since 1980 category: hurricanes",

        "Rule: hurricane_evacuation: IF hurricane category >= 3 AND distance_from_coast < 10_miles THEN mandatory evacuation required",

        "Search for heat wave data in major southeast cities",

        "Fact: Atlanta experienced 35 days above 95°F in 2023, compared to an average of 15 days in the 1990s category: heat_waves",

        "Fact: Heat-related hospitalizations in Southeast US cities have increased by 43% between 2010-2023 category: heat_waves",

        "Rule: heat_advisory: IF temperature > 95F AND heat_index > 105F THEN issue heat advisory and open cooling centers",

        "Search for flooding impact on agriculture in Mississippi Delta",

        "Fact: 2019 Mississippi River flooding caused $6.2 billion in agricultural losses across Arkansas, Mississippi, and Louisiana category: flooding",

        "Rule: flood_insurance: IF property in 100-year floodplain THEN require federal flood insurance for mortgages",
    ];

    for message in thread1_messages {
        println!("💬 User: {}", message);
        let response = agent.process_with_tools(message).await?;
        println!("🤖 Assistant: {}", response);
        println!();
    }

    // Create checkpoint
    let checkpoint1 = agent
        .memory_system
        .checkpoint("Thread 1 research complete")
        .await?;
    println!("💾 Created checkpoint: {}", checkpoint1);
    println!();

    // Thread 2: Continue research in new thread
    agent.switch_thread("thread_002".to_string());

    println!("📋 Thread 2: Analysis and Pattern Recognition");
    println!("{}", "─".repeat(60));

    let thread2_messages = vec![
        "What did I ask you to remember about my research project?",

        "What facts do we have about hurricanes?",

        "Search for information about heat wave trends in Atlanta and Charlotte over the past decade",

        "Fact: Charlotte's urban heat island effect amplifies temperatures by 5-8°F compared to surrounding areas category: heat_waves",

        "What rules have we established so far?",

        "Rule: agricultural_drought_response: IF rainfall < 50% of normal for 60 days AND crop_stage = critical THEN implement emergency irrigation protocols",

        "Fact: Southeast US coastal property insurance premiums have increased 300% since 2010 due to climate risks category: economic",

        "Search for successful climate adaptation strategies in Miami",

        "Fact: Miami Beach's $400 million stormwater pump system has reduced flooding events by 85% since 2015 category: adaptation",

        "Rule: building_codes: IF new_construction AND flood_zone THEN require elevation minimum 3 feet above base flood elevation",

        "What facts do we have about economic impacts?",
    ];

    for message in thread2_messages {
        println!("💬 User: {}", message);
        let response = agent.process_with_tools(message).await?;
        println!("🤖 Assistant: {}", response);
        println!();
    }

    // Show memory statistics
    println!("📊 Memory Statistics:");
    println!("{}", "═".repeat(50));
    let stats = agent.get_memory_stats().await?;
    println!("{}", serde_json::to_string_pretty(&stats)?);
    println!();

    // Thread 3: Demonstrate persistence and synthesis
    agent.switch_thread("thread_003".to_string());

    println!("📋 Thread 3: Synthesis and Policy Recommendations");
    println!("{}", "─".repeat(60));

    let thread3_messages = vec![
        "Can you recall what research topics I asked you to track?",

        "What facts do we have about heat waves?",

        "Fact: Federal disaster declarations for heat waves have increased 600% in Southeast US since 2000 category: heat_waves",

        "What are all the rules we've established for climate response?",

        "Fact: Georgia's agricultural sector lost $2.5 billion in 2022 due to extreme weather events category: economic",

        "Rule: infrastructure_resilience: IF critical_infrastructure AND climate_risk_score > 7 THEN require climate resilience assessment and upgrade plan",

        "Search for green infrastructure solutions for urban flooding",

        "Fact: Green infrastructure projects in Atlanta reduced stormwater runoff by 40% and provided $85 million in ecosystem services category: adaptation",

        "What facts have we collected about flooding?",

        "Rule: emergency_response: IF rainfall > 6_inches_24hr OR wind_speed > 75mph THEN activate emergency operations center",

        "Fact: Southeast US has experienced a 40% increase in extreme precipitation events (>3 inches in 24hr) since 1950 category: flooding",

        "What economic impact facts do we have across all categories?",
    ];

    for message in thread3_messages {
        println!("💬 User: {}", message);
        let response = agent.process_with_tools(message).await?;
        println!("🤖 Assistant: {}", response);
        println!();

        // Small delay for readability
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    }

    // Final statistics
    println!("📊 Final Memory Statistics:");
    println!("{}", "═".repeat(50));
    let final_stats = agent.get_memory_stats().await?;
    println!("{}", serde_json::to_string_pretty(&final_stats)?);
    println!();

    // VERSIONED STORAGE BENEFIT DEMONSTRATIONS
    println!("\n");
    println!("🚀 PROLLY TREE VERSIONED STORAGE ADVANTAGES");
    println!("{}", "█".repeat(80));
    println!("Demonstrating benefits that set ProllyTree apart from traditional approaches");
    println!("{}", "█".repeat(80));

    // Time Travel Debugging
    agent.demonstrate_time_travel_debugging().await?;

    // Memory Evolution Tracking
    agent.demonstrate_memory_evolution().await?;

    // Audit Trail & Transparency
    agent.demonstrate_audit_trail().await?;

    // Rollback & Error Recovery
    agent.demonstrate_rollback_recovery().await?;

    println!("🎉 Demo completed successfully!");
    println!();
    println!("Key Features Demonstrated:");
    println!("• 📝 Persistent scratchpad across threads (like LangGraph's memory store)");
    println!(
        "• 📚 Semantic fact storage with categories (hurricanes, heat_waves, flooding, economic)"
    );
    println!("• 📏 Procedural rule storage for climate response policies");
    println!("• 🔄 Context offloading between conversation threads");
    println!("• 🛠️ Seven different tools for comprehensive memory operations");
    println!("• 🧠 Cross-thread memory access for facts, rules, and scratchpad");
    println!("• 📊 Memory checkpointing and detailed statistics");
    println!("• 🔍 Category-based fact retrieval and rule management");
    println!("• 💾 Persistent storage of complex research data across sessions");
    println!("• 🤖 LLM-based intelligent tool selection with graceful fallback");
    println!();
    println!("🚀 VERSIONED STORAGE ADVANTAGES:");
    println!("• 🕰️  Time Travel Debugging: View complete evolution of data and decisions");
    println!("• 🧬 Memory Evolution Tracking: Monitor how agent knowledge becomes sophisticated");
    println!("• 📋 Audit Trail & Transparency: Immutable record for compliance and explainability");
    println!("• 🔄 Rollback & Error Recovery: Restore to any previous good state instantly");
    println!("• 🔍 Root Cause Analysis: Identify exactly when/how issues were introduced");
    println!("• ⚖️  Compliance Ready: Complete decision history for regulatory requirements");
    println!("• 🛡️  Data Integrity: Protection against corruption with instant recovery");
    println!("• 📈 Performance Optimization: Analyze patterns to improve agent efficiency");
    println!();

    if std::env::var("OPENAI_API_KEY").is_err() {
        println!("💡 Note: Running in memory-only mode. To enable AI responses:");
        println!("   export OPENAI_API_KEY=your_api_key_here");
        println!("   cargo run --example agent_context_offloading");
    }

    Ok(())
}
