/*
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod attack_simulator;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SecurityLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAlert {
    pub level: SecurityLevel,
    pub alert_type: AlertType,
    pub description: String,
    pub recommendations: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AlertType {
    InjectionAttempt,
    DataPoisoning,
    ContextManipulation,
    UnauthorizedAccess,
    AnomalousPattern,
}

pub struct SecurityMonitor {
    patterns: HashMap<String, f64>, // suspicious patterns -> risk score
    context_windows: HashMap<String, Vec<String>>, // session -> recent inputs
    max_context_size: usize,
    alert_threshold: f64,
}

impl Default for SecurityMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityMonitor {
    pub fn new() -> Self {
        let mut patterns = HashMap::new();

        // Known injection patterns
        patterns.insert("always".to_string(), 0.7);
        patterns.insert("never forget".to_string(), 0.8);
        patterns.insert("ignore previous".to_string(), 0.9);
        patterns.insert("system message".to_string(), 0.6);
        patterns.insert("remember that".to_string(), 0.5);
        patterns.insert("from now on".to_string(), 0.6);
        patterns.insert("permanently".to_string(), 0.7);

        // Financial-specific patterns
        patterns.insert("transfer to account".to_string(), 0.9);
        patterns.insert("send money to".to_string(), 0.9);
        patterns.insert("buy immediately".to_string(), 0.7);
        patterns.insert("urgent investment".to_string(), 0.8);

        Self {
            patterns,
            context_windows: HashMap::new(),
            max_context_size: 20,
            alert_threshold: 0.6,
        }
    }

    pub fn check_for_anomalies(&mut self, data: &crate::memory::ValidatedMemory) -> Result<()> {
        let content_lower = data.content.to_lowercase();
        let mut total_risk = 0.0;
        let mut detected_patterns = Vec::new();

        // Check for suspicious patterns
        for (pattern, risk) in &self.patterns {
            if content_lower.contains(pattern) {
                total_risk += risk;
                detected_patterns.push(pattern.clone());
            }
        }

        // Check for rapid-fire similar inputs (potential automation)
        if self.check_automation_pattern(&data.content)? {
            total_risk += 0.5;
            detected_patterns.push("automation".to_string());
        }

        // Generate alert if threshold exceeded
        if total_risk >= self.alert_threshold {
            let alert = SecurityAlert {
                level: self.risk_to_level(total_risk),
                alert_type: self.determine_alert_type(&detected_patterns),
                description: format!(
                    "Suspicious patterns detected: {} (risk: {:.2})",
                    detected_patterns.join(", "),
                    total_risk
                ),
                recommendations: self.generate_recommendations(&detected_patterns),
                confidence: (total_risk / detected_patterns.len() as f64).min(1.0),
            };

            return Err(anyhow::anyhow!("Security alert: {:?}", alert));
        }

        // Update context window
        self.update_context("default", &data.content);

        Ok(())
    }

    pub fn simulate_injection_attack(&mut self, payload: &str) -> Result<SecurityAlert> {
        let content_lower = payload.to_lowercase();
        let mut risk_score = 0.0;
        let mut detected_patterns = Vec::new();

        // Check injection patterns
        for (pattern, risk) in &self.patterns {
            if content_lower.contains(pattern) {
                risk_score += risk;
                detected_patterns.push(pattern.clone());
            }
        }

        // Additional checks for obvious injection attempts
        if content_lower.contains("remember") && content_lower.contains("always") {
            risk_score += 0.8;
            detected_patterns.push("memory_injection".to_string());
        }

        if content_lower.contains("instruction") || content_lower.contains("command") {
            risk_score += 0.7;
            detected_patterns.push("instruction_injection".to_string());
        }

        Ok(SecurityAlert {
            level: self.risk_to_level(risk_score),
            alert_type: AlertType::InjectionAttempt,
            description: format!(
                "Injection attempt detected with patterns: {} (risk: {:.2})",
                detected_patterns.join(", "),
                risk_score
            ),
            recommendations: vec![
                "Quarantine input in separate branch".to_string(),
                "Flag for human review".to_string(),
                "Validate against known good patterns".to_string(),
            ],
            confidence: risk_score.min(1.0),
        })
    }

    pub fn detect_data_poisoning(&self, sources: &[String]) -> Option<SecurityAlert> {
        // Check for unusual source patterns
        let trusted_sources = ["bloomberg", "yahoo_finance", "alpha_vantage"];
        let untrusted_count = sources
            .iter()
            .filter(|s| !trusted_sources.contains(&s.as_str()))
            .count();

        if untrusted_count > sources.len() / 2 {
            return Some(SecurityAlert {
                level: SecurityLevel::Medium,
                alert_type: AlertType::DataPoisoning,
                description: "Majority of sources are untrusted".to_string(),
                recommendations: vec![
                    "Require additional verification".to_string(),
                    "Cross-reference with trusted sources".to_string(),
                ],
                confidence: 0.7,
            });
        }

        None
    }

    fn check_automation_pattern(&mut self, content: &str) -> Result<bool> {
        let session = "default".to_string();
        let context = self.context_windows.entry(session).or_default().clone();

        // Check for repeated similar content
        let similarity_count = context
            .iter()
            .filter(|prev| self.similarity(content, prev) > 0.8)
            .count();

        Ok(similarity_count > 3)
    }

    fn similarity(&self, a: &str, b: &str) -> f64 {
        // Simple similarity check - in production, use more sophisticated methods
        let a_words: Vec<&str> = a.split_whitespace().collect();
        let b_words: Vec<&str> = b.split_whitespace().collect();

        if a_words.is_empty() || b_words.is_empty() {
            return 0.0;
        }

        let common_words = a_words.iter().filter(|word| b_words.contains(word)).count();

        (common_words as f64) / (a_words.len().max(b_words.len()) as f64)
    }

    fn update_context(&mut self, session: &str, content: &str) {
        let context = self.context_windows.entry(session.to_string()).or_default();
        context.push(content.to_string());

        // Keep only recent history
        if context.len() > self.max_context_size {
            context.remove(0);
        }
    }

    fn risk_to_level(&self, risk: f64) -> SecurityLevel {
        match risk {
            r if r >= 0.8 => SecurityLevel::Critical,
            r if r >= 0.6 => SecurityLevel::High,
            r if r >= 0.3 => SecurityLevel::Medium,
            _ => SecurityLevel::Low,
        }
    }

    fn determine_alert_type(&self, patterns: &[String]) -> AlertType {
        if patterns
            .iter()
            .any(|p| p.contains("inject") || p.contains("always") || p.contains("ignore"))
        {
            AlertType::InjectionAttempt
        } else if patterns
            .iter()
            .any(|p| p.contains("transfer") || p.contains("money"))
        {
            AlertType::DataPoisoning
        } else if patterns.contains(&"automation".to_string()) {
            AlertType::AnomalousPattern
        } else {
            AlertType::ContextManipulation
        }
    }

    fn generate_recommendations(&self, patterns: &[String]) -> Vec<String> {
        let mut recommendations = Vec::new();

        if patterns
            .iter()
            .any(|p| p.contains("inject") || p.contains("always"))
        {
            recommendations.extend(vec![
                "Create isolation branch for suspicious input".to_string(),
                "Validate against known good patterns".to_string(),
                "Require human approval before integration".to_string(),
            ]);
        }

        if patterns
            .iter()
            .any(|p| p.contains("transfer") || p.contains("money"))
        {
            recommendations.extend(vec![
                "Flag for immediate security review".to_string(),
                "Lock financial operations".to_string(),
                "Audit recent recommendations".to_string(),
            ]);
        }

        if patterns.contains(&"automation".to_string()) {
            recommendations.extend(vec![
                "Implement rate limiting".to_string(),
                "Verify human interaction".to_string(),
            ]);
        }

        if recommendations.is_empty() {
            recommendations.push("Monitor for additional suspicious activity".to_string());
        }

        recommendations
    }
}
