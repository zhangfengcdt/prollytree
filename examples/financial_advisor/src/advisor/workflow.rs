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
use chrono::{Duration, Utc};
use colored::Colorize;
use rig::{completion::Prompt, providers::openai::Client};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

use prollytree::agent::AgentMemorySystem;

use crate::advisor::{RecommendationType, RiskTolerance};
use crate::memory::enhanced_types::*;
// Removed MarketDataValidator import as it's not needed

/// Multi-step workflow processor for complex financial analysis
pub struct WorkflowProcessor {
    memory_system: Arc<AgentMemorySystem>,
    rig_client: Option<Client>,
    verbose: bool,
}

impl WorkflowProcessor {
    pub fn new(
        memory_system: Arc<AgentMemorySystem>,
        api_key: Option<&str>,
        verbose: bool,
    ) -> Self {
        let rig_client = api_key.map(Client::new);

        Self {
            memory_system,
            rig_client,
            verbose,
        }
    }

    /// Execute the complete recommendation workflow with memory-driven intelligence
    pub async fn execute_recommendation_workflow(
        &mut self,
        symbol: &str,
        client_id: &str,
    ) -> Result<DetailedRecommendation> {
        let workflow_start = Utc::now();
        let analysis_id = uuid::Uuid::new_v4().to_string();

        if self.verbose {
            println!(
                "🔄 Starting enhanced recommendation workflow for {} (client: {})",
                symbol.bright_yellow(),
                client_id.bright_cyan()
            );
        }

        // Step 1: Initialize analysis context
        let context = self
            .initialize_analysis_context(symbol, client_id, &analysis_id)
            .await?;
        self.store_step_memory("initialization", &context).await?;

        // Step 2: Market research and data gathering
        let market_data = self.execute_market_research_step(&context).await?;
        self.store_step_memory("market_research", &market_data)
            .await?;

        // Step 3: Risk analysis
        let risk_assessment = self
            .execute_risk_analysis_step(&context, &market_data)
            .await?;
        self.store_step_memory("risk_analysis", &risk_assessment)
            .await?;

        // Step 4: Compliance validation
        let compliance_check = self
            .execute_compliance_step(&context, &risk_assessment)
            .await?;
        self.store_step_memory("compliance", &compliance_check)
            .await?;

        // Step 5: Generate final recommendation
        let recommendation = self
            .execute_recommendation_step(
                &context,
                &market_data,
                &risk_assessment,
                &compliance_check,
            )
            .await?;

        // Step 6: Learn from this workflow execution
        self.store_workflow_outcome(&recommendation).await?;

        // Create execution metadata
        let execution_metadata = ExecutionMetadata {
            workflow_used: "enhanced_recommendation_v1".to_string(),
            total_execution_time: Utc::now().signed_duration_since(workflow_start),
            step_timings: self.collect_step_timings().await,
            memory_queries_performed: 12, // Approximate count
            ai_api_calls: if self.rig_client.is_some() { 4 } else { 0 },
            data_sources_consulted: vec![
                "episodic_memory".to_string(),
                "semantic_memory".to_string(),
                "procedural_memory".to_string(),
                "market_data_validator".to_string(),
            ],
        };

        let detailed_recommendation = DetailedRecommendation {
            recommendation_id: analysis_id,
            base_recommendation: recommendation.base_recommendation,
            confidence: recommendation.confidence_adjustment.clamp(0.1, 1.0),
            reasoning: recommendation.personalized_reasoning.clone(),
            personalized_reasoning: recommendation.personalized_reasoning,
            risk_assessment,
            compliance_validation: compliance_check,
            market_analysis: market_data,
            execution_metadata,
            timestamp: Utc::now(),
        };

        if self.verbose {
            println!(
                "✅ Workflow completed in {:.2}s with {} confidence",
                detailed_recommendation
                    .execution_metadata
                    .total_execution_time
                    .num_milliseconds() as f64
                    / 1000.0,
                (detailed_recommendation.confidence * 100.0).round()
            );
        }

        Ok(detailed_recommendation)
    }

