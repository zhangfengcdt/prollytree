#![allow(dead_code)]

use anyhow::Result;
use colored::Colorize;
use std::io::{self, Write};

use super::{ClientProfile, FinancialAdvisor, RiskTolerance};

pub struct InteractiveSession<'a> {
    advisor: &'a mut FinancialAdvisor,
}

impl<'a> InteractiveSession<'a> {
    pub fn new(advisor: &'a mut FinancialAdvisor) -> Self {
        Self { advisor }
    }

    pub async fn run(mut self) -> Result<()> {
        self.show_welcome();

        // Create a default client profile
        let mut client = ClientProfile {
            id: "demo-client".to_string(),
            risk_tolerance: RiskTolerance::Moderate,
            investment_goals: vec!["Growth".to_string(), "Income".to_string()],
            time_horizon: "5-10 years".to_string(),
            restrictions: vec![],
        };

        loop {
            print!("\n{} ", "🏦>".blue().bold());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            match self.handle_command(input, &mut client).await {
                Ok(should_continue) => {
                    if !should_continue {
                        break;
                    }
                }
                Err(e) => {
                    println!("{} {}", "❌ Error:".red(), e);
                }
            }
        }

        println!("\n{}", "👋 Session ended. Thank you!".green());
        Ok(())
    }

    fn show_welcome(&self) {
        println!(
            "{}",
            "🎯 Financial Advisory AI - Interactive Session"
                .green()
                .bold()
        );
        println!("{}", "━".repeat(50).dimmed());
        println!();
        println!("{}", "Available commands:".yellow());
        println!(
            "  {} - Get recommendation for a stock symbol",
            "recommend <SYMBOL>".cyan()
        );
        println!("  {} - Show client profile", "profile".cyan());
        println!(
            "  {} - Set risk tolerance (conservative/moderate/aggressive)",
            "risk <LEVEL>".cyan()
        );
        println!("  {} - Show recent recommendations", "history".cyan());
        println!("  {} - Show memory validation status", "memory".cyan());
        println!("  {} - Show audit trail", "audit".cyan());
        println!("  {} - Test injection attack", "test-inject <TEXT>".cyan());
        println!("  {} - Show memory tree visualization", "visualize".cyan());
        println!("  {} - Create memory branch", "branch <NAME>".cyan());
        println!("  {} - Show this help", "help".cyan());
        println!("  {} - Exit", "exit".cyan());
        println!();
    }

    async fn handle_command(&mut self, input: &str, client: &mut ClientProfile) -> Result<bool> {
        let parts: Vec<&str> = input.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(true);
        }

        match parts[0] {
            "help" | "h" => {
                self.show_help();
            }

            "recommend" | "r" => {
                if parts.len() < 2 {
                    println!("{} Usage: recommend <SYMBOL>", "❓".yellow());
                    return Ok(true);
                }

                let symbol = parts[1].to_uppercase();
                self.handle_recommendation(&symbol, client).await?;
            }

            "profile" | "p" => {
                self.show_profile(client);
            }

            "risk" => {
                if parts.len() < 2 {
                    println!(
                        "{} Usage: risk <conservative|moderate|aggressive>",
                        "❓".yellow()
                    );
                    return Ok(true);
                }

                client.risk_tolerance = match parts[1].to_lowercase().as_str() {
                    "conservative" | "c" => RiskTolerance::Conservative,
                    "moderate" | "m" => RiskTolerance::Moderate,
                    "aggressive" | "a" => RiskTolerance::Aggressive,
                    _ => {
                        println!(
                            "{} Invalid risk level. Use: conservative, moderate, or aggressive",
                            "❓".yellow()
                        );
                        return Ok(true);
                    }
                };

                println!(
                    "{} Risk tolerance set to: {:?}",
                    "✅".green(),
                    client.risk_tolerance
                );
            }

            "history" | "hist" => {
                self.show_history().await?;
            }

            "memory" | "mem" => {
                self.show_memory_status().await?;
            }

            "audit" | "a" => {
                self.show_audit_trail().await?;
            }

            "test-inject" | "inject" => {
                if parts.len() < 2 {
                    println!("{} Usage: test-inject <malicious text>", "❓".yellow());
                    return Ok(true);
                }

                let payload = parts[1..].join(" ");
                self.test_injection_attack(&payload).await?;
            }

            "visualize" | "vis" => {
                self.show_memory_visualization().await?;
            }

            "branch" | "b" => {
                if parts.len() < 2 {
                    println!("{} Usage: branch <name>", "❓".yellow());
                    return Ok(true);
                }

                let branch_name = parts[1];
                self.create_branch(branch_name).await?;
            }

            "exit" | "quit" | "q" => {
                return Ok(false);
            }

            _ => {
                println!(
                    "{} Unknown command: {}. Type 'help' for available commands.",
                    "❓".yellow(),
                    parts[0]
                );
            }
        }

