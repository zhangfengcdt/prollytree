#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use super::{ClientProfile, Recommendation, RecommendationType, RiskTolerance};
use crate::memory::{MemoryStore, ValidatedMemory};
use crate::validation::ValidationResult;

pub struct RecommendationEngine;

impl Default for RecommendationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RecommendationEngine {
    pub fn new() -> Self {
        Self
    }

    pub async fn generate(
        &self,
        symbol: &str,
        client: &ClientProfile,
        market_data: &ValidatedMemory,
        _memory_store: &MemoryStore,
    ) -> Result<Recommendation> {
        // Parse market data
        let data: serde_json::Value = serde_json::from_str(&market_data.content)?;
        let price = self.extract_average_price(&data)?;
        let pe_ratio = self.extract_average_pe_ratio(&data)?;

        // Generate recommendation based on client profile and market data
        let (recommendation_type, reasoning, confidence) =
            self.analyze_investment(symbol, price, pe_ratio, client);

        Ok(Recommendation {
            id: Uuid::new_v4().to_string(),
            client_id: client.id.clone(),
            symbol: symbol.to_string(),
            recommendation_type,
            reasoning,
            confidence,
            timestamp: Utc::now(),
            validation_result: ValidationResult {
                is_valid: true,
                confidence: market_data.confidence,
                hash: market_data.validation_hash,
                cross_references: market_data.sources.clone(),
                issues: vec![],
            },
            memory_version: format!("v-{}", Utc::now().timestamp()),
        })
    }

    fn extract_average_price(&self, data: &serde_json::Value) -> Result<f64> {
        let mut prices = Vec::new();

        // Extract prices from all sources
        if let Some(sources) = data.as_array() {
            for source in sources {
                if let Some((_, source_data)) = source
                    .as_array()
                    .and_then(|arr| arr.get(1))
                    .map(|data| (source, data))
                {
                    if let Some(price) = source_data.get("price").and_then(|p| p.as_f64()) {
                        prices.push(price);
                    }
                }
            }
        }

        if prices.is_empty() {
            return Err(anyhow::anyhow!("No price data found"));
        }

        Ok(prices.iter().sum::<f64>() / prices.len() as f64)
    }

    fn extract_average_pe_ratio(&self, data: &serde_json::Value) -> Result<f64> {
        let mut ratios = Vec::new();

        // Extract P/E ratios from all sources
        if let Some(sources) = data.as_array() {
            for source in sources {
                if let Some((_, source_data)) = source
                    .as_array()
                    .and_then(|arr| arr.get(1))
                    .map(|data| (source, data))
                {
                    if let Some(pe_ratio) = source_data.get("pe_ratio").and_then(|p| p.as_f64()) {
                        ratios.push(pe_ratio);
                    }
                }
            }
        }

        if ratios.is_empty() {
            return Ok(20.0); // Default P/E ratio
        }

        Ok(ratios.iter().sum::<f64>() / ratios.len() as f64)
    }

    fn analyze_investment(
        &self,
        symbol: &str,
        price: f64,
        pe_ratio: f64,
        client: &ClientProfile,
    ) -> (RecommendationType, String, f64) {
        let mut factors = Vec::new();
        let mut score: f64 = 0.0;

        // Analyze P/E ratio
        if pe_ratio < 15.0 {
            factors.push("Low P/E ratio indicates undervaluation");
            score += 0.3;
        } else if pe_ratio > 30.0 {
            factors.push("High P/E ratio suggests overvaluation");
            score -= 0.2;
        } else {
            factors.push("P/E ratio within reasonable range");
            score += 0.1;
        }

        // Analyze based on client risk tolerance
        match client.risk_tolerance {
            RiskTolerance::Conservative => {
                if pe_ratio < 20.0 {
                    factors.push("Suitable for conservative investor due to stable valuation");
                    score += 0.2;
                } else {
                    factors.push("May be too volatile for conservative profile");
                    score -= 0.3;
                }
            }
            RiskTolerance::Moderate => {
                factors.push("Fits moderate risk tolerance");
                score += 0.1;
            }
            RiskTolerance::Aggressive => {
                if pe_ratio > 25.0 {
                    factors.push("High growth potential suitable for aggressive investor");
                    score += 0.2;
                } else {
                    factors.push("May lack growth potential for aggressive strategy");
                    score -= 0.1;
                }
            }
        }

        // Simple sector analysis (in real implementation, this would be more sophisticated)
        if symbol.starts_with("AAPL") || symbol.starts_with("MSFT") || symbol.starts_with("GOOGL") {
            factors.push("Technology sector showing strong fundamentals");
            score += 0.2;
        }

        // Determine recommendation
        let (recommendation_type, confidence) = if score > 0.3 {
            (RecommendationType::Buy, (score * 0.8 + 0.2).min(0.95))
        } else if score < -0.2 {
            (RecommendationType::Sell, ((-score) * 0.7 + 0.3).min(0.9))
        } else {
            (RecommendationType::Hold, 0.6 + score.abs() * 0.2)
        };

        let reasoning = format!(
            "Analysis of {} at ${:.2} with P/E ratio {:.1}: {}. \
            Recommendation based on client risk tolerance ({:?}) and market conditions. \
            Factors considered: {}",
            symbol,
            price,
            pe_ratio,
            match recommendation_type {
                RecommendationType::Buy => "Positive outlook with good value proposition",
                RecommendationType::Sell => "Concerns about current valuation and risks",
                RecommendationType::Hold =>
                    "Mixed signals, maintaining current position recommended",
                RecommendationType::Rebalance => "Portfolio adjustment recommended",
            },
            client.risk_tolerance,
            factors.join("; ")
        );

        (recommendation_type, reasoning, confidence)
    }
}