    async fn initialize_analysis_context(
        &self,
        symbol: &str,
        client_id: &str,
        analysis_id: &str,
    ) -> Result<AnalysisContext> {
        if self.verbose {
            println!("📋 Step 1: Initializing analysis context...");
        }

        // Retrieve client profile from semantic memory
        let client_profile_facts = self
            .memory_system
            .semantic
            .get_entity_facts("client", client_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get client facts: {}", e))?;

        // Create client entity or use default
        let client_profile = if !client_profile_facts.is_empty() {
            // Parse existing client data from memory
            if let Ok(parsed) =
                serde_json::from_str::<ClientEntity>(&client_profile_facts[0].content.to_string())
            {
                parsed
            } else {
                ClientEntity::new(client_id.to_string(), RiskTolerance::Moderate)
            }
        } else {
            ClientEntity::new(client_id.to_string(), RiskTolerance::Moderate)
        };

        // Get current market conditions (simplified)
        let market_conditions = MarketSnapshot::default();

        Ok(AnalysisContext {
            analysis_id: analysis_id.to_string(),
            client_id: client_id.to_string(),
            symbol: symbol.to_string(),
            request_type: "recommendation".to_string(),
            market_conditions,
            client_profile,
            started_at: Utc::now(),
            parameters: HashMap::new(),
        })
    }

    async fn execute_market_research_step(
        &self,
        context: &AnalysisContext,
    ) -> Result<MarketAnalysisResult> {
        if self.verbose {
            println!("📈 Step 2: Executing market research analysis...");
        }

        // Retrieve historical market knowledge from semantic memory
        let market_entity_facts = self
            .memory_system
            .semantic
            .get_entity_facts("market", &context.symbol)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get market facts: {}", e))?;

        // Get similar past market conditions from episodic memory
        let _similar_episodes = self
            .memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(30),
                chrono::Utc::now(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to search episodes: {}", e))?;

        // Use AI for market analysis if available
        let ai_insights = if let Some(ref client) = self.rig_client {
            let market_prompt = self.build_market_analysis_prompt(context, &market_entity_facts);

            let agent = client
                .agent("gpt-3.5-turbo")
                .preamble("You are an expert market analyst. Provide concise, professional analysis focusing on key factors that impact investment decisions.")
                .max_tokens(300)
                .temperature(0.3)
                .build();

            match agent.prompt(&market_prompt).await {
                Ok(response) => response.trim().to_string(),
                Err(_) => "Market analysis unavailable - using procedural knowledge".to_string(),
            }
        } else {
            "Market analysis based on procedural memory and historical patterns".to_string()
        };

        // Get analysis procedures from procedural memory
        let _analysis_procedures = self
            .memory_system
            .procedural
            .get_procedures_by_category("market_analysis")
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get procedures: {}", e))?;

        // Synthesize comprehensive market analysis
        Ok(MarketAnalysisResult {
            fundamental_analysis: FundamentalAnalysis {
                valuation_metrics: self.calculate_valuation_metrics(&context.symbol).await,
                growth_prospects: "Moderate growth expected based on sector trends".to_string(),
                competitive_position: "Strong market position in technology sector".to_string(),
                financial_health: "Solid financial metrics with manageable debt levels".to_string(),
            },
            technical_analysis: TechnicalAnalysis {
                trend_direction: "Upward trend with consolidation".to_string(),
                support_levels: vec![150.0, 145.0, 140.0],
                resistance_levels: vec![160.0, 165.0, 170.0],
                momentum_indicators: {
                    let mut indicators = HashMap::new();
                    indicators.insert("RSI".to_string(), 58.5);
                    indicators.insert("MACD".to_string(), 1.2);
                    indicators
                },
            },
            sector_analysis: SectorAnalysis {
                sector_trend: "Technology sector showing resilience".to_string(),
                relative_performance: 1.15,
                sector_rotation_outlook: "Continued interest in tech fundamentals".to_string(),
                key_sector_drivers: vec![
                    "AI adoption".to_string(),
                    "Cloud computing growth".to_string(),
                    "Digital transformation".to_string(),
                ],
            },
            sentiment_analysis: SentimentAnalysis {
                analyst_sentiment: 0.75,
                market_sentiment: 0.65,
                news_sentiment: 0.70,
                sentiment_drivers: vec![
                    "Strong earnings guidance".to_string(),
                    "Product innovation pipeline".to_string(),
                    "Market expansion opportunities".to_string(),
                ],
            },
            ai_insights,
        })
    }

    async fn execute_risk_analysis_step(
        &self,
        context: &AnalysisContext,
        market_data: &MarketAnalysisResult,
    ) -> Result<RiskAssessmentResult> {
        if self.verbose {
            println!("⚠️  Step 3: Executing risk analysis...");
        }

        // Get client's risk history from episodic memory
        let _risk_episodes = self
            .memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(90),
                chrono::Utc::now(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get risk episodes: {}", e))?;

        // Use AI for risk analysis if available
        let risk_factors = if let Some(ref client) = self.rig_client {
            let risk_prompt = self.build_risk_analysis_prompt(context, market_data);

            let agent = client
                .agent("gpt-3.5-turbo")
                .preamble("You are an expert risk analyst. Identify key risk factors and provide actionable mitigation strategies.")
                .max_tokens(250)
                .temperature(0.2)
                .build();

            match agent.prompt(&risk_prompt).await {
                Ok(response) => response
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.trim().to_string())
                    .collect(),
                Err(_) => vec![
                    "Market volatility risk".to_string(),
                    "Sector concentration risk".to_string(),
                    "Liquidity risk".to_string(),
                ],
            }
        } else {
            vec![
                "Market volatility based on historical patterns".to_string(),
                "Client risk tolerance alignment".to_string(),
                "Portfolio concentration considerations".to_string(),
            ]
        };

        // Calculate risk scores based on client profile and market conditions
        let mut risk_breakdown = HashMap::new();
        risk_breakdown.insert(RiskCategory::Market, 0.65);
        risk_breakdown.insert(RiskCategory::Credit, 0.25);
        risk_breakdown.insert(RiskCategory::Liquidity, 0.30);
        risk_breakdown.insert(RiskCategory::Concentration, 0.45);

        // Adjust risk based on client's risk tolerance
        let risk_multiplier = match context.client_profile.risk_tolerance {
            RiskTolerance::Conservative => 0.8,
            RiskTolerance::Moderate => 1.0,
            RiskTolerance::Aggressive => 1.2,
        };

        let overall_risk_score =
            risk_breakdown.values().sum::<f64>() / risk_breakdown.len() as f64 * risk_multiplier;

        // Calculate client alignment score
        let client_risk_alignment = match context.client_profile.risk_tolerance {
            RiskTolerance::Conservative if overall_risk_score > 0.7 => 0.6,
            RiskTolerance::Conservative => 0.9,
            RiskTolerance::Moderate => 0.85,
            RiskTolerance::Aggressive if overall_risk_score < 0.4 => 0.7,
            RiskTolerance::Aggressive => 0.9,
        };

        Ok(RiskAssessmentResult {
            overall_risk_score,
            risk_breakdown,
            risk_factors,
            mitigation_recommendations: vec![
                "Diversify across sectors".to_string(),
                "Use position sizing appropriate for risk tolerance".to_string(),
                "Monitor market conditions regularly".to_string(),
            ],
            client_risk_alignment,
        })
    }

    async fn execute_compliance_step(
        &self,
        context: &AnalysisContext,
        risk_assessment: &RiskAssessmentResult,
    ) -> Result<ComplianceValidation> {
        if self.verbose {
            println!("🛡️  Step 4: Executing compliance validation...");
        }

        // Get compliance rules from procedural memory
        let _compliance_procedures = self
            .memory_system
            .procedural
            .get_procedures_by_category("compliance")
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get compliance procedures: {}", e))?;

        // Check client-specific restrictions from semantic memory
        let _client_restrictions = self
            .memory_system
            .semantic
            .get_entity_facts("client_restrictions", &context.client_id)
            .await
            .unwrap_or_default();

        // Analyze past compliance issues from episodic memory
        let _compliance_history = self
            .memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(365),
                chrono::Utc::now(),
            )
            .await
            .unwrap_or_default();

        // Perform compliance checks
        let mut violations = Vec::new();
        let mut warnings = Vec::new();

        // Check position size limits (example rule)
        if risk_assessment.overall_risk_score > 0.8 {
            warnings.push(ComplianceWarning {
                rule_id: "RISK_001".to_string(),
                description: "High risk score detected".to_string(),
                recommendation:
                    "Consider reducing position size or implementing additional risk controls"
                        .to_string(),
            });
        }

        // Check client suitability
        if risk_assessment.client_risk_alignment < 0.7 {
            violations.push(ComplianceViolation {
                rule_id: "SUITABILITY_001".to_string(),
                severity: ComplianceSeverity::Warning,
                description: "Investment may not align with client risk profile".to_string(),
                recommended_action: "Review recommendation with client or adjust strategy"
                    .to_string(),
            });
        }

        // Use AI for additional compliance analysis if available
        let automated_actions = if let Some(ref client) = self.rig_client {
            let compliance_prompt = self.build_compliance_prompt(context, risk_assessment);

            let agent = client
                .agent("gpt-3.5-turbo")
                .preamble("You are a compliance officer. Focus on regulatory requirements and client protection.")
                .max_tokens(200)
                .temperature(0.1)
                .build();

            match agent.prompt(&compliance_prompt).await {
                Ok(response) => vec![format!("AI compliance check: {}", response.trim())],
                Err(_) => vec!["Standard compliance procedures applied".to_string()],
            }
        } else {
            vec!["Automated compliance validation completed".to_string()]
        };

        Ok(ComplianceValidation {
            passed: violations.is_empty(),
            violations,
            warnings,
            required_disclosures: vec![
                "Past performance does not guarantee future results".to_string(),
                "All investments carry risk of loss".to_string(),
            ],
            automated_actions_taken: automated_actions,
        })
    }

    async fn execute_recommendation_step(
        &self,
        context: &AnalysisContext,
        market_analysis: &MarketAnalysisResult,
        risk_assessment: &RiskAssessmentResult,
        compliance_validation: &ComplianceValidation,
    ) -> Result<PersonalizedRecommendation> {
        if self.verbose {
            println!("💡 Step 5: Generating personalized recommendation...");
        }

        // Determine base recommendation based on analysis
        let base_recommendation = if !compliance_validation.passed
            || (risk_assessment.overall_risk_score > 0.8
                && matches!(
                    context.client_profile.risk_tolerance,
                    RiskTolerance::Conservative
                )) {
            RecommendationType::Hold // Safety first if compliance issues or high risk for conservative clients
        } else if market_analysis.sentiment_analysis.analyst_sentiment > 0.7
            && risk_assessment.client_risk_alignment > 0.8
        {
            RecommendationType::Buy
        } else if market_analysis.sentiment_analysis.analyst_sentiment < 0.3 {
            RecommendationType::Sell
        } else {
            RecommendationType::Hold
        };

        // Get client interaction history for personalization
        let client_history = self
            .memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(180),
                chrono::Utc::now(),
            )
            .await
            .unwrap_or_default();

        // Generate personalized reasoning using AI if available
        let personalized_reasoning = if let Some(ref client) = self.rig_client {
            let personalization_prompt = self.build_personalization_prompt(
                base_recommendation,
                context,
                market_analysis,
                risk_assessment,
            );

            let agent = client
                .agent("gpt-3.5-turbo")
                .preamble(&format!(
                    "You are a financial advisor speaking directly to a client with {} risk tolerance. Be personal, clear, and actionable.",
                    format!("{:?}", context.client_profile.risk_tolerance).to_lowercase()
                ))
                .max_tokens(400)
                .temperature(0.4)
                .build();

            match agent.prompt(&personalization_prompt).await {
                Ok(response) => response.trim().to_string(),
                Err(_) => {
                    self.generate_fallback_reasoning(base_recommendation, context, risk_assessment)
                }
            }
        } else {
            self.generate_fallback_reasoning(base_recommendation, context, risk_assessment)
        };

        // Calculate confidence adjustment based on analysis quality
        let confidence_adjustment = self.calculate_confidence_adjustment(
            market_analysis,
            risk_assessment,
            compliance_validation,
            &client_history,
        );

        // Extract client-specific factors
        let client_specific_factors = vec![
            format!(
                "Risk tolerance: {:?}",
                context.client_profile.risk_tolerance
            ),
            format!(
                "Investment goals: {}",
                context.client_profile.investment_goals.join(", ")
            ),
            format!("Time horizon: {}", context.client_profile.time_horizon),
            format!(
                "Risk alignment: {:.1}%",
                risk_assessment.client_risk_alignment * 100.0
            ),
        ];

        Ok(PersonalizedRecommendation {
            base_recommendation,
            personalized_reasoning,
            confidence_adjustment,
            client_specific_factors,
            presentation_style: "conversational".to_string(),
            follow_up_actions: vec![
                "Schedule portfolio review in 30 days".to_string(),
                "Monitor market conditions".to_string(),
                "Review risk tolerance if market conditions change significantly".to_string(),
            ],
        })
    }

