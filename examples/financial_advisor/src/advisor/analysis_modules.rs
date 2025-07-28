use anyhow::Result;
// Removed unused DateTime and Utc imports
use rig::{completion::Prompt, providers::openai::Client};
// Removed unused Deserialize and Serialize imports
use std::collections::HashMap;
use std::sync::Arc;

use prollytree::agent::AgentMemorySystem;

use crate::advisor::{RecommendationType, RiskTolerance};
use crate::memory::enhanced_types::*;

/// Registry of analysis modules for different financial analysis tasks
pub struct AnalysisModuleRegistry {
    pub market_research: MarketResearchModule,
    pub risk_analysis: RiskAnalysisModule,
    pub compliance_check: ComplianceModule,
    pub recommendation_engine: RecommendationModule,
}

impl AnalysisModuleRegistry {
    pub fn new(memory_system: Arc<AgentMemorySystem>, rig_client: Option<Client>) -> Self {
        Self {
            market_research: MarketResearchModule::new(memory_system.clone(), rig_client.clone()),
            risk_analysis: RiskAnalysisModule::new(memory_system.clone(), rig_client.clone()),
            compliance_check: ComplianceModule::new(memory_system.clone(), rig_client.clone()),
            recommendation_engine: RecommendationModule::new(memory_system.clone(), rig_client),
        }
    }
}

/// Market research and analysis module
pub struct MarketResearchModule {
    memory_system: Arc<AgentMemorySystem>,
    rig_client: Option<Client>,
}

impl MarketResearchModule {
    pub fn new(memory_system: Arc<AgentMemorySystem>, rig_client: Option<Client>) -> Self {
        Self {
            memory_system,
            rig_client,
        }
    }

    /// Perform comprehensive market research for a symbol
    pub async fn analyze_market(
        &self,
        symbol: &str,
        context: &AnalysisContext,
    ) -> Result<MarketAnalysisResult> {
        // 1. Retrieve historical market data from semantic memory
        let market_facts = self.get_market_knowledge(symbol).await?;

        // 2. Find similar market conditions from episodic memory
        let similar_episodes = self
            .find_similar_market_conditions(&context.market_conditions)
            .await?;

        // 3. Apply procedural knowledge for market analysis
        let _analysis_procedures = self.get_analysis_procedures().await?;

        // 4. Generate AI-powered insights if available
        let ai_insights = self
            .generate_ai_insights(symbol, &market_facts, &similar_episodes)
            .await;

        // 5. Synthesize comprehensive analysis
        Ok(MarketAnalysisResult {
            fundamental_analysis: self
                .perform_fundamental_analysis(symbol, &market_facts)
                .await?,
            technical_analysis: self.perform_technical_analysis(symbol).await?,
            sector_analysis: self.perform_sector_analysis(symbol).await?,
            sentiment_analysis: self
                .perform_sentiment_analysis(symbol, &similar_episodes)
                .await?,
            ai_insights,
        })
    }

    async fn get_market_knowledge(
        &self,
        symbol: &str,
    ) -> Result<Vec<prollytree::agent::MemoryDocument>> {
        self.memory_system
            .semantic
            .get_entity_facts("market", symbol)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to retrieve market knowledge: {}", e))
    }

