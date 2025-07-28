use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::advisor::{RecommendationType, RiskTolerance};

/// Enhanced semantic memory structures for financial entities

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEntity {
    pub client_id: String,
    pub risk_tolerance: RiskTolerance,
    pub investment_goals: Vec<String>,
    pub time_horizon: String,
    pub portfolio_value: f64,
    pub restrictions: Vec<String>,
    pub preferences: HashMap<String, serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEntity {
    pub symbol: String,
    pub sector: String,
    pub market_cap: u64,
    pub fundamentals: MarketFundamentals,
    pub analyst_ratings: Vec<AnalystRating>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketFundamentals {
    pub pe_ratio: f64,
    pub price: f64,
    pub volume: u64,
    pub market_cap: u64,
    pub dividend_yield: Option<f64>,
    pub revenue_growth: Option<f64>,
    pub earnings_growth: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub return_on_equity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalystRating {
    pub analyst: String,
    pub rating: String, // Buy, Hold, Sell
    pub target_price: Option<f64>,
    pub confidence: f64,
    pub date: DateTime<Utc>,
}

/// Enhanced episodic memory structures for financial experiences

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationEpisode {
    pub recommendation_id: String,
    pub client_id: String,
    pub symbol: String,
    pub action: RecommendationType,
    pub reasoning: String,
    pub confidence: f64,
    pub market_conditions: MarketSnapshot,
    pub outcome: Option<RecommendationOutcome>,
    pub timestamp: DateTime<Utc>,
    pub workflow_steps: Vec<WorkflowStepResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub market_trend: String,
    pub volatility_index: f64,
    pub interest_rates: f64,
    pub major_indices: HashMap<String, f64>,
    pub sector_performance: HashMap<String, f64>,
    pub economic_indicators: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationOutcome {
    pub actual_return: f64,
    pub time_to_outcome: chrono::Duration,
    pub client_satisfaction: Option<f64>,
    pub followed_recommendation: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepResult {
    pub step_name: String,
    pub execution_time: chrono::Duration,
    pub success: bool,
    pub key_findings: Vec<String>,
    pub confidence_impact: f64,
    pub memory_references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInteractionEpisode {
    pub interaction_id: String,
    pub client_id: String,
    pub interaction_type: InteractionType,
    pub summary: String,
    pub sentiment: f64,
    pub key_topics: Vec<String>,
    pub decisions_made: Vec<String>,
    pub follow_up_required: bool,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InteractionType {
    InitialConsultation,
    PortfolioReview,
    RecommendationDiscussion,
    RiskAssessment,
    ComplianceUpdate,
    EmergencyConsultation,
}

/// Enhanced procedural memory structures for financial workflows

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisWorkflow {
    pub name: String,
    pub description: String,
    pub steps: Vec<AnalysisStep>,
    pub success_rate: f64,
    pub applicable_conditions: Vec<String>,
    pub required_data: Vec<String>,
    pub expected_duration: chrono::Duration,
    pub created_at: DateTime<Utc>,
    pub last_optimized: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisStep {
    GatherMarketData {
        sources: Vec<String>,
        required_fields: Vec<String>,
        timeout_seconds: u64,
    },
    AnalyzeRisk {
        metrics: Vec<String>,
        thresholds: HashMap<String, f64>,
        weight_factors: HashMap<String, f64>,
    },
    CheckCompliance {
        rules: Vec<String>,
        severity_levels: Vec<String>,
        automated_actions: Vec<String>,
    },
    GenerateRecommendation {
        factors: Vec<String>,
        weight_matrix: HashMap<String, f64>,
        confidence_thresholds: HashMap<String, f64>,
    },
    PersonalizeOutput {
        client_factors: Vec<String>,
        adaptation_rules: Vec<String>,
        presentation_style: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceRule {
    pub rule_id: String,
    pub name: String,
    pub description: String,
    pub rule_type: ComplianceRuleType,
    pub severity: ComplianceSeverity,
    pub conditions: Vec<String>,
    pub automated_action: Option<String>,
    pub applicable_clients: Option<Vec<String>>,
    pub effective_date: DateTime<Utc>,
    pub expiry_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComplianceRuleType {
    PositionLimit,
    RiskConstraint,
    SuitabilityCheck,
    ConflictOfInterest,
    ReportingRequirement,
    KnowYourCustomer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComplianceSeverity {
    Info,
    Warning,
    Critical,
    Blocking,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessmentProcedure {
    pub procedure_id: String,
    pub name: String,
    pub risk_categories: Vec<RiskCategory>,
    pub calculation_method: String,
    pub weight_factors: HashMap<String, f64>,
    pub threshold_levels: HashMap<String, f64>,
    pub mitigation_strategies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum RiskCategory {
    MarketRisk,
    CreditRisk,
    LiquidityRisk,
    OperationalRisk,
    ConcentrationRisk,
    CurrencyRisk,
    InterestRateRisk,
}

/// Workflow execution context and results

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisContext {
    pub analysis_id: String,
    pub client_id: String,
    pub symbol: String,
    pub request_type: String,
    pub market_conditions: MarketSnapshot,
    pub client_profile: ClientEntity,
    pub started_at: DateTime<Utc>,
    pub parameters: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedRecommendation {
    pub recommendation_id: String,
    pub base_recommendation: RecommendationType,
    pub confidence: f64,
    pub reasoning: String,
    pub personalized_reasoning: String,
    pub risk_assessment: RiskAssessmentResult,
    pub compliance_validation: ComplianceValidation,
    pub market_analysis: MarketAnalysisResult,
    pub execution_metadata: ExecutionMetadata,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessmentResult {
    pub overall_risk_score: f64,
    pub risk_breakdown: HashMap<RiskCategory, f64>,
    pub risk_factors: Vec<String>,
    pub mitigation_recommendations: Vec<String>,
    pub client_risk_alignment: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceValidation {
    pub passed: bool,
    pub violations: Vec<ComplianceViolation>,
    pub warnings: Vec<ComplianceWarning>,
    pub required_disclosures: Vec<String>,
    pub automated_actions_taken: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    pub rule_id: String,
    pub severity: ComplianceSeverity,
    pub description: String,
    pub recommended_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceWarning {
    pub rule_id: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketAnalysisResult {
    pub fundamental_analysis: FundamentalAnalysis,
    pub technical_analysis: TechnicalAnalysis,
    pub sector_analysis: SectorAnalysis,
    pub sentiment_analysis: SentimentAnalysis,
    pub ai_insights: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundamentalAnalysis {
    pub valuation_metrics: HashMap<String, f64>,
    pub growth_prospects: String,
    pub competitive_position: String,
    pub financial_health: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalAnalysis {
    pub trend_direction: String,
    pub support_levels: Vec<f64>,
    pub resistance_levels: Vec<f64>,
    pub momentum_indicators: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorAnalysis {
    pub sector_trend: String,
    pub relative_performance: f64,
    pub sector_rotation_outlook: String,
    pub key_sector_drivers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentAnalysis {
    pub analyst_sentiment: f64,
    pub market_sentiment: f64,
    pub news_sentiment: f64,
    pub sentiment_drivers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    pub workflow_used: String,
    pub total_execution_time: chrono::Duration,
    pub step_timings: HashMap<String, chrono::Duration>,
    pub memory_queries_performed: u32,
    pub ai_api_calls: u32,
    pub data_sources_consulted: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalizedRecommendation {
    pub base_recommendation: RecommendationType,
    pub personalized_reasoning: String,
    pub confidence_adjustment: f64,
    pub client_specific_factors: Vec<String>,
    pub presentation_style: String,
    pub follow_up_actions: Vec<String>,
}

impl Default for MarketSnapshot {
    fn default() -> Self {
        Self {
            market_trend: "Neutral".to_string(),
            volatility_index: 20.0,
            interest_rates: 5.0,
            major_indices: HashMap::new(),
            sector_performance: HashMap::new(),
            economic_indicators: HashMap::new(),
        }
    }
}

impl ClientEntity {
    pub fn new(client_id: String, risk_tolerance: RiskTolerance) -> Self {
        let now = Utc::now();
        Self {
            client_id,
            risk_tolerance,
            investment_goals: Vec::new(),
            time_horizon: "Long-term".to_string(),
            portfolio_value: 0.0,
            restrictions: Vec::new(),
            preferences: HashMap::new(),
            created_at: now,
            last_updated: now,
        }
    }
}

impl MarketEntity {
    pub fn new(symbol: String, sector: String) -> Self {
        Self {
            symbol,
            sector,
            market_cap: 0,
            fundamentals: MarketFundamentals::default(),
            analyst_ratings: Vec::new(),
            last_updated: Utc::now(),
        }
    }
}

impl Default for MarketFundamentals {
    fn default() -> Self {
        Self {
            pe_ratio: 0.0,
            price: 0.0,
            volume: 0,
            market_cap: 0,
            dividend_yield: None,
            revenue_growth: None,
            earnings_growth: None,
            debt_to_equity: None,
            return_on_equity: None,
        }
    }
}