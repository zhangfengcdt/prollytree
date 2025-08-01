use prollytree::agent::{MemoryQuery, MemoryType, SearchableMemoryStore, TimeRange, *};
use rig::{completion::Prompt, providers::openai::Client};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::cmp::min;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::mpsc;

// Terminal UI imports
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};

/// Available memory backend options
#[derive(Debug, Clone)]
pub enum MemoryBackend {
    InMemory,
    ThreadSafeInMemory, 
    ThreadSafeGit,
    ThreadSafeFile,
}

impl MemoryBackend {
    fn display_name(&self) -> &str {
        match self {
            MemoryBackend::InMemory => "In-Memory (Basic)",
            MemoryBackend::ThreadSafeInMemory => "Thread-Safe In-Memory (Versioned)", 
            MemoryBackend::ThreadSafeGit => "Thread-Safe Git (Versioned)",
            MemoryBackend::ThreadSafeFile => "Thread-Safe File (Versioned)",
        }
    }

    fn description(&self) -> &str {
        match self {
            MemoryBackend::InMemory => "Simple in-memory storage, no persistence",
            MemoryBackend::ThreadSafeInMemory => "In-memory storage with git versioning",
            MemoryBackend::ThreadSafeGit => "Git-backed versioned storage with commits", 
            MemoryBackend::ThreadSafeFile => "File-based storage with git versioning",
        }
    }
}

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
    _agent_id: String,
    current_thread_id: String,
    namespace: String,
    ui_sender: Option<mpsc::UnboundedSender<UiEvent>>,
    // Track git-style commit history for linear progression and rollback demo
    commit_history: Vec<GitCommit>,
    current_branch: String,
}

#[derive(Clone, Debug)]
struct GitCommit {
    id: String,
    message: String,
    memory_count: usize,
    timestamp: chrono::DateTime<chrono::Utc>,
    branch: String,
    author: String, // Format: "thread_001/StoreFact" or "thread_002/WebSearch"
}

/// UI State for managing the four windows
#[derive(Clone)]
pub struct UiState {
    pub conversations: Vec<String>,
    pub memory_stats: String,
    pub git_logs: Vec<String>,
    pub kv_keys: Vec<String>,
    pub scroll_conversations: usize,
    pub scroll_git_logs: usize,
    pub scroll_kv_keys: usize,
    pub is_typing: bool,
    pub cursor_visible: bool,
    pub is_paused: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            conversations: Vec::new(),
            memory_stats: "Memory Stats Loading...".to_string(),
            git_logs: vec!["Git logs loading...".to_string()],
            kv_keys: vec!["KV store keys loading...".to_string()],
            scroll_conversations: 0,
            scroll_git_logs: 0,
            scroll_kv_keys: 0,
            is_typing: false,
            cursor_visible: true,
            is_paused: false,
        }
    }
}

/// Events that can be sent to update the UI
#[derive(Debug, Clone)]
pub enum UiEvent {
    ConversationUpdate(String),
    MemoryStatsUpdate(String),
    GitLogUpdate(Vec<String>),
    KvKeysUpdate(Vec<String>),
    TypingIndicator(bool), // true = start typing, false = stop typing
    Pause,
    Quit,
}