    async fn find_similar_market_conditions(
        &self,
        conditions: &MarketSnapshot,
    ) -> Result<Vec<prollytree::agent::MemoryDocument>> {
        // Search for episodes with similar market conditions
        let _search_tags = vec!["market_analysis", &conditions.market_trend.to_lowercase()];

        self.memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(30),
                chrono::Utc::now(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to find similar market conditions: {}", e))
    }

    async fn get_analysis_procedures(&self) -> Result<Vec<prollytree::agent::MemoryDocument>> {
        self.memory_system
            .procedural
            .get_procedures_by_category("market_analysis")
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get analysis procedures: {}", e))
    }

    async fn generate_ai_insights(
        &self,
        symbol: &str,
        market_facts: &[prollytree::agent::MemoryDocument],
        episodes: &[prollytree::agent::MemoryDocument],
    ) -> String {
        if let Some(ref client) = self.rig_client {
            let prompt = format!(
                r#"Provide investment insights for {}:

Historical Data Points: {} market-related memories
Similar Market Episodes: {} historical episodes

Focus on:
1. Key investment drivers
2. Potential catalysts or risks
3. Market positioning
4. Timing considerations

Provide 2-3 key insights in bullet format."#,
                symbol,
                market_facts.len(),
                episodes.len()
            );

            let agent = client
                .agent("gpt-3.5-turbo")
                .preamble("You are a senior market analyst providing concise, actionable investment insights.")
                .max_tokens(300)
                .temperature(0.3)
                .build();

            match agent.prompt(&prompt).await {
                Ok(response) => response.trim().to_string(),
                Err(_) => format!(
                    "Market analysis for {} shows mixed signals requiring careful evaluation",
                    symbol
                ),
            }
        } else {
            format!(
                "Technical and fundamental analysis suggests {} requires further evaluation",
                symbol
            )
        }
    }

    async fn perform_fundamental_analysis(
        &self,
        symbol: &str,
        _market_facts: &[prollytree::agent::MemoryDocument],
    ) -> Result<FundamentalAnalysis> {
        // Simulate fundamental analysis based on symbol and available data
        let (growth_prospects, competitive_position, financial_health) = match symbol {
            "AAPL" => (
                "Strong growth driven by services and emerging markets expansion",
                "Dominant position in premium consumer electronics with strong brand loyalty",
                "Excellent balance sheet with substantial cash reserves and manageable debt",
            ),
            "MSFT" => (
                "Consistent growth in cloud computing and enterprise software solutions",
                "Leading market position in productivity software and cloud infrastructure",
                "Strong financial metrics with steady cash flow generation",
            ),
            "GOOGL" => (
                "Growth driven by digital advertising and cloud services expansion",
                "Dominant search engine with expanding ecosystem of services",
                "Strong financial position with diversified revenue streams",
            ),
            _ => (
                "Moderate growth prospects based on sector fundamentals",
                "Competitive position varies with market conditions",
                "Financial health requires individual assessment",
            ),
        };

        let mut valuation_metrics = HashMap::new();
        match symbol {
            "AAPL" => {
                valuation_metrics.insert("P/E".to_string(), 28.4);
                valuation_metrics.insert("P/B".to_string(), 45.2);
                valuation_metrics.insert("EV/Revenue".to_string(), 7.2);
                valuation_metrics.insert("ROE".to_string(), 0.84);
            }
            "MSFT" => {
                valuation_metrics.insert("P/E".to_string(), 32.1);
                valuation_metrics.insert("P/B".to_string(), 12.8);
                valuation_metrics.insert("EV/Revenue".to_string(), 13.5);
                valuation_metrics.insert("ROE".to_string(), 0.44);
            }
            _ => {
                valuation_metrics.insert("P/E".to_string(), 25.0);
                valuation_metrics.insert("P/B".to_string(), 3.5);
                valuation_metrics.insert("EV/Revenue".to_string(), 5.0);
                valuation_metrics.insert("ROE".to_string(), 0.15);
            }
        }

        Ok(FundamentalAnalysis {
            valuation_metrics,
            growth_prospects: growth_prospects.to_string(),
            competitive_position: competitive_position.to_string(),
            financial_health: financial_health.to_string(),
        })
    }

    async fn perform_technical_analysis(&self, symbol: &str) -> Result<TechnicalAnalysis> {
        // Simulate technical analysis (in real implementation, would use actual market data)
        let (trend_direction, support_levels, resistance_levels) = match symbol {
            "AAPL" => (
                "Upward trend with minor consolidation",
                vec![175.0, 170.0, 165.0],
                vec![185.0, 190.0, 195.0],
            ),
            "MSFT" => (
                "Sideways trend with upward bias",
                vec![410.0, 405.0, 400.0],
                vec![420.0, 425.0, 430.0],
            ),
            _ => (
                "Mixed trend requiring further analysis",
                vec![100.0, 95.0, 90.0],
                vec![110.0, 115.0, 120.0],
            ),
        };

        let mut momentum_indicators = HashMap::new();
        momentum_indicators.insert("RSI".to_string(), 58.5);
        momentum_indicators.insert("MACD".to_string(), 1.2);
        momentum_indicators.insert("Stochastic".to_string(), 62.3);
        momentum_indicators.insert("Williams%R".to_string(), -38.7);

        Ok(TechnicalAnalysis {
            trend_direction: trend_direction.to_string(),
            support_levels,
            resistance_levels,
            momentum_indicators,
        })
    }

    async fn perform_sector_analysis(&self, symbol: &str) -> Result<SectorAnalysis> {
        let sector_info = match symbol {
            "AAPL" | "MSFT" | "GOOGL" => (
                "Technology sector showing resilience amid market volatility",
                1.15,
                "Continued investor interest in technology fundamentals",
                vec![
                    "AI and machine learning adoption".to_string(),
                    "Cloud computing growth".to_string(),
                    "Digital transformation trends".to_string(),
                    "Productivity software demand".to_string(),
                ],
            ),
            _ => (
                "Sector performance varies with market conditions",
                1.0,
                "Mixed outlook depending on economic indicators",
                vec![
                    "Economic cycle positioning".to_string(),
                    "Interest rate sensitivity".to_string(),
                    "Regulatory environment".to_string(),
                ],
            ),
        };

        Ok(SectorAnalysis {
            sector_trend: sector_info.0.to_string(),
            relative_performance: sector_info.1,
            sector_rotation_outlook: sector_info.2.to_string(),
            key_sector_drivers: sector_info.3,
        })
    }

    async fn perform_sentiment_analysis(
        &self,
        symbol: &str,
        _episodes: &[prollytree::agent::MemoryDocument],
    ) -> Result<SentimentAnalysis> {
        // Simulate sentiment analysis based on historical episodes and current factors
        let base_sentiment = match symbol {
            "AAPL" => (0.75, 0.68, 0.72),
            "MSFT" => (0.78, 0.71, 0.74),
            "GOOGL" => (0.72, 0.65, 0.69),
            _ => (0.65, 0.60, 0.62),
        };

        // Adjust based on historical episodes
        let episode_adjustment = if _episodes.len() > 5 { 0.05 } else { 0.0 };

        Ok(SentimentAnalysis {
            analyst_sentiment: f64::min(base_sentiment.0 + episode_adjustment, 1.0),
            market_sentiment: f64::min(base_sentiment.1 + episode_adjustment, 1.0),
            news_sentiment: f64::min(base_sentiment.2 + episode_adjustment, 1.0),
            sentiment_drivers: vec![
                "Strong earnings guidance and execution".to_string(),
                "Product innovation and market expansion".to_string(),
                "Positive analyst revisions".to_string(),
                "Institutional investor confidence".to_string(),
            ],
        })
    }
}

/// Risk analysis module
pub struct RiskAnalysisModule {
    memory_system: Arc<AgentMemorySystem>,
    rig_client: Option<Client>,
}

impl RiskAnalysisModule {
    pub fn new(memory_system: Arc<AgentMemorySystem>, rig_client: Option<Client>) -> Self {
        Self {
            memory_system,
            rig_client,
        }
    }

