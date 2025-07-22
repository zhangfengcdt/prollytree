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

        // Get stock-specific data for more detailed analysis
        let stock_data = self.get_stock_specific_data(symbol);
        
        // Sector-based analysis with realistic weightings
        let sector_score = self.analyze_sector_outlook(&stock_data.sector, symbol, pe_ratio);
        score += sector_score.0;
        factors.push(sector_score.1);

        // Company-specific fundamental analysis
        let fundamental_score = self.analyze_fundamentals(symbol, price, pe_ratio, &stock_data.sector);
        score += fundamental_score.0;
        factors.push(fundamental_score.1);

        // Risk tolerance alignment
        let risk_score = self.analyze_risk_alignment(client, pe_ratio, &stock_data.sector, symbol);
        score += risk_score.0;
        factors.push(risk_score.1);

        // Market conditions and valuation
        let valuation_score = self.analyze_valuation(symbol, price, pe_ratio, &stock_data.sector);
        score += valuation_score.0;
        factors.push(valuation_score.1);

        // Determine recommendation with more nuanced thresholds
        let (recommendation_type, confidence) = self.determine_recommendation(score, symbol, &stock_data.sector);

        let reasoning = format!(
            "Analysis of {} (${:.2}, P/E: {:.1}) in {} sector: {}. \
            Client risk profile ({:?}) consideration: {}. \
            Key factors: {}",
            symbol,
            price,
            pe_ratio,
            stock_data.sector,
            match recommendation_type {
                RecommendationType::Buy => "Strong positive outlook with favorable risk-reward ratio",
                RecommendationType::Sell => "Concerns about valuation and sector headwinds",
                RecommendationType::Hold => "Balanced outlook, current position appropriate",
                RecommendationType::Rebalance => "Portfolio optimization opportunity identified",
            },
            client.risk_tolerance,
            self.get_risk_alignment_summary(client, &stock_data.sector),
            factors.join("; ")
        );

        (recommendation_type, reasoning, confidence)
    }

    fn get_stock_specific_data(&self, symbol: &str) -> super::StockData {
        // Simulate stock data lookup - in real implementation, this would query a database or API
        match symbol.to_uppercase().as_str() {
            "AAPL" => super::StockData {
                price: 175.43,
                volume: 45_230_000,
                pe_ratio: 28.5,
                market_cap: 2_800_000_000_000,
                sector: "Technology".to_string(),
            },
            "GOOGL" => super::StockData {
                price: 142.56,
                volume: 22_150_000,
                pe_ratio: 24.8,
                market_cap: 1_750_000_000_000,
                sector: "Technology".to_string(),
            },
            "MSFT" => super::StockData {
                price: 415.26,
                volume: 18_940_000,
                pe_ratio: 32.1,
                market_cap: 3_100_000_000_000,
                sector: "Technology".to_string(),
            },
            "AMZN" => super::StockData {
                price: 155.89,
                volume: 35_670_000,
                pe_ratio: 45.2,
                market_cap: 1_650_000_000_000,
                sector: "Consumer Discretionary".to_string(),
            },
            "TSLA" => super::StockData {
                price: 248.42,
                volume: 78_920_000,
                pe_ratio: 65.4,
                market_cap: 780_000_000_000,
                sector: "Automotive".to_string(),
            },
            "META" => super::StockData {
                price: 501.34,
                volume: 15_230_000,
                pe_ratio: 22.7,
                market_cap: 1_250_000_000_000,
                sector: "Technology".to_string(),
            },
            "NVDA" => super::StockData {
                price: 875.28,
                volume: 28_450_000,
                pe_ratio: 55.8,
                market_cap: 2_150_000_000_000,
                sector: "Technology".to_string(),
            },
            "JPM" => super::StockData {
                price: 195.43,
                volume: 12_450_000,
                pe_ratio: 12.8,
                market_cap: 570_000_000_000,
                sector: "Financial Services".to_string(),
            },
            "JNJ" => super::StockData {
                price: 156.78,
                volume: 8_750_000,
                pe_ratio: 15.2,
                market_cap: 410_000_000_000,
                sector: "Healthcare".to_string(),
            },
            "V" => super::StockData {
                price: 278.94,
                volume: 6_230_000,
                pe_ratio: 31.4,
                market_cap: 590_000_000_000,
                sector: "Financial Services".to_string(),
            },
            "PYPL" => super::StockData {
                price: 78.94,
                volume: 14_560_000,
                pe_ratio: 18.9,
                market_cap: 85_000_000_000,
                sector: "Financial Services".to_string(),
            },
            // Default case for unknown symbols
            _ => super::StockData {
                price: 95.50 + (symbol.len() as f64 * 3.25),
                volume: 5_000_000 + (symbol.len() as u64 * 250_000),
                pe_ratio: 20.0 + (symbol.len() as f64 * 0.8),
                market_cap: 50_000_000_000 + (symbol.len() as u64 * 2_000_000_000),
                sector: "Mixed".to_string(),
            },
        }
    }

    fn analyze_sector_outlook(&self, sector: &str, symbol: &str, pe_ratio: f64) -> (f64, &'static str) {
        match sector {
            "Technology" => {
                match symbol {
                    "AAPL" => (0.15, "Technology leader with strong ecosystem and services growth"),
                    "MSFT" => (0.25, "Cloud dominance and enterprise AI positioning driving growth"),
                    "GOOGL" => (0.10, "Search monopoly but facing AI disruption challenges"),
                    "META" => (0.05, "Metaverse investments weighing on near-term profitability"),
                    "NVDA" => (0.35, "AI semiconductor leader with unprecedented demand"),
                    "INTC" => (-0.15, "Struggling to compete in advanced chip manufacturing"),
                    "ADBE" => (0.20, "Creative software dominance with growing AI integration"),
                    "CRM" => (0.10, "Enterprise software growth but increased competition"),
                    _ => (0.05, "Technology sector showing mixed fundamentals")
                }
            }
            "Healthcare" => {
                if pe_ratio < 20.0 {
                    (0.20, "Healthcare defensive characteristics with reasonable valuation")
                } else {
                    (0.10, "Healthcare stability but premium valuation concerns")
                }
            }
            "Financial Services" => {
                match symbol {
                    "JPM" => (0.15, "Benefiting from higher interest rates and strong credit quality"),
                    "V" | "MA" => (0.25, "Payment network dominance with recession-resistant model"),
                    "PYPL" => (-0.10, "Facing increased competition in digital payments"),
                    _ => (0.05, "Financial sector mixed amid rate environment")
                }
            }
            "Consumer Discretionary" => {
                match symbol {
                    "AMZN" => (0.10, "E-commerce leader but cloud growth slowing"),
                    "TSLA" => (-0.05, "EV pioneer but intensifying competition and valuation concerns"),
                    "HD" => (0.15, "Home improvement demand resilient despite economic headwinds"),
                    "DIS" => (0.00, "Streaming wars and theme park recovery offsetting each other"),
                    _ => (-0.05, "Consumer discretionary facing economic headwinds")
                }
            }
            "Consumer Staples" => (0.10, "Defensive characteristics attractive in uncertain environment"),
            "Communication Services" => (0.05, "Mixed outlook with streaming competition intensifying"),
            "Automotive" => (-0.10, "Traditional auto facing EV transition challenges"),
            _ => (0.00, "Sector outlook neutral")
        }
    }

    fn analyze_fundamentals(&self, symbol: &str, _price: f64, pe_ratio: f64, _sector: &str) -> (f64, &'static str) {
        // Symbol-specific fundamental analysis
        match symbol {
            "AAPL" => {
                if pe_ratio > 30.0 {
                    (-0.10, "Premium valuation limits upside despite strong fundamentals")
                } else {
                    (0.20, "Strong balance sheet and ecosystem moat justify valuation")
                }
            }
            "MSFT" => {
                if pe_ratio < 35.0 {
                    (0.25, "Exceptional fundamentals with AI and cloud leadership")
                } else {
                    (0.10, "Strong fundamentals but valuation becoming stretched")
                }
            }
            "GOOGL" => {
                if pe_ratio < 25.0 {
                    (0.15, "Search dominance and attractive valuation multiple")
                } else {
                    (0.05, "Core business strong but AI disruption risks emerging")
                }
            }
            "NVDA" => {
                if pe_ratio > 60.0 {
                    (0.10, "Revolutionary AI demand but extreme valuation multiples")
                } else {
                    (0.30, "AI revolution driving unprecedented earnings growth")
                }
            }
            "TSLA" => {
                if pe_ratio > 50.0 {
                    (-0.20, "Growth slowing while valuation remains extremely high")
                } else {
                    (0.10, "EV leadership position but facing increased competition")
                }
            }
            "AMZN" => {
                if pe_ratio < 50.0 {
                    (0.15, "E-commerce dominance and cloud profitability improving")
                } else {
                    (0.05, "Growth slowing and competition intensifying")
                }
            }
            "JPM" => {
                if pe_ratio < 15.0 {
                    (0.20, "Strong credit quality and attractive valuation in rate environment")
                } else {
                    (0.10, "Solid fundamentals but limited upside at current levels")
                }
            }
            "META" => {
                if pe_ratio < 25.0 {
                    (0.15, "Social media dominance and efficiency improvements")
                } else {
                    (0.00, "Metaverse investments creating uncertainty about returns")
                }
            }
            _ => {
                // Generic P/E analysis for unknown stocks
                if pe_ratio < 15.0 {
                    (0.15, "Attractive valuation multiple suggests potential upside")
                } else if pe_ratio > 30.0 {
                    (-0.10, "High valuation multiple limits risk-adjusted returns")
                } else {
                    (0.05, "Valuation multiple within reasonable range")
                }
            }
        }
    }

    fn analyze_risk_alignment(&self, client: &ClientProfile, pe_ratio: f64, sector: &str, symbol: &str) -> (f64, &'static str) {
        match client.risk_tolerance {
            RiskTolerance::Conservative => {
                match sector {
                    "Healthcare" | "Consumer Staples" | "Financial Services" => {
                        if pe_ratio < 20.0 {
                            (0.15, "Defensive sector characteristics align with conservative approach")
                        } else {
                            (0.05, "Defensive sector but valuation limits conservative appeal")
                        }
                    }
                    "Technology" => {
                        match symbol {
                            "AAPL" | "MSFT" => (0.10, "Quality tech names suitable for conservative growth"),
                            _ => (-0.15, "Technology volatility exceeds conservative risk parameters")
                        }
                    }
                    _ => (-0.10, "Sector volatility may not align with conservative objectives")
                }
            }
            RiskTolerance::Moderate => {
                if pe_ratio > 40.0 {
                    (0.00, "Moderate risk tolerance accommodates some growth premium")
                } else {
                    (0.10, "Balanced risk-reward profile fits moderate investment approach")
                }
            }
            RiskTolerance::Aggressive => {
                match symbol {
                    "NVDA" | "TSLA" | "META" => (0.20, "High growth potential aligns with aggressive risk appetite"),
                    _ if pe_ratio > 30.0 => (0.15, "Growth premium acceptable for aggressive strategy"),
                    _ => (0.05, "May lack sufficient growth potential for aggressive allocation")
                }
            }
        }
    }

    fn analyze_valuation(&self, _symbol: &str, _price: f64, pe_ratio: f64, sector: &str) -> (f64, &'static str) {
        // More sophisticated valuation analysis
        let sector_avg_pe = match sector {
            "Technology" => 28.0,
            "Healthcare" => 18.0,
            "Financial Services" => 13.0,
            "Consumer Discretionary" => 25.0,
            "Consumer Staples" => 22.0,
            "Communication Services" => 20.0,
            _ => 20.0,
        };

        let pe_premium = pe_ratio / sector_avg_pe;

        if pe_premium < 0.8 {
            (0.20, "Trading at discount to sector average suggests value opportunity")
        } else if pe_premium > 1.5 {
            (-0.15, "Significant premium to sector average limits margin of safety")
        } else if pe_premium > 1.2 {
            (-0.05, "Modest premium to sector average warrants caution")
        } else {
            (0.10, "Valuation reasonable relative to sector comparables")
        }
    }

    fn determine_recommendation(&self, score: f64, symbol: &str, _sector: &str) -> (RecommendationType, f64) {
        // More nuanced recommendation logic with symbol-specific thresholds
        let (rec_type, base_confidence) = if score > 0.4 {
            (RecommendationType::Buy, 0.75 + (score - 0.4) * 0.5)
        } else if score > 0.2 {
            (RecommendationType::Buy, 0.60 + (score - 0.2) * 0.75)
        } else if score > -0.1 {
            (RecommendationType::Hold, 0.55 + score.abs() * 0.3)
        } else if score > -0.3 {
            (RecommendationType::Hold, 0.65 + score.abs() * 0.2)
        } else {
            (RecommendationType::Sell, 0.70 + score.abs() * 0.25)
        };

        // Cap confidence and add some symbol-specific variance
        let symbol_variance = (symbol.len() % 7) as f64 * 0.02;
        let final_confidence = (base_confidence + symbol_variance).min(0.95).max(0.30);

        (rec_type, final_confidence)
    }

    fn get_risk_alignment_summary(&self, client: &ClientProfile, sector: &str) -> &'static str {
        match (&client.risk_tolerance, sector) {
            (RiskTolerance::Conservative, "Healthcare") => "matches defensive investment criteria",
            (RiskTolerance::Conservative, "Consumer Staples") => "aligns with stability requirements",
            (RiskTolerance::Conservative, "Technology") => "requires careful selection within sector",
            (RiskTolerance::Aggressive, "Technology") => "leverages high-growth sector exposure",
            (RiskTolerance::Moderate, _) => "provides balanced risk-reward profile",
            _ => "consideration of risk parameters integrated"
        }
    }
}
