#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
// OpenAI integration for AI-powered recommendations
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::memory::{MemoryCommit, MemoryStore, MemoryType, Storable, ValidatedMemory};
use crate::security::SecurityMonitor;
use crate::validation::{MemoryValidator, ValidationResult};

pub mod compliance;
pub mod interactive;
pub mod recommendations;
pub mod rig_agent;

use interactive::InteractiveSession;
use recommendations::RecommendationEngine;
use rig_agent::FinancialAnalysisAgent;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataSource {
    #[serde(rename = "Real Stock Data")]
    RealStockData,
    #[serde(rename = "Simulated Data")]
    SimulatedData,
}

struct StockData {
    price: f64,
    volume: u64,
    pe_ratio: f64,
    market_cap: u64,
    sector: String,
    data_source: DataSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisMode {
    #[serde(rename = "AI-Powered")]
    AIPowered,
    #[serde(rename = "Rule-Based")]
    RuleBased,
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
    pub analysis_mode: AnalysisMode,
    pub data_source: DataSource,
}

pub struct FinancialAdvisor {
    memory_store: MemoryStore,
    validator: MemoryValidator,
    security_monitor: SecurityMonitor,
    recommendation_engine: RecommendationEngine,
    rig_agent: FinancialAnalysisAgent,
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

        // Initialize Rig agent for AI analysis
        let rig_agent = FinancialAnalysisAgent::new_openai(api_key, false)?;

        Ok(Self {
            memory_store,
            validator,
            security_monitor,
            recommendation_engine,
            rig_agent,
            api_key: api_key.to_string(),
            verbose: false,
            current_session: Uuid::new_v4().to_string(),
            session_recommendations: Vec::new(),
        })
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
        // Update Rig agent verbosity by recreating it
        if let Ok(new_agent) = FinancialAnalysisAgent::new_openai(&self.api_key, verbose) {
            self.rig_agent = new_agent;
        }
    }

    pub async fn get_recommendation(
        &mut self,
        symbol: &str,
        client_profile: &ClientProfile,
        notes: Option<String>,
    ) -> Result<Recommendation> {
        self.get_recommendation_with_debug(symbol, client_profile, notes, false)
            .await
    }

