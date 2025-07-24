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

        // Load existing client profile or create default
        let mut client = match self.advisor.load_client_profile().await? {
            Some(profile) => {
                println!(
                    "{} Loaded existing client profile: {}",
                    "✅".green(),
                    profile.id
                );
                profile
            }
            None => {
                let default_profile = ClientProfile {
                    id: "demo-client".to_string(),
                    risk_tolerance: RiskTolerance::Moderate,
                    investment_goals: vec!["Growth".to_string(), "Income".to_string()],
                    time_horizon: "5-10 years".to_string(),
                    restrictions: vec![],
                };

                // Save the default profile so it persists
                match self.advisor.store_client_profile(&default_profile).await {
                    Ok(_) => {
                        println!(
                            "{} Created and saved new client profile: {}",
                            "🆕".blue(),
                            default_profile.id
                        );
                    }
                    Err(e) => {
                        println!(
                            "{} Created client profile but failed to save: {}",
                            "⚠️".yellow(),
                            e
                        );
                    }
                }

                default_profile
            }
        };

        loop {
            let actual_branch = self.advisor.get_actual_current_branch();
            let branch_display = format!(" [{}]", actual_branch.cyan());

            print!("\n{}{} ", "🏦".blue(), branch_display);
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
            "recommend <SYMBOL> [notes]".cyan()
        );
        println!("  {} - Show client profile", "profile".cyan());
        println!(
            "  {} - Set risk tolerance (conservative/moderate/aggressive)",
            "risk <LEVEL>".cyan()
        );
        println!("  {} - Show recent recommendations", "history".cyan());
        println!(
            "  {} - Show recommendations at specific commit",
            "history <commit>".cyan()
        );
        println!(
            "  {} - Show recommendations on branch",
            "history --branch <name>".cyan()
        );
        println!("  {} - Show memory validation status", "memory".cyan());
        println!("  {} - Show audit trail", "audit".cyan());
        println!(
            "  {} - Test injection attack",
            "test-inject <TEXT> [-- notes]".cyan()
        );
        println!("  {} - Show memory tree visualization", "visualize".cyan());
        println!(
            "  {} - Create and switch to memory branch",
            "branch <NAME>".cyan()
        );
        println!("  {} - Switch to existing branch", "switch <NAME>".cyan());
        println!("  {} - List all branches", "list-branches".cyan());
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
                    println!("{} Usage: recommend <SYMBOL> [notes]", "❓".yellow());
                    return Ok(true);
                }

                let symbol = parts[1].to_uppercase();
                let notes = if parts.len() > 2 {
                    Some(parts[2..].join(" "))
                } else {
                    None
                };
                self.handle_recommendation(&symbol, client, notes).await?;
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

                // Save the updated profile
                match self.advisor.store_client_profile(client).await {
                    Ok(_) => println!("{} Profile saved successfully", "💾".green()),
                    Err(e) => println!("{} Error: Failed to save profile: {}", "❌".red(), e),
                }

                println!(
                    "{} Risk tolerance set to: {:?}",
                    "✅".green(),
                    client.risk_tolerance
                );
            }

            "history" | "hist" => {
                if parts.len() >= 2 {
                    // history <commit_hash> or history --branch <branch_name>
                    if parts[1] == "--branch" && parts.len() >= 3 {
                        self.show_history_at_branch(parts[2]).await?;
                    } else {
                        self.show_history_at_commit(parts[1]).await?;
                    }
                } else {
                    self.show_history().await?;
                }
            }

            "memory" | "mem" => {
                self.show_memory_status().await?;
            }

            "audit" | "a" => {
                self.show_audit_trail().await?;
            }

            "test-inject" | "inject" => {
                if parts.len() < 2 {
                    println!(
                        "{} Usage: test-inject <malicious text> [-- notes]",
                        "❓".yellow()
                    );
                    return Ok(true);
                }

                // Find if there's a "--" separator for notes
                let separator_pos = parts.iter().position(|&p| p == "--");
                let (payload, notes) = if let Some(pos) = separator_pos {
                    let payload = parts[1..pos].join(" ");
                    let notes = if pos + 1 < parts.len() {
                        Some(parts[pos + 1..].join(" "))
                    } else {
                        None
                    };
                    (payload, notes)
                } else {
                    (parts[1..].join(" "), None)
                };

                self.test_injection_attack(&payload, notes).await?;
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

            "switch" | "sw" => {
                if parts.len() < 2 {
                    println!("{} Usage: switch <branch>", "❓".yellow());
                    return Ok(true);
                }

                let branch_name = parts[1];
                self.switch_branch(branch_name).await?;
            }

            "branch-info" | "bi" => {
                self.show_branch_info();
            }

            "debug-branch" | "db" => {
                println!("DEBUG: Testing git branch reading...");
                let actual = self.advisor.get_actual_current_branch();
                println!("Result: '{actual}'");
            }

            "list-branches" | "lb" => {
                self.list_branches();
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
            "recommend <SYMBOL> [notes]".cyan()
        );
        println!("  {} - Show client profile", "profile".cyan());
        println!(
            "  {} - Set risk tolerance (conservative/moderate/aggressive)",
            "risk <LEVEL>".cyan()
        );
        println!("  {} - Show recent recommendations", "history".cyan());
        println!(
            "  {} - Show recommendations at specific commit",
            "history <commit>".cyan()
        );
        println!(
            "  {} - Show recommendations on branch",
            "history --branch <name>".cyan()
        );
        println!("  {} - Show memory validation status", "memory".cyan());
        println!("  {} - Show audit trail", "audit".cyan());
        println!(
            "  {} - Test injection attack",
            "test-inject <TEXT> [-- notes]".cyan()
        );
        println!("  {} - Show memory tree visualization", "visualize".cyan());
        println!(
            "  {} - Create and switch to memory branch",
            "branch <NAME>".cyan()
        );
        println!("  {} - Switch to existing branch", "switch <NAME>".cyan());
        println!("  {} - List all branches", "list-branches".cyan());
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

    async fn handle_recommendation(
        &mut self,
        symbol: &str,
        client: &ClientProfile,
        notes: Option<String>,
    ) -> Result<()> {
        println!(
            "{} Generating recommendation for {}...",
            "🔍".yellow(),
            symbol
        );

        match self.advisor.get_recommendation(symbol, client, notes).await {
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
        println!("{}", "━".repeat(50).dimmed());

        // Query recent recommendations from memory store
        match self.advisor.get_recent_recommendations(10).await {
            Ok(recommendations) => {
                if recommendations.is_empty() {
                    println!("{} No previous recommendations found", "ℹ️".blue());
                    println!(
                        "{} Use 'recommend <SYMBOL>' to generate recommendations",
                        "💡".yellow()
                    );
                } else {
                    self.display_recommendations(&recommendations).await?;
                }
            }
            Err(e) => {
                println!("{} Failed to retrieve history: {}", "❌".red(), e);
            }
        }

        Ok(())
    }

    async fn show_history_at_commit(&self, commit: &str) -> Result<()> {
        println!(
            "{} at commit {}",
            "📜 Recommendations".blue().bold(),
            commit.dimmed()
        );
        println!("{}", "━".repeat(50).dimmed());

        match self.advisor.get_recommendations_at_commit(commit, 10).await {
            Ok(recommendations) => {
                if recommendations.is_empty() {
                    println!(
                        "{} No recommendations found at commit {}",
                        "ℹ️".blue(),
                        commit
                    );
                } else {
                    self.display_recommendations(&recommendations).await?;
                }
            }
            Err(e) => {
                println!(
                    "{} Failed to retrieve history at commit {}: {}",
                    "❌".red(),
                    commit,
                    e
                );
            }
        }

        Ok(())
    }

    async fn show_history_at_branch(&self, branch: &str) -> Result<()> {
        println!(
            "{} on branch {}",
            "📜 Recommendations".blue().bold(),
            branch.cyan()
        );
        println!("{}", "━".repeat(50).dimmed());

        match self.advisor.get_recommendations_at_branch(branch, 10).await {
            Ok(recommendations) => {
                if recommendations.is_empty() {
                    println!(
                        "{} No recommendations found on branch {}",
                        "ℹ️".blue(),
                        branch
                    );
                } else {
                    self.display_recommendations(&recommendations).await?;
                }
            }
            Err(e) => {
                println!(
                    "{} Failed to retrieve history on branch {}: {}",
                    "❌".red(),
                    branch,
                    e
                );
            }
        }

        Ok(())
    }

    async fn display_recommendations(
        &self,
        recommendations: &[crate::advisor::Recommendation],
    ) -> Result<()> {
        for (i, rec) in recommendations.iter().enumerate() {
            println!();
            println!("{} Recommendation #{}", "📊".green(), i + 1);
            println!("  {}: {}", "Symbol".cyan(), rec.symbol);
            println!(
                "  {}: {}",
                "Action".cyan(),
                rec.recommendation_type.as_str().bold()
            );
            println!("  {}: {:.1}%", "Confidence".cyan(), rec.confidence * 100.0);
            println!("  {}: {}", "Client ID".cyan(), rec.client_id);
            println!(
                "  {}: {}",
                "Date".cyan(),
                rec.timestamp.format("%Y-%m-%d %H:%M:%S")
            );

            // Show first line of reasoning
            let reasoning_lines: Vec<&str> = rec.reasoning.lines().collect();
            if !reasoning_lines.is_empty() {
                println!("  {}: {}", "Summary".cyan(), reasoning_lines[0]);
            }

            println!("  {}: {}", "Version".dimmed(), rec.memory_version);
        }
        Ok(())
    }

    async fn show_memory_status(&self) -> Result<()> {
        println!("{}", "🧠 Memory Status".blue().bold());
        println!("{}", "━".repeat(20).dimmed());

        // Get real memory status
        match self.advisor.get_memory_status().await {
            Ok(status) => {
                // Show memory consistency info with real data
                let validation_status = if status.validation_active {
                    "ACTIVE".bold().green()
                } else {
                    "INACTIVE".bold().red()
                };
                println!("{} Memory validation: {}", "✅".green(), validation_status);

                let security_status = if status.security_monitoring {
                    "ENABLED".bold().blue()
                } else {
                    "DISABLED".bold().red()
                };
                println!("{} Security monitoring: {}", "🛡️".blue(), security_status);

                let audit_status = if status.audit_enabled {
                    "LOGGING".bold().yellow()
                } else {
                    "DISABLED".bold().red()
                };
                println!("{} Audit trail: {}", "📝".yellow(), audit_status);

                let validation_mode = if status.storage_healthy && status.git_healthy {
                    "MULTI-SOURCE".bold().cyan()
                } else {
                    "LIMITED".bold().yellow()
                };
                println!("{} Cross-validation: {}", "🔍".cyan(), validation_mode);

                println!();
                println!("{}", "System Information:".yellow());
                println!(
                    "  {} Current branch: {}",
                    "🌿".green(),
                    status.current_branch
                );
                println!("  {} Latest commit: {}", "📝".blue(), status.current_commit);
                println!(
                    "  {} Total branches: {}",
                    "🌳".cyan(),
                    status.total_branches
                );
                println!(
                    "  {} Total commits: {}",
                    "📊".yellow(),
                    status.total_commits
                );

                println!();
                println!("{}", "Memory Statistics:".yellow());
                println!(
                    "  {} Recommendations: {}",
                    "💡".green(),
                    status.recommendation_count
                );
                println!(
                    "  {} Market data: {}",
                    "📈".blue(),
                    status.market_data_count
                );
                println!("  {} Audit entries: {}", "📋".yellow(), status.audit_count);

                println!();
                println!("{}", "Health Status:".yellow());
                let storage_indicator = if status.storage_healthy { "✅" } else { "❌" };
                println!(
                    "  {} Storage system: {}",
                    storage_indicator,
                    if status.storage_healthy {
                        "HEALTHY".green()
                    } else {
                        "ERROR".red()
                    }
                );
                let git_indicator = if status.git_healthy { "✅" } else { "❌" };
                println!(
                    "  {} Git repository: {}",
                    git_indicator,
                    if status.git_healthy {
                        "HEALTHY".green()
                    } else {
                        "ERROR".red()
                    }
                );
            }
            Err(e) => {
                println!("{} Failed to retrieve memory status: {}", "❌".red(), e);
                return Ok(());
            }
        }

        println!();

        // Get real validation sources
        match self.advisor.get_validation_sources().await {
            Ok(sources) => {
                println!("{}", "Validation Sources:".yellow());
                for source in sources {
                    let status_indicator = match source.status {
                        crate::memory::SourceStatus::Active => "🟢",
                        crate::memory::SourceStatus::Inactive => "🟡",
                        crate::memory::SourceStatus::Error => "🔴",
                        crate::memory::SourceStatus::Unknown => "⚪",
                    };

                    let response_info = if let Some(ms) = source.response_time_ms {
                        format!(" ({ms}ms)")
                    } else {
                        String::new()
                    };

                    println!(
                        "  {} {} ({:.0}% trust){}",
                        status_indicator,
                        source.name,
                        source.trust_level * 100.0,
                        response_info
                    );
                }
            }
            Err(e) => {
                println!(
                    "{} Failed to retrieve validation sources: {}",
                    "❌".red(),
                    e
                );
            }
        }

        Ok(())
    }

    async fn show_audit_trail(&self) -> Result<()> {
        println!("{}", "📋 Audit Trail".blue().bold());
        println!("{}", "━".repeat(20).dimmed());

        // Query the real audit trail
        match self.advisor.get_audit_trail().await {
            Ok(entries) => {
                if entries.is_empty() {
                    println!("{} No audit entries found", "ℹ️".blue());
                } else {
                    println!("Showing last {} audit entries:", entries.len().min(10));
                    println!();

                    for (i, entry) in entries.iter().take(10).enumerate() {
                        let icon = match entry.memory_type.as_str() {
                            "Recommendation" => "💡",
                            "MarketData" => "📈",
                            "Audit" => "📋",
                            "System" => "⚙️",
                            "Security" => "🛡️",
                            _ => "📝",
                        };

                        println!(
                            "{} {}",
                            format!("{} {}", icon, entry.timestamp.format("%Y-%m-%d %H:%M:%S"))
                                .dimmed(),
                            entry.action,
                        );

                        if i >= 9 {
                            // Only show first 10
                            break;
                        }
                    }

                    if entries.len() > 10 {
                        println!();
                        println!(
                            "{} ... and {} more entries",
                            "📝".dimmed(),
                            entries.len() - 10
                        );
                    }
                }
            }
            Err(e) => {
                println!("{} Failed to retrieve audit trail: {}", "❌".red(), e);
            }
        }

        Ok(())
    }

    async fn test_injection_attack(&mut self, payload: &str, notes: Option<String>) -> Result<()> {
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

                // Store the security test result
                match self
                    .advisor
                    .store_security_test(payload, &alert, notes)
                    .await
                {
                    Ok(_) => {
                        println!("{} Security test result saved to memory", "💾".green());
                    }
                    Err(e) => {
                        println!("{} Failed to save security test: {}", "⚠️".yellow(), e);
                    }
                }
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
        // Check if branch already exists
        if self.advisor.branch_exists(name) {
            println!("{} Branch '{}' already exists!", "⚠️".yellow(), name.bold());
            println!(
                "{} Use 'switch {}' to switch to the existing branch",
                "💡".blue(),
                name
            );
            return Ok(());
        }

        println!("{} Creating memory branch: {}", "🌿".green(), name.bold());

        // Create the branch using the memory store
        match self.advisor.create_and_switch_branch(name).await {
            Ok(_) => {
                println!("{} Branch '{}' created successfully", "✅".green(), name);
                println!("{} Switched to branch '{}'", "🔀".blue(), name);
                println!(
                    "{} You can now safely test scenarios without affecting main memory",
                    "💡".yellow()
                );
            }
            Err(e) => {
                println!("{} Failed to create/switch branch: {}", "❌".red(), e);
                return Err(e);
            }
        }

        Ok(())
    }

    async fn switch_branch(&mut self, name: &str) -> Result<()> {
        // Check if branch exists before trying to switch
        if !self.advisor.branch_exists(name) {
            println!("{} Branch '{}' does not exist!", "❌".red(), name.bold());
            println!(
                "{} Use 'branch {}' to create a new branch",
                "💡".blue(),
                name
            );
            return Ok(());
        }

        println!("{} Switching to branch: {}", "🔀".blue(), name.bold());

        match self.advisor.switch_to_branch(name).await {
            Ok(_) => {
                println!("{} Switched to branch '{}'", "✅".green(), name);
            }
            Err(e) => {
                println!("{} Failed to switch to branch: {}", "❌".red(), e);
                return Err(e);
            }
        }

        Ok(())
    }

    fn show_branch_info(&self) {
        // Show all branches like git branch command
        match self.advisor.memory_store.list_branches() {
            Ok(branches) => {
                let current_branch = self.advisor.get_actual_current_branch();

                if branches.is_empty() {
                    println!("No branches found");
                } else {
                    for branch in branches {
                        if branch == current_branch {
                            println!("* {}", branch.green());
                        } else {
                            println!("  {branch}");
                        }
                    }
                }
            }
            Err(e) => {
                println!("error: Failed to list branches: {e}");
            }
        }

        // Show sync status if there's a mismatch
        let cached_branch = self.advisor.current_branch();
        let actual_branch = self.advisor.get_actual_current_branch();

        if cached_branch != actual_branch {
            println!();
            println!(
                "{} Branch mismatch: cached='{}', actual='{}'",
                "warning:".yellow(),
                cached_branch,
                actual_branch
            );
        }
    }

    fn list_branches(&self) {
        println!("{}", "🌳 Available Branches".green().bold());
        println!("{}", "━".repeat(25).dimmed());

        match self.advisor.memory_store.list_branches() {
            Ok(branches) => {
                let current_branch = self.advisor.get_actual_current_branch();

                if branches.is_empty() {
                    println!("{} No branches found", "ℹ️".blue());
                } else {
                    for branch in branches {
                        if branch == current_branch {
                            println!("  {} {} (current)", "●".green(), branch.bold());
                        } else {
                            println!("  {} {}", "○".dimmed(), branch);
                        }
                    }
                }
            }
            Err(e) => {
                println!("{} Failed to list branches: {}", "❌".red(), e);
            }
        }
    }
}
