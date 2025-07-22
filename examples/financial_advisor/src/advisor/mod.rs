#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
// OpenAI integration for AI-powered recommendations
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::memory::{MemoryStore, MemoryType, ValidatedMemory};
use crate::security::SecurityMonitor;
use crate::validation::{MemoryValidator, ValidationResult};

pub mod compliance;
pub mod interactive;
pub mod recommendations;

use interactive::InteractiveSession;
use recommendations::RecommendationEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationType {
    Buy,
    Sell,
    Hold,
    Rebalance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientProfile {
    pub id: String,
    pub risk_tolerance: RiskTolerance,
    pub investment_goals: Vec<String>,
    pub time_horizon: String,
    pub restrictions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskTolerance {
    Conservative,
    Moderate,
    Aggressive,
}

#[derive(Debug, Clone)]
struct StockData {
    price: f64,
    volume: u64,
    pe_ratio: f64,
    market_cap: u64,
    sector: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub id: String,
    pub client_id: String,
    pub symbol: String,
    pub recommendation_type: RecommendationType,
    pub reasoning: String,
    pub confidence: f64,
    pub timestamp: DateTime<Utc>,
    pub validation_result: ValidationResult,
    pub memory_version: String,
}

pub struct FinancialAdvisor {
    memory_store: MemoryStore,
    validator: MemoryValidator,
    security_monitor: SecurityMonitor,
    recommendation_engine: RecommendationEngine,
    openai_client: reqwest::Client,
    api_key: String,
    verbose: bool,
    current_session: String,
    session_recommendations: Vec<Recommendation>, // Keep recommendations in memory for the session
}

impl FinancialAdvisor {
    pub async fn new(storage_path: &str, api_key: &str) -> Result<Self> {
        let memory_store = MemoryStore::new(storage_path).await?;
        let validator = MemoryValidator::default();
        let security_monitor = SecurityMonitor::new();
        let recommendation_engine = RecommendationEngine::new();

        // Initialize OpenAI client
        let openai_client = reqwest::Client::new();

        Ok(Self {
            memory_store,
            validator,
            security_monitor,
            recommendation_engine,
            openai_client,
            api_key: api_key.to_string(),
            verbose: false,
            current_session: Uuid::new_v4().to_string(),
            session_recommendations: Vec::new(),
        })
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    pub async fn get_recommendation(
        &mut self,
        symbol: &str,
        client_profile: &ClientProfile,
    ) -> Result<Recommendation> {
        if self.verbose {
            println!("🔍 Fetching market data for {symbol}...");
        }

        // Step 1: Validate and store market data
        let market_data = self.fetch_and_validate_market_data(symbol).await?;

        // Step 2: Check for memory consistency
        self.ensure_memory_consistency(&market_data).await?;

        // Step 3: Security check for any anomalies
        self.security_monitor.check_for_anomalies(&market_data)?;

        // Step 4: Generate recommendation with full context
        let mut recommendation = self
            .recommendation_engine
            .generate(symbol, client_profile, &market_data, &self.memory_store)
            .await?;

        // Step 4.5: Enhance reasoning with AI analysis
        if self.verbose {
            println!("🧠 Generating AI-powered analysis...");
        }

        let ai_reasoning = self
            .generate_ai_reasoning(
                symbol,
                &recommendation.recommendation_type,
                &serde_json::from_str::<serde_json::Value>(&market_data.content)
                    .unwrap_or(serde_json::json!({})),
                client_profile,
            )
            .await?;

        // Combine traditional and AI reasoning
        recommendation.reasoning = format!(
            "{}\n\n🤖 AI Analysis: {}",
            recommendation.reasoning, ai_reasoning
        );

        // Step 5: Store recommendation with audit trail
        self.store_recommendation(&recommendation).await?;
        
        // Also keep it in memory for this session
        self.session_recommendations.push(recommendation.clone());

        Ok(recommendation)
    }

    async fn fetch_and_validate_market_data(&mut self, symbol: &str) -> Result<ValidatedMemory> {
        // Simulate fetching from multiple sources
        let sources = vec![
            ("bloomberg", self.fetch_bloomberg_data(symbol).await?),
            ("yahoo_finance", self.fetch_yahoo_data(symbol).await?),
            (
                "alpha_vantage",
                self.fetch_alpha_vantage_data(symbol).await?,
            ),
        ];

        // Cross-validate data
        let validation_result = self.validator.validate_multi_source(&sources)?;

        if !validation_result.is_valid {
            return Err(anyhow::anyhow!(
                "Market data validation failed: {:?}",
                validation_result.issues
            ));
        }

        // Create validated memory entry
        let validated_memory = ValidatedMemory {
            id: Uuid::new_v4().to_string(),
            content: serde_json::to_string(&sources)?,
            timestamp: Utc::now(),
            validation_hash: validation_result.hash,
            sources: sources.iter().map(|(name, _)| name.to_string()).collect(),
            confidence: validation_result.confidence,
            cross_references: validation_result.cross_references,
        };

        // Store in versioned memory
        self.memory_store
            .store(MemoryType::MarketData, &validated_memory)
            .await?;

        Ok(validated_memory)
    }

    async fn ensure_memory_consistency(&mut self, new_data: &ValidatedMemory) -> Result<()> {
        // Check for contradictions with existing memories
        let related_memories = self
            .memory_store
            .query_related(&new_data.content, 10)
            .await?;

        for memory in related_memories {
            if self.validator.has_contradiction(&memory, new_data)? {
                // Create branch for investigation
                let branch_id = self
                    .memory_store
                    .create_branch("contradiction_check")
                    .await?;

                if self.verbose {
                    println!("⚠️  Potential contradiction detected, created branch: {branch_id}");
                }

                // Analyze and resolve contradiction
                // In production, this might trigger human review
                return Err(anyhow::anyhow!("Memory consistency check failed"));
            }
        }

        Ok(())
    }

    async fn store_recommendation(&mut self, recommendation: &Recommendation) -> Result<()> {
        let memory = ValidatedMemory {
            id: recommendation.id.clone(),
            content: serde_json::to_string(recommendation)?,
            timestamp: recommendation.timestamp,
            validation_hash: self
                .validator
                .hash_content(&serde_json::to_string(recommendation)?),
            sources: vec!["recommendation_engine".to_string()],
            confidence: recommendation.confidence,
            cross_references: vec![],
        };

        // Store with full audit trail
        self.memory_store
            .store_with_audit(
                MemoryType::Recommendation,
                &memory,
                &format!(
                    "Generated {} recommendation for {}",
                    recommendation.recommendation_type.as_str(),
                    recommendation.symbol
                ),
            )
            .await?;

        // Commit to create immutable version
        let version = self
            .memory_store
            .commit(&format!(
                "Recommendation: {} {} for client {}",
                recommendation.recommendation_type.as_str(),
                recommendation.symbol,
                recommendation.client_id,
            ))
            .await?;

        if self.verbose {
            println!("✅ Recommendation stored at version: {version}");
        }

        Ok(())
    }

    pub async fn run_interactive_session(&mut self) -> Result<()> {
        let session = InteractiveSession::new(self);
        session.run().await
    }

    pub async fn generate_compliance_report(
        &self,
        from: Option<String>,
        to: Option<String>,
    ) -> Result<String> {
        compliance::generate_report(&self.memory_store, from, to).await
    }

    pub async fn get_recent_recommendations(&self, limit: usize) -> Result<Vec<Recommendation>> {
        // First try to get from session memory (faster and more reliable)
        if !self.session_recommendations.is_empty() {
            let mut recs = self.session_recommendations.clone();
            recs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)); // Most recent first
            recs.truncate(limit);
            return Ok(recs);
        }
        
        // Fallback to database query
        self.memory_store.get_recent_recommendations(limit).await
    }

    async fn generate_ai_reasoning(
        &self,
        symbol: &str,
        recommendation_type: &RecommendationType,
        market_data: &serde_json::Value,
        client: &ClientProfile,
    ) -> Result<String> {
        // Build context from market data
        let price = market_data["price"].as_f64().unwrap_or(0.0);
        let pe_ratio = market_data["pe_ratio"].as_f64().unwrap_or(0.0);
        let volume = market_data["volume"].as_u64().unwrap_or(0);
        let sector = market_data["sector"].as_str().unwrap_or("Unknown");

        let prompt = format!(
            r#"You are a professional financial advisor providing investment recommendations.

            STOCK ANALYSIS:
            Symbol: {symbol}
            Current Price: ${price}
            P/E Ratio: {pe_ratio}
            Volume: {volume}
            Sector: {sector}
            
            CLIENT PROFILE:
            Risk Tolerance: {:?}
            Investment Goals: {}
            Time Horizon: {}
            Restrictions: {}

            RECOMMENDATION: {recommendation_type:?}

            Please provide a professional, concise investment analysis (2-3 sentences) explaining why this recommendation makes sense for this specific client profile. Focus on:
            1. Key financial metrics and their implications
            2. Alignment with client's risk tolerance and goals
            3. Sector trends or company-specific factors

            Keep the response professional, factual, and tailored to the client's profile."#,
            client.risk_tolerance,
            client.investment_goals.join(", "),
            client.time_horizon,
            client.restrictions.join(", "),
            symbol = symbol,
            price = price,
            pe_ratio = pe_ratio,
            volume = volume,
            sector = sector,
            recommendation_type = recommendation_type
        );

        // Make OpenAI API call
        let openai_request = serde_json::json!({
            "model": "gpt-3.5-turbo",
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_tokens": 200,
            "temperature": 0.3
        });

        let response = self
            .openai_client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&openai_request)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let openai_response: serde_json::Value = resp.json().await.unwrap_or_default();
                let content = openai_response
                    .get("choices")
                    .and_then(|choices| choices.get(0))
                    .and_then(|choice| choice.get("message"))
                    .and_then(|message| message.get("content"))
                    .and_then(|content| content.as_str())
                    .unwrap_or("AI analysis unavailable at this time.");

                Ok(content.to_string())
            }
            _ => {
                // Fallback to rule-based reasoning if OpenAI fails
                Ok(self.generate_fallback_reasoning(
                    symbol,
                    recommendation_type,
                    market_data,
                    client,
                ))
            }
        }
    }

    fn generate_fallback_reasoning(
        &self,
        symbol: &str,
        recommendation_type: &RecommendationType,
        market_data: &serde_json::Value,
        client: &ClientProfile,
    ) -> String {
        let price = market_data["price"].as_f64().unwrap_or(0.0);
        let pe_ratio = market_data["pe_ratio"].as_f64().unwrap_or(0.0);
        let sector = market_data["sector"].as_str().unwrap_or("Unknown");

        match recommendation_type {
            RecommendationType::Buy => {
                format!(
                    "{} shows strong fundamentals with a P/E ratio of {:.1}, trading at ${:.2}. \
                    Given your {:?} risk tolerance and {} investment horizon, this {} sector position \
                    aligns well with your portfolio diversification goals.",
                    symbol, pe_ratio, price, client.risk_tolerance, client.time_horizon, sector
                )
            }
            RecommendationType::Hold => {
                format!(
                    "{} is currently fairly valued at ${:.2} with stable fundamentals. \
                    This maintains your existing exposure while we monitor for better entry/exit opportunities \
                    that match your {:?} risk profile.",
                    symbol, price, client.risk_tolerance
                )
            }
            RecommendationType::Sell => {
                format!(
                    "{} appears overvalued at current levels of ${:.2} with elevated P/E of {:.1}. \
                    Given your {:?} risk tolerance, taking profits aligns with prudent portfolio management \
                    and your {} investment timeline.",
                    symbol, price, pe_ratio, client.risk_tolerance, client.time_horizon
                )
            }
            RecommendationType::Rebalance => {
                format!(
                    "Portfolio rebalancing for {} recommended to maintain target allocation. \
                    Current {} sector weighting may need adjustment to align with your {:?} risk profile \
                    and {} investment horizon.",
                    symbol, sector, client.risk_tolerance, client.time_horizon
                )
            }
        }
    }

    // Simulated data fetching methods with realistic stock data
    async fn fetch_bloomberg_data(&self, symbol: &str) -> Result<serde_json::Value> {
        // Simulate network latency for Bloomberg API (known to be fast)
        tokio::time::sleep(tokio::time::Duration::from_millis(
            50 + (symbol.len() % 3) as u64 * 10,
        ))
        .await;

        let stock_data = self.get_realistic_stock_data(symbol);

        // Add slight Bloomberg-specific variance (Bloomberg tends to be slightly lower)
        let price_variance = 0.98 + (symbol.len() % 5) as f64 * 0.01;
        let adjusted_price = stock_data.price * price_variance;

        Ok(serde_json::json!({
            "symbol": symbol,
            "price": (adjusted_price * 100.0).round() / 100.0,
            "volume": (stock_data.volume as f64 * 0.95) as u64,
            "pe_ratio": stock_data.pe_ratio * 0.98,
            "market_cap": stock_data.market_cap,
            "sector": stock_data.sector,
            "source": "bloomberg",
            "timestamp": Utc::now().to_rfc3339(),
        }))
    }

    async fn fetch_yahoo_data(&self, symbol: &str) -> Result<serde_json::Value> {
        // Simulate network latency for Yahoo Finance API (free tier, slower)
        tokio::time::sleep(tokio::time::Duration::from_millis(
            120 + (symbol.len() % 4) as u64 * 15,
        ))
        .await;

        let stock_data = self.get_realistic_stock_data(symbol);

        // Add slight Yahoo-specific variance (Yahoo tends to be slightly higher)
        let price_variance = 1.01 + (symbol.len() % 3) as f64 * 0.005;
        let adjusted_price = stock_data.price * price_variance;

        Ok(serde_json::json!({
            "symbol": symbol,
            "price": (adjusted_price * 100.0).round() / 100.0,
            "volume": (stock_data.volume as f64 * 1.02) as u64,
            "pe_ratio": stock_data.pe_ratio * 1.01,
            "market_cap": stock_data.market_cap,
            "sector": stock_data.sector,
            "source": "yahoo_finance",
            "timestamp": Utc::now().to_rfc3339(),
        }))
    }

    async fn fetch_alpha_vantage_data(&self, symbol: &str) -> Result<serde_json::Value> {
        // Simulate network latency for Alpha Vantage API (rate limited)
        tokio::time::sleep(tokio::time::Duration::from_millis(
            200 + (symbol.len() % 6) as u64 * 20,
        ))
        .await;

        let stock_data = self.get_realistic_stock_data(symbol);

        // Add slight Alpha Vantage-specific variance (most accurate, minimal variance)
        let price_variance = 0.995 + (symbol.len() % 7) as f64 * 0.003;
        let adjusted_price = stock_data.price * price_variance;

        Ok(serde_json::json!({
            "symbol": symbol,
            "price": (adjusted_price * 100.0).round() / 100.0,
            "volume": (stock_data.volume as f64 * 0.98) as u64,
            "pe_ratio": stock_data.pe_ratio * 0.995,
            "market_cap": stock_data.market_cap,
            "sector": stock_data.sector,
            "source": "alpha_vantage",
            "timestamp": Utc::now().to_rfc3339(),
        }))
    }

    fn get_realistic_stock_data(&self, symbol: &str) -> StockData {
        // Realistic stock data for popular stocks
        match symbol.to_uppercase().as_str() {
            "AAPL" => StockData {
                price: 175.43,
                volume: 45_230_000,
                pe_ratio: 28.5,
                market_cap: 2_800_000_000_000,
                sector: "Technology".to_string(),
            },
            "GOOGL" => StockData {
                price: 142.56,
                volume: 22_150_000,
                pe_ratio: 24.8,
                market_cap: 1_750_000_000_000,
                sector: "Technology".to_string(),
            },
            "MSFT" => StockData {
                price: 415.26,
                volume: 18_940_000,
                pe_ratio: 32.1,
                market_cap: 3_100_000_000_000,
                sector: "Technology".to_string(),
            },
            "AMZN" => StockData {
                price: 155.89,
                volume: 35_670_000,
                pe_ratio: 45.2,
                market_cap: 1_650_000_000_000,
                sector: "Consumer Discretionary".to_string(),
            },
            "TSLA" => StockData {
                price: 248.42,
                volume: 78_920_000,
                pe_ratio: 65.4,
                market_cap: 780_000_000_000,
                sector: "Automotive".to_string(),
            },
            "META" => StockData {
                price: 501.34,
                volume: 15_230_000,
                pe_ratio: 22.7,
                market_cap: 1_250_000_000_000,
                sector: "Technology".to_string(),
            },
            "NVDA" => StockData {
                price: 875.28,
                volume: 28_450_000,
                pe_ratio: 55.8,
                market_cap: 2_150_000_000_000,
                sector: "Technology".to_string(),
            },
            "NFLX" => StockData {
                price: 425.67,
                volume: 4_230_000,
                pe_ratio: 34.5,
                market_cap: 185_000_000_000,
                sector: "Communication Services".to_string(),
            },
            "JPM" => StockData {
                price: 195.43,
                volume: 12_450_000,
                pe_ratio: 12.8,
                market_cap: 570_000_000_000,
                sector: "Financial Services".to_string(),
            },
            "JNJ" => StockData {
                price: 156.78,
                volume: 8_750_000,
                pe_ratio: 15.2,
                market_cap: 410_000_000_000,
                sector: "Healthcare".to_string(),
            },
            "V" => StockData {
                price: 278.94,
                volume: 6_230_000,
                pe_ratio: 31.4,
                market_cap: 590_000_000_000,
                sector: "Financial Services".to_string(),
            },
            "PG" => StockData {
                price: 165.23,
                volume: 5_890_000,
                pe_ratio: 26.7,
                market_cap: 395_000_000_000,
                sector: "Consumer Staples".to_string(),
            },
            "UNH" => StockData {
                price: 512.87,
                volume: 2_340_000,
                pe_ratio: 23.9,
                market_cap: 485_000_000_000,
                sector: "Healthcare".to_string(),
            },
            "HD" => StockData {
                price: 345.67,
                volume: 3_120_000,
                pe_ratio: 22.1,
                market_cap: 350_000_000_000,
                sector: "Consumer Discretionary".to_string(),
            },
            "MA" => StockData {
                price: 456.23,
                volume: 2_890_000,
                pe_ratio: 33.6,
                market_cap: 425_000_000_000,
                sector: "Financial Services".to_string(),
            },
            "DIS" => StockData {
                price: 112.45,
                volume: 11_230_000,
                pe_ratio: 38.7,
                market_cap: 205_000_000_000,
                sector: "Communication Services".to_string(),
            },
            "PYPL" => StockData {
                price: 78.94,
                volume: 14_560_000,
                pe_ratio: 18.9,
                market_cap: 85_000_000_000,
                sector: "Financial Services".to_string(),
            },
            "ADBE" => StockData {
                price: 567.23,
                volume: 1_890_000,
                pe_ratio: 41.2,
                market_cap: 255_000_000_000,
                sector: "Technology".to_string(),
            },
            "CRM" => StockData {
                price: 234.56,
                volume: 4_120_000,
                pe_ratio: 48.3,
                market_cap: 225_000_000_000,
                sector: "Technology".to_string(),
            },
            "INTC" => StockData {
                price: 24.67,
                volume: 42_340_000,
                pe_ratio: 14.8,
                market_cap: 105_000_000_000,
                sector: "Technology".to_string(),
            },
            // Default case for unknown symbols
            _ => StockData {
                price: 95.50 + (symbol.len() as f64 * 3.25), // Vary by symbol length
                volume: 5_000_000 + (symbol.len() as u64 * 250_000),
                pe_ratio: 20.0 + (symbol.len() as f64 * 0.8),
                market_cap: 50_000_000_000 + (symbol.len() as u64 * 2_000_000_000),
                sector: "Mixed".to_string(),
            },
        }
    }
}

impl RecommendationType {
    pub fn as_str(&self) -> &str {
        match self {
            RecommendationType::Buy => "BUY",
            RecommendationType::Sell => "SELL",
            RecommendationType::Hold => "HOLD",
            RecommendationType::Rebalance => "REBALANCE",
        }
    }
}
