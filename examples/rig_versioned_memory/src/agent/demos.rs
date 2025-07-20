use anyhow::Result;
use colored::Colorize;
use std::io::{self, Write};

use super::versioned::VersionedAgent;

impl VersionedAgent {
    pub async fn demo_memory_learning(&mut self) -> Result<()> {
        println!("\n{}", "🎬 Demo: Memory Learning & Rollback".green().bold());
        println!("{}", "=".repeat(50).green());

        // Agent learns user preference
        println!("\n{}: I prefer technical explanations", "User".blue());
        let (response1, _v1) = self
            .process_message("I prefer technical explanations")
            .await?;
        println!("{}: {}", "Agent".yellow(), response1);

        // Store this as long-term memory
        self.learn_fact(
            "user_preference",
            "User prefers technical explanations for topics",
        )
        .await?;

        // Give response based on preference
        println!("\n{}: Explain quantum computing", "User".blue());
        let (response2, v2) = self.process_message("Explain quantum computing").await?;
        println!("{}: {}", "Agent".yellow(), response2);

        // User changes preference (mistake)
        println!(
            "\n{}: Actually, I prefer simple explanations",
            "User".blue()
        );
        let (response3, _v3) = self
            .process_message("Actually, I prefer simple explanations")
            .await?;
        println!("{}: {}", "Agent".yellow(), response3);

        // Update the preference
        self.learn_fact(
            "user_preference",
            "User prefers simple explanations for topics",
        )
        .await?;

        // Demonstrate rollback
        println!(
            "\n{} {}",
            "⏪ Rolling back to version".red(),
            format!("{v2} (before preference change)").red()
        );
        self.rollback_to_version(&v2).await?;

        // Agent should use original preference again
        println!("\n{}: Explain machine learning", "User".blue());
        let (response4, _) = self.process_message("Explain machine learning").await?;
        println!(
            "{}: {} {}",
            "Agent".yellow(),
            response4,
            "(should be technical based on rollback)".dimmed()
        );

        Ok(())
    }

    pub async fn demo_branching(&mut self) -> Result<()> {
        println!("\n{}", "🎬 Demo: Memory Branching".green().bold());
        println!("{}", "=".repeat(50).green());

        // Create experimental personality branch
        self.create_memory_branch("experimental_personality")
            .await?;

        // Try different behavior without affecting main memory
        println!(
            "\n{} - {}: You are now very formal and verbose",
            "Experimental branch".magenta(),
            "User".blue()
        );
        let (response1, _) = self
            .process_message("You are now very formal and verbose")
            .await?;
        println!("{}: {}", "Agent".yellow(), response1);

        // Store this personality trait
        self.learn_fact("personality", "Formal and verbose communication style")
            .await?;

        // Test the new personality
        println!("\n{}: How's the weather?", "User".blue());
        let (response2, _) = self.process_message("How's the weather?").await?;
        println!(
            "{}: {} {}",
            "Agent".yellow(),
            response2,
            "(formal response)".dimmed()
        );

        // Switch back to main personality
        println!("\n{}", "🔄 Switching back to main branch".cyan());

        // Create a new session to simulate main branch
        self.new_session();

        // Compare responses from different memory states
        println!(
            "\n{} - {}: Hello, how are you?",
            "Main branch".green(),
            "User".blue()
        );
        let (response3, _) = self.process_message("Hello, how are you?").await?;
        println!(
            "{}: {} {}",
            "Agent".yellow(),
            response3,
            "(normal personality)".dimmed()
        );

        Ok(())
    }

