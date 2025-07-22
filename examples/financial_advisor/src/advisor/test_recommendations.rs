#[cfg(test)]
mod tests {
    use crate::advisor::{ClientProfile, RiskTolerance, RecommendationEngine};
    use crate::memory::ValidatedMemory;
    use chrono::Utc;

    #[tokio::test]
    async fn test_varied_recommendations() {
        let engine = RecommendationEngine::new();
        let client = ClientProfile {
            id: "test-client".to_string(),
            risk_tolerance: RiskTolerance::Moderate,
            investment_goals: vec!["Growth".to_string()],
            time_horizon: "5-10 years".to_string(),
            restrictions: vec![],
        };

        // Create mock market data
        let market_data = ValidatedMemory {
            id: "test".to_string(),
            content: r#"[["source1", {"price": 175.0, "pe_ratio": 28.5}]]"#.to_string(),
            timestamp: Utc::now(),
            validation_hash: [0u8; 32],
            sources: vec!["test".to_string()],
            confidence: 0.9,
            cross_references: vec![],
        };

        // Test different symbols to ensure varied results
        let symbols = vec!["AAPL", "MSFT", "TSLA", "JPM", "NVDA"];
        let mut recommendations = Vec::new();

        for symbol in symbols {
            let rec = engine.generate(symbol, &client, &market_data, &crate::memory::MemoryStore::new("test").await.unwrap()).await;
            assert!(rec.is_ok(), "Failed to generate recommendation for {}", symbol);
            let rec = rec.unwrap();
            recommendations.push((symbol, rec.recommendation_type.as_str().to_string(), rec.confidence));
            println!("Symbol: {}, Recommendation: {}, Confidence: {:.1}%", 
                     symbol, rec.recommendation_type.as_str(), rec.confidence * 100.0);
        }

        // Verify we have different recommendations and confidences
        let unique_recs: std::collections::HashSet<_> = recommendations.iter().map(|(_, rec, _)| rec).collect();
        let unique_confs: std::collections::HashSet<_> = recommendations.iter().map(|(_, _, conf)| (conf * 100.0) as i32).collect();

        println!("Unique recommendations: {}", unique_recs.len());
        println!("Unique confidence levels: {}", unique_confs.len());

        // We should have some variety in recommendations or confidence levels
        assert!(unique_recs.len() > 1 || unique_confs.len() > 2, 
                "Recommendations should show variety across different symbols");
    }

    #[tokio::test]
    async fn test_risk_tolerance_impact() {
        let engine = RecommendationEngine::new();
        
        let conservative_client = ClientProfile {
            id: "conservative".to_string(),
            risk_tolerance: RiskTolerance::Conservative,
            investment_goals: vec!["Stability".to_string()],
            time_horizon: "5-10 years".to_string(),
            restrictions: vec![],
        };

        let aggressive_client = ClientProfile {
            id: "aggressive".to_string(),
            risk_tolerance: RiskTolerance::Aggressive,
            investment_goals: vec!["Growth".to_string()],
            time_horizon: "5-10 years".to_string(),
            restrictions: vec![],
        };

        let market_data = ValidatedMemory {
            id: "test".to_string(),
            content: r#"[["source1", {"price": 875.0, "pe_ratio": 55.8}]]"#.to_string(),
            timestamp: Utc::now(),
            validation_hash: [0u8; 32],
            sources: vec!["test".to_string()],
            confidence: 0.9,
            cross_references: vec![],
        };

        let memory_store = crate::memory::MemoryStore::new("test").await.unwrap();

        // Test NVDA with different risk tolerances
        let conservative_rec = engine.generate("NVDA", &conservative_client, &market_data, &memory_store).await.unwrap();
        let aggressive_rec = engine.generate("NVDA", &aggressive_client, &market_data, &memory_store).await.unwrap();

        println!("Conservative: {} with {:.1}% confidence", 
                 conservative_rec.recommendation_type.as_str(), 
                 conservative_rec.confidence * 100.0);
        println!("Aggressive: {} with {:.1}% confidence", 
                 aggressive_rec.recommendation_type.as_str(), 
                 aggressive_rec.confidence * 100.0);

        // Aggressive clients should generally have higher confidence for growth stocks
        assert!(aggressive_rec.confidence >= conservative_rec.confidence - 0.05, 
                "Aggressive risk tolerance should not significantly decrease confidence for growth stocks");
    }
}