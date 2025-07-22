#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::memory::ValidatedMemory;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub confidence: f64,
    pub hash: [u8; 32],
    pub cross_references: Vec<String>,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub description: String,
    pub source: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum IssueSeverity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ValidationPolicy {
    Strict,   // All sources must agree
    Majority, // Majority of sources must agree
    Lenient,  // At least one trusted source
}

pub struct CrossReference {
    pub source: String,
    pub confidence_weight: f64,
}

pub struct MemoryValidator {
    policy: ValidationPolicy,
    sources: HashMap<String, f64>, // source -> trust score
    min_sources: usize,
    consistency_threshold: f64,
}

impl Default for MemoryValidator {
    fn default() -> Self {
        let mut sources = HashMap::new();
        sources.insert("bloomberg".to_string(), 0.95);
        sources.insert("yahoo_finance".to_string(), 0.85);
        sources.insert("alpha_vantage".to_string(), 0.80);
        sources.insert("internal".to_string(), 1.0);

        Self {
            policy: ValidationPolicy::Majority,
            sources,
            min_sources: 2,
            consistency_threshold: 0.05, // 5% variance allowed
        }
    }
}

impl MemoryValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(mut self, policy: ValidationPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn add_source(mut self, name: &str, trust_score: f64) -> Self {
        self.sources.insert(name.to_string(), trust_score);
        self
    }

    pub fn min_sources(mut self, count: usize) -> Self {
        self.min_sources = count;
        self
    }

    pub fn validate_multi_source(
        &self,
        data: &[(&str, serde_json::Value)],
    ) -> Result<ValidationResult> {
        let mut issues = Vec::new();

        // Check minimum sources
        if data.len() < self.min_sources {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Critical,
                description: format!(
                    "Insufficient sources: {} < {}",
                    data.len(),
                    self.min_sources
                ),
                source: "validator".to_string(),
            });

            return Ok(ValidationResult {
                is_valid: false,
                confidence: 0.0,
                hash: [0u8; 32],
                cross_references: vec![],
                issues,
            });
        }

        // Extract and compare values
        let mut price_values = Vec::new();
        let mut volume_values = Vec::new();
        let mut trusted_sources = Vec::new();

        for (source, value) in data {
            let trust_score = self.sources.get(*source).unwrap_or(&0.5);

            if *trust_score < 0.5 {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Warning,
                    description: format!("Low trust source: {source}"),
                    source: source.to_string(),
                });
            }

            // Extract values for comparison
            if let Some(price) = value.get("price").and_then(|p| p.as_f64()) {
                price_values.push((source.to_string(), price, *trust_score));
            }

            if let Some(volume) = value.get("volume").and_then(|v| v.as_i64()) {
                volume_values.push((source.to_string(), volume as f64, *trust_score));
            }

            trusted_sources.push(source.to_string());
        }

        // Validate consistency
        let price_consistency = self.check_consistency(&price_values, "price");
        let volume_consistency = self.check_consistency(&volume_values, "volume");

        issues.extend(price_consistency.1);
        issues.extend(volume_consistency.1);

        // Calculate overall confidence
        let confidence =
            self.calculate_confidence(&price_values, price_consistency.0, volume_consistency.0);

        // Generate hash of validated data
        let hash = self.hash_content(&serde_json::to_string(data)?);

        // Determine validity based on policy
        let is_valid = match self.policy {
            ValidationPolicy::Strict => issues
                .iter()
                .all(|i| !matches!(i.severity, IssueSeverity::Critical)),
            ValidationPolicy::Majority => {
                let critical_count = issues
                    .iter()
                    .filter(|i| matches!(i.severity, IssueSeverity::Critical))
                    .count();
                critical_count < data.len() / 2
            }
            ValidationPolicy::Lenient => trusted_sources
                .iter()
                .any(|s| self.sources.get(s).unwrap_or(&0.0) > &0.8),
        };

        Ok(ValidationResult {
            is_valid,
            confidence,
            hash,
            cross_references: trusted_sources,
            issues,
        })
    }

    pub fn has_contradiction(
        &self,
        memory1: &ValidatedMemory,
        memory2: &ValidatedMemory,
    ) -> Result<bool> {
        // Parse JSON content
        let content1: serde_json::Value = serde_json::from_str(&memory1.content)?;
        let content2: serde_json::Value = serde_json::from_str(&memory2.content)?;

        // Check for contradicting values
        if let (Some(symbol1), Some(symbol2)) = (content1.get("symbol"), content2.get("symbol")) {
            if symbol1 == symbol2 {
                // Same symbol, check for contradicting data
                if let (Some(price1), Some(price2)) = (
                    content1.get("price").and_then(|p| p.as_f64()),
                    content2.get("price").and_then(|p| p.as_f64()),
                ) {
                    let time_diff = (memory1.timestamp - memory2.timestamp).num_seconds().abs();

                    // If timestamps are close but prices differ significantly
                    if time_diff < 300 {
                        // 5 minutes
                        let price_diff = ((price1 - price2) / price1).abs();
                        if price_diff > self.consistency_threshold * 2.0 {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    pub fn hash_content(&self, content: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hasher.finalize().into()
    }

    fn check_consistency(
        &self,
        values: &[(String, f64, f64)],
        field: &str,
    ) -> (bool, Vec<ValidationIssue>) {
        let mut issues = Vec::new();

        if values.is_empty() {
            return (true, issues);
        }

        // Calculate weighted average
        let weighted_sum: f64 = values.iter().map(|(_, val, weight)| val * weight).sum();
        let weight_sum: f64 = values.iter().map(|(_, _, weight)| weight).sum();
        let avg = weighted_sum / weight_sum;

        // Check each value against average
        for (source, value, _) in values {
            let variance = ((value - avg) / avg).abs();

            if variance > self.consistency_threshold {
                issues.push(ValidationIssue {
                    severity: if variance > self.consistency_threshold * 2.0 {
                        IssueSeverity::Critical
                    } else {
                        IssueSeverity::Warning
                    },
                    description: format!(
                        "{} variance too high: {:.2}% (value: {}, avg: {})",
                        field,
                        variance * 100.0,
                        value,
                        avg
                    ),
                    source: source.clone(),
                });
            }
        }

        (issues.is_empty(), issues)
    }

    fn calculate_confidence(
        &self,
        values: &[(String, f64, f64)],
        price_ok: bool,
        volume_ok: bool,
    ) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        // Base confidence from source trust scores
        let avg_trust: f64 =
            values.iter().map(|(_, _, trust)| trust).sum::<f64>() / values.len() as f64;

        // Adjust for consistency
        let consistency_factor = match (price_ok, volume_ok) {
            (true, true) => 1.0,
            (true, false) | (false, true) => 0.8,
            (false, false) => 0.5,
        };

        avg_trust * consistency_factor
    }
}
