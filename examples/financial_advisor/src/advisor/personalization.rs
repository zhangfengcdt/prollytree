use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rig::{completion::Prompt, providers::openai::Client};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use prollytree::agent::AgentMemorySystem;

use crate::advisor::RiskTolerance;
use crate::memory::enhanced_types::*;

/// Personalization engine that adapts recommendations based on client memory and behavior
pub struct PersonalizationEngine {
    memory_system: Arc<AgentMemorySystem>,
    rig_client: Option<Client>,
    client_models: HashMap<String, ClientBehaviorModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientBehaviorModel {
    pub client_id: String,
    pub decision_patterns: Vec<DecisionPattern>,
    pub risk_evolution: RiskEvolution,
    pub communication_preferences: CommunicationPreferences,
    pub outcome_sensitivity: OutcomeSensitivity,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPattern {
    pub pattern_type: DecisionPatternType,
    pub frequency: f64,
    pub success_rate: f64,
    pub context_factors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionPatternType {
    FollowsRecommendations,
    CounterRecommendations,
    DelaysDecisions,
    SeeksAdditionalOpinions,
    ImpulsiveDecisions,
    ResearchFocused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskEvolution {
    pub initial_tolerance: RiskTolerance,
    pub current_tolerance: RiskTolerance,
    pub tolerance_changes: Vec<RiskToleranceChange>,
    pub volatility_comfort: f64,
    pub loss_aversion: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskToleranceChange {
    pub from: RiskTolerance,
    pub to: RiskTolerance,
    pub trigger_event: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationPreferences {
    pub preferred_detail_level: DetailLevel,
    pub preferred_language_style: LanguageStyle,
    pub emphasis_areas: Vec<EmphasisArea>,
    pub response_timing: ResponseTiming,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DetailLevel {
    Brief,
    Moderate,
    Comprehensive,
    Technical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LanguageStyle {
    Professional,
    Conversational,
    Educational,
    Direct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmphasisArea {
    RiskManagement,
    GrowthPotential,
    IncomeGeneration,
    TaxEfficiency,
    LiquidityNeeds,
    TimeHorizon,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseTiming {
    Immediate,
    ConsiderativeTime,
    ResearchPeriod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeSensitivity {
    pub loss_sensitivity: f64,
    pub gain_satisfaction: f64,
    pub regret_avoidance: f64,
    pub confirmation_bias: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PersonalizationInsights {
    pub client_id: String,
    pub behavioral_score: f64,
    pub recommended_approach: RecommendationApproach,
    pub confidence_adjustments: Vec<ConfidenceAdjustment>,
    pub communication_strategy: CommunicationStrategy,
    pub risk_considerations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecommendationApproach {
    pub presentation_style: String,
    pub focus_areas: Vec<String>,
    pub evidence_level: String,
    pub risk_framing: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfidenceAdjustment {
    pub factor: String,
    pub adjustment: f64,
    pub reasoning: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommunicationStrategy {
    pub opening_approach: String,
    pub key_messages: Vec<String>,
    pub follow_up_strategy: String,
}

impl PersonalizationEngine {
    pub fn new(memory_system: Arc<AgentMemorySystem>, rig_client: Option<Client>) -> Self {
        Self {
            memory_system,
            rig_client,
            client_models: HashMap::new(),
        }
    }

    /// Personalize a recommendation based on client behavior and memory
    pub async fn personalize_recommendation(
        &mut self,
        base_recommendation: &DetailedRecommendation,
        client_id: &str,
    ) -> Result<PersonalizedRecommendation> {
        // 1. Get or build client behavior model
        let behavior_model = self.get_or_build_client_model(client_id).await?;

        // 2. Analyze client interaction history
        let interaction_patterns = self.analyze_client_interactions(client_id).await?;

        // 3. Get outcome history for confidence adjustment
        let outcome_history = self.get_client_outcome_history(client_id).await?;

        // 4. Generate personalization insights
        let insights = self
            .generate_personalization_insights(
                &behavior_model,
                &interaction_patterns,
                &outcome_history,
            )
            .await?;

        // 5. Adapt the recommendation
        let personalized_recommendation = self
            .adapt_recommendation_to_client(base_recommendation, &behavior_model, &insights)
            .await?;

        // 6. Store personalization decision for learning
        self.store_personalization_decision(
            client_id,
            &base_recommendation.recommendation_id,
            &insights,
        )
        .await?;

        Ok(personalized_recommendation)
    }

    /// Build or update client behavior model from interaction history
    async fn get_or_build_client_model(&mut self, client_id: &str) -> Result<ClientBehaviorModel> {
        if let Some(model) = self.client_models.get(client_id) {
            // Check if model needs updating (older than 7 days)
            if Utc::now().signed_duration_since(model.last_updated) < Duration::days(7) {
                return Ok(model.clone());
            }
        }

        // Build new model from memory
        let model = self.build_client_model_from_memory(client_id).await?;
        self.client_models
            .insert(client_id.to_string(), model.clone());

        Ok(model)
    }

    async fn build_client_model_from_memory(&self, client_id: &str) -> Result<ClientBehaviorModel> {
        // Get client interactions from episodic memory
        let interactions = self
            .memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(180),
                chrono::Utc::now(),
            )
            .await
            .unwrap_or_default();

        // Get recommendation outcomes
        let outcomes = self
            .memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(365),
                chrono::Utc::now(),
            )
            .await
            .unwrap_or_default();

        // Get client profile for risk evolution
        let client_facts = self
            .memory_system
            .semantic
            .get_entity_facts("client", client_id)
            .await
            .unwrap_or_default();

        // Analyze patterns
        let decision_patterns = self
            .extract_decision_patterns(&interactions, &outcomes)
            .await;
        let risk_evolution = self
            .analyze_risk_evolution(&client_facts, &interactions)
            .await;
        let communication_preferences = self.infer_communication_preferences(&interactions).await;
        let outcome_sensitivity = self.calculate_outcome_sensitivity(&outcomes).await;

        Ok(ClientBehaviorModel {
            client_id: client_id.to_string(),
            decision_patterns,
            risk_evolution,
            communication_preferences,
            outcome_sensitivity,
            last_updated: Utc::now(),
        })
    }

    async fn extract_decision_patterns(
        &self,
        interactions: &[prollytree::agent::MemoryDocument],
        outcomes: &[prollytree::agent::MemoryDocument],
    ) -> Vec<DecisionPattern> {
        let mut patterns = Vec::new();

        // Analyze if client follows recommendations
        let total_recommendations = interactions.len();
        let followed_recommendations = outcomes
            .iter()
            .filter(|outcome| {
                // Parse outcome and check if recommendation was followed
                if let Ok(parsed) =
                    serde_json::from_str::<serde_json::Value>(&outcome.content.to_string())
                {
                    parsed
                        .get("followed_recommendation")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                } else {
                    false
                }
            })
            .count();

        if total_recommendations > 0 {
            let follow_rate = followed_recommendations as f64 / total_recommendations as f64;
            patterns.push(DecisionPattern {
                pattern_type: DecisionPatternType::FollowsRecommendations,
                frequency: follow_rate,
                success_rate: self
                    .calculate_success_rate_for_pattern(outcomes, true)
                    .await,
                context_factors: vec![
                    "recommendation_confidence".to_string(),
                    "market_conditions".to_string(),
                ],
            });
        }

        // Analyze decision timing
        let avg_response_time = self.calculate_average_response_time(interactions).await;
        if avg_response_time > Duration::days(2) {
            patterns.push(DecisionPattern {
                pattern_type: DecisionPatternType::DelaysDecisions,
                frequency: 0.7, // Simplified calculation
                success_rate: 0.6,
                context_factors: vec![
                    "market_volatility".to_string(),
                    "recommendation_complexity".to_string(),
                ],
            });
        }

        patterns
    }

    async fn analyze_risk_evolution(
        &self,
        client_facts: &[prollytree::agent::MemoryDocument],
        interactions: &[prollytree::agent::MemoryDocument],
    ) -> RiskEvolution {
        let mut tolerance_changes = Vec::new();
        let mut initial_tolerance = RiskTolerance::Moderate;
        let mut current_tolerance = RiskTolerance::Moderate;

        // Parse client profile for initial risk tolerance
        if let Some(fact) = client_facts.first() {
            if let Ok(client_data) = serde_json::from_str::<ClientEntity>(&fact.content.to_string())
            {
                initial_tolerance = client_data.risk_tolerance;
                current_tolerance = client_data.risk_tolerance;
            }
        }

        // Look for risk tolerance changes in interactions
        for interaction in interactions {
            if let Ok(parsed) =
                serde_json::from_str::<ClientInteractionEpisode>(&interaction.content.to_string())
            {
                if parsed.interaction_type == InteractionType::RiskAssessment {
                    // Extract risk tolerance changes from interaction summary
                    if parsed.summary.contains("updated")
                        && parsed.summary.contains("risk tolerance")
                    {
                        tolerance_changes.push(RiskToleranceChange {
                            from: initial_tolerance,
                            to: current_tolerance,
                            trigger_event: parsed.summary.clone(),
                            timestamp: parsed.timestamp,
                        });
                    }
                }
            }
        }

        RiskEvolution {
            initial_tolerance,
            current_tolerance,
            tolerance_changes,
            volatility_comfort: 0.6, // Would be calculated from actual behavior
            loss_aversion: 0.7,
        }
    }

    async fn infer_communication_preferences(
        &self,
        interactions: &[prollytree::agent::MemoryDocument],
    ) -> CommunicationPreferences {
        // Analyze interaction patterns to infer preferences
        let mut detail_requests = 0;
        let mut quick_responses = 0;
        let total_interactions = interactions.len();

        for interaction in interactions {
            if let Ok(parsed) =
                serde_json::from_str::<ClientInteractionEpisode>(&interaction.content.to_string())
            {
                // Check for detail level indicators
                if parsed.key_topics.contains(&"detailed_analysis".to_string()) {
                    detail_requests += 1;
                }

                // Check response timing
                if parsed.summary.contains("quick") || parsed.summary.contains("brief") {
                    quick_responses += 1;
                }
            }
        }

        let detail_level = if total_interactions > 0 {
            let detail_ratio = detail_requests as f64 / total_interactions as f64;
            if detail_ratio > 0.7 {
                DetailLevel::Comprehensive
            } else if detail_ratio > 0.4 {
                DetailLevel::Moderate
            } else {
                DetailLevel::Brief
            }
        } else {
            DetailLevel::Moderate
        };

        CommunicationPreferences {
            preferred_detail_level: detail_level,
            preferred_language_style: LanguageStyle::Conversational,
            emphasis_areas: vec![EmphasisArea::RiskManagement, EmphasisArea::GrowthPotential],
            response_timing: if quick_responses > total_interactions / 2 {
                ResponseTiming::Immediate
            } else {
                ResponseTiming::ConsiderativeTime
            },
        }
    }

    async fn calculate_outcome_sensitivity(
        &self,
        outcomes: &[prollytree::agent::MemoryDocument],
    ) -> OutcomeSensitivity {
        let mut total_returns = Vec::new();
        let mut satisfaction_scores = Vec::new();

        for outcome in outcomes {
            if let Ok(parsed) =
                serde_json::from_str::<serde_json::Value>(&outcome.content.to_string())
            {
                if let Some(return_val) = parsed.get("actual_return").and_then(|v| v.as_f64()) {
                    total_returns.push(return_val);
                }
                if let Some(satisfaction) =
                    parsed.get("client_satisfaction").and_then(|v| v.as_f64())
                {
                    satisfaction_scores.push(satisfaction);
                }
            }
        }

        // Calculate sensitivity metrics
        let loss_sensitivity = if !total_returns.is_empty() {
            let negative_returns: Vec<_> = total_returns.iter().filter(|&&r| r < 0.0).collect();
            if !negative_returns.is_empty() {
                negative_returns.iter().map(|&&r| r.abs()).sum::<f64>()
                    / negative_returns.len() as f64
            } else {
                0.5
            }
        } else {
            0.6
        };

        OutcomeSensitivity {
            loss_sensitivity,
            gain_satisfaction: 0.7,
            regret_avoidance: 0.6,
            confirmation_bias: 0.5,
        }
    }

    async fn analyze_client_interactions(
        &self,
        _client_id: &str,
    ) -> Result<Vec<InteractionPattern>> {
        let interactions = self
            .memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(180),
                chrono::Utc::now(),
            )
            .await
            .unwrap_or_default();

        let mut patterns = Vec::new();

        // Analyze interaction frequency
        if interactions.len() > 5 {
            patterns.push(InteractionPattern {
                pattern_name: "High Engagement".to_string(),
                frequency: interactions.len() as f64 / 30.0, // Interactions per month
                impact_score: 0.8,
            });
        }

        // Analyze sentiment trends
        let mut sentiment_trend = Vec::new();
        for interaction in &interactions {
            if let Ok(parsed) =
                serde_json::from_str::<ClientInteractionEpisode>(&interaction.content.to_string())
            {
                sentiment_trend.push(parsed.sentiment);
            }
        }

        if !sentiment_trend.is_empty() {
            let avg_sentiment = sentiment_trend.iter().sum::<f64>() / sentiment_trend.len() as f64;
            patterns.push(InteractionPattern {
                pattern_name: "Sentiment Trend".to_string(),
                frequency: avg_sentiment,
                impact_score: if avg_sentiment > 0.7 { 0.9 } else { 0.5 },
            });
        }

        Ok(patterns)
    }

    async fn get_client_outcome_history(&self, _client_id: &str) -> Result<Vec<OutcomeRecord>> {
        let outcomes = self
            .memory_system
            .episodic
            .get_episodes_in_period(
                chrono::Utc::now() - chrono::Duration::days(365),
                chrono::Utc::now(),
            )
            .await
            .unwrap_or_default();

        let mut records = Vec::new();
        for outcome in outcomes {
            if let Ok(parsed) =
                serde_json::from_str::<RecommendationOutcome>(&outcome.content.to_string())
            {
                records.push(OutcomeRecord {
                    return_value: parsed.actual_return,
                    satisfaction: parsed.client_satisfaction.unwrap_or(0.5),
                    followed_advice: parsed.followed_recommendation,
                    timestamp: chrono::Utc::now(), // Use current time since MemoryDocument doesn't have timestamp field
                });
            }
        }

        Ok(records)
    }

    async fn generate_personalization_insights(
        &self,
        behavior_model: &ClientBehaviorModel,
        interaction_patterns: &[InteractionPattern],
        outcome_history: &[OutcomeRecord],
    ) -> Result<PersonalizationInsights> {
        let behavioral_score = self
            .calculate_behavioral_score(behavior_model, interaction_patterns)
            .await;

        let recommended_approach = RecommendationApproach {
            presentation_style: match behavior_model
                .communication_preferences
                .preferred_language_style
            {
                LanguageStyle::Professional => "formal_professional".to_string(),
                LanguageStyle::Conversational => "warm_conversational".to_string(),
                LanguageStyle::Educational => "informative_educational".to_string(),
                LanguageStyle::Direct => "concise_direct".to_string(),
            },
            focus_areas: behavior_model
                .communication_preferences
                .emphasis_areas
                .iter()
                .map(|area| format!("{area:?}").to_lowercase())
                .collect(),
            evidence_level: match behavior_model
                .communication_preferences
                .preferred_detail_level
            {
                DetailLevel::Brief => "summary".to_string(),
                DetailLevel::Moderate => "balanced".to_string(),
                DetailLevel::Comprehensive => "detailed".to_string(),
                DetailLevel::Technical => "technical".to_string(),
            },
            risk_framing: match behavior_model.risk_evolution.current_tolerance {
                RiskTolerance::Conservative => "safety_focused".to_string(),
                RiskTolerance::Moderate => "balanced_perspective".to_string(),
                RiskTolerance::Aggressive => "opportunity_focused".to_string(),
            },
        };

        let confidence_adjustments = self
            .calculate_confidence_adjustments(behavior_model, outcome_history)
            .await;

        let communication_strategy = self.develop_communication_strategy(behavior_model).await;

        Ok(PersonalizationInsights {
            client_id: behavior_model.client_id.clone(),
            behavioral_score,
            recommended_approach,
            confidence_adjustments,
            communication_strategy,
            risk_considerations: vec![
                format!(
                    "Client shows {} risk tolerance",
                    format!("{:?}", behavior_model.risk_evolution.current_tolerance).to_lowercase()
                ),
                format!(
                    "Loss sensitivity: {:.1}",
                    behavior_model.outcome_sensitivity.loss_sensitivity
                ),
                format!(
                    "Decision pattern: follows recommendations {:.1}% of the time",
                    behavior_model
                        .decision_patterns
                        .iter()
                        .find(|p| matches!(
                            p.pattern_type,
                            DecisionPatternType::FollowsRecommendations
                        ))
                        .map(|p| p.frequency * 100.0)
                        .unwrap_or(50.0)
                ),
            ],
        })
    }

    async fn adapt_recommendation_to_client(
        &self,
        base_recommendation: &DetailedRecommendation,
        behavior_model: &ClientBehaviorModel,
        insights: &PersonalizationInsights,
    ) -> Result<PersonalizedRecommendation> {
        // Generate personalized reasoning using AI if available
        let personalized_reasoning = if let Some(ref client) = self.rig_client {
            let prompt =
                self.build_personalization_prompt(base_recommendation, behavior_model, insights);

            let agent = client
                .agent("gpt-3.5-turbo")
                .preamble(&format!(
                    "You are a financial advisor who knows this client very well. Adapt your communication to their {} style and {} detail preference. Be personal and considerate of their {} risk tolerance.",
                    format!("{:?}", behavior_model.communication_preferences.preferred_language_style).to_lowercase(),
                    format!("{:?}", behavior_model.communication_preferences.preferred_detail_level).to_lowercase(),
                    format!("{:?}", behavior_model.risk_evolution.current_tolerance).to_lowercase()
                ))
                .max_tokens(500)
                .temperature(0.4)
                .build();

            match agent.prompt(&prompt).await {
                Ok(response) => response.trim().to_string(),
                Err(_) => self
                    .generate_fallback_personalized_reasoning(base_recommendation, behavior_model),
            }
        } else {
            self.generate_fallback_personalized_reasoning(base_recommendation, behavior_model)
        };

        // Calculate confidence adjustment based on client behavior
        let confidence_adjustment = base_recommendation.confidence
            + insights
                .confidence_adjustments
                .iter()
                .map(|adj| adj.adjustment)
                .sum::<f64>()
                .clamp(-0.3, 0.3);

        // Extract client-specific factors
        let client_specific_factors = vec![
            format!(
                "Behavioral score: {:.1}/10",
                insights.behavioral_score * 10.0
            ),
            format!(
                "Communication style: {:?}",
                behavior_model
                    .communication_preferences
                    .preferred_language_style
            ),
            format!(
                "Detail preference: {:?}",
                behavior_model
                    .communication_preferences
                    .preferred_detail_level
            ),
            format!(
                "Risk evolution: {:?} → {:?}",
                behavior_model.risk_evolution.initial_tolerance,
                behavior_model.risk_evolution.current_tolerance
            ),
            format!(
                "Decision pattern: {:.1}% follow rate",
                behavior_model
                    .decision_patterns
                    .iter()
                    .find(|p| matches!(p.pattern_type, DecisionPatternType::FollowsRecommendations))
                    .map(|p| p.frequency * 100.0)
                    .unwrap_or(50.0)
            ),
        ];

        Ok(PersonalizedRecommendation {
            base_recommendation: base_recommendation.base_recommendation,
            personalized_reasoning,
            confidence_adjustment,
            client_specific_factors,
            presentation_style: insights.recommended_approach.presentation_style.clone(),
            follow_up_actions: self
                .generate_personalized_follow_up_actions(behavior_model)
                .await,
        })
    }

    // Helper methods

    async fn calculate_success_rate_for_pattern(
        &self,
        outcomes: &[prollytree::agent::MemoryDocument],
        followed: bool,
    ) -> f64 {
        let relevant_outcomes: Vec<_> = outcomes
            .iter()
            .filter(|outcome| {
                if let Ok(parsed) =
                    serde_json::from_str::<serde_json::Value>(&outcome.content.to_string())
                {
                    parsed
                        .get("followed_recommendation")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                        == followed
                } else {
                    false
                }
            })
            .collect();

        if relevant_outcomes.is_empty() {
            return 0.5;
        }

        let successful_outcomes = relevant_outcomes
            .iter()
            .filter(|outcome| {
                if let Ok(parsed) =
                    serde_json::from_str::<serde_json::Value>(&outcome.content.to_string())
                {
                    parsed
                        .get("actual_return")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0)
                        > 0.0
                } else {
                    false
                }
            })
            .count();

        successful_outcomes as f64 / relevant_outcomes.len() as f64
    }

    async fn calculate_average_response_time(
        &self,
        _interactions: &[prollytree::agent::MemoryDocument],
    ) -> Duration {
        // Simplified calculation - in reality would track actual response times
        Duration::days(1) // Default to 1 day
    }

    async fn calculate_behavioral_score(
        &self,
        behavior_model: &ClientBehaviorModel,
        _interaction_patterns: &[InteractionPattern],
    ) -> f64 {
        let mut score = 0.5; // Base score

        // Adjust based on decision patterns
        for pattern in &behavior_model.decision_patterns {
            match pattern.pattern_type {
                DecisionPatternType::FollowsRecommendations => {
                    score += pattern.frequency * 0.2;
                }
                DecisionPatternType::ResearchFocused => {
                    score += pattern.frequency * 0.1;
                }
                DecisionPatternType::ImpulsiveDecisions => {
                    score -= pattern.frequency * 0.1;
                }
                _ => {}
            }
        }

        // Adjust based on interaction patterns
        for pattern in _interaction_patterns {
            if pattern.pattern_name == "High Engagement" {
                score += 0.1;
            }
        }

        score.clamp(0.0, 1.0)
    }

    async fn calculate_confidence_adjustments(
        &self,
        behavior_model: &ClientBehaviorModel,
        _outcome_history: &[OutcomeRecord],
    ) -> Vec<ConfidenceAdjustment> {
        let mut adjustments = Vec::new();

        // Adjustment based on follow rate
        if let Some(follow_pattern) = behavior_model
            .decision_patterns
            .iter()
            .find(|p| matches!(p.pattern_type, DecisionPatternType::FollowsRecommendations))
        {
            adjustments.push(ConfidenceAdjustment {
                factor: "Recommendation Follow Rate".to_string(),
                adjustment: (follow_pattern.frequency - 0.5) * 0.2,
                reasoning: format!(
                    "Client follows recommendations {:.1}% of the time",
                    follow_pattern.frequency * 100.0
                ),
            });
        }

        // Adjustment based on outcome history
        if !_outcome_history.is_empty() {
            let avg_satisfaction = _outcome_history.iter().map(|r| r.satisfaction).sum::<f64>()
                / _outcome_history.len() as f64;

            adjustments.push(ConfidenceAdjustment {
                factor: "Historical Satisfaction".to_string(),
                adjustment: (avg_satisfaction - 0.5) * 0.15,
                reasoning: format!(
                    "Average client satisfaction: {:.1}/10",
                    avg_satisfaction * 10.0
                ),
            });
        }

        adjustments
    }

    async fn develop_communication_strategy(
        &self,
        _behavior_model: &ClientBehaviorModel,
    ) -> CommunicationStrategy {
        let opening_approach = match _behavior_model.communication_preferences.preferred_language_style {
            LanguageStyle::Professional => "Good day. I've completed a comprehensive analysis for your consideration.",
            LanguageStyle::Conversational => "Hi! I've put together some thoughts on your investment that I think you'll find interesting.",
            LanguageStyle::Educational => "Let me walk you through the analysis and explain what we're seeing in the market.",
            LanguageStyle::Direct => "Here's my recommendation based on current market conditions and your profile.",
        }.to_string();

        let key_messages = _behavior_model
            .communication_preferences
            .emphasis_areas
            .iter()
            .map(|area| match area {
                EmphasisArea::RiskManagement => {
                    "This recommendation aligns with your risk management objectives".to_string()
                }
                EmphasisArea::GrowthPotential => {
                    "The growth potential here fits your investment timeline".to_string()
                }
                EmphasisArea::IncomeGeneration => {
                    "This supports your income generation goals".to_string()
                }
                EmphasisArea::TaxEfficiency => {
                    "We've considered the tax implications for your situation".to_string()
                }
                EmphasisArea::LiquidityNeeds => {
                    "This maintains appropriate liquidity for your needs".to_string()
                }
                EmphasisArea::TimeHorizon => {
                    "The timing aligns well with your investment horizon".to_string()
                }
            })
            .collect();

        CommunicationStrategy {
            opening_approach,
            key_messages,
            follow_up_strategy: "I'll check in with you in a few days to see how you're feeling about this recommendation.".to_string(),
        }
    }

    fn build_personalization_prompt(
        &self,
        base_recommendation: &DetailedRecommendation,
        behavior_model: &ClientBehaviorModel,
        insights: &PersonalizationInsights,
    ) -> String {
        format!(
            r#"Personalize this investment recommendation for a specific client:

RECOMMENDATION: {:?} with {:.1}% confidence
REASONING: {}

CLIENT BEHAVIORAL PROFILE:
- Communication Style: {:?}
- Detail Preference: {:?}
- Risk Tolerance: {:?}
- Decision Pattern: Follows advice {:.1}% of time
- Loss Sensitivity: {:.1}/10

PERSONALIZATION GUIDANCE:
- Presentation Style: {}
- Focus Areas: {}
- Risk Framing: {}

Rewrite the recommendation in a way that:
1. Matches their communication style and detail preference
2. Addresses their specific risk tolerance and behavioral patterns
3. Uses the recommended presentation style
4. Feels personal and tailored to their unique situation

Keep the core recommendation the same but make the explanation feel like it was written specifically for this client."#,
            base_recommendation.base_recommendation,
            base_recommendation.confidence * 100.0,
            base_recommendation.reasoning,
            behavior_model
                .communication_preferences
                .preferred_language_style,
            behavior_model
                .communication_preferences
                .preferred_detail_level,
            behavior_model.risk_evolution.current_tolerance,
            behavior_model
                .decision_patterns
                .iter()
                .find(|p| matches!(p.pattern_type, DecisionPatternType::FollowsRecommendations))
                .map(|p| p.frequency * 100.0)
                .unwrap_or(50.0),
            behavior_model.outcome_sensitivity.loss_sensitivity * 10.0,
            insights.recommended_approach.presentation_style,
            insights.recommended_approach.focus_areas.join(", "),
            insights.recommended_approach.risk_framing
        )
    }

    fn generate_fallback_personalized_reasoning(
        &self,
        base_recommendation: &DetailedRecommendation,
        behavior_model: &ClientBehaviorModel,
    ) -> String {
        match behavior_model.communication_preferences.preferred_language_style {
            LanguageStyle::Conversational => format!(
                "I've been thinking about your situation, and I believe this {} recommendation really makes sense for you. Given your {} risk tolerance and the way you like to approach investments, this feels like a natural fit. {}",
                format!("{:?}", base_recommendation.base_recommendation).to_lowercase(),
                format!("{:?}", behavior_model.risk_evolution.current_tolerance).to_lowercase(),
                if base_recommendation.confidence > 0.8 { "I'm quite confident this aligns well with your goals." } else { "While there's always some uncertainty, I think this is a solid choice for your situation." }
            ),
            LanguageStyle::Professional => format!(
                "Based on our analysis and your established investment profile, I recommend a {} position. This recommendation takes into account your {} risk tolerance and aligns with your stated investment objectives. The confidence level of {:.1}% reflects the strength of the underlying analysis.",
                format!("{:?}", base_recommendation.base_recommendation).to_lowercase(),
                format!("{:?}", behavior_model.risk_evolution.current_tolerance).to_lowercase(),
                base_recommendation.confidence * 100.0
            ),
            LanguageStyle::Direct => format!(
                "{} recommendation. Risk level appropriate for {} tolerance. Confidence: {:.1}%. Rationale: {}",
                format!("{:?}", base_recommendation.base_recommendation).to_uppercase(),
                format!("{:?}", behavior_model.risk_evolution.current_tolerance).to_lowercase(),
                base_recommendation.confidence * 100.0,
                base_recommendation.reasoning.split('.').next().unwrap_or("")
            ),
            LanguageStyle::Educational => format!(
                "Let me explain why I'm suggesting a {} approach for this investment. Given your {} risk tolerance, this recommendation fits well within your comfort zone. Here's how I arrived at this conclusion: {}. The {:.1}% confidence level reflects the quality of available data and market conditions.",
                format!("{:?}", base_recommendation.base_recommendation).to_lowercase(),
                format!("{:?}", behavior_model.risk_evolution.current_tolerance).to_lowercase(),
                base_recommendation.reasoning,
                base_recommendation.confidence * 100.0
            ),
        }
    }

    async fn generate_personalized_follow_up_actions(
        &self,
        _behavior_model: &ClientBehaviorModel,
    ) -> Vec<String> {
        let mut actions = Vec::new();

        // Base follow-up actions
        actions.push("Schedule follow-up discussion in 30 days".to_string());

        // Personalized based on communication preferences
        match _behavior_model.communication_preferences.response_timing {
            ResponseTiming::Immediate => {
                actions.push("Provide quick market updates if conditions change".to_string());
            }
            ResponseTiming::ConsiderativeTime => {
                actions.push("Send detailed market analysis next week".to_string());
            }
            ResponseTiming::ResearchPeriod => {
                actions.push("Share additional research materials for review".to_string());
            }
        }

        // Based on decision patterns
        for pattern in &_behavior_model.decision_patterns {
            match pattern.pattern_type {
                DecisionPatternType::ResearchFocused => {
                    actions.push("Prepare additional research documentation".to_string());
                }
                DecisionPatternType::SeeksAdditionalOpinions => {
                    actions.push("Offer to discuss alternative viewpoints".to_string());
                }
                DecisionPatternType::DelaysDecisions => {
                    actions.push("Set gentle reminder for decision timeline".to_string());
                }
                _ => {}
            }
        }

        // Based on risk tolerance
        match _behavior_model.risk_evolution.current_tolerance {
            RiskTolerance::Conservative => {
                actions.push("Monitor for any risk level changes".to_string());
            }
            RiskTolerance::Aggressive => {
                actions.push("Watch for additional growth opportunities".to_string());
            }
            _ => {}
        }

        actions
    }

    async fn store_personalization_decision(
        &self,
        client_id: &str,
        recommendation_id: &str,
        insights: &PersonalizationInsights,
    ) -> Result<()> {
        let _decision_record = serde_json::json!({
            "client_id": client_id,
            "recommendation_id": recommendation_id,
            "personalization_insights": insights,
            "timestamp": Utc::now()
        });

        // Note: In a full implementation, personalization decisions would be stored
        // self.memory_system.episodic.store_episode(...).await?;

        Ok(())
    }
}

// Supporting types
#[derive(Debug)]
pub struct InteractionPattern {
    pub pattern_name: String,
    pub frequency: f64,
    pub impact_score: f64,
}

#[derive(Debug)]
pub struct OutcomeRecord {
    pub return_value: f64,
    pub satisfaction: f64,
    pub followed_advice: bool,
    pub timestamp: DateTime<Utc>,
}