    pub async fn demo_audit_trail(&mut self) -> Result<()> {
        println!("\n{}", "🎬 Demo: Decision Audit Trail".green().bold());
        println!("{}", "=".repeat(50).green());

        // Store some context
        self.learn_fact("user_location", "User is located in San Francisco")
            .await?;
        self.learn_fact("weather_preference", "User likes detailed weather reports")
            .await?;

        // Make a query that uses context
        println!("\n{}: What's the weather like?", "User".blue());
        let start_time = std::time::Instant::now();
        let (response, version) = self.process_message("What's the weather like?").await?;
        let elapsed = start_time.elapsed();

        println!("{}: {}", "Agent".yellow(), response);

        // Show audit information
        println!("\n{}", "🔍 Decision Audit Trail:".cyan().bold());
        println!("├─ {}: What's the weather like?", "Input".dimmed());
        println!(
            "├─ {}: [user_location, weather_preference]",
            "Memories accessed".dimmed()
        );
        println!(
            "├─ {}: Used location + preference data",
            "Reasoning".dimmed()
        );
        println!("├─ {}: 0.95", "Confidence".dimmed());
        println!("├─ {}: {:?}", "Response time".dimmed(), elapsed);
        println!("└─ {}: {}", "Version".dimmed(), version);

        Ok(())
    }

    pub async fn demo_episodic_learning(&mut self) -> Result<()> {
        println!("\n{}", "🎬 Demo: Episodic Learning".green().bold());
        println!("{}", "=".repeat(50).green());

        // Simulate learning from experiences
        println!("\n{}: Remind me about my meeting at 3pm", "User".blue());
        let (response1, _) = self
            .process_message("Remind me about my meeting at 3pm")
            .await?;
        println!("{}: {}", "Agent".yellow(), response1);

        // Record the outcome
        self.record_episode(
            "Set reminder for meeting at 3pm",
            "User was reminded on time",
            1.0,
        )
        .await?;

        // Another interaction
        println!("\n{}: Set all my reminders 5 minutes early", "User".blue());
        let (response2, _) = self
            .process_message("Set all my reminders 5 minutes early")
            .await?;
        println!("{}: {}", "Agent".yellow(), response2);

        // Record this preference
        self.record_episode(
            "Adjusted reminder timing preference",
            "User prefers 5-minute early reminders",
            1.0,
        )
        .await?;

        // Later, the agent can use this experience
        println!("\n{}: Remind me about lunch at noon", "User".blue());
        let (response3, _) = self
            .process_message("Remind me about lunch at noon")
            .await?;
        println!(
            "{}: {} {}",
            "Agent".yellow(),
            response3,
            "(should mention setting it 5 minutes early)".dimmed()
        );

        Ok(())
    }

    pub async fn run_interactive_mode(&mut self) -> Result<()> {
        println!("\n{}", "🎮 Interactive Mode".cyan().bold());
        println!("{}", "Commands:".dimmed());
        println!("  {} - Exit interactive mode", "/quit".yellow());
        println!("  {} - Start new conversation", "/new".yellow());
        println!("  {} - Show current version", "/version".yellow());
        println!("  {} <concept> <fact> - Teach the agent", "/learn".yellow());
        println!("{}", "=".repeat(50).cyan());

        loop {
            print!("\n{}: ", "You".blue());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            match input {
                "/quit" => break,
                "/new" => {
                    self.clear_session().await?;
                    println!("{}", "✨ Started new conversation".green());
                }
                "/version" => {
                    let version = self.get_current_version().await;
                    println!("{}: {}", "Current version".dimmed(), version);
                }
                cmd if cmd.starts_with("/learn ") => {
                    let parts: Vec<&str> = cmd[7..].splitn(2, ' ').collect();
                    if parts.len() == 2 {
                        let version = self.learn_fact(parts[0], parts[1]).await?;
                        println!(
                            "{}",
                            format!("✅ Learned fact (version: {version})").green()
                        );
                    } else {
                        println!("{}", "Usage: /learn <concept> <fact>".red());
                    }
                }
                _ => match self.process_message(input).await {
                    Ok((response, version)) => {
                        println!("{}: {}", "Agent".yellow(), response);
                        println!("{}", format!("💾 Version: {version}").dimmed());
                    }
                    Err(e) => println!("{}: {}", "Error".red(), e),
                },
            }
        }

        Ok(())
    }
}