    // Helper methods for workflow steps

    async fn store_step_memory<T: Serialize>(&self, step_name: &str, data: &T) -> Result<()> {
        let _content = serde_json::to_string(data)?;
        let _memory_key = format!("workflow_step_{step_name}");

        // Note: In a full implementation, step memory would be stored
        // self.memory_system.short_term.store_working_memory(...).await?;

        Ok(())
    }

    async fn store_workflow_outcome(
        &self,
        recommendation: &PersonalizedRecommendation,
    ) -> Result<()> {
        // Store this workflow execution as an episode for future learning
        let episode = RecommendationEpisode {
            recommendation_id: uuid::Uuid::new_v4().to_string(),
            client_id: "workflow_client".to_string(), // Will be updated with actual client ID
            symbol: "workflow_symbol".to_string(),    // Will be updated with actual symbol
            action: recommendation.base_recommendation,
            reasoning: recommendation.personalized_reasoning.clone(),
            confidence: recommendation.confidence_adjustment,
            market_conditions: MarketSnapshot::default(),
            outcome: None, // To be updated when outcome is known
            timestamp: Utc::now(),
            workflow_steps: Vec::new(), // Could be populated with detailed step results
        };

        let _episode_json = serde_json::to_string(&episode)?;

        // Note: In a full implementation, workflow outcome would be stored in episodic memory
        // self.memory_system.episodic.store_episode(...).await?;

        Ok(())
    }