    pub async fn get_recommendation_with_debug(
        &mut self,
        symbol: &str,
        client_profile: &ClientProfile,
        notes: Option<String>,
        debug_mode: bool,
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
        let stock_data = self.get_realistic_stock_data(symbol);
        let mut recommendation = self
            .recommendation_engine
            .generate(symbol, client_profile, &market_data, &self.memory_store)
            .await?;

        // Set the data source
        recommendation.data_source = stock_data.data_source;

        // Step 4.5: Enhance reasoning with AI analysis
        if self.verbose {
            println!("🧠 Generating AI-powered analysis...");
        }

        let (ai_reasoning, analysis_mode) = self
            .generate_rig_analysis_with_debug(
                symbol,
                &recommendation.recommendation_type,
                &serde_json::from_str::<serde_json::Value>(&market_data.content)
                    .unwrap_or(serde_json::json!({})),
                client_profile,
                debug_mode,
            )
            .await?;

        // Set the analysis mode
        recommendation.analysis_mode = analysis_mode;

        // Combine traditional and AI reasoning with mode indicator
        recommendation.reasoning = match recommendation.analysis_mode {
            AnalysisMode::AIPowered => format!(
                "{}\n\n🤖 AI Analysis: {}",
                recommendation.reasoning, ai_reasoning
            ),
            AnalysisMode::RuleBased => format!(
                "{}\n\n📊 Rule-Based Analysis: {}",
                recommendation.reasoning, ai_reasoning
            ),
        };

        // Step 5: Store recommendation with audit trail
        self.store_recommendation(&recommendation, notes).await?;

        // Keep in session memory for quick access
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

    async fn store_recommendation(
        &mut self,
        recommendation: &Recommendation,
        notes: Option<String>,
    ) -> Result<()> {
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

        // Create custom commit message based on notes
        let commit_message = if let Some(user_notes) = notes {
            format!(
                "Finance Advisor: recommend {} ({})",
                recommendation.symbol, user_notes
            )
        } else {
            format!("Finance Advisor: recommend {}", recommendation.symbol)
        };

        // Store with full audit trail and custom commit message using typed storage
        self.memory_store
            .store_typed_with_audit_and_commit(
                recommendation,
                MemoryType::Recommendation,
                &memory,
                &format!(
                    "Generated {} recommendation for {}",
                    recommendation.recommendation_type.as_str(),
                    recommendation.symbol
                ),
                &commit_message,
            )
            .await?;

        if self.verbose {
            println!("✅ Recommendation stored. ID: {}", recommendation.id);
        }

        Ok(())
    }

    async fn store_security_test(
        &mut self,
        payload: &str,
        alert: &crate::security::SecurityAlert,
        notes: Option<String>,
    ) -> Result<()> {
        use crate::memory::MemoryType;
        use uuid::Uuid;

        let security_test = serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "payload": payload,
            "alert_level": format!("{:?}", alert.level),
            "alert_type": format!("{:?}", alert.alert_type),
            "description": alert.description,
            "confidence": alert.confidence,
            "recommendations": alert.recommendations,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let memory = ValidatedMemory {
            id: Uuid::new_v4().to_string(),
            content: security_test.to_string(),
            timestamp: Utc::now(),
            validation_hash: self.validator.hash_content(&security_test.to_string()),
            sources: vec!["security_monitor".to_string()],
            confidence: alert.confidence,
            cross_references: vec![],
        };

        // Create custom commit message based on notes
        let commit_message = if let Some(user_notes) = notes {
            format!("Finance Advisor: security test ({user_notes})")
        } else {
            "Finance Advisor: security test".to_string()
        };

        // Store with full audit trail and custom commit message
        self.memory_store
            .store_with_audit_and_commit(
                MemoryType::Security,
                &memory,
                &format!("Security test: {}", alert.description),
                &commit_message,
            )
            .await?;

        if self.verbose {
            println!("✅ Security test stored. ID: {}", memory.id);
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
        self.memory_store.get_recent_recommendations(limit).await
    }

    pub async fn get_recommendations_at_commit(
        &self,
        commit: &str,
        limit: usize,
    ) -> Result<Vec<Recommendation>> {
        self.memory_store
            .get_recommendations(None, Some(commit), Some(limit))
            .await
    }

    pub async fn get_recommendations_at_branch(
        &self,
        branch: &str,
        limit: usize,
    ) -> Result<Vec<Recommendation>> {
        self.memory_store
            .get_recommendations(Some(branch), None, Some(limit))
            .await
    }

    pub async fn get_memory_status(&self) -> Result<crate::memory::MemoryStatus> {
        self.memory_store.get_memory_status().await
    }

    pub async fn get_validation_sources(&self) -> Result<Vec<crate::memory::ValidationSource>> {
        self.memory_store.get_validation_sources().await
    }

    pub async fn get_audit_trail(&self) -> Result<Vec<crate::memory::AuditEntry>> {
        self.memory_store.get_audit_trail(None, None).await
    }

    pub async fn store_client_profile(&mut self, profile: &ClientProfile) -> Result<()> {
        self.memory_store.store_client_profile(profile).await
    }

    pub async fn load_client_profile(&self) -> Result<Option<ClientProfile>> {
        self.memory_store.load_client_profile().await
    }

    pub async fn create_and_switch_branch(&mut self, name: &str) -> Result<()> {
        // Create the branch
        self.memory_store.create_branch(name).await?;

        // Switch to the newly created branch
        self.memory_store.checkout(name).await?;

        Ok(())
    }

    pub fn current_branch(&self) -> &str {
        self.memory_store.current_branch()
    }

    pub fn get_actual_current_branch(&self) -> String {
        self.memory_store.get_actual_current_branch()
    }

    pub async fn switch_to_branch(&mut self, name: &str) -> Result<()> {
        // Just switch to the branch (no creation)
        self.memory_store.checkout(name).await?;
        Ok(())
    }

    pub fn branch_exists(&self, name: &str) -> bool {
        // Check if branch exists by listing all branches
        if let Ok(branches) = self.memory_store.list_branches() {
            branches.contains(&name.to_string())
        } else {
            false
        }
    }

    pub fn list_branches(&self) -> Result<Vec<String>> {
        self.memory_store.list_branches()
    }

    pub async fn get_memory_history(&self, limit: Option<usize>) -> Result<Vec<MemoryCommit>> {
        self.memory_store.get_memory_history(limit).await
    }

    async fn generate_rig_analysis_with_debug(
        &self,
        symbol: &str,
        recommendation_type: &RecommendationType,
        market_data: &serde_json::Value,
        client: &ClientProfile,
        debug_mode: bool,
    ) -> Result<(String, AnalysisMode)> {
        use rig_agent::AnalysisRequest;

        // Extract market data
        let price = market_data["price"].as_f64().unwrap_or(0.0);
        let pe_ratio = market_data["pe_ratio"].as_f64().unwrap_or(0.0);
        let volume = market_data["volume"].as_u64().unwrap_or(0);
        let sector = market_data["sector"].as_str().unwrap_or("Unknown");

        let request = AnalysisRequest {
            symbol: symbol.to_string(),
            price,
            pe_ratio,
            volume,
            sector: sector.to_string(),
            recommendation_type: recommendation_type.clone(),
            client_profile: client.clone(),
        };

        let response = self
            .rig_agent
            .generate_analysis(&request, debug_mode)
            .await?;
        Ok((response.reasoning, response.analysis_mode))
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
                data_source: DataSource::RealStockData,
            },
            "GOOGL" => StockData {
                price: 142.56,
                volume: 22_150_000,
                pe_ratio: 24.8,
                market_cap: 1_750_000_000_000,
                sector: "Technology".to_string(),
                data_source: DataSource::RealStockData,
            },
            "MSFT" => StockData {
                price: 415.26,
                volume: 18_940_000,
                pe_ratio: 32.1,
                market_cap: 3_100_000_000_000,
                sector: "Technology".to_string(),
                data_source: DataSource::RealStockData,
            },
            "AMZN" => StockData {
                price: 155.89,
                volume: 35_670_000,
                pe_ratio: 45.2,
                market_cap: 1_650_000_000_000,
                sector: "Consumer Discretionary".to_string(),
                data_source: DataSource::RealStockData,
            },
            "TSLA" => StockData {
                price: 248.42,
                volume: 78_920_000,
                pe_ratio: 65.4,
                market_cap: 780_000_000_000,
                sector: "Automotive".to_string(),
                data_source: DataSource::RealStockData,
            },
            "META" => StockData {
                price: 501.34,
                volume: 15_230_000,
                pe_ratio: 22.7,
                market_cap: 1_250_000_000_000,
                sector: "Technology".to_string(),
                data_source: DataSource::RealStockData,
            },
            "NVDA" => StockData {
                price: 875.28,
                volume: 28_450_000,
                pe_ratio: 55.8,
                market_cap: 2_150_000_000_000,
                sector: "Technology".to_string(),
                data_source: DataSource::RealStockData,
            },
            "NFLX" => StockData {
                price: 425.67,
                volume: 4_230_000,
                pe_ratio: 34.5,
                market_cap: 185_000_000_000,
                sector: "Communication Services".to_string(),
                data_source: DataSource::RealStockData,
            },
            "JPM" => StockData {
                price: 195.43,
                volume: 12_450_000,
                pe_ratio: 12.8,
                market_cap: 570_000_000_000,
                sector: "Financial Services".to_string(),
                data_source: DataSource::RealStockData,
            },
            "JNJ" => StockData {
                price: 156.78,
                volume: 8_750_000,
                pe_ratio: 15.2,
                market_cap: 410_000_000_000,
                sector: "Healthcare".to_string(),
                data_source: DataSource::RealStockData,
            },
            "V" => StockData {
                price: 278.94,
                volume: 6_230_000,
                pe_ratio: 31.4,
                market_cap: 590_000_000_000,
                sector: "Financial Services".to_string(),
                data_source: DataSource::RealStockData,
            },
            "PG" => StockData {
                price: 165.23,
                volume: 5_890_000,
                pe_ratio: 26.7,
                market_cap: 395_000_000_000,
                sector: "Consumer Staples".to_string(),
                data_source: DataSource::RealStockData,
            },
            "UNH" => StockData {
                price: 512.87,
                volume: 2_340_000,
                pe_ratio: 23.9,
                market_cap: 485_000_000_000,
                sector: "Healthcare".to_string(),
                data_source: DataSource::RealStockData,
            },
            "HD" => StockData {
                price: 345.67,
                volume: 3_120_000,
                pe_ratio: 22.1,
                market_cap: 350_000_000_000,
                sector: "Consumer Discretionary".to_string(),
                data_source: DataSource::RealStockData,
            },
            "MA" => StockData {
                price: 456.23,
                volume: 2_890_000,
                pe_ratio: 33.6,
                market_cap: 425_000_000_000,
                sector: "Financial Services".to_string(),
                data_source: DataSource::RealStockData,
            },
            "DIS" => StockData {
                price: 112.45,
                volume: 11_230_000,
                pe_ratio: 38.7,
                market_cap: 205_000_000_000,
                sector: "Communication Services".to_string(),
                data_source: DataSource::RealStockData,
            },
            "PYPL" => StockData {
                price: 78.94,
                volume: 14_560_000,
                pe_ratio: 18.9,
                market_cap: 85_000_000_000,
                sector: "Financial Services".to_string(),
                data_source: DataSource::RealStockData,
            },
            "ADBE" => StockData {
                price: 567.23,
                volume: 1_890_000,
                pe_ratio: 41.2,
                market_cap: 255_000_000_000,
                sector: "Technology".to_string(),
                data_source: DataSource::RealStockData,
            },
            "CRM" => StockData {
                price: 234.56,
                volume: 4_120_000,
                pe_ratio: 48.3,
                market_cap: 225_000_000_000,
                sector: "Technology".to_string(),
                data_source: DataSource::RealStockData,
            },
            "INTC" => StockData {
                price: 24.67,
                volume: 42_340_000,
                pe_ratio: 14.8,
                market_cap: 105_000_000_000,
                sector: "Technology".to_string(),
                data_source: DataSource::RealStockData,
            },
            // Default case for unknown symbols
            _ => StockData {
                price: 95.50 + (symbol.len() as f64 * 3.25), // Vary by symbol length
                volume: 5_000_000 + (symbol.len() as u64 * 250_000),
                pe_ratio: 20.0 + (symbol.len() as f64 * 0.8),
                market_cap: 50_000_000_000 + (symbol.len() as u64 * 2_000_000_000),
                sector: "Mixed".to_string(),
                data_source: DataSource::SimulatedData,
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

    pub fn as_serde_str(&self) -> &str {
        match self {
            RecommendationType::Buy => "Buy",
            RecommendationType::Sell => "Sell",
            RecommendationType::Hold => "Hold",
            RecommendationType::Rebalance => "Rebalance",
        }
    }
}

impl Storable for Recommendation {
    fn table_name() -> &'static str {
        "recommendations"
    }

    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn store_to_db(
        &self,
        glue: &mut gluesql_core::prelude::Glue<prollytree::sql::ProllyStorage<32>>,
        memory: &ValidatedMemory,
    ) -> impl std::future::Future<Output = Result<()>> {
        let sql = format!(
            r#"INSERT INTO recommendations
            (id, client_id, symbol, recommendation_type, reasoning, confidence,
             validation_hash, memory_version, timestamp)
            VALUES ('{}', '{}', '{}', '{}', '{}', {}, '{}', '{}', {})"#,
            self.id,
            self.client_id,
            self.symbol,
            self.recommendation_type.as_serde_str(),
            self.reasoning.replace('\'', "''"),
            self.confidence,
            hex::encode(memory.validation_hash),
            self.memory_version,
            self.timestamp.timestamp()
        );

        async move {
            glue.execute(&sql).await?;
            Ok(())
        }
    }

    fn load_from_db(
        glue: &mut gluesql_core::prelude::Glue<prollytree::sql::ProllyStorage<32>>,
        limit: Option<usize>,
    ) -> impl std::future::Future<Output = Result<Vec<ValidatedMemory>>> {
        let sql = if let Some(limit) = limit {
            format!("SELECT id, client_id, symbol, recommendation_type, reasoning, confidence, validation_hash, memory_version, timestamp FROM recommendations ORDER BY timestamp DESC LIMIT {limit}")
        } else {
            "SELECT id, client_id, symbol, recommendation_type, reasoning, confidence, validation_hash, memory_version, timestamp FROM recommendations ORDER BY timestamp DESC".to_string()
        };

        async move {
            let results = glue.execute(&sql).await?;

            use gluesql_core::data::Value;
            let mut memories = Vec::new();

            for payload in results {
                if let gluesql_core::prelude::Payload::Select { labels: _, rows } = payload {
                    for row in rows {
                        if row.len() >= 9 {
                            let id = match &row[0] {
                                Value::Str(s) => s.clone(),
                                _ => continue,
                            };

                            let client_id = match &row[1] {
                                Value::Str(s) => s.clone(),
                                _ => continue,
                            };

                            let symbol = match &row[2] {
                                Value::Str(s) => s.clone(),
                                _ => continue,
                            };

                            let recommendation_type = match &row[3] {
                                Value::Str(s) => s.clone(),
                                _ => "Unknown".to_string(),
                            };

                            let reasoning = match &row[4] {
                                Value::Str(s) => s.clone(),
                                _ => "".to_string(),
                            };

                            let confidence = match &row[5] {
                                Value::F64(f) => *f,
                                _ => 0.0,
                            };

                            let memory_version = match &row[7] {
                                Value::Str(s) => s.clone(),
                                _ => "".to_string(),
                            };

                            let timestamp = match &row[8] {
                                Value::I64(ts) => chrono::DateTime::from_timestamp(*ts, 0)
                                    .unwrap_or_else(chrono::Utc::now)
                                    .to_rfc3339(),
                                _ => chrono::Utc::now().to_rfc3339(),
                            };

                            let validation_hash = match &row[6] {
                                Value::Str(s) => hex::decode(s)
                                    .unwrap_or_default()
                                    .try_into()
                                    .unwrap_or([0u8; 32]),
                                _ => [0u8; 32],
                            };

                            let content = serde_json::json!({
                                "id": id,
                                "client_id": client_id,
                                "symbol": symbol,
                                "recommendation_type": recommendation_type,
                                "reasoning": reasoning,
                                "confidence": confidence,
                                "memory_version": memory_version,
                                "timestamp": timestamp,
                                "validation_result": {
                                    "is_valid": true,
                                    "confidence": confidence,
                                    "hash": validation_hash.to_vec(),
                                    "cross_references": [],
                                    "issues": []
                                }
                            })
                            .to_string();

                            let memory = ValidatedMemory {
                                id,
                                content,
                                validation_hash,
                                sources: vec!["recommendation_engine".to_string()],
                                confidence,
                                timestamp: match &row[8] {
                                    Value::I64(ts) => chrono::DateTime::from_timestamp(*ts, 0)
                                        .unwrap_or_else(chrono::Utc::now),
                                    _ => chrono::Utc::now(),
                                },
                                cross_references: vec![],
                            };

                            memories.push(memory);
                        }
                    }
                }
            }

            Ok(memories)
        }
    }
}

impl Storable for ClientProfile {
    fn table_name() -> &'static str {
        "client_profiles"
    }

    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn store_to_db(
        &self,
        glue: &mut gluesql_core::prelude::Glue<prollytree::sql::ProllyStorage<32>>,
        memory: &ValidatedMemory,
    ) -> impl std::future::Future<Output = Result<()>> {
        let sql = format!(
            r#"INSERT INTO client_profiles
            (id, content, timestamp, validation_hash, sources, confidence)
            VALUES ('{}', '{}', {}, '{}', '{}', {})"#,
            memory.id,
            memory.content.replace('\'', "''"),
            memory.timestamp.timestamp(),
            hex::encode(memory.validation_hash),
            memory.sources.join(","),
            memory.confidence
        );

        async move {
            glue.execute(&sql).await?;
            Ok(())
        }
    }

    fn load_from_db(
        glue: &mut gluesql_core::prelude::Glue<prollytree::sql::ProllyStorage<32>>,
        _limit: Option<usize>,
    ) -> impl std::future::Future<Output = Result<Vec<ValidatedMemory>>> {
        let sql = "SELECT id, content, timestamp, validation_hash, sources, confidence FROM client_profiles ORDER BY timestamp DESC LIMIT 1";

        async move {
            let results = glue.execute(sql).await?;

            use gluesql_core::data::Value;
            let mut memories = Vec::new();

            for payload in results {
                if let gluesql_core::prelude::Payload::Select { labels: _, rows } = payload {
                    for row in rows {
                        if row.len() >= 6 {
                            let memory = ValidatedMemory {
                                id: match &row[0] {
                                    Value::Str(s) => s.clone(),
                                    _ => continue,
                                },
                                content: match &row[1] {
                                    Value::Str(s) => s.clone(),
                                    _ => continue,
                                },
                                timestamp: match &row[2] {
                                    Value::I64(ts) => chrono::DateTime::from_timestamp(*ts, 0)
                                        .unwrap_or_else(chrono::Utc::now),
                                    _ => chrono::Utc::now(),
                                },
                                validation_hash: match &row[3] {
                                    Value::Str(s) => hex::decode(s)
                                        .unwrap_or_default()
                                        .try_into()
                                        .unwrap_or([0u8; 32]),
                                    _ => [0u8; 32],
                                },
                                sources: match &row[4] {
                                    Value::Str(s) => s.split(',').map(String::from).collect(),
                                    _ => vec![],
                                },
                                confidence: match &row[5] {
                                    Value::F64(f) => *f,
                                    _ => 0.0,
                                },
                                cross_references: vec![],
                            };
                            memories.push(memory);
                        }
                    }
                }
            }

            Ok(memories)
        }
    }
}
