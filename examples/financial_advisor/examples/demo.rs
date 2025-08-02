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
use chrono::Duration;
use colored::Colorize;

use financial_advisor::advisor::enhanced_advisor::EnhancedFinancialAdvisor;
use financial_advisor::advisor::RiskTolerance;
use financial_advisor::memory::enhanced_types::*;

/// Comprehensive demonstration of the enhanced financial advisor capabilities
#[tokio::main]
async fn main() -> Result<()> {
    println!(
        "{}",
        "🏦 Enhanced Financial Advisor Demonstration"
            .bright_blue()
            .bold()
    );
    println!(
        "{}",
        "Showcasing advanced agent memory and multi-step workflows".dimmed()
    );
    println!("{}", "━".repeat(70).dimmed());
    println!();

    // Initialize the enhanced advisor
    let storage_path = "/tmp/enhanced_advisor_demo";
    std::fs::create_dir_all(storage_path)?;

    let api_key = std::env::var("OPENAI_API_KEY").ok();
    if api_key.is_none() {
        println!("💡 Tip: Set OPENAI_API_KEY environment variable for AI-powered analysis");
    }

    let mut advisor = EnhancedFinancialAdvisor::new(storage_path, api_key.as_deref(), true).await?;

    println!("✅ Enhanced Financial Advisor initialized with agent memory system");
    println!();

    // Demonstration 1: Multi-Client Scenarios
    println!(
        "{}",
        "📋 Demonstration 1: Multi-Client Personalized Analysis"
            .bright_green()
            .bold()
    );
    println!("{}", "━".repeat(50).dimmed());

    await_demo_clients(&mut advisor).await?;

    println!();

    // Demonstration 2: Complex Workflow Analysis
    println!(
        "{}",
        "📋 Demonstration 2: Deep Research & Multi-Step Analysis"
            .bright_green()
            .bold()
    );
    println!("{}", "━".repeat(50).dimmed());

    await_demo_deep_analysis(&mut advisor).await?;

    println!();

    // Demonstration 3: Portfolio Management
    println!(
        "{}",
        "📋 Demonstration 3: Portfolio Rebalancing & Risk Management"
            .bright_green()
            .bold()
    );
    println!("{}", "━".repeat(50).dimmed());

    await_demo_portfolio_management(&mut advisor).await?;

    println!();

    // Demonstration 4: Learning and Adaptation
    println!(
        "{}",
        "📋 Demonstration 4: Learning from Outcomes"
            .bright_green()
            .bold()
    );
    println!("{}", "━".repeat(50).dimmed());

    await_demo_learning(&mut advisor).await?;

    println!();

    // Demonstration 5: Memory System Capabilities
    println!(
        "{}",
        "📋 Demonstration 5: Advanced Memory Features"
            .bright_green()
            .bold()
    );
    println!("{}", "━".repeat(50).dimmed());

    await_demo_memory_features(&mut advisor).await?;

    println!();

    // Final summary
    println!("{}", "🎯 Demonstration Complete!".bright_green().bold());
    println!("{}", "━".repeat(70).dimmed());

    let stats = advisor.get_memory_statistics().await?;
    println!(
        "📊 Total memories created: {}",
        stats.overall.total_memories
    );
    println!(
        "💾 Storage utilization: {:.2} MB",
        stats.overall.total_size_bytes as f64 / 1024.0 / 1024.0
    );
    println!("🔄 Active threads: {}", stats.short_term.active_threads);

    println!();
    println!("💡 This demonstration showcased:");
    println!("  • Memory-driven personalization across multiple clients");
    println!("  • Complex multi-step financial analysis workflows");
    println!("  • Deep market research with AI integration");
    println!("  • Portfolio rebalancing with risk management");
    println!("  • Learning from recommendation outcomes");
    println!("  • Advanced memory system optimization");
    println!();
    println!(
        "📂 Memory data persisted to: {}",
        storage_path.bright_cyan()
    );

    Ok(())
}