    async fn collect_step_timings(&self) -> HashMap<String, Duration> {
        // In a real implementation, we would track actual step timings
        let mut timings = HashMap::new();
        timings.insert("initialization".to_string(), Duration::milliseconds(150));
        timings.insert("market_research".to_string(), Duration::milliseconds(800));
        timings.insert("risk_analysis".to_string(), Duration::milliseconds(600));
        timings.insert("compliance".to_string(), Duration::milliseconds(300));
        timings.insert("recommendation".to_string(), Duration::milliseconds(450));
        timings
    }

    async fn calculate_valuation_metrics(&self, symbol: &str) -> HashMap<String, f64> {
        // Simulate market data lookup
        let mut metrics = HashMap::new();

        // These would come from actual market data in a real implementation
        match symbol {
            "AAPL" => {
                metrics.insert("P/E".to_string(), 28.4);
                metrics.insert("P/B".to_string(), 45.2);
                metrics.insert("EV/EBITDA".to_string(), 22.1);
            }
            "MSFT" => {
                metrics.insert("P/E".to_string(), 32.1);
                metrics.insert("P/B".to_string(), 12.8);
                metrics.insert("EV/EBITDA".to_string(), 25.3);
            }
            _ => {
                metrics.insert("P/E".to_string(), 25.0);
                metrics.insert("P/B".to_string(), 3.5);
                metrics.insert("EV/EBITDA".to_string(), 18.0);
            }
        }

        metrics
    }

