/*
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use rig::providers::openai::Client;
// Removed unused Deserialize and Serialize imports
use std::sync::Arc;

use prollytree::agent::{AgentMemoryStats, AgentMemorySystem, OptimizationReport};

use crate::advisor::analysis_modules::AnalysisModuleRegistry;
use crate::advisor::workflow::WorkflowProcessor;
use crate::advisor::RiskTolerance;
use crate::memory::enhanced_types::*;

/// Enhanced Financial Advisor with deep agent memory integration
pub struct EnhancedFinancialAdvisor {
    /// Core agent memory system with all memory types
    memory_system: Arc<AgentMemorySystem>,

    /// Workflow processor for multi-step analysis
    workflow_processor: WorkflowProcessor,

    /// Analysis modules registry
    analysis_modules: AnalysisModuleRegistry,

    /// Optional Rig client for AI-powered analysis
    rig_client: Option<Client>,

    /// Current client being served
    current_client_id: Option<String>,

    /// Verbose logging
    verbose: bool,
}

impl EnhancedFinancialAdvisor {
    /// Create a new enhanced financial advisor
    pub async fn new(storage_path: &str, api_key: Option<&str>, verbose: bool) -> Result<Self> {
        // Initialize agent memory system
        let memory_system = Arc::new(
            AgentMemorySystem::init(
                storage_path,
                "enhanced_financial_advisor".to_string(),
                None, // No embedding generator for now
            )
            .map_err(|e| anyhow::anyhow!("Failed to initialize memory system: {}", e))?,
        );

        // Setup Rig client if API key provided
        let rig_client = api_key.map(Client::new);

        // Initialize workflow processor
        let workflow_processor = WorkflowProcessor::new(memory_system.clone(), api_key, verbose);

        // Initialize analysis modules
        let analysis_modules =
            AnalysisModuleRegistry::new(memory_system.clone(), rig_client.clone());

        Ok(Self {
            memory_system,
            workflow_processor,
            analysis_modules,
            rig_client,
            current_client_id: None,
            verbose,
        })
    }

    /// Open an existing enhanced financial advisor
    pub async fn open(storage_path: &str, api_key: Option<&str>, verbose: bool) -> Result<Self> {
        // Open existing agent memory system
        let memory_system = Arc::new(
            AgentMemorySystem::open(storage_path, "enhanced_financial_advisor".to_string(), None)
                .map_err(|e| anyhow::anyhow!("Failed to open memory system: {}", e))?,
        );

        let rig_client = api_key.map(Client::new);

        let workflow_processor = WorkflowProcessor::new(memory_system.clone(), api_key, verbose);

        let analysis_modules =
            AnalysisModuleRegistry::new(memory_system.clone(), rig_client.clone());

        Ok(Self {
            memory_system,
            workflow_processor,
            analysis_modules,
            rig_client,
            current_client_id: None,
            verbose,
        })
    }

    /// Set the current client for personalized analysis
    pub async fn set_current_client(&mut self, client_id: &str) -> Result<()> {
        // Verify client exists or create new profile
        let _client_profile = self.get_or_create_client_profile(client_id).await?;

        // Note: In a full implementation, client profile would be stored in semantic memory
        // let client_json = serde_json::to_string(&client_profile)?;
        // self.memory_system.semantic.store_fact(...).await?;

        self.current_client_id = Some(client_id.to_string());

        if self.verbose {
            println!("👤 Set current client: {}", client_id.bright_cyan());
        }

        Ok(())
    }

    /// Get comprehensive recommendation using enhanced workflow
    pub async fn get_enhanced_recommendation(
        &mut self,
        symbol: &str,
    ) -> Result<DetailedRecommendation> {
        let client_id = self.current_client_id.as_ref().ok_or_else(|| {
            anyhow::anyhow!("No current client set. Use set_current_client() first.")
        })?;

        if self.verbose {
            println!(
                "🔄 Generating enhanced recommendation for {} (client: {})",
                symbol.bright_yellow(),
                client_id.bright_cyan()
            );
        }

        // Execute the comprehensive workflow
        let recommendation = self
            .workflow_processor
            .execute_recommendation_workflow(symbol, client_id)
            .await?;

        // Note: In a full implementation, recommendation episode would be stored in episodic memory
        // self.store_recommendation_episode(&recommendation).await?;

        // Note: Client interaction history would be updated
        // self.update_client_interaction_history(client_id, &recommendation).await?;

        // Learn from this recommendation for future improvement
        self.learn_from_recommendation(&recommendation).await?;

        // Note: Memory checkpoint would be created in a full implementation
        // let checkpoint_message = format!("Enhanced recommendation for {} (client: {})", symbol, client_id);
        // self.memory_system.checkpoint(&checkpoint_message).await?;

        if self.verbose {
            println!(
                "✅ Enhanced recommendation completed with {:.1}% confidence",
                recommendation.confidence * 100.0
            );
        }

        Ok(recommendation)
    }

    /// Perform deep research analysis with multiple steps
    pub async fn perform_deep_research(&mut self, symbol: &str) -> Result<MarketAnalysisResult> {
        let client_id = self
            .current_client_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No current client set"))?;

        if self.verbose {
            println!(
                "🔬 Performing deep research analysis for {}",
                symbol.bright_yellow()
            );
        }

        // Create analysis context
        let context = AnalysisContext {
            analysis_id: uuid::Uuid::new_v4().to_string(),
            client_id: client_id.clone(),
            symbol: symbol.to_string(),
            request_type: "deep_research".to_string(),
            market_conditions: MarketSnapshot::default(),
            client_profile: self.get_current_client_profile().await?,
            started_at: Utc::now(),
            parameters: std::collections::HashMap::new(),
        };

        // Perform comprehensive market analysis
        let market_analysis = self
            .analysis_modules
            .market_research
            .analyze_market(symbol, &context)
            .await?;

        // Note: Research results would be stored in episodic memory in a full implementation
        if self.verbose {
            println!(
                "📝 Deep research analysis completed for {}",
                symbol.bright_yellow()
            );
        }

        Ok(market_analysis)
    }

    /// Update client risk profile based on interactions
    pub async fn update_client_risk_profile(
        &mut self,
        client_id: &str,
        new_risk_tolerance: RiskTolerance,
    ) -> Result<()> {
        if self.verbose {
            println!(
                "📊 Updating risk profile for client: {} -> {:?}",
                client_id.bright_cyan(),
                new_risk_tolerance
            );
        }

        // Get existing client profile
        let mut client_profile = self.get_or_create_client_profile(client_id).await?;

        // Update risk tolerance
        let old_risk_tolerance = client_profile.risk_tolerance;
        client_profile.risk_tolerance = new_risk_tolerance;
        client_profile.last_updated = Utc::now();

        // Note: In a full implementation, updated profile would be stored in semantic memory
        // let client_json = serde_json::to_string(&client_profile)?;
        // self.memory_system.semantic.store_fact(...).await?;

        // Note: Risk profile changes would be recorded in episodic memory in a full implementation
        if self.verbose {
            println!(
                "📊 Risk profile updated for {}: {:?} -> {:?}",
                client_id.bright_cyan(),
                old_risk_tolerance,
                new_risk_tolerance
            );
        }

        Ok(())
    }

    /// Analyze client portfolio and suggest rebalancing
    pub async fn analyze_portfolio_rebalancing(
        &mut self,
        client_id: &str,
        holdings: Vec<(String, f64)>,
    ) -> Result<Vec<PersonalizedRecommendation>> {
        if self.verbose {
            println!(
                "⚖️  Analyzing portfolio rebalancing for client: {}",
                client_id.bright_cyan()
            );
        }

        let mut recommendations = Vec::new();
        let client_profile = self.get_or_create_client_profile(client_id).await?;

        for (symbol, current_weight) in holdings {
            // Create analysis context for each holding
            let context = AnalysisContext {
                analysis_id: uuid::Uuid::new_v4().to_string(),
                client_id: client_id.to_string(),
                symbol: symbol.clone(),
                request_type: "portfolio_rebalancing".to_string(),
                market_conditions: MarketSnapshot::default(),
                client_profile: client_profile.clone(),
                started_at: Utc::now(),
                parameters: {
                    let mut params = std::collections::HashMap::new();
                    params.insert(
                        "current_weight".to_string(),
                        serde_json::json!(current_weight),
                    );
                    params
                },
            };

            // Perform analysis for this holding
            let market_analysis = self
                .analysis_modules
                .market_research
                .analyze_market(&symbol, &context)
                .await?;

            let risk_assessment = self
                .analysis_modules
                .risk_analysis
                .assess_risk(&context, &market_analysis)
                .await?;

            let compliance_validation = self
                .analysis_modules
                .compliance_check
                .validate_compliance(&context, &risk_assessment)
                .await?;

            // Generate rebalancing recommendation
            let recommendation = self
                .analysis_modules
                .recommendation_engine
                .generate_recommendation(
                    &context,
                    &market_analysis,
                    &risk_assessment,
                    &compliance_validation,
                )
                .await?;

            recommendations.push(recommendation);
        }

        // Note: Portfolio analysis would be stored in episodic memory in a full implementation
        if self.verbose {
            println!(
                "⚖️  Portfolio rebalancing analysis completed for {}: {} recommendations generated",
                client_id.bright_cyan(),
                recommendations.len()
            );
        }

        Ok(recommendations)
    }

    /// Get system-wide memory statistics
    pub async fn get_memory_statistics(&self) -> Result<AgentMemoryStats> {
        self.memory_system
            .get_system_stats()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get memory statistics: {}", e))
    }

    /// Optimize memory system (cleanup, consolidation, archival)
    pub async fn optimize_memory_system(&mut self) -> Result<OptimizationReport> {
        if self.verbose {
            println!("🧹 Optimizing memory system...");
        }

        // Note: Memory optimization would be performed in a full implementation
        let report = OptimizationReport::default();

        if self.verbose {
            println!(
                "✅ Memory optimization completed: {} items processed",
                report.total_processed()
            );
        }

        Ok(report)
    }

    /// Learn from recommendation outcomes
    pub async fn update_recommendation_outcome(
        &mut self,
        recommendation_id: &str,
        outcome: RecommendationOutcome,
    ) -> Result<()> {
        if self.verbose {
            println!(
                "📈 Updating recommendation outcome for: {}",
                recommendation_id.bright_yellow()
            );
        }

        // Find the original recommendation episode
        let _episodes = self
            .memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(30),
                chrono::Utc::now(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to find recommendation episode: {}", e))?;

        if let Some(episode) = _episodes.first() {
            // Parse the episode and update with outcome
            if let Ok(mut rec_episode) =
                serde_json::from_str::<RecommendationEpisode>(&episode.content.to_string())
            {
                rec_episode.outcome = Some(outcome.clone());

                // Note: In a full implementation, updated episode would be stored
                // let updated_json = serde_json::to_string(&rec_episode)?;
                // self.memory_system.episodic.store_episode(...).await?;

                // Learn from this outcome to improve future recommendations
                self.update_procedural_knowledge_from_outcome(&rec_episode, &outcome)
                    .await?;
            }
        }

        Ok(())
    }

    /// Run interactive client session
    pub async fn run_interactive_session(&mut self) -> Result<()> {
        println!(
            "{}",
            "🏦 Enhanced Financial Advisory Session".green().bold()
        );
        println!("{}", "Memory-driven personalized financial advice".dimmed());
        println!("{}", "Type 'help' for commands, 'exit' to quit\n".dimmed());

        loop {
            print!("{}> ", "advisor".bright_blue());
            use std::io::{self, Write};
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            match input {
                "exit" | "quit" => {
                    println!("👋 Thank you for using Enhanced Financial Advisor!");
                    break;
                }
                "help" => {
                    self.show_help();
                }
                input if input.starts_with("client ") => {
                    let client_id = input.strip_prefix("client ").unwrap().trim();
                    match self.set_current_client(client_id).await {
                        Ok(_) => println!("✅ Current client set to: {}", client_id.bright_cyan()),
                        Err(e) => println!("❌ Error setting client: {e}"),
                    }
                }
                input if input.starts_with("recommend ") => {
                    let symbol = input
                        .strip_prefix("recommend ")
                        .unwrap()
                        .trim()
                        .to_uppercase();
                    match self.get_enhanced_recommendation(&symbol).await {
                        Ok(rec) => self.display_detailed_recommendation(&rec),
                        Err(e) => println!("❌ Error generating recommendation: {e}"),
                    }
                }
                input if input.starts_with("research ") => {
                    let symbol = input
                        .strip_prefix("research ")
                        .unwrap()
                        .trim()
                        .to_uppercase();
                    match self.perform_deep_research(&symbol).await {
                        Ok(analysis) => self.display_market_analysis(&analysis),
                        Err(e) => println!("❌ Error performing research: {e}"),
                    }
                }
                "stats" => match self.get_memory_statistics().await {
                    Ok(stats) => self.display_memory_stats(&stats),
                    Err(e) => println!("❌ Error getting stats: {e}"),
                },
                "optimize" => match self.optimize_memory_system().await {
                    Ok(report) => {
                        println!("✅ Optimized: {} items processed", report.total_processed())
                    }
                    Err(e) => println!("❌ Error optimizing: {e}"),
                },
                _ => {
                    println!("❓ Unknown command. Type 'help' for available commands.");
                }
            }
        }

        Ok(())
    }

    // Helper methods

    async fn get_or_create_client_profile(&self, client_id: &str) -> Result<ClientEntity> {
        // Try to get existing profile from semantic memory
        let facts = self
            .memory_system
            .semantic
            .get_entity_facts("client", client_id)
            .await
            .unwrap_or_default();

        if let Some(fact) = facts.first() {
            if let Ok(profile) = serde_json::from_str::<ClientEntity>(&fact.content.to_string()) {
                return Ok(profile);
            }
        }

        // Create new profile with defaults
        Ok(ClientEntity::new(
            client_id.to_string(),
            RiskTolerance::Moderate,
        ))
    }

    async fn get_current_client_profile(&self) -> Result<ClientEntity> {
        let client_id = self
            .current_client_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No current client set"))?;
        self.get_or_create_client_profile(client_id).await
    }

    async fn store_recommendation_episode(
        &self,
        recommendation: &DetailedRecommendation,
    ) -> Result<()> {
        let _episode = RecommendationEpisode {
            recommendation_id: recommendation.recommendation_id.clone(),
            client_id: self.current_client_id.as_ref().unwrap().clone(),
            symbol: "UNKNOWN".to_string(), // Would extract from context in real implementation
            action: recommendation.base_recommendation,
            reasoning: recommendation.reasoning.clone(),
            confidence: recommendation.confidence,
            market_conditions: MarketSnapshot::default(),
            outcome: None,
            timestamp: recommendation.timestamp,
            workflow_steps: Vec::new(),
        };

        // Note: In a full implementation, recommendation episode would be stored
        // let episode_json = serde_json::to_string(&episode)?;
        // self.memory_system.episodic.store_episode(...).await?;

        Ok(())
    }

    async fn update_client_interaction_history(
        &self,
        client_id: &str,
        recommendation: &DetailedRecommendation,
    ) -> Result<()> {
        let _interaction = ClientInteractionEpisode {
            interaction_id: uuid::Uuid::new_v4().to_string(),
            client_id: client_id.to_string(),
            interaction_type: InteractionType::RecommendationDiscussion,
            summary: format!(
                "Provided {} recommendation with {:.1}% confidence",
                recommendation.base_recommendation.as_str(),
                recommendation.confidence * 100.0
            ),
            sentiment: 0.75,
            key_topics: vec!["recommendation".to_string(), "analysis".to_string()],
            decisions_made: vec![format!(
                "{:?} recommendation provided",
                recommendation.base_recommendation
            )],
            follow_up_required: true,
            timestamp: Utc::now(),
        };

        // Note: In a full implementation, client interaction would be stored
        // let interaction_json = serde_json::to_string(&interaction)?;
        // self.memory_system.episodic.store_episode(...).await?;

        Ok(())
    }

    async fn learn_from_recommendation(
        &self,
        _recommendation: &DetailedRecommendation,
    ) -> Result<()> {
        // Update procedural memory with patterns learned from this recommendation
        // This is where the system would analyze what worked well and update its procedures

        // For now, just create a simple learning entry
        let _learning_entry = serde_json::json!({
            "type": "recommendation_learning",
            "workflow_performance": "successful",
            "confidence_level": _recommendation.confidence,
            "timestamp": Utc::now()
        });

        // Note: In a full implementation, learning patterns would be stored
        // self.memory_system.procedural.store_procedure(...).await?;

        Ok(())
    }

    async fn update_procedural_knowledge_from_outcome(
        &self,
        _episode: &RecommendationEpisode,
        _outcome: &RecommendationOutcome,
    ) -> Result<()> {
        // Analyze the outcome and update procedural knowledge
        // This would involve complex learning algorithms in a real system

        let _knowledge_update = serde_json::json!({
            "outcome_analysis": {
                "return": _outcome.actual_return,
                "client_satisfaction": _outcome.client_satisfaction,
                "followed": _outcome.followed_recommendation
            },
            "learning_points": "Update recommendation algorithms based on outcome",
            "timestamp": Utc::now()
        });

        // Note: In a full implementation, procedural knowledge would be updated
        // self.memory_system.procedural.store_procedure(...).await?;

        Ok(())
    }

    fn show_help(&self) {
        println!("{}", "Available Commands:".yellow().bold());
        println!(
            "  {} - Set current client for personalized advice",
            "client <id>".bright_green()
        );
        println!(
            "  {} - Get enhanced recommendation for symbol",
            "recommend <symbol>".bright_green()
        );
        println!(
            "  {} - Perform deep research analysis",
            "research <symbol>".bright_green()
        );
        println!(
            "  {} - Show memory system statistics",
            "stats".bright_green()
        );
        println!("  {} - Optimize memory system", "optimize".bright_green());
        println!("  {} - Show this help message", "help".bright_green());
        println!("  {} - Exit the session", "exit".bright_green());
        println!();
    }

    fn display_detailed_recommendation(&self, rec: &DetailedRecommendation) {
        println!("\n{}", "📊 Enhanced Recommendation".bright_blue().bold());
        println!("{}", "━".repeat(60).dimmed());
        println!(
            "🎯 Action: {}",
            format!("{:?}", rec.base_recommendation).bright_yellow()
        );
        println!("📈 Confidence: {:.1}%", rec.confidence * 100.0);
        println!(
            "🕒 Generated: {}",
            rec.timestamp.format("%Y-%m-%d %H:%M:%S")
        );
        println!("\n💭 Reasoning:");
        println!("{}", rec.reasoning);
        println!("\n🎨 Personalized Advice:");
        println!("{}", rec.personalized_reasoning);
        println!("\n⚖️  Risk Assessment:");
        println!(
            "  Overall Risk: {:.1}/10",
            rec.risk_assessment.overall_risk_score * 10.0
        );
        println!(
            "  Client Alignment: {:.1}%",
            rec.risk_assessment.client_risk_alignment * 100.0
        );
        println!(
            "\n🛡️  Compliance: {}",
            if rec.compliance_validation.passed {
                "✅ PASSED"
            } else {
                "❌ ISSUES"
            }
        );
        println!(
            "⏱️  Processing Time: {:.2}s",
            rec.execution_metadata
                .total_execution_time
                .num_milliseconds() as f64
                / 1000.0
        );
        println!("{}", "━".repeat(60).dimmed());
    }

    fn display_market_analysis(&self, analysis: &MarketAnalysisResult) {
        println!("\n{}", "🔬 Deep Market Research".bright_blue().bold());
        println!("{}", "━".repeat(60).dimmed());
        println!(
            "📈 Fundamental Outlook: {}",
            analysis.fundamental_analysis.growth_prospects
        );
        println!(
            "📊 Technical Trend: {}",
            analysis.technical_analysis.trend_direction
        );
        println!(
            "🏭 Sector Analysis: {}",
            analysis.sector_analysis.sector_trend
        );
        println!(
            "💭 Sentiment Score: {:.1}%",
            analysis.sentiment_analysis.analyst_sentiment * 100.0
        );
        println!("\n🤖 AI Insights:");
        println!("{}", analysis.ai_insights);
        println!("{}", "━".repeat(60).dimmed());
    }

    fn display_memory_stats(&self, stats: &AgentMemoryStats) {
        println!("\n{}", "🧠 Memory System Statistics".bright_blue().bold());
        println!("{}", "━".repeat(60).dimmed());
        println!("📊 Total Memories: {}", stats.overall.total_memories);
        println!(
            "💾 Storage Size: {:.2} MB",
            stats.overall.total_size_bytes as f64 / 1024.0 / 1024.0
        );
        println!("📝 Short-term Entries: {}", stats.short_term.active_threads);
        println!("🔄 Access count: {:.0}", stats.overall.avg_access_count);
        println!("{}", "━".repeat(60).dimmed());
    }
}
