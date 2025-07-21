use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use dotenv::dotenv;
use std::env;

mod advisor;
mod benchmarks;
mod memory;
mod security;
mod validation;
mod visualization;

use advisor::FinancialAdvisor;
use memory::display::MemoryVisualizer;
use security::attack_simulator::AttackSimulator;

#[derive(Parser)]
#[command(name = "financial-advisor")]
#[command(about = "Financial Advisory AI with Versioned Memory - Demonstrating Memory Consistency")]
struct Cli {
    /// Path to store the agent memory
    #[arg(short, long, default_value = "./advisor_memory")]
    storage: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start interactive advisory session
    Advise {
        /// Enable verbose memory operations display
        #[arg(short, long)]
        verbose: bool,
    },

    /// Visualize memory evolution
    Visualize {
        /// Show memory tree structure
        #[arg(short, long)]
        tree: bool,
        /// Show validation chains
        #[arg(short, long)]
        validation: bool,
        /// Show audit trail
        #[arg(short, long)]
        audit: bool,
    },

    /// Run attack simulations
    Attack {
        #[command(subcommand)]
        attack_type: AttackType,
    },

    /// Run performance benchmarks
    Benchmark {
        /// Number of operations
        #[arg(short, long, default_value = "1000")]
        operations: usize,
    },

    /// Show integration examples
    Examples,

    /// Audit memory for compliance
    Audit {
        /// Start date for audit (YYYY-MM-DD)
        #[arg(short, long)]
        from: Option<String>,
        /// End date for audit (YYYY-MM-DD)
        #[arg(short, long)]
        to: Option<String>,
    },
}

#[derive(Subcommand)]
enum AttackType {
    /// Attempt direct memory injection
    Injection {
        /// The malicious instruction to inject
        #[arg(short, long)]
        payload: String,
    },

    /// Simulate data poisoning attack
    Poisoning {
        /// Number of subtle manipulation attempts
        #[arg(short, long, default_value = "5")]
        attempts: usize,
    },

    /// Test hallucination prevention
    Hallucination {
        /// Topic to hallucinate about
        #[arg(short, long)]
        topic: String,
    },

    /// Context confusion attack
    ContextBleed {
        /// Number of parallel sessions
        #[arg(short, long, default_value = "3")]
        sessions: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("financial_advisor=debug")
        .init();

    let cli = Cli::parse();

    println!(
        "{}",
        "🏦 Financial Advisory AI - Memory Consistency Demo"
            .bold()
            .blue()
    );
    println!("{}", "━".repeat(60).dimmed());
    println!("📂 Memory storage: {}", cli.storage.yellow());
    println!();

    // Get API key
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!(
                "{}",
                "❌ Please set OPENAI_API_KEY environment variable".red()
            );
            std::process::exit(1);
        }
    };

    match cli.command {
        Commands::Advise { verbose } => {
            run_advisory_session(&cli.storage, &api_key, verbose).await?;
        }

        Commands::Visualize {
            tree,
            validation,
            audit,
        } => {
            run_visualization(&cli.storage, tree, validation, audit).await?;
        }

        Commands::Attack { attack_type } => {
            run_attack_simulation(&cli.storage, &api_key, attack_type).await?;
        }

        Commands::Benchmark { operations } => {
            run_benchmarks(&cli.storage, operations).await?;
        }

        Commands::Examples => {
            show_integration_examples();
        }

        Commands::Audit { from, to } => {
            run_compliance_audit(&cli.storage, from, to).await?;
        }
    }

    Ok(())
}

async fn run_advisory_session(storage: &str, api_key: &str, verbose: bool) -> Result<()> {
    println!(
        "{}",
        "🎯 Starting Financial Advisory Session".green().bold()
    );
    println!("{}", "Type 'help' for commands, 'exit' to quit".dimmed());
    println!();

    let mut advisor = FinancialAdvisor::new(storage, api_key).await?;
    advisor.set_verbose(verbose);

    advisor.run_interactive_session().await?;

    Ok(())
}

async fn run_visualization(storage: &str, tree: bool, validation: bool, audit: bool) -> Result<()> {
    println!("{}", "📊 Memory Visualization".green().bold());

    let visualizer = MemoryVisualizer::new(storage)?;

    if tree {
        println!("\n{}", "🌳 Memory Tree Structure:".yellow());
        visualizer.show_memory_tree().await?;
    }

    if validation {
        println!("\n{}", "✅ Validation Chains:".yellow());
        visualizer.show_validation_chains().await?;
    }

    if audit {
        println!("\n{}", "📜 Audit Trail:".yellow());
        visualizer.show_audit_trail().await?;
    }

    Ok(())
}

async fn run_attack_simulation(
    storage: &str,
    api_key: &str,
    attack_type: AttackType,
) -> Result<()> {
    println!("{}", "🚨 Attack Simulation Mode".red().bold());
    println!("{}", "Testing memory security and consistency...".dimmed());
    println!();

    let mut simulator = AttackSimulator::new(storage, api_key).await?;

    match attack_type {
        AttackType::Injection { payload } => {
            simulator.simulate_injection_attack(&payload).await?;
        }
        AttackType::Poisoning { attempts } => {
            simulator.simulate_poisoning_attack(attempts).await?;
        }
        AttackType::Hallucination { topic } => {
            simulator.test_hallucination_prevention(&topic).await?;
        }
        AttackType::ContextBleed { sessions } => {
            simulator.simulate_context_bleed(sessions).await?;
        }
    }

    Ok(())
}

async fn run_benchmarks(storage: &str, operations: usize) -> Result<()> {
    println!("{}", "⚡ Performance Benchmarks".yellow().bold());
    println!("Running {operations} operations...");
    println!();

    benchmarks::run_all_benchmarks(storage, operations).await?;

    Ok(())
}

fn show_integration_examples() {
    println!("{}", "📚 Integration Examples".cyan().bold());
    println!();

    println!("{}", "1. Basic Usage:".yellow());
    println!(
        "{}",
        r#"
use financial_advisor::{FinancialAdvisor, ValidationPolicy};

let advisor = FinancialAdvisor::new("./memory", api_key).await?;
advisor.set_validation_policy(ValidationPolicy::Strict);

let recommendation = advisor.get_recommendation(
    "AAPL", 
    client_profile
).await?;
"#
        .dimmed()
    );

    println!("\n{}", "2. Custom Validation:".yellow());
    println!(
        "{}",
        r#"
use financial_advisor::{MemoryValidator, CrossReference};

let validator = MemoryValidator::new()
    .add_source("bloomberg", 0.9)
    .add_source("yahoo_finance", 0.7)
    .min_sources(2);
    
advisor.set_validator(validator);
"#
        .dimmed()
    );

    println!("\n{}", "3. Audit Trail:".yellow());
    println!(
        "{}",
        r#"
use financial_advisor::AuditLogger;

let audit_trail = advisor.get_audit_trail(
    start_date,
    end_date,
    Some("AAPL recommendation")
).await?;

for entry in audit_trail {
    println!("{}: {}", entry.timestamp, entry.action);
}
"#
        .dimmed()
    );
}

async fn run_compliance_audit(
    storage: &str,
    from: Option<String>,
    to: Option<String>,
) -> Result<()> {
    println!("{}", "📋 Compliance Audit Report".blue().bold());
    println!();

    let advisor = FinancialAdvisor::new(storage, "").await?;
    let report = advisor.generate_compliance_report(from, to).await?;

    println!("{report}");

    Ok(())
}