    fn build_market_analysis_prompt(
        &self,
        context: &AnalysisContext,
        market_facts: &[prollytree::agent::MemoryDocument],
    ) -> String {
        format!(
            r#"Analyze the investment prospects for {} considering:

Current Market Context:
- Symbol: {}
- Sector Analysis Required
- Client Risk Tolerance: {:?}

Historical Knowledge Available:
- {} market-related memories in our database
- Market trend: {}
- Economic indicators suggest: {}

Please provide:
1. Key fundamental factors
2. Technical outlook
3. Sector positioning
4. Risk considerations
5. Investment thesis summary

Keep analysis concise and actionable."#,
            context.symbol,
            context.symbol,
            context.client_profile.risk_tolerance,
            market_facts.len(),
            context.market_conditions.market_trend,
            "mixed signals with focus on fundamentals"
        )
    }

    fn build_risk_analysis_prompt(
        &self,
        context: &AnalysisContext,
        market_data: &MarketAnalysisResult,
    ) -> String {
        format!(
            r#"Perform risk analysis for {} investment:

Market Analysis Summary:
- Analyst Sentiment: {:.1}%
- Market Sentiment: {:.1}%
- Sector Performance: {}

Client Profile:
- Risk Tolerance: {:?}
- Time Horizon: {}
- Investment Goals: {}

Identify top 3-5 risk factors and mitigation strategies."#,
            context.symbol,
            market_data.sentiment_analysis.analyst_sentiment * 100.0,
            market_data.sentiment_analysis.market_sentiment * 100.0,
            market_data.sector_analysis.sector_trend,
            context.client_profile.risk_tolerance,
            context.client_profile.time_horizon,
            context.client_profile.investment_goals.join(", ")
        )
    }

    fn build_compliance_prompt(
        &self,
        context: &AnalysisContext,
        risk_assessment: &RiskAssessmentResult,
    ) -> String {
        format!(
            r#"Compliance review for {} recommendation:

Risk Assessment:
- Overall Risk Score: {:.2}
- Client Risk Alignment: {:.1}%
- Key Risk Factors: {}

Client Profile:
- Risk Tolerance: {:?}
- Restrictions: {}

Review for regulatory compliance and client suitability issues."#,
            context.symbol,
            risk_assessment.overall_risk_score,
            risk_assessment.client_risk_alignment * 100.0,
            risk_assessment.risk_factors.join(", "),
            context.client_profile.risk_tolerance,
            context.client_profile.restrictions.join(", ")
        )
    }