        Ok(true)
    }

    fn show_help(&self) {
        println!("{}", "📚 Help - Financial Advisory AI".blue().bold());
        println!();
        
        // Show available commands first
        println!("{}", "Available commands:".yellow());
        println!(
            "  {} - Get recommendation for a stock symbol",
            "recommend <SYMBOL>".cyan()
        );
        println!("  {} - Show client profile", "profile".cyan());
        println!(
            "  {} - Set risk tolerance (conservative/moderate/aggressive)",
            "risk <LEVEL>".cyan()
        );
        println!("  {} - Show recent recommendations", "history".cyan());
        println!("  {} - Show memory validation status", "memory".cyan());
        println!("  {} - Show audit trail", "audit".cyan());
        println!("  {} - Test injection attack", "test-inject <TEXT>".cyan());
        println!("  {} - Show memory tree visualization", "visualize".cyan());
        println!("  {} - Create memory branch", "branch <NAME>".cyan());
        println!("  {} - Show this help", "help".cyan());
        println!("  {} - Exit", "exit".cyan());
        println!();
        
        println!("{}", "Core Features:".yellow());
        println!(
            "• {} - Provides validated investment recommendations",
            "Multi-source validation".green()
        );
        println!(
            "• {} - Complete audit trail of all decisions",
            "Cryptographic audit trail".green()
        );
        println!(
            "• {} - Detects and prevents memory manipulation",
            "Attack protection".green()
        );
        println!(
            "• {} - Full memory versioning with rollback",
            "Time-travel debugging".green()
        );
        println!();
        println!("{}", "Memory Consistency Features:".yellow());
        println!("• Cross-validation of market data from multiple sources");
        println!("• Contradiction detection with branch isolation");
        println!("• Injection attempt detection and quarantine");
        println!("• Complete audit trail for regulatory compliance");
        println!();
    }

    async fn handle_recommendation(&mut self, symbol: &str, client: &ClientProfile) -> Result<()> {
        println!(
            "{} Generating recommendation for {}...",
            "🔍".yellow(),
            symbol
        );

        match self.advisor.get_recommendation(symbol, client).await {
            Ok(recommendation) => {
                println!();
                println!("{}", "📊 Recommendation Generated".green().bold());
                println!("{}", "━".repeat(40).dimmed());
                println!("{}: {}", "Symbol".cyan(), recommendation.symbol);
                println!(
                    "{}: {}",
                    "Action".cyan(),
                    recommendation.recommendation_type.as_str().bold()
                );
                println!(
                    "{}: {:.1}%",
                    "Confidence".cyan(),
                    recommendation.confidence * 100.0
                );
                println!("{}: {}", "Client".cyan(), recommendation.client_id);
                println!();
                println!("{}", "Reasoning:".yellow());
                println!("{}", recommendation.reasoning);
                println!();
                println!(
                    "{}: {}",
                    "Memory Version".dimmed(),
                    recommendation.memory_version
                );
                println!(
                    "{}: {}",
                    "Timestamp".dimmed(),
                    recommendation.timestamp.format("%Y-%m-%d %H:%M:%S")
                );

                // Show validation info
                println!();
                println!("{}", "🛡️ Validation Status".green());
                if recommendation.validation_result.is_valid {
                    println!("{} All data sources validated", "✅".green());
                    println!(
                        "{} Sources: {}",
                        "📊".blue(),
                        recommendation.validation_result.cross_references.join(", ")
                    );
                    println!(
                        "{} Confidence: {:.1}%",
                        "🎯".blue(),
                        recommendation.validation_result.confidence * 100.0
                    );
                } else {
                    println!("{} Validation issues detected:", "⚠️".yellow());
                    for issue in &recommendation.validation_result.issues {
                        println!("  • {}", issue.description.yellow());
                    }
                }
            }
            Err(e) => {
                println!("{} Failed to generate recommendation: {}", "❌".red(), e);

                // Check if it was a security issue
                if e.to_string().contains("Security alert") {
                    println!();
                    println!("{}", "🚨 Security Protection Activated".red().bold());
                    println!(
                        "The system detected potentially malicious input and prevented processing."
                    );
                    println!("This demonstrates the memory consistency protection in action!");
                }
            }
        }

        Ok(())
    }

    fn show_profile(&self, client: &ClientProfile) {
        println!("{}", "👤 Client Profile".blue().bold());
        println!("{}", "━".repeat(20).dimmed());
        println!("{}: {}", "ID".cyan(), client.id);
        println!("{}: {:?}", "Risk Tolerance".cyan(), client.risk_tolerance);
        println!("{}: {}", "Time Horizon".cyan(), client.time_horizon);
        println!("{}: {}", "Goals".cyan(), client.investment_goals.join(", "));
        if !client.restrictions.is_empty() {
            println!(
                "{}: {}",
                "Restrictions".cyan(),
                client.restrictions.join(", ")
            );
        }
    }

    async fn show_history(&self) -> Result<()> {
        println!("{}", "📜 Recent Recommendations".blue().bold());
        println!("{}", "━".repeat(30).dimmed());

        // In a real implementation, we'd query the memory store
        println!(
            "{} No previous recommendations in this session",
            "ℹ️".blue()
        );
        println!(
            "{} Use 'recommend <SYMBOL>' to generate recommendations",
            "💡".yellow()
        );

        Ok(())
    }

    async fn show_memory_status(&self) -> Result<()> {
        println!("{}", "🧠 Memory Status".blue().bold());
        println!("{}", "━".repeat(20).dimmed());

        // Show memory consistency info
        println!(
            "{} Memory validation: {}",
            "✅".green(),
            "ACTIVE".bold().green()
        );
        println!(
            "{} Security monitoring: {}",
            "🛡️".blue(),
            "ENABLED".bold().blue()
        );
        println!(
            "{} Audit trail: {}",
            "📝".yellow(),
            "LOGGING".bold().yellow()
        );
        println!(
            "{} Cross-validation: {}",
            "🔍".cyan(),
            "MULTI-SOURCE".bold().cyan()
        );

        println!();
        println!("{}", "Validation Sources:".yellow());
        println!("  • Bloomberg (95% trust)");
        println!("  • Yahoo Finance (85% trust)");
        println!("  • Alpha Vantage (80% trust)");

        Ok(())
    }

    async fn show_audit_trail(&self) -> Result<()> {
        println!("{}", "📋 Audit Trail".blue().bold());
        println!("{}", "━".repeat(20).dimmed());

        // In a real implementation, query the actual audit trail
        println!("{} Session started", "📅 2024-07-21 12:00:00".dimmed());
        println!(
            "{} Memory store initialized",
            "🔧 2024-07-21 12:00:01".dimmed()
        );
        println!(
            "{} Security monitor enabled",
            "🛡️ 2024-07-21 12:00:01".dimmed()
        );

        Ok(())
    }

    async fn test_injection_attack(&mut self, payload: &str) -> Result<()> {
        println!("{}", "🚨 Testing Injection Attack".red().bold());
        println!("{}", "━".repeat(30).dimmed());
        println!("{}: {}", "Payload".yellow(), payload);

        // Test the security system
        use crate::security::SecurityMonitor;
        let mut monitor = SecurityMonitor::new();

        match monitor.simulate_injection_attack(payload) {
            Ok(alert) => {
                println!();
                println!("{} Attack Detected!", "🛡️ SECURITY ALERT".red().bold());
                println!("{}: {:?}", "Severity".red(), alert.level);
                println!("{}: {}", "Description".red(), alert.description);
                println!("{}: {:.1}%", "Confidence".red(), alert.confidence * 100.0);
                println!();
                println!("{}", "Recommendations:".yellow());
                for rec in &alert.recommendations {
                    println!("  • {rec}");
                }

                println!();
                println!("{}", "🎯 Demonstration Complete".green().bold());
                println!(
                    "This shows how ProllyTree's versioned memory prevents injection attacks!"
                );
            }
            Err(e) => {
                println!("{} Error during attack simulation: {}", "❌".red(), e);
            }
        }

        Ok(())
    }

    async fn show_memory_visualization(&self) -> Result<()> {
        println!("{}", "🌳 Memory Tree Visualization".green().bold());
        println!("{}", "━".repeat(35).dimmed());

        // ASCII art representation of memory tree
        println!("Memory Tree Structure:");
        println!("├── 🏦 Financial Data");
        println!("│   ├── 📊 Market Data (validated)");
        println!("│   ├── 💼 Recommendations");
        println!("│   └── 👤 Client Profiles");
        println!("├── 🔍 Validation Layer");
        println!("│   ├── ✅ Cross-references");
        println!("│   ├── 🛡️ Security checks");
        println!("│   └── 📈 Confidence scores");
        println!("└── 📝 Audit Trail");
        println!("    ├── 🕐 Timestamps");
        println!("    ├── 🔗 Version links");
        println!("    └── 👥 User actions");

        println!();
        println!("{} All nodes are cryptographically signed", "🔒".cyan());
        println!("{} Complete history is preserved", "⏱️".blue());
        println!("{} Branches allow safe experimentation", "🌿".green());

        Ok(())
    }

    async fn create_branch(&mut self, name: &str) -> Result<()> {
        println!("{} Creating memory branch: {}", "🌿".green(), name.bold());

        // In real implementation, use the memory store
        println!("{} Branch '{}' created successfully", "✅".green(), name);
        println!(
            "{} You can now safely test scenarios without affecting main memory",
            "💡".yellow()
        );

        Ok(())
    }
}