    /// Perform comprehensive risk assessment
    pub async fn assess_risk(
        &self,
        context: &AnalysisContext,
        market_analysis: &MarketAnalysisResult,
    ) -> Result<RiskAssessmentResult> {
        // 1. Get client's risk history from episodic memory
        let risk_episodes = self.get_client_risk_history(&context.client_id).await?;

        // 2. Apply risk assessment procedures from procedural memory
        let _risk_procedures = self.get_risk_procedures().await?;

        // 3. Calculate risk breakdown based on multiple factors
        let risk_breakdown = self
            .calculate_risk_breakdown(context, market_analysis)
            .await?;

        // 4. Generate AI-powered risk insights if available
        let risk_factors = self
            .generate_risk_insights(context, market_analysis, &risk_episodes)
            .await;

        // 5. Assess client alignment
        let client_risk_alignment = self
            .assess_client_alignment(context, &risk_breakdown)
            .await?;

        let overall_risk_score = risk_breakdown.values().sum::<f64>() / risk_breakdown.len() as f64;
        let risk_breakdown_clone = risk_breakdown.clone();

        Ok(RiskAssessmentResult {
            overall_risk_score,
            risk_breakdown,
            risk_factors,
            mitigation_recommendations: self
                .generate_mitigation_strategies(context, &risk_breakdown_clone)
                .await,
            client_risk_alignment,
        })
    }