async fn await_demo_clients(advisor: &mut EnhancedFinancialAdvisor) -> Result<()> {
    // Client 1: Conservative retiree
    println!(
        "👤 {}",
        "Client 1: Sarah (Conservative Retiree)".bright_cyan()
    );

    advisor.set_current_client("sarah_retired").await?;
    advisor
        .update_client_risk_profile("sarah_retired", RiskTolerance::Conservative)
        .await?;

    let recommendation1 = advisor.get_enhanced_recommendation("JNJ").await?;
    print_recommendation_summary(&recommendation1, "JNJ");

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Client 2: Young aggressive investor
    println!(
        "👤 {}",
        "Client 2: Mike (Young Aggressive Investor)".bright_cyan()
    );

    advisor.set_current_client("mike_young").await?;
    advisor
        .update_client_risk_profile("mike_young", RiskTolerance::Aggressive)
        .await?;

    let recommendation2 = advisor.get_enhanced_recommendation("NVDA").await?;
    print_recommendation_summary(&recommendation2, "NVDA");

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Client 3: Moderate family investor
    println!(
        "👤 {}",
        "Client 3: Jennifer (Family Moderate Investor)".bright_cyan()
    );

    advisor.set_current_client("jennifer_family").await?;
    advisor
        .update_client_risk_profile("jennifer_family", RiskTolerance::Moderate)
        .await?;

    let recommendation3 = advisor.get_enhanced_recommendation("MSFT").await?;
    print_recommendation_summary(&recommendation3, "MSFT");

    Ok(())
}

async fn await_demo_deep_analysis(advisor: &mut EnhancedFinancialAdvisor) -> Result<()> {
    advisor.set_current_client("mike_young").await?;

    println!("🔬 Performing deep research analysis on AAPL...");

    let market_analysis = advisor.perform_deep_research("AAPL").await?;

    println!("📈 Fundamental Analysis:");
    println!(
        "   Growth Prospects: {}",
        market_analysis.fundamental_analysis.growth_prospects
    );
    println!(
        "   Competitive Position: {}",
        market_analysis.fundamental_analysis.competitive_position
    );

    println!("📊 Technical Analysis:");
    println!(
        "   Trend Direction: {}",
        market_analysis.technical_analysis.trend_direction
    );
    println!(
        "   Support Levels: {:?}",
        market_analysis.technical_analysis.support_levels
    );

    println!("🏭 Sector Analysis:");
    println!(
        "   Sector Trend: {}",
        market_analysis.sector_analysis.sector_trend
    );
    println!(
        "   Relative Performance: {:.1}%",
        market_analysis.sector_analysis.relative_performance * 100.0
    );

    println!("💭 Sentiment Analysis:");
    println!(
        "   Analyst Sentiment: {:.1}%",
        market_analysis.sentiment_analysis.analyst_sentiment * 100.0
    );
    println!(
        "   Market Sentiment: {:.1}%",
        market_analysis.sentiment_analysis.market_sentiment * 100.0
    );

    if !market_analysis.ai_insights.is_empty() {
        println!("🤖 AI Insights:");
        println!("   {}", market_analysis.ai_insights);
    }

    Ok(())
}

async fn await_demo_portfolio_management(advisor: &mut EnhancedFinancialAdvisor) -> Result<()> {
    advisor.set_current_client("jennifer_family").await?;

    println!("⚖️  Analyzing portfolio rebalancing for diversified family portfolio...");

    // Simulate current portfolio holdings
    let holdings = vec![
        ("AAPL".to_string(), 0.25),  // 25% Apple
        ("MSFT".to_string(), 0.20),  // 20% Microsoft
        ("JNJ".to_string(), 0.15),   // 15% Johnson & Johnson
        ("V".to_string(), 0.15),     // 15% Visa
        ("PG".to_string(), 0.10),    // 10% Procter & Gamble
        ("GOOGL".to_string(), 0.15), // 15% Google
    ];

    let rebalancing_recommendations = advisor
        .analyze_portfolio_rebalancing("jennifer_family", holdings)
        .await?;

    println!("📊 Rebalancing Analysis Results:");
    for (i, rec) in rebalancing_recommendations.iter().enumerate() {
        println!(
            "   {}. {:?} - Confidence: {:.1}%",
            i + 1,
            rec.base_recommendation,
            rec.confidence_adjustment * 100.0
        );
        println!(
            "      Reasoning: {}",
            rec.personalized_reasoning
                .split('.')
                .next()
                .unwrap_or("Analysis complete")
        );
    }

    Ok(())
}

