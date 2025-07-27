#![allow(dead_code)]

use anyhow::Result;
use colored::Colorize;
use rig::{completion::Prompt, providers::openai::Client};
use serde::{Deserialize, Serialize};

use super::{AnalysisMode, ClientProfile, RecommendationType};

/// Financial advisor agent powered by Rig framework
pub struct FinancialAnalysisAgent {
    client: Client,
    verbose: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisRequest {
    pub symbol: String,
    pub price: f64,
    pub pe_ratio: f64,
    pub volume: u64,
    pub sector: String,
    pub recommendation_type: RecommendationType,
    pub client_profile: ClientProfile,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResponse {
    pub reasoning: String,
    pub analysis_mode: AnalysisMode,
}

impl FinancialAnalysisAgent {
    /// Create a new financial analysis agent with OpenAI using Rig
    pub fn new_openai(api_key: &str, verbose: bool) -> Result<Self> {
        let client = Client::new(api_key);

        Ok(Self { client, verbose })
    }

    /// Generate AI-powered financial analysis using Rig framework
    pub async fn generate_analysis(
        &self,
        request: &AnalysisRequest,
        debug_mode: bool,
    ) -> Result<AnalysisResponse> {
        let prompt = self.build_analysis_prompt(request);

        if debug_mode {
            println!();
            println!("{}", "🔍 Rig Agent Prompt Debug".bright_cyan().bold());
            println!("{}", "━".repeat(60).dimmed());
            println!("{prompt}");
            println!("{}", "━".repeat(60).dimmed());
            println!();
        }

        if self.verbose {
            println!("🧠 Generating AI-powered analysis with Rig...");
        }

        // Create Rig agent with proper system prompt
        let agent = self
            .client
            .agent("gpt-3.5-turbo")
            .preamble(
                r#"You are a professional financial advisor providing investment recommendations.
               
You will receive detailed stock analysis data and client profile information.
Your task is to provide a professional, concise investment analysis (2-3 sentences)
explaining why the given recommendation makes sense for the specific client profile.

Focus on:
1. Key financial metrics and their implications
2. Alignment with client's risk tolerance and goals 
3. Sector trends or company-specific factors

Keep the response professional, factual, and tailored to the client's profile.
Respond with only the analysis text, no additional formatting or preamble."#,
            )
            .max_tokens(200)
            .temperature(0.3)
            .build();

        // Use Rig's agent to get completion
        match agent.prompt(&prompt).await {
            Ok(response) => Ok(AnalysisResponse {
                reasoning: response.trim().to_string(),
                analysis_mode: AnalysisMode::AIPowered,
            }),
            Err(e) => {
                if self.verbose {
                    println!("⚠️ Rig AI analysis failed: {e}, falling back to rule-based");
                }
                // Fallback to rule-based analysis
                Ok(AnalysisResponse {
                    reasoning: self.generate_fallback_reasoning(request),
                    analysis_mode: AnalysisMode::RuleBased,
                })
            }
        }
    }

    fn build_analysis_prompt(&self, request: &AnalysisRequest) -> String {
        format!(
            r#"STOCK ANALYSIS:
Symbol: {}
Current Price: ${}
P/E Ratio: {}
Volume: {}
Sector: {}

CLIENT PROFILE:
Risk Tolerance: {:?}
Investment Goals: {}
Time Horizon: {}
Restrictions: {}

RECOMMENDATION: {:?}

Provide your professional analysis:"#,
            request.symbol,
            request.price,
            request.pe_ratio,
            request.volume,
            request.sector,
            request.client_profile.risk_tolerance,
            request.client_profile.investment_goals.join(", "),
            request.client_profile.time_horizon,
            request.client_profile.restrictions.join(", "),
            request.recommendation_type
        )
    }

    fn generate_fallback_reasoning(&self, request: &AnalysisRequest) -> String {
        match request.recommendation_type {
            RecommendationType::Buy => {
                format!(
                    "{} shows strong fundamentals with a P/E ratio of {:.1}, trading at ${:.2}. \
                    Given your {:?} risk tolerance and {} investment horizon, this {} sector position \
                    aligns well with your portfolio diversification goals.",
                    request.symbol,
                    request.pe_ratio,
                    request.price,
                    request.client_profile.risk_tolerance,
                    request.client_profile.time_horizon,
                    request.sector
                )
            }
            RecommendationType::Hold => {
                format!(
                    "{} is currently fairly valued at ${:.2} with stable fundamentals. \
                    This maintains your existing exposure while we monitor for better entry/exit opportunities \
                    that match your {:?} risk profile.",
                    request.symbol,
                    request.price,
                    request.client_profile.risk_tolerance
                )
            }
            RecommendationType::Sell => {
                format!(
                    "{} appears overvalued at current levels of ${:.2} with elevated P/E of {:.1}. \
                    Given your {:?} risk tolerance, taking profits aligns with prudent portfolio management \
                    and your {} investment timeline.",
                    request.symbol,
                    request.price,
                    request.pe_ratio,
                    request.client_profile.risk_tolerance,
                    request.client_profile.time_horizon
                )
            }
            RecommendationType::Rebalance => {
                format!(
                    "Portfolio rebalancing for {} recommended to maintain target allocation. \
                    Current {} sector weighting may need adjustment to align with your {:?} risk profile \
                    and {} investment horizon.",
                    request.symbol,
                    request.sector,
                    request.client_profile.risk_tolerance,
                    request.client_profile.time_horizon
                )
            }
        }
    }
}
