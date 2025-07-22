use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use dotenv::dotenv;
use std::env;

use rig_versioned_memory::{agent::VersionedAgent, utils};

#[derive(Parser)]
#[command(name = "rig-memory-demo")]
#[command(about = "ProllyTree Versioned AI Agent Demo", long_about = None)]
struct Cli {
    /// Path to store the agent memory (default: ./demo_agent_memory)
    #[arg(short, long, global = true, default_value = "./demo_agent_memory")]
    storage: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run memory learning and rollback demo
    Learning,
    /// Run memory branching demo
    Branching,
    /// Run audit trail demo
    Audit,
    /// Run episodic learning demo
    Episodic,
    /// Run all demos
    All,
    /// Start interactive chat mode
    Chat,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments first
    let cli = Cli::parse();

    // Load environment variables from .env file if present
    dotenv().ok();

    utils::print_banner();

    println!(
        "📂 {}: {}",
        "Memory storage location".dimmed(),
        cli.storage.yellow()
    );

    // Get API key
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            utils::print_error("Please set OPENAI_API_KEY environment variable");
            println!("\n{}", "You can either:".dimmed());
            println!(
                "  1. Export it: {}",
                "export OPENAI_API_KEY=\"your-key-here\"".yellow()
            );
            println!(
                "  2. Create a .env file with: {}",
                "OPENAI_API_KEY=your-key-here".yellow()
            );
            std::process::exit(1);
        }
    };

    // Initialize agent
    let mut agent = match VersionedAgent::new(api_key, &cli.storage).await {
        Ok(agent) => {
            utils::print_success(&format!(
                "Initialized versioned memory store at: {}",
                cli.storage
            ));
            agent
        }
        Err(e) => {
            utils::print_error(&format!("Failed to initialize agent: {e}"));
            std::process::exit(1);
        }
    };

    match cli.command {
        Some(Commands::Learning) => {
            agent.demo_memory_learning().await?;
        }
        Some(Commands::Branching) => {
            agent.demo_branching().await?;
        }
        Some(Commands::Audit) => {
            agent.demo_audit_trail().await?;
        }
        Some(Commands::Episodic) => {
            agent.demo_episodic_learning().await?;
        }
        Some(Commands::All) => {
            agent.demo_memory_learning().await?;
            utils::print_demo_separator();

            agent.demo_branching().await?;
            utils::print_demo_separator();

            agent.demo_audit_trail().await?;
            utils::print_demo_separator();

            agent.demo_episodic_learning().await?;
        }
        Some(Commands::Chat) | None => {
            // Default to interactive mode
            println!(
                "\n{}",
                "No demo specified, starting interactive mode...".dimmed()
            );
            agent.run_interactive_mode().await?;
        }
    }

    println!("\n{}", "👋 Demo completed!".green().bold());
    Ok(())
}