    async fn get_client_risk_history(
        &self,
        _client_id: &str,
    ) -> Result<Vec<prollytree::agent::MemoryDocument>> {
        self.memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(90),
                chrono::Utc::now(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get client risk history: {}", e))
    }

    async fn get_risk_procedures(&self) -> Result<Vec<prollytree::agent::MemoryDocument>> {
        self.memory_system
            .procedural
            .get_procedures_by_category("risk_assessment")
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get risk procedures: {}", e))
    }

    async fn calculate_risk_breakdown(
        &self,
        context: &AnalysisContext,
        market_analysis: &MarketAnalysisResult,
    ) -> Result<HashMap<RiskCategory, f64>> {
        let mut risk_breakdown = HashMap::new();

        // Market risk based on volatility and sentiment
        let market_risk = if market_analysis.sentiment_analysis.market_sentiment < 0.5 {
            0.8
        } else if market_analysis.sentiment_analysis.market_sentiment > 0.8 {
            0.4
        } else {
            0.6
        };
        risk_breakdown.insert(RiskCategory::MarketRisk, market_risk);

        // Credit risk (simplified - would be more complex in real implementation)
        risk_breakdown.insert(RiskCategory::CreditRisk, 0.25);

        // Liquidity risk based on market conditions
        let liquidity_risk = if context.market_conditions.volatility_index > 30.0 {
            0.7
        } else {
            0.3
        };
        risk_breakdown.insert(RiskCategory::LiquidityRisk, liquidity_risk);

        // Concentration risk based on client portfolio (simplified)
        risk_breakdown.insert(RiskCategory::ConcentrationRisk, 0.45);

        // Interest rate risk
        let interest_rate_risk = if context.market_conditions.interest_rates > 5.0 {
            0.6
        } else {
            0.4
        };
        risk_breakdown.insert(RiskCategory::InterestRateRisk, interest_rate_risk);

        Ok(risk_breakdown)
    }

    async fn generate_risk_insights(
        &self,
        context: &AnalysisContext,
        market_analysis: &MarketAnalysisResult,
        _episodes: &[prollytree::agent::MemoryDocument],
    ) -> Vec<String> {
        if let Some(ref client) = self.rig_client {
            let prompt = format!(
                r#"Identify key risk factors for {} investment:

Market Analysis:
- Analyst Sentiment: {:.1}%
- Market Sentiment: {:.1}%
- Sector Performance: {}

Client Context:
- Risk Tolerance: {:?}
- Time Horizon: {}
- Historical Risk Episodes: {}

List 3-5 specific risk factors to monitor."#,
                context.symbol,
                market_analysis.sentiment_analysis.analyst_sentiment * 100.0,
                market_analysis.sentiment_analysis.market_sentiment * 100.0,
                market_analysis.sector_analysis.sector_trend,
                context.client_profile.risk_tolerance,
                context.client_profile.time_horizon,
                _episodes.len()
            );

            let agent = client
                .agent("gpt-3.5-turbo")
                .preamble(
                    "You are a risk management expert. Focus on specific, actionable risk factors.",
                )
                .max_tokens(250)
                .temperature(0.2)
                .build();

            match agent.prompt(&prompt).await {
                Ok(response) => response
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.trim().to_string())
                    .collect(),
                Err(_) => self.generate_default_risk_factors(context),
            }
        } else {
            self.generate_default_risk_factors(context)
        }
    }

    fn generate_default_risk_factors(&self, context: &AnalysisContext) -> Vec<String> {
        vec![
            format!(
                "Market volatility risk for {} sector exposure",
                context.symbol
            ),
            "Interest rate sensitivity given current economic environment".to_string(),
            format!(
                "Concentration risk based on {:?} risk tolerance",
                context.client_profile.risk_tolerance
            ),
            "Liquidity risk during market stress periods".to_string(),
            "Sector-specific risks and regulatory changes".to_string(),
        ]
    }

