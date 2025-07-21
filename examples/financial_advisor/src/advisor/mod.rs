#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
// For now, commenting out rig-core imports to focus on memory consistency demo
// In a real implementation, these would be used for LLM interactions
// use rig_core::providers::openai::{Client, CompletionModel, OpenAI};
// use rig_core::completion::Prompt;
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
    api_key: String,
    verbose: bool,
    current_session: String,
}

impl FinancialAdvisor {
    pub async fn new(storage_path: &str, api_key: &str) -> Result<Self> {
        let memory_store = MemoryStore::new(storage_path).await?;
        let validator = MemoryValidator::default();
        let security_monitor = SecurityMonitor::new();
        let recommendation_engine = RecommendationEngine::new();
        Ok(Self {
            memory_store,
            validator,
            security_monitor,
            recommendation_engine,
            api_key: api_key.to_string(),
            verbose: false,
            current_session: Uuid::new_v4().to_string(),
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
        let recommendation = self
            .recommendation_engine
            .generate(symbol, client_profile, &market_data, &self.memory_store)
            .await?;

        // Step 5: Store recommendation with audit trail
        self.store_recommendation(&recommendation).await?;

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

    // Simulated data fetching methods
    async fn fetch_bloomberg_data(&self, symbol: &str) -> Result<serde_json::Value> {
        // In production, this would call Bloomberg API
        Ok(serde_json::json!({
            "symbol": symbol,
            "price": 150.25,
            "volume": 1000000,
            "pe_ratio": 25.5,
            "source": "bloomberg",
            "timestamp": Utc::now().to_rfc3339(),
        }))
    }

    async fn fetch_yahoo_data(&self, symbol: &str) -> Result<serde_json::Value> {
        // In production, this would call Yahoo Finance API
        Ok(serde_json::json!({
            "symbol": symbol,
            "price": 150.30,
            "volume": 1000500,
            "pe_ratio": 25.4,
            "source": "yahoo_finance",
            "timestamp": Utc::now().to_rfc3339(),
        }))
    }

    async fn fetch_alpha_vantage_data(&self, symbol: &str) -> Result<serde_json::Value> {
        // In production, this would call Alpha Vantage API
        Ok(serde_json::json!({
            "symbol": symbol,
            "price": 150.28,
            "volume": 1000200,
            "pe_ratio": 25.45,
            "source": "alpha_vantage",
            "timestamp": Utc::now().to_rfc3339(),
        }))
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