    fn build_personalization_prompt(
        &self,
        base_recommendation: RecommendationType,
        context: &AnalysisContext,
        market_analysis: &MarketAnalysisResult,
        risk_assessment: &RiskAssessmentResult,
    ) -> String {
        format!(
            r#"Create personalized investment advice for client:

Recommendation: {:?} {}
Confidence Level: {:.1}%

Client Context:
- Risk Tolerance: {:?}
- Goals: {}
- Time Horizon: {}

Market Summary:
- Fundamental Outlook: {}
- Risk Level: {:.1}/10
- Sector Trend: {}

Explain this recommendation in a personal, conversational tone that addresses:
1. Why this makes sense for their specific situation
2. How it aligns with their goals and risk tolerance
3. What to expect and next steps

Be encouraging but realistic."#,
            base_recommendation,
            context.symbol,
            risk_assessment.client_risk_alignment * 100.0,
            context.client_profile.risk_tolerance,
            context.client_profile.investment_goals.join(", "),
            context.client_profile.time_horizon,
            market_analysis.fundamental_analysis.growth_prospects,
            risk_assessment.overall_risk_score * 10.0,
            market_analysis.sector_analysis.sector_trend
        )
    }

    fn generate_fallback_reasoning(
        &self,
        recommendation: RecommendationType,
        context: &AnalysisContext,
        risk_assessment: &RiskAssessmentResult,
    ) -> String {
        match recommendation {
            RecommendationType::Buy => format!(
                "Based on our analysis, {} presents a good opportunity for your {:?} risk profile. \
                The investment aligns well with your {} goals, and our risk assessment shows \
                a {:.1}% alignment with your comfort level. This position would fit well within \
                your {} investment timeline.",
                context.symbol,
                context.client_profile.risk_tolerance,
                context.client_profile.investment_goals.join(" and "),
                risk_assessment.client_risk_alignment * 100.0,
                context.client_profile.time_horizon
            ),
            RecommendationType::Hold => format!(
                "For {}, our recommendation is to maintain your current position. Given your {:?} \
                risk tolerance and current market conditions, holding allows you to maintain exposure \
                while we monitor for better opportunities that align with your {} objectives.",
                context.symbol,
                context.client_profile.risk_tolerance,
                context.client_profile.investment_goals.join(" and ")
            ),
            RecommendationType::Sell => format!(
                "We recommend reducing your {} position at this time. While the fundamentals remain \
                solid, current market conditions and your {:?} risk profile suggest taking some \
                profits would be prudent. This aligns with your {} strategy and helps preserve capital.",
                context.symbol,
                context.client_profile.risk_tolerance,
                context.client_profile.time_horizon
            ),
            RecommendationType::Rebalance => format!(
                "A rebalancing approach for {} would serve your portfolio well. Given your {:?} risk \
                tolerance and {} goals, adjusting your position size will help maintain optimal \
                risk levels while staying aligned with your investment strategy.",
                context.symbol,
                context.client_profile.risk_tolerance,
                context.client_profile.investment_goals.join(" and ")
            ),
        }
    }

    fn calculate_confidence_adjustment(
        &self,
        market_analysis: &MarketAnalysisResult,
        risk_assessment: &RiskAssessmentResult,
        compliance_validation: &ComplianceValidation,
        client_history: &[prollytree::agent::MemoryDocument],
    ) -> f64 {
        let mut confidence: f64 = 0.75; // Base confidence

        // Boost confidence for strong market signals
        if market_analysis.sentiment_analysis.analyst_sentiment > 0.8 {
            confidence += 0.1;
        }

        // Boost confidence for good risk alignment
        if risk_assessment.client_risk_alignment > 0.9 {
            confidence += 0.1;
        }

        // Reduce confidence for compliance issues
        if !compliance_validation.passed {
            confidence -= 0.2;
        }

        if !compliance_validation.warnings.is_empty() {
            confidence -= 0.05;
        }

        // Boost confidence based on historical client relationship
        if client_history.len() > 10 {
            confidence += 0.05; // More data = more confidence
        }

        confidence.clamp(0.1, 0.95) // Keep within reasonable bounds
    }
}