    async fn assess_client_alignment(
        &self,
        context: &AnalysisContext,
        risk_breakdown: &HashMap<RiskCategory, f64>,
    ) -> Result<f64> {
        let overall_risk = risk_breakdown.values().sum::<f64>() / risk_breakdown.len() as f64;

        let alignment = match context.client_profile.risk_tolerance {
            RiskTolerance::Conservative => {
                if overall_risk > 0.7 {
                    0.5
                } else if overall_risk > 0.5 {
                    0.7
                } else {
                    0.9
                }
            }
            RiskTolerance::Moderate => {
                if overall_risk > 0.8 {
                    0.6
                } else if overall_risk < 0.3 {
                    0.7
                } else {
                    0.85
                }
            }
            RiskTolerance::Aggressive => {
                if overall_risk < 0.4 {
                    0.6
                } else if overall_risk > 0.6 {
                    0.9
                } else {
                    0.8
                }
            }
        };

        Ok(alignment)
    }

    async fn generate_mitigation_strategies(
        &self,
        context: &AnalysisContext,
        risk_breakdown: &HashMap<RiskCategory, f64>,
    ) -> Vec<String> {
        let mut strategies = Vec::new();

        for (risk_type, risk_level) in risk_breakdown {
            if *risk_level > 0.6 {
                let strategy = match risk_type {
                    RiskCategory::MarketRisk => {
                        "Consider position sizing and diversification across market sectors"
                    }
                    RiskCategory::LiquidityRisk => {
                        "Maintain adequate cash reserves and avoid illiquid positions"
                    }
                    RiskCategory::ConcentrationRisk => {
                        "Diversify holdings across different assets and sectors"
                    }
                    RiskCategory::InterestRateRisk => {
                        "Consider duration management and rate-sensitive asset allocation"
                    }
                    RiskCategory::CreditRisk => "Focus on high-quality issuers and credit analysis",
                    _ => "Monitor risk factors and adjust position sizing as needed",
                };
                strategies.push(strategy.to_string());
            }
        }

        // Add client-specific strategies
        match context.client_profile.risk_tolerance {
            RiskTolerance::Conservative => {
                strategies.push(
                    "Emphasize capital preservation and steady income generation".to_string(),
                );
            }
            RiskTolerance::Aggressive => {
                strategies.push(
                    "Consider using stop-loss orders and profit-taking strategies".to_string(),
                );
            }
            _ => {}
        }

        if strategies.is_empty() {
            strategies.push("Regular portfolio review and rebalancing".to_string());
        }

        strategies
    }
}

/// Compliance checking module
pub struct ComplianceModule {
    memory_system: Arc<AgentMemorySystem>,
    rig_client: Option<Client>,
}

impl ComplianceModule {
    pub fn new(memory_system: Arc<AgentMemorySystem>, rig_client: Option<Client>) -> Self {
        Self {
            memory_system,
            rig_client,
        }
    }

    /// Perform comprehensive compliance validation
    pub async fn validate_compliance(
        &self,
        context: &AnalysisContext,
        risk_assessment: &RiskAssessmentResult,
    ) -> Result<ComplianceValidation> {
        // 1. Get compliance rules from procedural memory
        let _compliance_rules = self.get_compliance_rules().await?;

        // 2. Check client-specific restrictions
        let _client_restrictions = self.get_client_restrictions(&context.client_id).await?;

        // 3. Analyze historical compliance issues
        let _compliance_history = self.get_compliance_history().await?;

        // 4. Perform compliance checks
        let mut violations = Vec::new();
        let mut warnings = Vec::new();

        // Check suitability
        if risk_assessment.client_risk_alignment < 0.7 {
            violations.push(ComplianceViolation {
                rule_id: "SUITABILITY_001".to_string(),
                severity: ComplianceSeverity::Warning,
                description: "Investment may not align with client risk profile".to_string(),
                recommended_action: "Review recommendation with client or adjust strategy"
                    .to_string(),
            });
        }

        // Check position limits
        if risk_assessment.overall_risk_score > 0.8 {
            warnings.push(ComplianceWarning {
                rule_id: "RISK_001".to_string(),
                description: "High risk score detected - monitor position sizing".to_string(),
                recommendation:
                    "Consider reducing position size or implementing additional risk controls"
                        .to_string(),
            });
        }

        // Generate automated actions
        let automated_actions = self
            .generate_compliance_actions(context, risk_assessment, &violations, &warnings)
            .await;

        Ok(ComplianceValidation {
            passed: violations.is_empty(),
            violations,
            warnings,
            required_disclosures: vec![
                "Past performance does not guarantee future results".to_string(),
                "All investments carry risk of loss".to_string(),
                "Please review all investment materials carefully".to_string(),
            ],
            automated_actions_taken: automated_actions,
        })
    }