async fn await_demo_learning(advisor: &mut EnhancedFinancialAdvisor) -> Result<()> {
    advisor.set_current_client("sarah_retired").await?;

    println!("📈 Simulating recommendation outcomes and learning...");

    // Get a recommendation first
    let recommendation = advisor.get_enhanced_recommendation("PG").await?;

    // Simulate a positive outcome after some time
    let outcome = RecommendationOutcome {
        actual_return: 0.085, // 8.5% return
        time_to_outcome: Duration::days(90),
        client_satisfaction: Some(0.9), // Very satisfied
        followed_recommendation: true,
        notes: "Client was very pleased with steady growth and dividend income".to_string(),
    };

    println!("✅ Positive outcome recorded:");
    println!("   Return: {:.1}%", outcome.actual_return * 100.0);
    println!(
        "   Client Satisfaction: {:.1}/10",
        outcome.client_satisfaction.unwrap_or(0.0) * 10.0
    );
    println!(
        "   Followed Advice: {}",
        if outcome.followed_recommendation {
            "Yes"
        } else {
            "No"
        }
    );

    // Update the recommendation outcome
    advisor
        .update_recommendation_outcome(&recommendation.recommendation_id, outcome)
        .await?;

    println!("🧠 Learning algorithms updated with outcome data");

    // Simulate another recommendation to show adaptation
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let adapted_recommendation = advisor.get_enhanced_recommendation("UNH").await?;
    println!("🎯 Subsequent recommendation generated with learned patterns:");
    print_recommendation_summary(&adapted_recommendation, "UNH");

    Ok(())
}

async fn await_demo_memory_features(advisor: &mut EnhancedFinancialAdvisor) -> Result<()> {
    println!("🧠 Demonstrating advanced memory system features...");

    // Get current memory statistics
    let stats = advisor.get_memory_statistics().await?;
    println!("📊 Current Memory State:");
    println!("   Total Memories: {}", stats.overall.total_memories);
    println!("   Short-term Threads: {}", stats.short_term.active_threads);
    println!(
        "   Storage Size: {:.2} MB",
        stats.overall.total_size_bytes as f64 / 1024.0 / 1024.0
    );
    println!("   Avg Access Count: {:.2}", stats.overall.avg_access_count);

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Demonstrate memory optimization
    println!("🧹 Performing memory system optimization...");
    let optimization_report = advisor.optimize_memory_system().await?;

    println!("✅ Optimization Results:");
    println!(
        "   Expired memories cleaned: {}",
        optimization_report.expired_cleaned
    );
    println!(
        "   Memories consolidated: {}",
        optimization_report.memories_consolidated
    );
    println!(
        "   Memories archived: {}",
        optimization_report.memories_archived
    );
    println!(
        "   Low-value memories pruned: {}",
        optimization_report.memories_pruned
    );
    println!(
        "   Total items processed: {}",
        optimization_report.total_processed()
    );

    // Show updated statistics
    let updated_stats = advisor.get_memory_statistics().await?;
    println!("📊 Post-Optimization State:");
    println!(
        "   Total Memories: {} (change: {:+})",
        updated_stats.overall.total_memories,
        updated_stats.overall.total_memories as i64 - stats.overall.total_memories as i64
    );

    Ok(())
}

fn print_recommendation_summary(recommendation: &DetailedRecommendation, symbol: &str) {
    println!(
        "   📊 {} Recommendation: {:?} ({:.1}% confidence)",
        symbol.bright_yellow(),
        recommendation.base_recommendation,
        recommendation.confidence * 100.0
    );

    println!(
        "   💭 Key Insight: {}",
        recommendation
            .personalized_reasoning
            .split('.')
            .next()
            .unwrap_or("Analysis complete")
            .trim()
    );

    println!(
        "   ⚖️  Risk Score: {:.1}/10 (Alignment: {:.1}%)",
        recommendation.risk_assessment.overall_risk_score * 10.0,
        recommendation.risk_assessment.client_risk_alignment * 100.0
    );

    println!(
        "   🛡️  Compliance: {}",
        if recommendation.compliance_validation.passed {
            "✅ PASSED".green()
        } else {
            "❌ ISSUES".red()
        }
    );

    println!(
        "   ⏱️  Processing: {:.2}s",
        recommendation
            .execution_metadata
            .total_execution_time
            .num_milliseconds() as f64
            / 1000.0
    );

    println!();
}

// Helper function to create a delay for better demo pacing
async fn _demo_pause() {
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
}