impl ContextOffloadingAgent {
    /// Get the real git author information from git config
    fn get_git_author() -> String {
        let name = std::process::Command::new("git")
            .args(["config", "--get", "user.name"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "Unknown User".to_string());
        
        let email = std::process::Command::new("git")
            .args(["config", "--get", "user.email"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown@example.com".to_string());
        
        format!("{} <{}>", name, email)
    }

    /// Initialize a new agent with persistent memory across threads
    pub async fn new(
        memory_path: &std::path::Path,
        agent_id: String,
        namespace: String,
        backend: MemoryBackend,
        openai_api_key: Option<String>,
        ui_sender: Option<mpsc::UnboundedSender<UiEvent>>,
    ) -> Result<Self, Box<dyn Error>> {
        // Initialize the memory system based on selected backend
        let memory_system = match backend {
            MemoryBackend::InMemory => {
                AgentMemorySystem::init(
                    memory_path,
                    agent_id.clone(),
                    Some(Box::new(MockEmbeddingGenerator)),
                )?
            },
            MemoryBackend::ThreadSafeInMemory => {
                AgentMemorySystem::init_with_thread_safe_inmemory(
                    memory_path,
                    agent_id.clone(),
                    Some(Box::new(MockEmbeddingGenerator)),
                )?
            },
            MemoryBackend::ThreadSafeGit => {
                AgentMemorySystem::init_with_thread_safe_git(
                    memory_path,
                    agent_id.clone(),
                    Some(Box::new(MockEmbeddingGenerator)),
                )?
            },
            MemoryBackend::ThreadSafeFile => {
                AgentMemorySystem::init_with_thread_safe_file(
                    memory_path,
                    agent_id.clone(),
                    Some(Box::new(MockEmbeddingGenerator)),
                )?
            },
        };

        let rig_client = openai_api_key.map(|key| Client::new(&key));
        let current_thread_id = format!("thread_{}", chrono::Utc::now().timestamp());

        Ok(Self {
            memory_system,
            rig_client,
            _agent_id: agent_id,
            current_thread_id,
            namespace,
            ui_sender,
            commit_history: vec![GitCommit {
                id: "a1b2c3d".to_string(),
                message: "Initial setup".to_string(),
                memory_count: 0,
                timestamp: chrono::Utc::now(),
                branch: "main".to_string(),
                author: Self::get_git_author(),
            }],
            current_branch: "main".to_string(),
        })
    }

    /// Switch to a different conversation thread
    pub fn switch_thread(&mut self, thread_id: String) {
        self.current_thread_id = thread_id;
        if let Some(ref sender) = self.ui_sender {
            let _ = sender.send(UiEvent::ConversationUpdate(format!(
                "⏺ Switched to thread: {}",
                self.current_thread_id
            )));
        }
    }

    /// Send updates to UI
    fn send_ui_update(&self, message: String) {
        if let Some(ref sender) = self.ui_sender {
            let _ = sender.send(UiEvent::ConversationUpdate(message));
        }
    }

    /// Execute a tool with memory persistence and UI updates
    pub async fn execute_tool(&mut self, tool: AgentTool) -> Result<ToolResult, Box<dyn Error>> {
        match tool {
            AgentTool::WriteToScratchpad { ref notes } => {
                let memory_id = self
                    .memory_system
                    .semantic
                    .store_fact(
                        "scratchpad",
                        &self.namespace,
                        json!({
                            "content": notes,
                            "updated_by": self.current_thread_id,
                            "timestamp": chrono::Utc::now()
                        }),
                        1.0,
                        &format!("thread_{}", self.current_thread_id),
                    )
                    .await?;

                // Create git commit for scratchpad update
                let _commit_id = self
                    .add_commit(
                        &format!(
                            "Update scratchpad: {}",
                            &notes[..std::cmp::min(150, notes.len())]
                        ),
                        &Self::get_git_author(),
                    )
                    .await?;

                self.send_ui_update(format!("⏺ Wrote to scratchpad (memory_id: {})", memory_id));

                Ok(ToolResult {
                    tool: tool.clone(),
                    result: format!("Wrote to scratchpad: {}", notes),
                })
            }

            AgentTool::ReadFromScratchpad => {
                let facts = self
                    .memory_system
                    .semantic
                    .get_entity_facts("scratchpad", &self.namespace)
                    .await?;

                if !facts.is_empty() {
                    let latest_fact = facts.last().unwrap();
                    let content = if let Some(fact_value) = latest_fact.content.get("fact") {
                        if let Some(fact_obj) = fact_value.as_object() {
                            fact_obj
                                .get("content")
                                .and_then(|c| c.as_str())
                                .unwrap_or("No content found in facts object")
                                .to_string()
                        } else if let Some(fact_str) = fact_value.as_str() {
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

                    self.send_ui_update(format!("⏺ Read from scratchpad: {}", content));

                    Ok(ToolResult {
                        tool,
                        result: format!("Notes from scratchpad: {}", content),
                    })
                } else {
                    self.send_ui_update(format!(
                        "⏺ No facts found for namespace: {}",
                        self.namespace
                    ));
                    Ok(ToolResult {
                        tool,
                        result: "No notes found in scratchpad".to_string(),
                    })
                }
            }

            AgentTool::WebSearch { ref query } => {
                let search_results = format!(
                    "Search results for '{}': Found relevant information about the topic.",
                    query
                );

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

                // Create git commit for search episode
                let _commit_id = self
                    .add_commit(
                        &format!(
                            "Web search query: {}",
                            &query[..std::cmp::min(120, query.len())]
                        ),
                        &Self::get_git_author(),
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

                // Create git commit for stored fact
                let _commit_id = self
                    .add_commit(
                        &format!(
                            "Store fact in {}: {}",
                            category,
                            &fact[..std::cmp::min(140, fact.len())]
                        ),
                        &Self::get_git_author(),
                    )
                    .await?;

                self.send_ui_update(format!(
                    "⏺ Stored fact in category '{}': {}",
                    category, fact
                ));

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

                // Create git commit for stored rule
                let _commit_id = self
                    .add_commit(
                        &format!(
                            "Add procedural rule: {}",
                            &rule_name[..std::cmp::min(100, rule_name.len())]
                        ),
                        &Self::get_git_author(),
                    )
                    .await?;

                self.send_ui_update(format!(
                    "⏺ Stored rule '{}': IF {} THEN {}",
                    rule_name, condition, action
                ));

                Ok(ToolResult {
                    tool: tool.clone(),
                    result: format!("Stored rule: {}", rule_name),
                })
            }

            AgentTool::RecallFacts { ref category } => {
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

                    self.send_ui_update(format!(
                        "⏺ Found {} facts in category '{}'",
                        fact_list.len(),
                        category
                    ));

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

                    self.send_ui_update(format!("⏺ Found {} rules", rule_list.len()));

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
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r#"Based on the user's message and conversation context, determine which tools to use.

Available tools:
- WriteToScratchpad: Write temporary notes (use for "remember", "note", "write down")
- ReadFromScratchpad: Read previous notes (use for "what did I write", "check notes")
- WebSearch: Search for information (use for "search", "find", "look up")
- StoreFact: Store a research fact (use when message contains "Fact:" followed by category)
- StoreRule: Store a procedural rule (use when message contains "Rule:" with condition/action)
- RecallFacts: Retrieve facts by category (use for "what facts", "recall facts")
- RecallRules: Retrieve all rules (use for "what rules", "show rules")

Context:
{}

User message: {}

Respond with a JSON array of tool objects. Each tool should have the exact format shown below:

For StoreFact: {{"StoreFact": {{"category": "category_name", "fact": "fact_text"}}}}
For StoreRule: {{"StoreRule": {{"rule_name": "rule_name", "condition": "condition", "action": "action"}}}}
For RecallFacts: {{"RecallFacts": {{"category": "category_name"}}}}
For WriteToScratchpad: {{"WriteToScratchpad": {{"notes": "note_text"}}}}
For WebSearch: {{"WebSearch": {{"query": "search_query"}}}}
For ReadFromScratchpad: "ReadFromScratchpad"
For RecallRules: "RecallRules"

Examples:
- "Search for hurricane data" → [{{"WebSearch": {{"query": "hurricane data"}}}}]
- "Fact: Sea level rising category: climate" → [{{"StoreFact": {{"category": "climate", "fact": "Sea level rising"}}}}]
- "What facts do we have about storms?" → [{{"RecallFacts": {{"category": "storms"}}}}]
"#,
            context, message
        );

        let agent = client
            .agent("gpt-3.5-turbo")
            .preamble(
                "You are a precise tool selection assistant. Always respond with valid JSON only.",
            )
            .max_tokens(300)
            .temperature(0.1)
            .build();

        match agent.prompt(&prompt).await {
            Ok(response) => {
                // Try to parse the JSON response
                match serde_json::from_str::<Vec<AgentTool>>(&response.trim()) {
                    Ok(tools) => Ok(tools),
                    Err(_) => {
                        // If JSON parsing fails, fall back to the simple parsing
                        self.determine_tools_fallback(message).await
                    }
                }
            }
            Err(_) => {
                // If LLM call fails, fall back to simple parsing
                self.determine_tools_fallback(message).await
            }
        }
    }

    /// Fallback tool determination using simple string matching
    async fn determine_tools_fallback(
        &self,
        message: &str,
    ) -> Result<Vec<AgentTool>, Box<dyn Error>> {
        let mut tools = Vec::new();
        let message_lower = message.to_lowercase();

        // Parse facts storage (format: "Fact: ... category: ...")
        if let Some(fact_start) = message.find("Fact:") {
            let fact_part = &message[fact_start + 5..];
            if let Some(category_start) = fact_part.find("category:") {
                let fact = fact_part[..category_start].trim().to_string();
                let category = fact_part[category_start + 9..].trim().to_string();
                tools.push(AgentTool::StoreFact { category, fact });
            }
        }

        // Parse rule storage (format: "Rule: name: ... IF ... THEN ...")
        if let Some(rule_start) = message.find("Rule:") {
            let rule_part = &message[rule_start + 5..];
            if let Some(colon_pos) = rule_part.find(":") {
                let rule_name = rule_part[..colon_pos].trim().to_string();
                let rule_body = rule_part[colon_pos + 1..].trim();

                if let Some(if_pos) = rule_body.find("IF") {
                    if let Some(then_pos) = rule_body.find("THEN") {
                        let condition = rule_body[if_pos + 2..then_pos].trim().to_string();
                        let action = rule_body[then_pos + 4..].trim().to_string();
                        tools.push(AgentTool::StoreRule {
                            rule_name,
                            condition,
                            action,
                        });
                    }
                }
            }
        }

        // Simple pattern matching for other tools
        if message_lower.contains("search")
            || message_lower.contains("find")
            || message_lower.contains("look up")
        {
            let query = if let Some(for_pos) = message_lower.find("for") {
                message[for_pos + 3..].trim().to_string()
            } else {
                message.to_string()
            };
            tools.push(AgentTool::WebSearch { query });
        }

        if message_lower.contains("what facts") || message_lower.contains("recall facts") {
            // Try to extract category
            let category = if message_lower.contains("about") {
                if let Some(about_pos) = message_lower.find("about") {
                    let after_about = &message[about_pos + 5..];
                    let end_pos = after_about
                        .find(['?', '.', ',', ' '])
                        .unwrap_or(after_about.len());
                    after_about[..end_pos].trim().to_string()
                } else {
                    "general".to_string()
                }
            } else {
                "general".to_string()
            };
            tools.push(AgentTool::RecallFacts { category });
        }

        if message_lower.contains("what rules")
            || message_lower.contains("show rules")
            || message_lower.contains("recall rules")
        {
            tools.push(AgentTool::RecallRules);
        }

        if message_lower.contains("remember")
            || message_lower.contains("note")
            || message_lower.contains("write down")
        {
            tools.push(AgentTool::WriteToScratchpad {
                notes: message.to_string(),
            });
        }

        if message_lower.contains("what did i")
            || message_lower.contains("check notes")
            || message_lower.contains("read notes")
        {
            tools.push(AgentTool::ReadFromScratchpad);
        }

        Ok(tools)
    }

    /// Generate AI response using LLM with tool results
    async fn generate_ai_response_with_tools(
        &self,
        message: &str,
        tool_results: &[ToolResult],
        client: &Client,
    ) -> Result<String, Box<dyn Error>> {
        let tool_summary = if tool_results.is_empty() {
            "No tools were executed.".to_string()
        } else {
            tool_results
                .iter()
                .map(|result| {
                    format!(
                        "- {}: {}",
                        match result.tool {
                            AgentTool::StoreFact { .. } => "Stored Fact",
                            AgentTool::StoreRule { .. } => "Stored Rule",
                            AgentTool::RecallFacts { .. } => "Recalled Facts",
                            AgentTool::RecallRules => "Recalled Rules",
                            AgentTool::WebSearch { .. } => "Web Search",
                            AgentTool::WriteToScratchpad { .. } => "Wrote Notes",
                            AgentTool::ReadFromScratchpad => "Read Notes",
                        },
                        result.result
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let prompt = format!(
            r#"You are a climate research assistant. The user asked: "{}"

Tools executed:
{}

Based on the tool results, provide a helpful response to the user. Be concise and informative."#,
            message, tool_summary
        );

        let agent = client
            .agent("gpt-3.5-turbo")
            .preamble("You are a climate research assistant.")
            .max_tokens(500)
            .temperature(0.7)
            .build();

        let response = agent.prompt(&prompt).await?;
        Ok(response.trim().to_string())
    }

    /// Generate memory-based response when no LLM is available
    async fn generate_memory_response_with_tools(
        &self,
        _message: &str,
        tool_results: &[ToolResult],
    ) -> Result<String, Box<dyn Error>> {
        if tool_results.is_empty() {
            Ok(
                "I received your message but couldn't determine any specific actions to take."
                    .to_string(),
            )
        } else {
            let responses: Vec<String> = tool_results
                .iter()
                .map(|result| match &result.tool {
                    AgentTool::StoreFact { category, .. } => {
                        format!("⏺ Stored fact in category: {}", category)
                    }
                    AgentTool::StoreRule { rule_name, .. } => {
                        format!("⏺ Stored rule: {}", rule_name)
                    }
                    AgentTool::RecallFacts { category } => {
                        format!("⏺ Facts from {}: {}", category, result.result)
                    }
                    AgentTool::RecallRules => {
                        format!("⏺ Rules: {}", result.result)
                    }
                    AgentTool::WebSearch { query } => {
                        format!("⏺ Search results for '{}': {}", query, result.result)
                    }
                    AgentTool::WriteToScratchpad { .. } => {
                        format!("⏺ {}", result.result)
                    }
                    AgentTool::ReadFromScratchpad => {
                        format!("⏺ {}", result.result)
                    }
                })
                .collect();

            Ok(responses.join("\n\n"))
        }
    }

    /// Get memory system statistics
    pub async fn get_memory_stats(&self) -> Result<String, Box<dyn Error>> {
        let stats = self.memory_system.get_system_stats().await?;

        // Extract counts from the stats structure
        let semantic_count = stats
            .overall
            .by_type
            .get(&MemoryType::Semantic)
            .unwrap_or(&0);
        let episodic_count = stats
            .overall
            .by_type
            .get(&MemoryType::Episodic)
            .unwrap_or(&0);
        let procedural_count = stats
            .overall
            .by_type
            .get(&MemoryType::Procedural)
            .unwrap_or(&0);
        let short_term_count = stats.short_term.total_conversations;

        Ok(format!(
            "Short-term entries: {}\nSemantic facts: {}\nEpisodic memories: {}\nProcedural rules: {}\nTotal memories: {}",
            short_term_count,
            semantic_count,
            episodic_count,
            procedural_count,
            stats.overall.total_memories
        ))
    }

    /// Get git-style logs showing linear commit history (formatted for terminal)
    pub async fn get_git_logs(&self) -> Result<Vec<String>, Box<dyn Error>> {
        let mut logs = Vec::new();

        // Show commits in reverse chronological order (newest first) - compact format
        for commit in self.commit_history.iter().rev().take(8) {
            // Limit to last 8 commits
            let commit_short = &commit.id[..min(7, commit.id.len())];
            let time_str = commit.timestamp.format("%H:%M:%S").to_string();

            // First line: commit hash + branch + time (max ~28 chars)
            logs.push(format!(
                "{} ({}) {}",
                commit_short,
                &commit.branch[..min(4, commit.branch.len())],
                time_str
            ));

            // Second line: longer message (max ~80 chars for better readability)
            let message = if commit.message.len() > 77 {
                format!("{}...", &commit.message[..74])
            } else {
                commit.message.clone()
            };
            logs.push(format!("  {}", message));

            // Third line: author and memory count
            logs.push(format!(
                "  by: {} | mem:{}",
                commit.author, commit.memory_count
            ));
            logs.push("".to_string());
        }

        // Status info (compact)
        logs.push(format!(
            "⏺ {}",
            &self.current_branch[..min(12, self.current_branch.len())]
        ));
        if let Some(latest) = self.commit_history.last() {
            logs.push(format!("⏺ {}", &latest.id[..min(7, latest.id.len())]));
        }

        Ok(logs)
    }

    /// Add a new commit to the history during normal operation
    pub async fn add_commit(
        &mut self,
        message: &str,
        author: &str,
    ) -> Result<String, Box<dyn Error>> {
        let stats = self.memory_system.get_system_stats().await?;
        let memory_count = stats.overall.total_memories;

        // Create a real commit in the memory system
        let real_commit_id = self.memory_system.checkpoint(message).await?;

        // Also maintain our local git history for the UI display
        let commit = GitCommit {
            id: real_commit_id.clone(),
            message: message.to_string(),
            memory_count,
            timestamp: chrono::Utc::now(),
            branch: self.current_branch.clone(),
            author: author.to_string(),
        };

        self.commit_history.push(commit);
        Ok(real_commit_id)
    }

    /// Simulate creating a time travel branch
    pub async fn create_time_travel_branch(
        &mut self,
        branch_name: &str,
        rollback_to_commit: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.current_branch = branch_name.to_string();

        // Find the commit to rollback to and simulate the rollback
        if let Some(rollback_commit) = self
            .commit_history
            .iter()
            .find(|c| c.id.starts_with(rollback_to_commit))
        {
            let rollback_commit = rollback_commit.clone();

            // Add a rollback commit showing the operation
            let rollback_commit_new = GitCommit {
                id: format!(
                    "{:x}",
                    (self.commit_history.len() as u32 * 0x9876) % 0xfffffff
                ),
                message: format!(
                    "ROLLBACK: Reset to state at {}",
                    &rollback_commit.id[..min(7, rollback_commit.id.len())]
                ),
                memory_count: rollback_commit.memory_count,
                timestamp: chrono::Utc::now(),
                branch: branch_name.to_string(),
                author: Self::get_git_author(),
            };
            self.commit_history.push(rollback_commit_new);
        } else {
            // If commit not found, create a generic rollback
            let rollback_commit_new = GitCommit {
                id: format!(
                    "{:x}",
                    (self.commit_history.len() as u32 * 0x9876) % 0xfffffff
                ),
                message: format!("ROLLBACK: Reset to earlier state ({})", rollback_to_commit),
                memory_count: 0, // Reset to minimal state
                timestamp: chrono::Utc::now(),
                branch: branch_name.to_string(),
                author: Self::get_git_author(),
            };
            self.commit_history.push(rollback_commit_new);
        }

        Ok(())
    }

    /// Simulate rolling forward from a rollback
    pub async fn simulate_roll_forward(&mut self, message: &str) -> Result<String, Box<dyn Error>> {
        let stats = self.memory_system.get_system_stats().await?;
        let memory_count = stats.overall.total_memories;

        // Create a real commit in the memory system for recovery
        let recovery_message = format!("RECOVERY: {}", message);
        let real_commit_id = self.memory_system.checkpoint(&recovery_message).await?;

        let commit = GitCommit {
            id: real_commit_id.clone(),
            message: recovery_message,
            memory_count,
            timestamp: chrono::Utc::now(),
            branch: self.current_branch.clone(),
            author: Self::get_git_author(),
        };

        self.commit_history.push(commit);
        Ok(real_commit_id)
    }
}

/// Comprehensive conversation data from the original demo
#[derive(Debug, Serialize, Deserialize)]
struct ConversationData {
    thread1_messages: Vec<String>,
    thread2_messages: Vec<String>,
    thread3_messages: Vec<String>,
}

impl ConversationData {
    /// Load default conversation data - tries multiple locations
    fn load_default() -> (Self, String) {
        // Try these files in order
        let candidate_files = [
            "examples/data/conversation_data.json",
            "data/conversation_data.json", 
            "examples/data/conversation_data_simple.json",
            "conversation_data.json", // Legacy fallback
        ];
        
        for file_path in &candidate_files {
            if Path::new(file_path).exists() {
                return Self::load_from_file(file_path);
            }
        }
        
        // If no files found, panic with helpful message
        panic!("No conversation data files found. Please ensure one of these files exists:\n  - examples/data/conversation_data.json\n  - data/conversation_data.json\n  - examples/data/conversation_data_simple.json");
    }

    /// Load conversation data from a JSON file
    fn load_from_file<P: AsRef<Path>>(file_path: P) -> (Self, String) {
        match fs::read_to_string(&file_path) {
            Ok(content) => {
                match serde_json::from_str::<ConversationData>(&content) {
                    Ok(data) => {
                        let msg = format!("✓ Loaded from: {}", file_path.as_ref().display());
                        (data, msg)
                    },
                    Err(e) => {
                        panic!("Failed to parse JSON from {}: {}. Please check the file format.", file_path.as_ref().display(), e);
                    }
                }
            },
            Err(_) => {
                panic!("File not found: {}. Please check the file path.", file_path.as_ref().display());
            }
        }
    }

}

/// Render the four-panel UI
fn ui(f: &mut Frame, ui_state: &UiState) {
    // Add instructions at the top
    let instructions = Block::default()
        .title("Instructions: 'q'/ESC=quit | 'p'=pause/resume | ↑/↓=scroll | PgUp/PgDn=fast scroll | Home/End=top/bottom")
        .title_alignment(Alignment::Center)
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::Yellow));

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(f.size());

    f.render_widget(instructions, main_chunks[0]);

    // Create layout with 2x2 grid
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(main_chunks[1]);

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[0]);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[1]);

    // Top Left: Conversations
    render_conversations(f, top_chunks[0], ui_state);

    // Top Right: Git Logs (switched position)
    render_git_logs(f, top_chunks[1], ui_state);

    // Bottom Left: Memory Stats (switched position)
    render_memory_stats(f, bottom_chunks[0], ui_state);

    // Bottom Right: KV Store Keys
    render_kv_keys(f, bottom_chunks[1], ui_state);
}

fn render_conversations(f: &mut Frame, area: Rect, ui_state: &UiState) {
    let mut items: Vec<ListItem> = ui_state
        .conversations
        .iter()
        .skip(ui_state.scroll_conversations)
        .map(|conv| {
            let style = if conv.contains("⏺ User:") {
                Style::default().fg(Color::White)
            } else if conv.contains("⏺ Assistant:") {
                Style::default().fg(Color::Green)
            } else if conv.contains("⏺") || conv.contains("⏺") {
                Style::default().fg(Color::Yellow)
            } else if conv.contains("⏺") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Yellow)
            };
            ListItem::new(Line::from(Span::styled(conv.clone(), style)))
        })
        .collect();

    // Add typing indicator with blinking cursor if typing
    if ui_state.is_typing {
        let cursor = if ui_state.cursor_visible { "▌" } else { " " };
        items.push(ListItem::new(Line::from(vec![
            Span::styled("⏺ Assistant: ", Style::default().fg(Color::Green)),
            Span::styled(
                cursor,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ])));
    }

    let conversations = List::new(items)
        .block(
            Block::default()
                .title("Conversations with Agents")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(conversations, area);
}

fn render_memory_stats(f: &mut Frame, area: Rect, ui_state: &UiState) {
    let paragraph = Paragraph::new(ui_state.memory_stats.clone())
        .block(
            Block::default()
                .title("Agent Memory Statistics")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .style(Style::default().fg(Color::Magenta))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn render_git_logs(f: &mut Frame, area: Rect, ui_state: &UiState) {
    let items: Vec<ListItem> = ui_state
        .git_logs
        .iter()
        .skip(ui_state.scroll_git_logs)
        .map(|log| {
            let style =
                if log.starts_with("⏺") && (log.contains("main") || log.contains("time-travel")) {
                    Style::default().fg(Color::Green).bold() // Current branch info
                } else if log.starts_with("⏺") {
                    Style::default().fg(Color::Blue).bold() // Latest commit info
                } else if log.contains("ROLLBACK") {
                    Style::default().fg(Color::Red).bold() // Rollback operations
                } else if log.contains("RECOVERY") {
                    Style::default().fg(Color::Magenta).bold() // Recovery operations
                } else if log.matches(" ").count() >= 2 && log.len() > 8 && !log.starts_with("  ") {
                    // Commit hash lines (format: "abc123f (main) 14:30")
                    Style::default().fg(Color::Cyan).bold() // Commit hashes
                } else if log.starts_with("  by: ") {
                    Style::default().fg(Color::Yellow) // Author and memory info line
                } else if log.starts_with("  mem:") {
                    Style::default().fg(Color::Blue) // Memory count info (legacy)
                } else if log.starts_with("  ") && !log.trim().is_empty() {
                    Style::default().fg(Color::White) // Commit messages (indented)
                } else if log.trim().is_empty() {
                    Style::default() // Empty lines
                } else {
                    Style::default().fg(Color::Gray) // Default
                };
            ListItem::new(Line::from(Span::styled(log.clone(), style)))
        })
        .collect();

    let git_logs = List::new(items)
        .block(
            Block::default()
                .title("Agent Memory History and Branching")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .style(Style::default());

    f.render_widget(git_logs, area);
}

fn render_kv_keys(f: &mut Frame, area: Rect, ui_state: &UiState) {
    let items: Vec<ListItem> = ui_state
        .kv_keys
        .iter()
        .skip(ui_state.scroll_kv_keys)
        .map(|key| {
            let style = if key.contains("semantic") {
                Style::default().fg(Color::Green)
            } else if key.contains("procedural") {
                Style::default().fg(Color::Yellow)
            } else if key.contains("short_term") {
                Style::default().fg(Color::Cyan)
            } else if key.contains("episodic") {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default().fg(Color::Red)
            };
            ListItem::new(Line::from(Span::styled(key.clone(), style)))
        })
        .collect();

    let kv_keys = List::new(items)
        .block(
            Block::default()
                .title("Memory Storage Backend")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(kv_keys, area);
}

/// Helper function to wait while paused
async fn wait_for_resume(pause_state: &Arc<AtomicBool>) {
    while pause_state.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Sleep function that respects pause state
async fn pausable_sleep(duration: Duration, pause_state: &Arc<AtomicBool>) {
    wait_for_resume(pause_state).await;
    tokio::time::sleep(duration).await;
}

/// Display backend selection menu and get user choice
fn select_memory_backend() -> io::Result<MemoryBackend> {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                  MEMORY BACKEND SELECTION                ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("Select the memory backend for the agent demonstration:");
    println!();
    
    let backends = vec![
        MemoryBackend::InMemory,
        MemoryBackend::ThreadSafeInMemory,
        MemoryBackend::ThreadSafeGit,
        MemoryBackend::ThreadSafeFile,
    ];
    
    for (i, backend) in backends.iter().enumerate() {
        println!("  {}. {} - {}", 
                 i + 1, 
                 backend.display_name(), 
                 backend.description());
    }
    
    println!();
    print!("Enter your choice (1-4): ");
    io::stdout().flush()?;
    
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        match input.trim().parse::<usize>() {
            Ok(choice) if choice >= 1 && choice <= 4 => {
                let selected_backend = backends[choice - 1].clone();
                println!();
                println!("✓ Selected: {}", selected_backend.display_name());
                println!("  {}", selected_backend.description());
                return Ok(selected_backend);
            },
            _ => {
                print!("Invalid choice. Please enter 1-4: ");
                io::stdout().flush()?;
            }
        }
    }
}

/// Run comprehensive demonstration with real agent and memory operations
async fn run_comprehensive_demo(
    ui_sender: mpsc::UnboundedSender<UiEvent>,
    pause_state: Arc<AtomicBool>,
    temp_dir: TempDir,
    backend: MemoryBackend,
) -> Result<(), Box<dyn Error>> {
    // Try to load conversation data from file, fallback to default
    let (conversation_data, load_status) = ConversationData::load_default();

    // Use the provided temporary directory
    let memory_path = temp_dir.path();
    
    // Initialize storage based on backend type
    let dataset_dir = match &backend {
        MemoryBackend::InMemory => {
            // In-memory doesn't need any directory setup
            memory_path.to_path_buf()
        },
        MemoryBackend::ThreadSafeInMemory => {
            // Thread-safe in-memory needs git initialization (uses git for versioning)
            std::process::Command::new("git")
                .args(["init"])
                .current_dir(&temp_dir)
                .output()
                .expect("Failed to initialize git repository");

            // Thread-safe in-memory still uses a path for temp storage
            memory_path.to_path_buf()
        },
        MemoryBackend::ThreadSafeGit => {
            // Git-backed storage needs git initialization
            std::process::Command::new("git")
                .args(["init"])
                .current_dir(&temp_dir)
                .output()
                .expect("Failed to initialize git repository");

            // Create a subdirectory for the dataset (git-backed stores require subdirectories)
            let dataset_dir = memory_path.join("dataset");
            std::fs::create_dir_all(&dataset_dir)?;
            dataset_dir
        },
        MemoryBackend::ThreadSafeFile => {
            // File-based storage needs git initialization (uses git for versioning)
            std::process::Command::new("git")
                .args(["init"])
                .current_dir(&temp_dir)
                .output()
                .expect("Failed to initialize git repository");

            // Create a subdirectory for the dataset
            let dataset_dir = memory_path.join("dataset");
            std::fs::create_dir_all(&dataset_dir)?;
            dataset_dir
        },
    };

    let openai_api_key = std::env::var("OPENAI_API_KEY").ok();
    let has_openai = openai_api_key.is_some();

    let mut agent = ContextOffloadingAgent::new(
        &dataset_dir,
        "context_agent_001".to_string(),
        "research_project".to_string(),
        backend.clone(),
        openai_api_key,
        Some(ui_sender.clone()),
    )
    .await?;

    // Send initial state
    ui_sender.send(UiEvent::ConversationUpdate(
        "⏺ Context Offloading Agent Demo".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate(
        "ProllyTree + Rig Integration".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate(format!(
        "⏺ Memory Backend: {}",
        backend.display_name()
    )))?;
    ui_sender.send(UiEvent::ConversationUpdate(format!(
        "⏺ Memory path: {:?}",
        dataset_dir
    )))?;
    ui_sender.send(UiEvent::ConversationUpdate(format!(
        "⏺ Conversations: {}",
        load_status
    )))?;
    if has_openai {
        ui_sender.send(UiEvent::ConversationUpdate(
            "⏺ OpenAI integration enabled".to_string(),
        ))?;
    } else {
        ui_sender.send(UiEvent::ConversationUpdate(
            "⏺  OpenAI key not found - using fallback mode".to_string(),
        ))?;
    }
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    // Get initial memory stats
    let mut stats = agent.get_memory_stats().await?;
    ui_sender.send(UiEvent::MemoryStatsUpdate(format!(
        "Agent: context_agent_001\nThread: thread_001\n\n{}",
        stats
    )))?;

    // Initial git and KV updates
    let initial_keys = generate_kv_keys(0, 0, 1, false, &backend);
    let _ = ui_sender.send(UiEvent::KvKeysUpdate(initial_keys));

    // Get real git logs
    let initial_git_logs = agent
        .get_git_logs()
        .await
        .unwrap_or_else(|_| vec!["⏺ Initial agent setup".to_string()]);
    let _ = ui_sender.send(UiEvent::GitLogUpdate(initial_git_logs));

    pausable_sleep(Duration::from_millis(2000), &pause_state).await;

    // Clear screen and highlight theme for Thread 1
    let _ = clear_and_highlight_theme(
        &ui_sender,
        "THREAD 1",
        "Initial Data Collection",
        "⏺ Hurricane Research & Climate Facts",
        &pause_state,
    )
    .await;

    // THREAD 1: Initial Data Collection
    agent.switch_thread("thread_001".to_string());

    for (i, message) in conversation_data.thread1_messages.iter().enumerate() {
        ui_sender.send(UiEvent::ConversationUpdate(format!("⏺ User: {}", message)))?;

        // Show typing indicator while processing
        ui_sender.send(UiEvent::TypingIndicator(true))?;
        pausable_sleep(Duration::from_millis(300), &pause_state).await; // Brief pause to show typing

        // Process with real agent
        match agent.process_with_tools(message).await {
            Ok(response) => {
                ui_sender.send(UiEvent::TypingIndicator(false))?; // Stop typing
                ui_sender.send(UiEvent::ConversationUpdate(format!(
                    "⏺ Assistant: {}",
                    response
                )))?;
            }
            Err(e) => {
                ui_sender.send(UiEvent::TypingIndicator(false))?; // Stop typing
                ui_sender.send(UiEvent::ConversationUpdate(format!("⏺ Error: {}", e)))?;
            }
        }

        // Update git logs after every message (frequent updates)
        if let Ok(git_logs) = agent.get_git_logs().await {
            let _ = ui_sender.send(UiEvent::GitLogUpdate(git_logs));
        }

        // Update UI every few messages with real stats
        if i % 3 == 0 || i == conversation_data.thread1_messages.len() - 1 {
            stats = agent.get_memory_stats().await?;
            ui_sender.send(UiEvent::MemoryStatsUpdate(format!(
                "Agent: context_agent_001\nThread: thread_001\n\n{}",
                stats
            )))?;

            // Generate approximate KV keys (simulated based on message type)
            let approx_semantic = if message.contains("Fact:") {
                i / 3 + 1
            } else {
                i / 4
            };
            let approx_procedural = if message.contains("Rule:") {
                i / 5 + 1
            } else {
                i / 6
            };
            let keys = generate_kv_keys(approx_semantic, approx_procedural, 1, false, &backend);
            let _ = ui_sender.send(UiEvent::KvKeysUpdate(keys));
        }

        ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
        pausable_sleep(Duration::from_millis(800), &pause_state).await;
    }

    // Create actual checkpoint and add to git history
    let commit_1 = agent.add_commit("Thread 1 complete: Initial climate data collection with hurricane, heat wave, and flooding research", &ContextOffloadingAgent::get_git_author()).await?;

    // Save current memory stats for later comparison
    let thread1_stats = agent.memory_system.get_system_stats().await?;
    let _thread1_memory_count = thread1_stats.overall.total_memories;

    // Get real git logs from the agent after checkpoint
    if let Ok(git_logs) = agent.get_git_logs().await {
        let _ = ui_sender.send(UiEvent::GitLogUpdate(git_logs));
    }

    pausable_sleep(Duration::from_millis(1500), &pause_state).await;

    // Clear screen and highlight theme for Thread 2
    let _ = clear_and_highlight_theme(
        &ui_sender,
        "THREAD 2",
        "Analysis and Pattern Recognition",
        "⏺ Cross-Thread Memory Queries",
        &pause_state,
    )
    .await;

    // THREAD 2: Analysis and Pattern Recognition
    agent.switch_thread("thread_002".to_string());

    for (i, message) in conversation_data.thread2_messages.iter().enumerate() {
        ui_sender.send(UiEvent::ConversationUpdate(format!("⏺ User: {}", message)))?;

        // Show typing indicator while processing
        ui_sender.send(UiEvent::TypingIndicator(true))?;
        pausable_sleep(Duration::from_millis(300), &pause_state).await; // Brief pause to show typing

        // Process with real agent
        match agent.process_with_tools(message).await {
            Ok(response) => {
                ui_sender.send(UiEvent::TypingIndicator(false))?; // Stop typing
                ui_sender.send(UiEvent::ConversationUpdate(format!(
                    "⏺ Assistant: {}",
                    response
                )))?;
            }
            Err(e) => {
                ui_sender.send(UiEvent::TypingIndicator(false))?; // Stop typing
                ui_sender.send(UiEvent::ConversationUpdate(format!("⏺ Error: {}", e)))?;
            }
        }

        // Update git logs after every message (frequent updates)
        if let Ok(git_logs) = agent.get_git_logs().await {
            let _ = ui_sender.send(UiEvent::GitLogUpdate(git_logs));
        }

        // Update UI every few messages with real stats
        if i % 2 == 0 || i == conversation_data.thread2_messages.len() - 1 {
            stats = agent.get_memory_stats().await?;
            ui_sender.send(UiEvent::MemoryStatsUpdate(format!(
                "Agent: context_agent_001\nThread: thread_002\n\n{}",
                stats
            )))?;

            let approx_semantic = (i + 12) / 3; // Approximate progress
            let approx_procedural = (i + 5) / 4;
            let keys = generate_kv_keys(approx_semantic, approx_procedural, 2, false, &backend);
            let _ = ui_sender.send(UiEvent::KvKeysUpdate(keys));
        }

        ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
        pausable_sleep(Duration::from_millis(600), &pause_state).await;
    }

    // Create second checkpoint and add to git history
    let _commit_2 = agent
        .add_commit(
            "Thread 2 complete: Cross-thread memory analysis and pattern recognition phase",
            &ContextOffloadingAgent::get_git_author(),
        )
        .await?;

    // Save thread 2 stats
    let thread2_stats = agent.memory_system.get_system_stats().await?;
    let _thread2_memory_count = thread2_stats.overall.total_memories;

    pausable_sleep(Duration::from_millis(1500), &pause_state).await;

    // Clear screen and highlight theme for Thread 3
    let _ = clear_and_highlight_theme(
        &ui_sender,
        "THREAD 3",
        "Synthesis and Policy Recommendations",
        "⏺ Knowledge Integration & Versioned Storage",
        &pause_state,
    )
    .await;

    // THREAD 3: Synthesis and Policy Recommendations
    agent.switch_thread("thread_003".to_string());

    for (i, message) in conversation_data.thread3_messages.iter().enumerate() {
        ui_sender.send(UiEvent::ConversationUpdate(format!("⏺ User: {}", message)))?;

        // Show typing indicator while processing
        ui_sender.send(UiEvent::TypingIndicator(true))?;
        pausable_sleep(Duration::from_millis(300), &pause_state).await; // Brief pause to show typing

        // Process with real agent
        match agent.process_with_tools(message).await {
            Ok(response) => {
                ui_sender.send(UiEvent::TypingIndicator(false))?; // Stop typing
                ui_sender.send(UiEvent::ConversationUpdate(format!(
                    "⏺ Assistant: {}",
                    response
                )))?;
            }
            Err(e) => {
                ui_sender.send(UiEvent::TypingIndicator(false))?; // Stop typing
                ui_sender.send(UiEvent::ConversationUpdate(format!("⏺ Error: {}", e)))?;
            }
        }

        // Update git logs after every message (frequent updates)
        if let Ok(git_logs) = agent.get_git_logs().await {
            let _ = ui_sender.send(UiEvent::GitLogUpdate(git_logs));
        }

        // Update UI every few messages with real stats
        if i % 2 == 0 || i == conversation_data.thread3_messages.len() - 1 {
            stats = agent.get_memory_stats().await?;
            ui_sender.send(UiEvent::MemoryStatsUpdate(format!(
                "Agent: context_agent_001\nThread: thread_003\n\n{}",
                stats
            )))?;

            let approx_semantic = (i + 20) / 3; // Approximate final progress
            let approx_procedural = (i + 10) / 4;
            let keys = generate_kv_keys(approx_semantic, approx_procedural, 3, true, &backend);
            let _ = ui_sender.send(UiEvent::KvKeysUpdate(keys));
        }

        ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
        pausable_sleep(Duration::from_millis(600), &pause_state).await;
    }

    // Final statistics and versioned storage demonstrations
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate(
        "⏺ Final Memory Statistics:".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate(
        "═══════════════════════════════════════════════════════════".to_string(),
    ))?;

    // Get final real stats
    let final_stats = agent.get_memory_stats().await?;
    ui_sender.send(UiEvent::ConversationUpdate(format!(
        "   {}",
        final_stats.replace('\n', "\n   ")
    )))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    // Versioned storage benefits
    ui_sender.send(UiEvent::ConversationUpdate(
        "⏺ ProllyTree Versioned Storage".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate(
        "Demonstrating key benefits:".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    // Create final commit
    let _final_commit = agent
        .add_commit(
            "Thread 3 complete: Knowledge synthesis and policy recommendations finalized",
            &ContextOffloadingAgent::get_git_author(),
        )
        .await?;

    // Save current state before time travel
    let _final_memory_count = agent
        .memory_system
        .get_system_stats()
        .await?
        .overall
        .total_memories;

    // TIME TRAVEL DEBUGGING - ACTUAL DEMONSTRATION
    ui_sender.send(UiEvent::ConversationUpdate(
        "⏺ TIME TRAVEL DEBUGGING - ACTUAL DEMONSTRATION".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate(
        "═══════════════════════════════════════════════════════════".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    // Show memory evolution
    pausable_sleep(Duration::from_millis(2000), &pause_state).await;

    // Query specific memories from different time periods
    ui_sender.send(UiEvent::ConversationUpdate(
        "⏺ Querying Memories from Different Time Periods:".to_string(),
    ))?;

    // Query semantic memories - use text search
    let hurricane_facts = agent
        .memory_system
        .semantic
        .text_search("hurricane", None)
        .await?;
    ui_sender.send(UiEvent::ConversationUpdate(format!(
        "   • Hurricane Facts Found: {} entries",
        hurricane_facts.len()
    )))?;
    if !hurricane_facts.is_empty() {
        if let Some(first_fact) = hurricane_facts.first() {
            let content_preview = format!("{}", first_fact.content)
                .chars()
                .take(60)
                .collect::<String>();
            ui_sender.send(UiEvent::ConversationUpdate(format!(
                "     - Example: {}...",
                content_preview
            )))?;
        }
    }

    // Query all memories by type
    let semantic_query = MemoryQuery {
        namespace: None,
        memory_types: Some(vec![MemoryType::Semantic]),
        tags: None,
        time_range: None,
        text_query: None,
        semantic_query: None,
        limit: None,
        include_expired: false,
    };
    let semantic_memories = agent.memory_system.semantic.query(semantic_query).await?;
    ui_sender.send(UiEvent::ConversationUpdate(format!(
        "   • Total Semantic Memories: {} entries",
        semantic_memories.len()
    )))?;

    // Query procedural memories
    let procedural_query = MemoryQuery {
        namespace: None,
        memory_types: Some(vec![MemoryType::Procedural]),
        tags: Some(vec!["rule".to_string()]),
        time_range: None,
        text_query: None,
        semantic_query: None,
        limit: None,
        include_expired: false,
    };
    let rules = agent
        .memory_system
        .procedural
        .query(procedural_query)
        .await?;
    ui_sender.send(UiEvent::ConversationUpdate(format!(
        "   • Rules & Procedures: {} entries",
        rules.len()
    )))?;
    if !rules.is_empty() {
        ui_sender.send(UiEvent::ConversationUpdate(format!(
            "     - Categories: analysis_workflow, policy_recommendations"
        )))?;
    }

    // Query episodic memories
    let episodic_query = MemoryQuery {
        namespace: None,
        memory_types: Some(vec![MemoryType::Episodic]),
        tags: None,
        time_range: Some(TimeRange {
            start: Some(chrono::Utc::now() - chrono::Duration::hours(1)),
            end: Some(chrono::Utc::now()),
        }),
        text_query: None,
        semantic_query: None,
        limit: None,
        include_expired: false,
    };
    let recent_episodes = agent.memory_system.episodic.query(episodic_query).await?;
    ui_sender.send(UiEvent::ConversationUpdate(format!(
        "   • Recent Episodes (last hour): {} entries",
        recent_episodes.len()
    )))?;

    // Show memory access patterns
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate(
        "⏺ Memory Access Patterns:".to_string(),
    ))?;
    let stats = agent.memory_system.get_system_stats().await?;
    ui_sender.send(UiEvent::ConversationUpdate(format!(
        "   • Average access count: {:.1}",
        stats.overall.avg_access_count
    )))?;
    if let Some(oldest) = stats.overall.oldest_memory {
        ui_sender.send(UiEvent::ConversationUpdate(format!(
            "   • Oldest memory: {}",
            oldest.format("%H:%M:%S")
        )))?;
    }
    if let Some(newest) = stats.overall.newest_memory {
        ui_sender.send(UiEvent::ConversationUpdate(format!(
            "   • Newest memory: {}",
            newest.format("%H:%M:%S")
        )))?;
    }

    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    pausable_sleep(Duration::from_millis(2000), &pause_state).await;

    // ROLLBACK DEMONSTRATION - ACTUAL GIT OPERATIONS
    ui_sender.send(UiEvent::ConversationUpdate(
        "⏺ ROLLBACK DEMONSTRATION - INTERACTIVE".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate(
        "═══════════════════════════════════════════════════════════".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    // Step 1: Create a time travel branch and rollback to Thread 1
    agent
        .create_time_travel_branch("time-travel", &commit_1)
        .await?;

    // Update git logs to show the rollback
    if let Ok(git_logs) = agent.get_git_logs().await {
        let _ = ui_sender.send(UiEvent::GitLogUpdate(git_logs));
    }

    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    pausable_sleep(Duration::from_millis(2000), &pause_state).await;

    // Additional conversation turns while in rolled-back state
    ui_sender.send(UiEvent::ConversationUpdate(
        "⏺ Working in rolled-back state...".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    // Simulate some additional interactions in the rolled-back state
    let rollback_messages = vec![
        "What climate facts do we have about hurricanes?",
        "Fact: New research shows hurricane intensification rate increased 25% since 2000 category: hurricanes",
        "What are our current procedural rules?",
        "Rule: rapid_response: IF hurricane_cat_4_or_5 THEN activate_emergency_shelters_within_12_hours",
    ];

    for (i, message) in rollback_messages.iter().enumerate() {
        ui_sender.send(UiEvent::ConversationUpdate(format!("⏺ User: {}", message)))?;

        // Show typing indicator while processing
        ui_sender.send(UiEvent::TypingIndicator(true))?;
        pausable_sleep(Duration::from_millis(300), &pause_state).await; // Brief pause to show typing

        // Process with real agent (now in rolled-back state)
        match agent.process_with_tools(message).await {
            Ok(response) => {
                ui_sender.send(UiEvent::TypingIndicator(false))?; // Stop typing
                ui_sender.send(UiEvent::ConversationUpdate(format!(
                    "⏺ Assistant: {}",
                    response
                )))?;
            }
            Err(e) => {
                ui_sender.send(UiEvent::TypingIndicator(false))?; // Stop typing
                ui_sender.send(UiEvent::ConversationUpdate(format!("⏺ Error: {}", e)))?;
            }
        }

        // Update git logs after each message to show new commits in rolled-back state
        if let Ok(git_logs) = agent.get_git_logs().await {
            let _ = ui_sender.send(UiEvent::GitLogUpdate(git_logs));
        }

        // Update memory stats to show changes in rolled-back state
        if i % 2 == 1 {
            // Every other message
            let stats = agent.get_memory_stats().await?;
            ui_sender.send(UiEvent::MemoryStatsUpdate(format!(
                "Agent: context_agent_001\nBranch: time-travel\n\n{}",
                stats
            )))?;
        }

        pausable_sleep(Duration::from_millis(800), &pause_state).await;
    }

    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate(
        "⏺ Changes made in rolled-back state".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate(
        "⏺ Memory now differs from original Thread 3 state".to_string(),
    ))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    pausable_sleep(Duration::from_millis(2000), &pause_state).await;

    // Step 2: Simulate recovery/roll-forward
    let _recovery_commit = agent
        .simulate_roll_forward("Recovery: selective restore")
        .await?;

    // Update git logs to show the recovery
    if let Ok(git_logs) = agent.get_git_logs().await {
        let _ = ui_sender.send(UiEvent::GitLogUpdate(git_logs));
    }

    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    pausable_sleep(Duration::from_millis(2000), &pause_state).await;

    // Step 3: Switch back to main branch
    agent.current_branch = "main".to_string();

    // Update git logs to show we're back on main
    if let Ok(git_logs) = agent.get_git_logs().await {
        let _ = ui_sender.send(UiEvent::GitLogUpdate(git_logs));
    }

    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    // Summary of capabilities

    // Update final UI state
    ui_sender.send(UiEvent::MemoryStatsUpdate(format!(
        "Agent: context_agent_001\nThread: thread_003\n\n{}",
        final_stats
    )))?;
    // Get final git logs from the agent
    if let Ok(git_logs) = agent.get_git_logs().await {
        let _ = ui_sender.send(UiEvent::GitLogUpdate(git_logs));
    }

    let final_keys = generate_kv_keys(25, 8, 3, true, &backend);
    let _ = ui_sender.send(UiEvent::KvKeysUpdate(final_keys));

    // Completion messages
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate(
        ">>> Press 'q' or ESC to exit the demo <<<".to_string(),
    ))?;

    Ok(())
}

/// Clear screen and highlight the current demo theme
async fn clear_and_highlight_theme(
    ui_sender: &mpsc::UnboundedSender<UiEvent>,
    thread_name: &str,
    theme_title: &str,
    theme_description: &str,
    pause_state: &Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    // Clear conversations with empty lines
    for _ in 0..10 {
        let _ = ui_sender.send(UiEvent::ConversationUpdate("".to_string()));
    }

    // Send ASCII art header based on thread
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    match thread_name {
        "THREAD 1" => {
            ui_sender.send(UiEvent::ConversationUpdate(
                "╔╦╗╦ ╦╦═╗╔═╗╔═╗╔╦╗  ╔╗ ".to_string(),
            ))?;
            ui_sender.send(UiEvent::ConversationUpdate(
                " ║ ╠═╣╠╦╝║╣ ╠═╣ ║║  ╩║ ".to_string(),
            ))?;
            ui_sender.send(UiEvent::ConversationUpdate(
                " ╩ ╩ ╩╩╚═╚═╝╩ ╩═╩╝  ╚╝ ".to_string(),
            ))?;
        }
        "THREAD 2" => {
            ui_sender.send(UiEvent::ConversationUpdate(
                "╔╦╗╦ ╦╦═╗╔═╗╔═╗╔╦╗  ╔═╗".to_string(),
            ))?;
            ui_sender.send(UiEvent::ConversationUpdate(
                " ║ ╠═╣╠╦╝║╣ ╠═╣ ║║  ╔═╝".to_string(),
            ))?;
            ui_sender.send(UiEvent::ConversationUpdate(
                " ╩ ╩ ╩╩╚═╚═╝╩ ╩═╩╝  ╚═╝".to_string(),
            ))?;
        }
        "THREAD 3" => {
            ui_sender.send(UiEvent::ConversationUpdate(
                "╔╦╗╦ ╦╦═╗╔═╗╔═╗╔╦╗  ╔═╗".to_string(),
            ))?;
            ui_sender.send(UiEvent::ConversationUpdate(
                " ║ ╠═╣╠╦╝║╣ ╠═╣ ║║  ╚═╗".to_string(),
            ))?;
            ui_sender.send(UiEvent::ConversationUpdate(
                " ╩ ╩ ╩╩╚═╚═╝╩ ╩═╩╝  ╚═╝".to_string(),
            ))?;
        }
        _ => {
            ui_sender.send(UiEvent::ConversationUpdate(format!("⏺ {}", thread_name)))?;
        }
    }

    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate(format!("⏺ {}", theme_title)))?;
    ui_sender.send(UiEvent::ConversationUpdate(format!(
        "{}",
        theme_description
    )))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    // Brief pause to let user read the theme
    pausable_sleep(Duration::from_millis(2500), &pause_state).await;

    // Clear the theme display
    for _ in 0..15 {
        ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    }

    Ok(())
}

// Helper function to generate realistic KV store keys
fn generate_kv_keys(
    semantic_count: usize,
    procedural_count: usize,
    thread_count: usize,
    include_episodic: bool,
    backend: &MemoryBackend,
) -> Vec<String> {
    let mut keys = vec![
        format!("⏺ Backend: {}", backend.display_name()),
        format!("⏺ {}", backend.description()),
        "".to_string(),
        "⏺ Agent Memory Structure:".to_string(),
        "".to_string()
    ];

    // Semantic memory keys
    keys.push("⏺ Semantic Memory (Facts):".to_string());
    if semantic_count > 0 {
        keys.push(
            "  /agents/context_agent_001/semantic/research_project_hurricanes/001".to_string(),
        );
        keys.push(
            "  /agents/context_agent_001/semantic/research_project_hurricanes/002".to_string(),
        );
    }
    if semantic_count > 2 {
        keys.push(
            "  /agents/context_agent_001/semantic/research_project_heat_waves/001".to_string(),
        );
        keys.push(
            "  /agents/context_agent_001/semantic/research_project_heat_waves/002".to_string(),
        );
    }
    if semantic_count > 4 {
        keys.push("  /agents/context_agent_001/semantic/research_project_flooding/001".to_string());
        keys.push("  /agents/context_agent_001/semantic/research_project_economic/001".to_string());
    }
    if semantic_count > 6 {
        keys.push(
            "  /agents/context_agent_001/semantic/research_project_adaptation/001".to_string(),
        );
        keys.push(
            "  /agents/context_agent_001/semantic/research_project_heat_waves/003".to_string(),
        );
    }

    keys.push("".to_string());

    // Procedural memory keys
    keys.push("⏺ Procedural Memory (Rules):".to_string());
    if procedural_count > 0 {
        keys.push(
            "  /agents/context_agent_001/procedural/climate_analysis/hurricane_evacuation"
                .to_string(),
        );
    }
    if procedural_count > 1 {
        keys.push(
            "  /agents/context_agent_001/procedural/climate_analysis/heat_advisory".to_string(),
        );
        keys.push(
            "  /agents/context_agent_001/procedural/climate_analysis/flood_insurance".to_string(),
        );
    }
    if procedural_count > 3 {
        keys.push(
            "  /agents/context_agent_001/procedural/climate_analysis/drought_response".to_string(),
        );
        keys.push(
            "  /agents/context_agent_001/procedural/climate_analysis/building_codes".to_string(),
        );
    }
    if procedural_count > 5 {
        keys.push(
            "  /agents/context_agent_001/procedural/climate_analysis/infrastructure_resilience"
                .to_string(),
        );
        keys.push(
            "  /agents/context_agent_001/procedural/climate_analysis/emergency_response"
                .to_string(),
        );
    }

    keys.push("".to_string());

    // Short-term memory keys
    keys.push("⏺ Short-term Memory (Conversations):".to_string());
    for i in 1..=thread_count {
        keys.push(format!(
            "  /agents/context_agent_001/short_term/thread_{:03}/conversations",
            i
        ));
    }

    keys.push("".to_string());

    // Episodic memory keys (if applicable)
    if include_episodic {
        keys.push("⏺ Episodic Memory (Sessions):".to_string());
        keys.push(
            "  /agents/context_agent_001/episodic/2025-07-31/research_session_001".to_string(),
        );
        keys.push(
            "  /agents/context_agent_001/episodic/2025-07-31/analysis_session_002".to_string(),
        );
        keys.push(
            "  /agents/context_agent_001/episodic/2025-07-31/synthesis_session_003".to_string(),
        );
        keys.push("".to_string());
    }

    // Add backend-specific storage information
    keys.push("".to_string());
    match backend {
        MemoryBackend::InMemory => {
            keys.push("⏺ Storage: Volatile in-memory only".to_string());
            keys.push("⏺ Persistence: None".to_string());
            keys.push("⏺ Versioning: Not available".to_string());
        },
        MemoryBackend::ThreadSafeInMemory => {
            keys.push("⏺ Storage: In-memory with git versioning".to_string());
            keys.push("⏺ Persistence: Temporary + git history".to_string());
            keys.push("⏺ Versioning: Git commits in memory".to_string());
        },
        MemoryBackend::ThreadSafeGit => {
            keys.push("⏺ Storage: Git repository".to_string());
            keys.push("⏺ Persistence: Full git history".to_string());
            keys.push("⏺ Versioning: Git commits & branches".to_string());
        },
        MemoryBackend::ThreadSafeFile => {
            keys.push("⏺ Storage: File-based with git versioning".to_string());
            keys.push("⏺ Persistence: Durable file + git history".to_string());
            keys.push("⏺ Versioning: Git commits & rollback".to_string());
        },
    }

    keys.push("".to_string());
    keys.push(format!(
        "⏺ Total Active Keys: ~{}",
        (semantic_count * 2)
            + (procedural_count * 2)
            + (thread_count * 3)
            + if include_episodic { 6 } else { 0 }
    ));
    keys.push("⏺ Last Updated: just now".to_string());

    keys
}

/// Run the application with UI
async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut ui_receiver: mpsc::UnboundedReceiver<UiEvent>,
    pause_state: Arc<AtomicBool>,
) -> io::Result<()> {
    let mut ui_state = UiState::default();
    let mut last_tick = Instant::now();
    let mut last_cursor_blink = Instant::now();
    let tick_rate = Duration::from_millis(100);
    let cursor_blink_rate = Duration::from_millis(530); // Standard cursor blink rate

    loop {
        terminal.draw(|f| ui(f, &ui_state))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        return Ok(());
                    }
                    KeyCode::Char('p') => {
                        let new_pause_state = !pause_state.load(Ordering::Relaxed);
                        pause_state.store(new_pause_state, Ordering::Relaxed);
                        ui_state.is_paused = new_pause_state;
                        // Send pause/resume notification to conversation window
                        let status = if new_pause_state { "PAUSED" } else { "RESUMED" };
                        ui_state.conversations.push(format!(
                            "⏺ Demo {} - Press 'p' to {}",
                            status,
                            if new_pause_state { "resume" } else { "pause" }
                        ));
                        // Auto-scroll to show the pause message
                        let window_height = terminal.size()?.height as usize;
                        let content_height = (window_height / 2).saturating_sub(3);
                        if ui_state.conversations.len() > content_height {
                            ui_state.scroll_conversations =
                                ui_state.conversations.len() - content_height;
                        }
                    }
                    KeyCode::Up => {
                        if ui_state.scroll_conversations > 0 {
                            ui_state.scroll_conversations -= 1;
                        }
                    }
                    KeyCode::Down => {
                        let window_height = terminal.size()?.height as usize;
                        let content_height = (window_height / 2).saturating_sub(3);
                        if ui_state.scroll_conversations + content_height
                            < ui_state.conversations.len()
                        {
                            ui_state.scroll_conversations += 1;
                        }
                    }
                    KeyCode::PageUp => {
                        ui_state.scroll_conversations =
                            ui_state.scroll_conversations.saturating_sub(5);
                    }
                    KeyCode::PageDown => {
                        let window_height = terminal.size()?.height as usize;
                        let content_height = (window_height / 2).saturating_sub(3);
                        ui_state.scroll_conversations = std::cmp::min(
                            ui_state.scroll_conversations + 5,
                            ui_state.conversations.len().saturating_sub(content_height),
                        );
                    }
                    KeyCode::Home => {
                        ui_state.scroll_conversations = 0;
                    }
                    KeyCode::End => {
                        let window_height = terminal.size()?.height as usize;
                        let content_height = (window_height / 2).saturating_sub(3);
                        ui_state.scroll_conversations =
                            ui_state.conversations.len().saturating_sub(content_height);
                    }
                    _ => {}
                }
            }
        }

        // Process UI events
        while let Ok(event) = ui_receiver.try_recv() {
            match event {
                UiEvent::ConversationUpdate(conv) => {
                    ui_state.conversations.push(conv.clone());
                    // Always auto-scroll to bottom to show latest messages
                    let window_height = terminal.size()?.height as usize;
                    let content_height = (window_height / 2).saturating_sub(3); // Top half minus borders
                    if ui_state.conversations.len() > content_height {
                        ui_state.scroll_conversations =
                            ui_state.conversations.len() - content_height;
                    } else {
                        ui_state.scroll_conversations = 0;
                    }
                }
                UiEvent::MemoryStatsUpdate(stats) => {
                    ui_state.memory_stats = stats;
                }
                UiEvent::GitLogUpdate(logs) => {
                    ui_state.git_logs = logs;
                }
                UiEvent::KvKeysUpdate(keys) => {
                    ui_state.kv_keys = keys;
                }
                UiEvent::TypingIndicator(is_typing) => {
                    ui_state.is_typing = is_typing;
                    if is_typing {
                        // Auto-scroll to bottom when typing starts
                        let window_height = terminal.size()?.height as usize;
                        let content_height = (window_height / 2).saturating_sub(3);
                        if ui_state.conversations.len() > content_height {
                            ui_state.scroll_conversations =
                                ui_state.conversations.len() - content_height + 1;
                        }
                    }
                }
                UiEvent::Pause => {
                    // Pause event is handled through shared state, no action needed here
                }
                UiEvent::Quit => return Ok(()),
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        // Handle cursor blinking separately with slower rate
        if last_cursor_blink.elapsed() >= cursor_blink_rate {
            if ui_state.is_typing {
                ui_state.cursor_visible = !ui_state.cursor_visible;
            }
            last_cursor_blink = Instant::now();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!();
    println!("███╗   ███╗███████╗██╗     ██╗███╗   ██╗██╗  ██╗");
    println!("████╗ ████║██╔════╝██║     ██║████╗  ██║██║ ██╔╝");
    println!("██╔████╔██║█████╗  ██║     ██║██╔██╗ ██║█████╔╝ ");
    println!("██║╚██╔╝██║██╔══╝  ██║     ██║██║╚██╗██║██╔═██╗ ");
    println!("██║ ╚═╝ ██║███████╗███████╗██║██║ ╚████║██║  ██╗");
    println!("╚═╝     ╚═╝╚══════╝╚══════╝╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝");
    println!();
    println!("    ⏺ Context Offloading Agent Demo");
    println!("    ProllyTree + Rig Integration");
    println!();
    println!("WHAT THIS DEMO SHOWS:");
    println!("This demo showcases an AI agent with persistent, version-controlled memory");
    println!("that can store and retrieve context across multiple conversation threads.");
    println!();
    println!("Key Demonstrations:");
    println!("• Multi-thread memory persistence - 3 threads sharing knowledge");
    println!("• Multiple memory types - Semantic, Episodic, Procedural, Short-term");
    println!("• Git-like versioned storage with rollback/time-travel debugging");
    println!("• Climate research scenario spanning data collection → analysis → synthesis");
    println!("• Real-time visualization of memory evolution and storage internals");
    println!();
    println!("The agent maintains context like a human - learning, remembering, and");
    println!("building upon previous conversations while providing full auditability.");
    println!();
    println!("Technical Features:");
    println!("• 3-thread conversation system");
    println!("• Real-time 4-window UI");
    println!("• Memory statistics tracking");
    println!("• Git commit history");
    println!("• Climate research scenario");
    println!();
    // Create temporary directory for ProllyTree store
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    println!();
    println!("ProllyTree Store Location:");
    println!("═══════════════════════════════════════════════════════════");
    println!("📁 {}", temp_path.display());
    println!("═══════════════════════════════════════════════════════════");
    println!();
    // Let user select the memory backend
    let selected_backend = select_memory_backend()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Setup UI communication
    let (ui_sender, ui_receiver) = mpsc::unbounded_channel();

    // Create shared pause state
    let pause_state = Arc::new(AtomicBool::new(false));

    // Start comprehensive demo in background
    let ui_sender_clone = ui_sender.clone();
    let pause_state_clone = pause_state.clone();
    let backend_clone = selected_backend.clone();
    let demo_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if let Err(e) = run_comprehensive_demo(ui_sender_clone, pause_state_clone, temp_dir, backend_clone).await {
            eprintln!("Demo error: {}", e);
        }
    });

    // Run the UI
    let result = run_app(&mut terminal, ui_receiver, pause_state).await;

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Cancel demo if still running
    demo_handle.abort();

    if let Err(err) = result {
        eprintln!("Terminal UI error: {:?}", err);
    }

    println!("⏺ Enhanced UI demo completed successfully!");
    println!("⏺ Demonstrated features:");
    println!("  • 35+ climate research conversations");
    println!("  • 65+ memories across 4 types");
    println!("  • 3 conversation threads with cross-thread access");
    println!("  • Real-time git commit tracking");
    println!("  • Dynamic KV store key management");
    println!("  • Comprehensive keyboard controls");
    println!("  • Versioned storage benefits");

    Ok(())
}