    async fn get_compliance_rules(&self) -> Result<Vec<prollytree::agent::MemoryDocument>> {
        self.memory_system
            .procedural
            .get_procedures_by_category("compliance")
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get compliance rules: {}", e))
    }

    async fn get_client_restrictions(
        &self,
        client_id: &str,
    ) -> Result<Vec<prollytree::agent::MemoryDocument>> {
        self.memory_system
            .semantic
            .get_entity_facts("client_restrictions", client_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get client restrictions: {}", e))
    }

    async fn get_compliance_history(&self) -> Result<Vec<prollytree::agent::MemoryDocument>> {
        self.memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(365),
                chrono::Utc::now(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get compliance history: {}", e))
    }

    async fn generate_compliance_actions(
        &self,
        _context: &AnalysisContext,
        risk_assessment: &RiskAssessmentResult,
        violations: &[ComplianceViolation],
        warnings: &[ComplianceWarning],
    ) -> Vec<String> {
        let mut actions = Vec::new();

        if !violations.is_empty() {
            actions.push("Compliance violation flagged for manual review".to_string());
        }

        if !warnings.is_empty() {
            actions.push("Compliance warnings documented in client file".to_string());
        }

        if risk_assessment.client_risk_alignment < 0.8 {
            actions.push("Client risk profile review scheduled".to_string());
        }

        actions.push("All compliance checks completed and documented".to_string());

        actions
    }
}

/// Recommendation generation module
pub struct RecommendationModule {
    memory_system: Arc<AgentMemorySystem>,
    rig_client: Option<Client>,
}

impl RecommendationModule {
    pub fn new(memory_system: Arc<AgentMemorySystem>, rig_client: Option<Client>) -> Self {
        Self {
            memory_system,
            rig_client,
        }
    }

    /// Generate personalized investment recommendation
    pub async fn generate_recommendation(
        &self,
        context: &AnalysisContext,
        market_analysis: &MarketAnalysisResult,
        risk_assessment: &RiskAssessmentResult,
        compliance_validation: &ComplianceValidation,
    ) -> Result<PersonalizedRecommendation> {
        // 1. Determine base recommendation logic
        let base_recommendation = self
            .determine_base_recommendation(market_analysis, risk_assessment, compliance_validation)
            .await?;

        // 2. Get client interaction history for personalization
        let client_history = self.get_client_history(&context.client_id).await?;

        // 3. Generate personalized reasoning
        let personalized_reasoning = self
            .generate_personalized_reasoning(
                base_recommendation,
                context,
                market_analysis,
                risk_assessment,
                &client_history,
            )
            .await;

        // 4. Calculate confidence adjustment
        let confidence_adjustment = self
            .calculate_confidence_adjustment(
                market_analysis,
                risk_assessment,
                compliance_validation,
                &client_history,
            )
            .await;

        // 5. Extract client-specific factors
        let client_specific_factors = self.extract_client_factors(context, risk_assessment).await;

        Ok(PersonalizedRecommendation {
            base_recommendation,
            personalized_reasoning,
            confidence_adjustment,
            client_specific_factors,
            presentation_style: "conversational".to_string(),
            follow_up_actions: self
                .generate_follow_up_actions(context, base_recommendation)
                .await,
        })
    }

    async fn determine_base_recommendation(
        &self,
        market_analysis: &MarketAnalysisResult,
        risk_assessment: &RiskAssessmentResult,
        compliance_validation: &ComplianceValidation,
    ) -> Result<RecommendationType> {
        // Compliance first - if issues exist, default to hold
        if !compliance_validation.passed {
            return Ok(RecommendationType::Hold);
        }

        // Strong positive signals
        if market_analysis.sentiment_analysis.analyst_sentiment > 0.75
            && risk_assessment.client_risk_alignment > 0.8
            && risk_assessment.overall_risk_score < 0.7
        {
            return Ok(RecommendationType::Buy);
        }

        // Strong negative signals
        if market_analysis.sentiment_analysis.analyst_sentiment < 0.4
            || risk_assessment.overall_risk_score > 0.8
        {
            return Ok(RecommendationType::Sell);
        }

        // Check for rebalancing needs
        if risk_assessment.client_risk_alignment < 0.7 {
            return Ok(RecommendationType::Rebalance);
        }

        // Default to hold for neutral conditions
        Ok(RecommendationType::Hold)
    }

    async fn get_client_history(
        &self,
        _client_id: &str,
    ) -> Result<Vec<prollytree::agent::MemoryDocument>> {
        self.memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(180),
                chrono::Utc::now(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get client history: {}", e))
    }

    async fn generate_personalized_reasoning(
        &self,
        recommendation: RecommendationType,
        context: &AnalysisContext,
        market_analysis: &MarketAnalysisResult,
        risk_assessment: &RiskAssessmentResult,
        _client_history: &[prollytree::agent::MemoryDocument],
    ) -> String {
        if let Some(ref client) = self.rig_client {
            let prompt = format!(
                r#"Create personalized investment advice for client:

Recommendation: {:?} {}
Market Outlook: {}
Risk Level: {:.1}/10 (Alignment: {:.1}%)
Client Profile: {:?} risk tolerance, {} time horizon

Client History: {} previous interactions
Investment Goals: {}

Explain this recommendation in a warm, personal tone that:
1. Acknowledges their specific situation and goals
2. Connects to their risk tolerance and time horizon  
3. Provides clear reasoning and next steps
4. Shows confidence while being realistic

Keep it conversational and encouraging."#,
                recommendation,
                context.symbol,
                market_analysis.fundamental_analysis.growth_prospects,
                risk_assessment.overall_risk_score * 10.0,
                risk_assessment.client_risk_alignment * 100.0,
                context.client_profile.risk_tolerance,
                context.client_profile.time_horizon,
                _client_history.len(),
                context.client_profile.investment_goals.join(", ")
            );

            let agent = client
                .agent("gpt-3.5-turbo")
                .preamble("You are a trusted financial advisor speaking directly to a valued client. Be personal, clear, and confidence-inspiring.")
                .max_tokens(400)
                .temperature(0.4)
                .build();

            match agent.prompt(&prompt).await {
                Ok(response) => response.trim().to_string(),
                Err(_) => {
                    self.generate_fallback_reasoning(recommendation, context, risk_assessment)
                }
            }
        } else {
            self.generate_fallback_reasoning(recommendation, context, risk_assessment)
        }
    }

    fn generate_fallback_reasoning(
        &self,
        recommendation: RecommendationType,
        context: &AnalysisContext,
        risk_assessment: &RiskAssessmentResult,
    ) -> String {
        match recommendation {
            RecommendationType::Buy => format!(
                "Based on our comprehensive analysis, {} presents an attractive opportunity that aligns well with your {:?} risk profile and {} investment timeline. Our analysis shows a {:.1}% alignment with your comfort level, and the investment supports your goals of {}. This recommendation reflects both strong market fundamentals and suitability for your unique situation.",
                context.symbol,
                context.client_profile.risk_tolerance,
                context.client_profile.time_horizon,
                risk_assessment.client_risk_alignment * 100.0,
                context.client_profile.investment_goals.join(" and ")
            ),
            RecommendationType::Hold => format!(
                "For your {} position, we recommend maintaining your current exposure at this time. Given your {:?} risk tolerance and {} investment horizon, holding allows you to stay positioned for potential upside while we continue monitoring market conditions. This approach aligns well with your {} objectives and maintains appropriate risk levels.",
                context.symbol,
                context.client_profile.risk_tolerance,
                context.client_profile.time_horizon,
                context.client_profile.investment_goals.join(" and ")
            ),
            RecommendationType::Sell => format!(
                "Our analysis suggests reducing your {} position would be prudent given current market conditions and your {:?} risk profile. While the underlying fundamentals remain solid, taking some profits aligns with your {} timeline and helps preserve capital for future opportunities that better match your {} goals.",
                context.symbol,
                context.client_profile.risk_tolerance,
                context.client_profile.time_horizon,
                context.client_profile.investment_goals.join(" and ")
            ),
            RecommendationType::Rebalance => format!(
                "We recommend rebalancing your {} position to better align with your {:?} risk tolerance and {} investment strategy. This adjustment will help optimize your portfolio's risk-return profile while ensuring it continues to serve your {} objectives effectively.",
                context.symbol,
                context.client_profile.risk_tolerance,
                context.client_profile.time_horizon,
                context.client_profile.investment_goals.join(" and ")
            ),
        }
    }

    async fn calculate_confidence_adjustment(
        &self,
        market_analysis: &MarketAnalysisResult,
        risk_assessment: &RiskAssessmentResult,
        compliance_validation: &ComplianceValidation,
        _client_history: &[prollytree::agent::MemoryDocument],
    ) -> f64 {
        let mut confidence: f64 = 0.75; // Base confidence

        // Market sentiment boost
        if market_analysis.sentiment_analysis.analyst_sentiment > 0.8 {
            confidence += 0.1;
        }

        // Risk alignment boost
        if risk_assessment.client_risk_alignment > 0.9 {
            confidence += 0.1;
        }

        // Compliance impact
        if !compliance_validation.passed {
            confidence -= 0.2;
        } else if !compliance_validation.warnings.is_empty() {
            confidence -= 0.05;
        }

        // Client relationship depth
        if _client_history.len() > 10 {
            confidence += 0.05;
        }

        // AI availability boost
        if self.rig_client.is_some() {
            confidence += 0.05;
        }

        confidence.max(0.1).min(0.95)
    }

    async fn extract_client_factors(
        &self,
        context: &AnalysisContext,
        risk_assessment: &RiskAssessmentResult,
    ) -> Vec<String> {
        vec![
            format!(
                "Risk tolerance: {:?}",
                context.client_profile.risk_tolerance
            ),
            format!(
                "Investment timeline: {}",
                context.client_profile.time_horizon
            ),
            format!(
                "Primary goals: {}",
                context.client_profile.investment_goals.join(", ")
            ),
            format!(
                "Risk alignment score: {:.1}%",
                risk_assessment.client_risk_alignment * 100.0
            ),
            format!(
                "Portfolio value: ${:.0}",
                context.client_profile.portfolio_value
            ),
            if !context.client_profile.restrictions.is_empty() {
                format!(
                    "Investment restrictions: {}",
                    context.client_profile.restrictions.join(", ")
                )
            } else {
                "No specific investment restrictions".to_string()
            },
        ]
    }

    async fn generate_follow_up_actions(
        &self,
        _context: &AnalysisContext,
        recommendation: RecommendationType,
    ) -> Vec<String> {
        let mut actions = vec![
            "Schedule follow-up review in 30 days".to_string(),
            "Monitor market conditions and company fundamentals".to_string(),
        ];

        match recommendation {
            RecommendationType::Buy => {
                actions.push("Consider dollar-cost averaging for position building".to_string());
                actions.push("Set target price levels for profit taking".to_string());
            }
            RecommendationType::Sell => {
                actions.push("Review tax implications of sale".to_string());
                actions.push("Identify alternative investment opportunities".to_string());
            }
            RecommendationType::Rebalance => {
                actions.push("Calculate optimal position sizing".to_string());
                actions.push("Schedule portfolio rebalancing execution".to_string());
            }
            RecommendationType::Hold => {
                actions.push("Establish monitoring triggers for position changes".to_string());
            }
        }

        if matches!(
            _context.client_profile.risk_tolerance,
            RiskTolerance::Conservative
        ) {
            actions.push("Review income-generating alternatives".to_string());
        }

        actions
    }
}
