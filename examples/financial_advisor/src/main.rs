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
    #[arg(short, long, default_value = "./advisor_memory/data")]
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

    /// Git memory operations
    Memory {
        #[command(subcommand)]
        git_command: GitCommand,
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

#[derive(Subcommand)]
enum GitCommand {
    /// Show memory commit history
    History {
        /// Limit number of commits to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Create a new memory branch
    Branch {
        /// Branch name
        name: String,
    },

    /// List all memory branches
    Branches,

    /// Show commit details
    Show {
        /// Commit hash or branch name
        commit: String,
    },

    /// View memory at specific commit
    At {
        /// Commit hash
        commit: String,
        /// Memory type filter
        #[arg(short, long)]
        memory_type: Option<String>,
    },

    /// Compare memory between two time points
    Compare {
        /// From time (YYYY-MM-DD HH:MM or commit hash)
        from: String,
        /// To time (YYYY-MM-DD HH:MM or commit hash)
        to: String,
    },

    /// Merge memory branch
    Merge {
        /// Branch to merge
        branch: String,
    },

    /// Revert to specific commit
    Revert {
        /// Commit hash to revert to
        commit: String,
    },

    /// Visualize memory evolution
    Graph {
        /// Show full commit graph
        #[arg(short, long)]
        full: bool,
        /// Memory type filter
        #[arg(short, long)]
        memory_type: Option<String>,
    },

    /// Memory statistics and analytics
    Stats {
        /// Date range for statistics (YYYY-MM-DD)
        #[arg(short, long)]
        since: Option<String>,
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

        Commands::Memory { git_command } => {
            run_memory_command(&cli.storage, git_command).await?;
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

async fn run_memory_command(storage: &str, git_command: GitCommand) -> Result<()> {
    println!("{}", "🧠 Memory Operations".cyan().bold());
    println!("{}", "━".repeat(50).dimmed());
    println!();

    use financial_advisor::memory::MemoryStore;
    let mut memory_store = MemoryStore::new(storage).await?;

    match git_command {
        GitCommand::History { limit } => {
            println!(
                "{}",
                format!("📜 Memory Commit History (last {limit})").yellow()
            );
            println!();

            let history = memory_store.get_memory_history(Some(limit)).await?;

            for commit in history {
                let memory_icon = match commit.memory_type {
                    financial_advisor::memory::MemoryType::MarketData => "📈",
                    financial_advisor::memory::MemoryType::Recommendation => "💡",
                    financial_advisor::memory::MemoryType::Audit => "📋",
                    financial_advisor::memory::MemoryType::ClientProfile => "👤",
                    financial_advisor::memory::MemoryType::System => "⚙️",
                    financial_advisor::memory::MemoryType::Security => "🛡️",
                };

                println!(
                    "{} {} {}",
                    memory_icon,
                    commit.hash[..8].yellow(),
                    commit.message
                );
                println!(
                    "   {} | {}",
                    commit
                        .timestamp
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                        .dimmed(),
                    format!("{:?}", commit.memory_type).cyan()
                );
                println!();
            }
        }

        GitCommand::Branch { name } => {
            println!("{}", format!("🌿 Creating memory branch: {name}").green());
            println!();

            memory_store.create_branch(&name).await?;
            println!("✅ Branch '{}' created successfully", name.green());
        }

        GitCommand::Branches => {
            println!("{}", "🌿 Memory Branches".green());
            println!();

            let branches = memory_store.list_branches()?;
            let current_branch = memory_store.current_branch();

            for branch in branches {
                if branch == current_branch {
                    println!("* {} {}", branch.green().bold(), "(current)".dimmed());
                } else {
                    println!("  {branch}");
                }
            }
        }

        GitCommand::Show { commit } => {
            println!("{}", format!("🔍 Commit Details: {commit}").yellow());
            println!();

            let details = memory_store.show_memory_commit(&commit).await?;

            println!("Commit: {}", details.hash.yellow());
            println!("Author: {}", details.author);
            println!("Date:   {}", details.timestamp.format("%Y-%m-%d %H:%M:%S"));
            println!("Message: {}", details.message);
            println!();
            println!("Memory Impact: {}", details.memory_impact.cyan());

            if !details.changed_files.is_empty() {
                println!();
                println!("Changed files:");
                for file in details.changed_files {
                    println!("  📄 {file}");
                }
            }
        }

        GitCommand::At {
            commit,
            memory_type,
        } => {
            println!("{}", format!("⏰ Memory at commit: {commit}").yellow());
            println!();

            match memory_type.as_deref() {
                Some("recommendations") => {
                    let recommendations =
                        memory_store.get_recommendations_at_commit(&commit).await?;
                    println!("📊 Found {} recommendations:", recommendations.len());
                    for rec in recommendations.iter().take(5) {
                        println!("  💡 {} (confidence: {:.2})", rec.id, rec.confidence);
                    }
                }
                Some("market_data") => {
                    let market_data = memory_store
                        .get_market_data_at_commit(&commit, None)
                        .await?;
                    println!("📈 Found {} market data entries:", market_data.len());
                    for data in market_data.iter().take(5) {
                        println!("  📈 {} (confidence: {:.2})", data.id, data.confidence);
                    }
                }
                _ => {
                    // Show all memory types
                    let recommendations = memory_store
                        .get_recommendations_at_commit(&commit)
                        .await
                        .unwrap_or_default();
                    let market_data = memory_store
                        .get_market_data_at_commit(&commit, None)
                        .await
                        .unwrap_or_default();

                    println!("📊 Recommendations: {}", recommendations.len());
                    println!("📈 Market Data: {}", market_data.len());
                    println!(
                        "📋 Total memories: {}",
                        recommendations.len() + market_data.len()
                    );
                }
            }
        }

        GitCommand::Compare { from, to } => {
            println!("{}", format!("🔄 Comparing: {from} → {to}").yellow());
            println!();

            // Try to parse as timestamps, otherwise use as commit hashes
            if let (Ok(from_time), Ok(to_time)) = (
                chrono::DateTime::parse_from_str(
                    &format!("{from} 00:00:00 +0000"),
                    "%Y-%m-%d %H:%M:%S %z",
                ),
                chrono::DateTime::parse_from_str(
                    &format!("{to} 23:59:59 +0000"),
                    "%Y-%m-%d %H:%M:%S %z",
                ),
            ) {
                let comparison = memory_store
                    .compare_memory_states(from_time.into(), to_time.into())
                    .await?;

                println!("📊 Memory Changes Summary:");
                println!(
                    "  Recommendations: {}",
                    if comparison.recommendation_changes >= 0 {
                        format!("+{}", comparison.recommendation_changes).green()
                    } else {
                        comparison.recommendation_changes.to_string().red()
                    }
                );
                println!(
                    "  Market Data: {}",
                    if comparison.market_data_changes >= 0 {
                        format!("+{}", comparison.market_data_changes).green()
                    } else {
                        comparison.market_data_changes.to_string().red()
                    }
                );
                println!(
                    "  Total Change: {}",
                    if comparison.total_memory_change >= 0 {
                        format!("+{}", comparison.total_memory_change).green()
                    } else {
                        comparison.total_memory_change.to_string().red()
                    }
                );
                println!();
                println!("{}", comparison.summary);
            } else {
                println!("⚠️ Date parsing not implemented for commit hashes yet");
                println!("Please use YYYY-MM-DD format");
            }
        }

        GitCommand::Merge { branch } => {
            println!("{}", format!("🔀 Merging branch: {branch}").yellow());
            println!();

            let result = memory_store.merge_memory_branch(&branch).await?;
            println!("✅ Merge completed: {}", result.green());
        }

        GitCommand::Revert { commit } => {
            println!("{}", format!("⏪ Reverting to commit: {commit}").yellow());
            println!();

            let new_commit = memory_store.revert_to_commit(&commit).await?;
            println!(
                "✅ Reverted to commit {}. New commit: {}",
                commit.green(),
                new_commit.green()
            );
        }

        GitCommand::Graph { full, memory_type } => {
            println!("{}", "📊 Memory Evolution Graph".cyan().bold());
            println!("{}", "━".repeat(50).dimmed());
            println!();

            let limit = if full { None } else { Some(20) };
            let history = memory_store.get_memory_history(limit).await?;

            if history.is_empty() {
                println!("No memory history found.");
                return Ok(());
            }

            // Filter by memory type if specified
            let filtered_history: Vec<_> = if let Some(filter_type) = memory_type {
                history
                    .into_iter()
                    .filter(|commit| {
                        format!("{:?}", commit.memory_type)
                            .to_lowercase()
                            .contains(&filter_type.to_lowercase())
                    })
                    .collect()
            } else {
                history
            };

            // Draw ASCII graph
            println!("Time flows ↓");
            println!();

            for (i, commit) in filtered_history.iter().enumerate() {
                let memory_icon = match commit.memory_type {
                    financial_advisor::memory::MemoryType::MarketData => "📈",
                    financial_advisor::memory::MemoryType::Recommendation => "💡",
                    financial_advisor::memory::MemoryType::Audit => "📋",
                    financial_advisor::memory::MemoryType::ClientProfile => "👤",
                    financial_advisor::memory::MemoryType::System => "⚙️",
                    financial_advisor::memory::MemoryType::Security => "🛡️",
                };

                let connector = if i == 0 { " " } else { "│" };
                let branch_char = if i == filtered_history.len() - 1 {
                    "└"
                } else {
                    "├"
                };

                if i > 0 {
                    println!("{}   {}", connector, "│".dimmed());
                }

                println!(
                    "{} {} {}",
                    branch_char.cyan(),
                    memory_icon,
                    commit.hash[..8].yellow()
                );

                println!(
                    "{}   {} {}",
                    if i == filtered_history.len() - 1 {
                        " "
                    } else {
                        "│"
                    },
                    "└─".dimmed(),
                    commit.message.green()
                );

                println!(
                    "{}     {} | {}",
                    if i == filtered_history.len() - 1 {
                        " "
                    } else {
                        "│"
                    },
                    commit.timestamp.format("%m-%d %H:%M").to_string().dimmed(),
                    format!("{:?}", commit.memory_type).cyan()
                );
            }

            println!();
            println!(
                "Legend: 📈 Market Data | 💡 Recommendations | 📋 Audit | 👤 Clients | ⚙️ System"
            );
        }

        GitCommand::Stats { since } => {
            println!("{}", "📊 Memory Analytics".cyan().bold());
            println!("{}", "━".repeat(50).dimmed());
            println!();

            let history = memory_store.get_memory_history(None).await?;

            if history.is_empty() {
                println!("No memory history found.");
                return Ok(());
            }

            // Filter by date if specified
            let filtered_history: Vec<_> = if let Some(since_date) = since {
                if let Ok(since_time) = chrono::NaiveDate::parse_from_str(&since_date, "%Y-%m-%d") {
                    let since_datetime = since_time.and_hms_opt(0, 0, 0).unwrap().and_utc();
                    history
                        .into_iter()
                        .filter(|commit| commit.timestamp >= since_datetime)
                        .collect()
                } else {
                    history
                }
            } else {
                history
            };

            // Count by memory type
            let mut stats = std::collections::HashMap::new();
            for commit in &filtered_history {
                *stats.entry(commit.memory_type).or_insert(0) += 1;
            }

            println!("🎯 Memory Commit Statistics:");
            println!();

            let total = filtered_history.len();
            for (memory_type, count) in stats {
                let percentage = if total > 0 {
                    (count as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                let icon = match memory_type {
                    financial_advisor::memory::MemoryType::MarketData => "📈",
                    financial_advisor::memory::MemoryType::Recommendation => "💡",
                    financial_advisor::memory::MemoryType::Audit => "📋",
                    financial_advisor::memory::MemoryType::ClientProfile => "👤",
                    financial_advisor::memory::MemoryType::System => "⚙️",
                    financial_advisor::memory::MemoryType::Security => "🛡️",
                };

                let bar_length = (percentage / 100.0 * 30.0) as usize;
                let bar = "█".repeat(bar_length);
                let empty_bar = "░".repeat(30 - bar_length);

                println!(
                    "{} {:12} │{}{} │ {:3} ({:.1}%)",
                    icon,
                    format!("{:?}", memory_type),
                    bar.green(),
                    empty_bar.dimmed(),
                    count,
                    percentage
                );
            }

            println!();
            println!("📈 Summary:");
            println!("  Total commits: {}", total.to_string().yellow());

            if !filtered_history.is_empty() {
                let oldest = &filtered_history.last().unwrap();
                let newest = &filtered_history.first().unwrap();
                println!(
                    "  Time range: {} → {}",
                    oldest.timestamp.format("%Y-%m-%d").to_string().dimmed(),
                    newest.timestamp.format("%Y-%m-%d").to_string().green()
                );

                let duration = newest.timestamp - oldest.timestamp;
                let days = duration.num_days();
                if days > 0 {
                    let commits_per_day = total as f64 / days as f64;
                    println!("  Activity: {commits_per_day:.1} commits/day");
                }
            }
        }
    }

    Ok(())
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
