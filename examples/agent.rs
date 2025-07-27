use chrono::Duration;
use prollytree::agent::*;
use serde_json::json;
use std::error::Error;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🧠 Agent Memory System Demo");
    println!("============================");

    // Create a temporary directory for this demo
    let temp_dir = TempDir::new()?;
    let memory_path = temp_dir.path();

    println!("📁 Initializing memory system at: {:?}", memory_path);

    // Initialize the agent memory system
    let mut memory_system = AgentMemorySystem::init(
        memory_path,
        "demo_agent".to_string(),
        Some(Box::new(MockEmbeddingGenerator)), // Use mock embeddings
    )?;

    println!("✅ Memory system initialized successfully!\n");

    // Demonstrate Short-Term Memory
    println!("🔄 Short-Term Memory Demo");
    println!("--------------------------");

    let thread_id = "conversation_001";

    // Store some conversation turns
    memory_system
        .short_term
        .store_conversation_turn(
            thread_id,
            "user",
            "Hello! I'm looking for help with my project.",
            None,
        )
        .await?;

    memory_system.short_term.store_conversation_turn(
        thread_id,
        "assistant", 
        "Hello! I'd be happy to help you with your project. What kind of project are you working on?",
        None,
    ).await?;

    memory_system
        .short_term
        .store_conversation_turn(
            thread_id,
            "user",
            "I'm building a web application using Rust and need advice on database design.",
            None,
        )
        .await?;

    // Store some working memory
    memory_system
        .short_term
        .store_working_memory(
            thread_id,
            "user_context",
            json!({
                "project_type": "web_application",
                "language": "rust",
                "focus_area": "database_design"
            }),
            None,
        )
        .await?;

    // Retrieve conversation history
    let conversation = memory_system
        .short_term
        .get_conversation_history(thread_id, None)
        .await?;
    println!("📝 Stored {} conversation turns", conversation.len());

    for (i, turn) in conversation.iter().enumerate() {
        let role = turn
            .content
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("unknown");
        let content = turn
            .content
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("");
        println!("  {}. {}: {}", i + 1, role, content);
    }

    // Get working memory
    if let Some(context) = memory_system
        .short_term
        .get_working_memory(thread_id, "user_context")
        .await?
    {
        println!("🧠 Working memory context: {}", context);
    }

    println!();

    // Demonstrate Semantic Memory
    println!("🧩 Semantic Memory Demo");
    println!("------------------------");

    // Store facts about entities
    memory_system
        .semantic
        .store_fact(
            "programming_language",
            "rust",
            json!({
                "type": "systems_programming",
                "paradigm": ["functional", "imperative", "object-oriented"],
                "memory_safety": true,
                "performance": "high",
                "use_cases": ["web_backends", "system_tools", "blockchain"]
            }),
            0.95,
            "knowledge_base",
        )
        .await?;

    memory_system
        .semantic
        .store_fact(
            "database",
            "postgresql",
            json!({
                "type": "relational",
                "acid_compliant": true,
                "supports_json": true,
                "good_for": ["web_applications", "analytics", "geospatial"]
            }),
            0.9,
            "knowledge_base",
        )
        .await?;

    // Store relationships
    memory_system
        .semantic
        .store_relationship(
            ("programming_language", "rust"),
            ("database", "postgresql"),
            "commonly_used_with",
            Some(json!({
                "drivers": ["tokio-postgres", "sqlx", "diesel"],
                "compatibility": "excellent"
            })),
            0.85,
        )
        .await?;

    // Retrieve entity facts
    let rust_facts = memory_system
        .semantic
        .get_entity_facts("programming_language", "rust")
        .await?;
    println!("📊 Stored {} facts about Rust", rust_facts.len());

    for fact in &rust_facts {
        if let Some(fact_data) = fact.content.get("fact") {
            println!("  - {}", fact_data);
        }
    }

    // Get relationships
    let rust_relationships = memory_system
        .semantic
        .get_entity_relationships("programming_language", "rust")
        .await?;
    println!(
        "🔗 Found {} relationships for Rust",
        rust_relationships.len()
    );

    println!();

    // Demonstrate Episodic Memory
    println!("📚 Episodic Memory Demo");
    println!("------------------------");

    // Store an interaction episode
    memory_system
        .episodic
        .store_interaction(
            "technical_consultation",
            vec!["demo_agent".to_string(), "user".to_string()],
            "User sought advice on Rust web development and database design",
            json!({
                "topics_discussed": ["rust", "web_development", "database_design", "postgresql"],
                "user_experience_level": "intermediate",
                "consultation_outcome": "successful"
            }),
            Some(0.8), // Positive sentiment
        )
        .await?;

    // Store a learning episode
    memory_system
        .episodic
        .store_episode(
            "knowledge_acquisition",
            "Learned about user's project requirements and provided relevant technical guidance",
            json!({
                "knowledge_domain": "web_development",
                "interaction_type": "Q&A",
                "topics": ["rust", "postgresql", "database_design"]
            }),
            Some(json!({
                "knowledge_transferred": true,
                "user_satisfaction": "high"
            })),
            0.7,
        )
        .await?;

    // Query recent episodes
    let recent_episodes = memory_system
        .episodic
        .get_episodes_in_period(chrono::Utc::now() - Duration::hours(1), chrono::Utc::now())
        .await?;

    println!("📅 Found {} recent episodes", recent_episodes.len());
    for episode in &recent_episodes {
        if let Some(desc) = episode.content.get("description").and_then(|d| d.as_str()) {
            println!("  - {}", desc);
        }
    }

    println!();

    // Demonstrate Procedural Memory
    println!("⚙️ Procedural Memory Demo");
    println!("--------------------------");

    // Store a procedure for database design
    memory_system.procedural.store_procedure(
        "database_design",
        "design_web_app_schema", 
        "Standard procedure for designing database schema for web applications",
        vec![
            json!({"step": 1, "action": "Identify main entities and their attributes"}),
            json!({"step": 2, "action": "Define relationships between entities"}),
            json!({"step": 3, "action": "Normalize the schema to reduce redundancy"}),
            json!({"step": 4, "action": "Add indexes for performance optimization"}),
            json!({"step": 5, "action": "Consider security and access patterns"}),
        ],
        Some(json!({
            "applicable_when": "designing new web application database",
            "prerequisites": ["basic SQL knowledge", "understanding of application requirements"]
        })),
        10, // High priority
    ).await?;

    // Store a rule
    memory_system
        .procedural
        .store_rule(
            "consultation",
            "provide_code_examples",
            json!({
                "if": "user asks about implementation",
                "and": "topic is within knowledge domain"
            }),
            json!({
                "then": "provide concrete code examples",
                "include": ["comments", "error handling", "best practices"]
            }),
            8,    // Medium-high priority
            true, // Enabled
        )
        .await?;

    // Get procedures by category
    let db_procedures = memory_system
        .procedural
        .get_procedures_by_category("database_design")
        .await?;
    println!(
        "📋 Found {} database design procedures",
        db_procedures.len()
    );

    for procedure in &db_procedures {
        if let Some(name) = procedure.content.get("name").and_then(|n| n.as_str()) {
            if let Some(steps) = procedure.content.get("steps").and_then(|s| s.as_array()) {
                println!("  - {}: {} steps", name, steps.len());
            }
        }
    }

    // Get active rules
    let consultation_rules = memory_system
        .procedural
        .get_active_rules_by_category("consultation")
        .await?;
    println!(
        "📝 Found {} active consultation rules",
        consultation_rules.len()
    );

    println!();

    // Demonstrate System Operations
    println!("🔧 System Operations Demo");
    println!("--------------------------");

    // Create a checkpoint
    let checkpoint_id = memory_system.checkpoint("Demo session complete").await?;
    println!("💾 Created checkpoint: {}", checkpoint_id);

    // Get system statistics
    let stats = memory_system.get_system_stats().await?;
    println!("📊 System Statistics:");
    println!("  - Total memories: {}", stats.overall.total_memories);
    println!("  - Short-term threads: {}", stats.short_term.total_threads);
    println!(
        "  - Short-term conversations: {}",
        stats.short_term.total_conversations
    );
    println!("  - By type: {:?}", stats.overall.by_type);

    // Run system optimization
    println!("\n🧹 Running system optimization...");
    let optimization_report = memory_system.optimize().await?;
    println!("✅ Optimization complete:");
    println!(
        "  - Expired cleaned: {}",
        optimization_report.expired_cleaned
    );
    println!(
        "  - Memories consolidated: {}",
        optimization_report.memories_consolidated
    );
    println!(
        "  - Memories archived: {}",
        optimization_report.memories_archived
    );
    println!(
        "  - Memories pruned: {}",
        optimization_report.memories_pruned
    );
    println!(
        "  - Total processed: {}",
        optimization_report.total_processed()
    );

    println!("\n🎉 Demo completed successfully!");
    println!("The agent memory system is now ready for production use.");

    Ok(())
}
