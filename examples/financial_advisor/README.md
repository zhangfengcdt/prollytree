# Financial Advisory AI with Versioned Memory

A comprehensive financial advisory system demonstrating ProllyTree's capabilities with both traditional versioned memory and advanced agent memory features. From basic git-like storage to sophisticated multi-step AI workflows with deep personalization.

## 🚀 Two Implementation Levels

### 1. **Original Financial Advisor**
Secure, auditable AI financial advisor with git-like versioned memory, temporal queries, and complete audit trails.

### 2. **Enhanced Financial Advisor** 
Sophisticated, memory-driven system utilizing the full ProllyTree agent memory abstraction with multi-step AI workflows, behavioral learning, and complex financial analysis.

---

## 📋 Quick Start Guide

### Basic Financial Advisor

```bash
# 1. Setup storage with git
mkdir -p /tmp/advisor && cd /tmp/advisor && git init

# 2. Set OpenAI API key (optional, for AI reasoning)
export OPENAI_API_KEY="your-api-key"

# 3. Run the basic advisor
cargo run -- --storage /tmp/advisor/data advise
```

### Enhanced Financial Advisor

```bash
# 1. Run enhanced interactive session
cargo run -- enhanced --verbose

# 2. Run comprehensive demonstration
cargo run --example enhanced_demo

# 3. With AI integration
OPENAI_API_KEY="your-key" cargo run -- enhanced --verbose
```

---

# Part I: Original Financial Advisor

## Core Features

- **Versioned Memory**: Git-like storage with branches, commits, and history
- **AI Recommendations**: OpenAI-powered analysis with risk-aware insights
- **Security**: Injection detection, anomaly monitoring, audit trails
- **Multi-Source Validation**: Cross-validates data from multiple sources

## How Recommendations Work

The `recommend <SYMBOL>` command generates AI-powered investment advice through a sophisticated pipeline:

### 1. Data Collection (Simulated)
The system simulates fetching real-time market data from three sources:
- **Bloomberg**: Premium data with 95% trust weight (50ms latency)
- **Yahoo Finance**: Free tier with 85% trust weight (120ms latency)  
- **Alpha Vantage**: Rate-limited with 80% trust weight (200ms latency)

```
🏦 [main] recommend AAPL
🔍 Fetching market data for AAPL...
📡 Validating data from 3 sources...
```

### 2. Data Validation & Cross-Reference
Each source returns realistic market data based on actual stock characteristics:
```json
{
  "price": 177.89,
  "pe_ratio": 28.4,
  "volume": 53_245_678,
  "market_cap": 2_800_000_000_000,
  "sector": "Technology"
}
```

The validator:
- Compares prices across sources (must be within 2% variance)
- Generates SHA-256 hash for data integrity
- Assigns confidence score based on source agreement
- Stores validated data in versioned memory

### 3. Security Checks
Before processing, the security monitor scans for:
- SQL injection patterns
- Malicious payloads  
- Data anomalies
- Manipulation attempts

### 4. AI-Powered Analysis
The recommendation engine considers:
- **Client Profile**: Risk tolerance, investment timeline, goals
- **Market Data**: Price, P/E ratio, volume, sector trends
- **Historical Context**: Past recommendations on current branch

With OpenAI API:
```
🧠 Generating AI-powered analysis...
📊 Recommendation Generated
Symbol: AAPL
Action: BUY
Confidence: 85.0%
Reasoning: Strong fundamentals with P/E of 28.4...

🤖 AI Analysis: Apple shows robust growth potential with 
upcoming product launches and services expansion. The current 
valuation offers an attractive entry point for long-term investors.
```

## Key Commands (Original)

### Recommendations & Profiles
- `recommend <SYMBOL>` - Get AI recommendation with market analysis
- `profile` - View/edit client profile  
- `risk <conservative|moderate|aggressive>` - Set risk tolerance

### Branch Management  
- `branch <NAME>` - Create strategy branch
- `switch <NAME>` - Change branches
- `visualize` - Show branch tree with commits

### Time Travel
- `history` - Recent recommendations
- `history <commit>` - View at specific commit
- `history --branch <name>` - Compare branches

### Security & Audit
- `memory` - System status and validation
- `audit` - Complete operation history
- `test-inject <TEXT>` - Test security (try SQL injection!)

---

# Part II: Enhanced Financial Advisor

## 🚀 What's New in the Enhanced Version

### Core Enhancements

- **Complete Agent Memory Integration**: Utilizes all four memory types (short-term, semantic, episodic, procedural) from `src/agent`
- **Multi-Step Workflow Engine**: Complex analysis pipelines that leverage memory for context and learning
- **Deep Personalization**: Client-specific adaptations based on behavioral patterns and interaction history
- **Advanced AI Integration**: Enhanced Rig framework usage with contextual memory-driven prompts
- **Learning and Adaptation**: System learns from recommendation outcomes to improve future advice

### New Architecture Components

#### 1. Enhanced Memory Schema (`src/memory/enhanced_types.rs`)
Financial-specific data structures for comprehensive memory storage:

- **ClientEntity**: Rich client profiles with risk evolution tracking
- **MarketEntity**: Comprehensive market data with analyst ratings
- **RecommendationEpisode**: Detailed recommendation history with outcomes
- **AnalysisWorkflow**: Procedural knowledge for multi-step analysis
- **ComplianceRule**: Automated compliance checking and validation

#### 2. Workflow Processor (`src/advisor/workflow.rs`)
Orchestrates complex multi-step financial analysis:

```rust
pub async fn execute_recommendation_workflow(
    &mut self,
    symbol: &str,
    client_id: &str,
) -> Result<DetailedRecommendation>
```

**Workflow Steps:**
1. Initialize analysis context with client memory
2. Market research using semantic and episodic memory
3. Risk analysis with historical pattern recognition
4. Compliance validation with automated rule checking
5. Personalized recommendation generation
6. Learning outcome storage for future improvement

#### 3. Analysis Modules (`src/advisor/analysis_modules.rs`)
Specialized analysis components:

- **MarketResearchModule**: Comprehensive market analysis with AI insights
- **RiskAnalysisModule**: Multi-dimensional risk assessment
- **ComplianceModule**: Automated regulatory compliance checking
- **RecommendationModule**: Personalized recommendation generation

#### 4. Personalization Engine (`src/advisor/personalization.rs`)
Advanced client behavior modeling and adaptation:

- **ClientBehaviorModel**: Tracks decision patterns, risk evolution, communication preferences
- **PersonalizationInsights**: AI-driven adaptation strategies
- **Memory-Driven Learning**: Continuous improvement from client interactions

#### 5. Enhanced Financial Advisor (`src/advisor/enhanced_advisor.rs`)
Main coordinator that brings everything together:

```rust
pub struct EnhancedFinancialAdvisor {
    memory_system: Arc<AgentMemorySystem>,
    workflow_processor: WorkflowProcessor,
    analysis_modules: AnalysisModuleRegistry,
    rig_client: Option<Client>,
}
```

## 🎯 Key Features Demonstrated

### Memory-Driven Intelligence
- **Semantic Memory**: Store and retrieve client profiles, market entities, compliance rules
- **Episodic Memory**: Learn from past interactions, recommendation outcomes, market events
- **Procedural Memory**: Codify analysis workflows, compliance procedures, learning algorithms
- **Short-Term Memory**: Maintain context during multi-step analysis workflows

### Memory System Architecture
```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  Short-term     │    │    Semantic      │    │   Episodic      │
│  Working Memory │    │  Knowledge Base  │    │   Experience    │
│                 │    │                  │    │                 │
│ • Analysis      │◄──►│ • Client Profiles│◄──►│ • Interactions  │
│   Context       │    │ • Market Data    │    │ • Outcomes      │
│ • Step Results  │    │ • Compliance     │    │ • Decisions     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                │
                                ▼
                    ┌──────────────────┐
                    │   Procedural     │
                    │   Workflows      │
                    │                  │
                    │ • Analysis Steps │
                    │ • Risk Procedures│
                    │ • Compliance     │
                    └──────────────────┘
```

### Complex Workflow Capabilities
- **Multi-Source Data Validation**: Cross-reference market data from multiple sources
- **Sequential Analysis Steps**: Each step builds on previous results and memory context
- **Dynamic Adaptation**: Workflow adapts based on client profile and market conditions
- **Memory Checkpoint Creation**: Track workflow execution for debugging and learning

### Advanced Personalization
- **Behavioral Pattern Recognition**: Learn client decision-making patterns
- **Communication Style Adaptation**: Adjust language and detail level per client preferences
- **Risk Tolerance Evolution**: Track how client risk preferences change over time
- **Outcome-Based Learning**: Improve recommendations based on historical success rates

### Compliance and Risk Management
- **Automated Compliance Checking**: Real-time validation against regulatory rules
- **Multi-Dimensional Risk Assessment**: Comprehensive risk analysis across categories
- **Client Suitability Validation**: Ensure recommendations align with client profiles
- **Audit Trail Generation**: Complete documentation for regulatory compliance

## Enhanced Commands

### Interactive Session Commands
- `client <id>` - Set current client for personalized analysis
- `recommend <symbol>` - Get enhanced recommendation with full workflow
- `research <symbol>` - Perform deep research analysis
- `stats` - Show memory system statistics
- `optimize` - Optimize memory system performance

### Usage Examples

#### Multi-Client Scenarios

```rust
// Client 1: Conservative retiree
advisor.set_current_client("sarah_retired").await?;
advisor.update_client_risk_profile("sarah_retired", RiskTolerance::Conservative).await?;
let conservative_rec = advisor.get_enhanced_recommendation("JNJ").await?;

// Client 2: Aggressive young investor  
advisor.set_current_client("mike_young").await?;
advisor.update_client_risk_profile("mike_young", RiskTolerance::Aggressive).await?;
let aggressive_rec = advisor.get_enhanced_recommendation("NVDA").await?;
```

#### Portfolio Management

```rust
let holdings = vec![
    ("AAPL".to_string(), 0.25),
    ("MSFT".to_string(), 0.20),
    ("JNJ".to_string(), 0.15),
];

let rebalancing_recs = advisor.analyze_portfolio_rebalancing("client_id", holdings).await?;
```

#### Learning from Outcomes

```rust
let outcome = RecommendationOutcome {
    actual_return: 0.085, // 8.5% return
    client_satisfaction: Some(0.9),
    followed_recommendation: true,
    notes: "Client very satisfied with results".to_string(),
};

advisor.update_recommendation_outcome(&recommendation_id, outcome).await?;
```

---

## 📊 Comparison: Original vs Enhanced

| Feature | Original | Enhanced |
|---------|----------|----------|
| Memory Types | Basic versioned storage | 4 specialized memory types |
| Analysis Depth | Single-step with AI fallback | Multi-step workflow with memory context |
| Personalization | Rule-based client profiles | Behavioral learning and adaptation |
| Compliance | Basic validation | Automated rule checking with memory |
| Learning | Static recommendations | Outcome-based continuous improvement |
| AI Integration | Simple prompt-response | Memory-contextual intelligent prompts |
| Workflow | Linear analysis | Complex multi-step orchestration |
| Data Persistence | File-based storage | Agent memory with relationships |

---

## 🛠️ Complete Command Reference

```bash
# Command line interface
cargo run -- [OPTIONS] <COMMAND>

Commands:
  advise     Interactive advisory session (original)
  enhanced   Enhanced advisory session with agent memory
  visualize  Memory tree visualization
  attack     Security testing suite
  benchmark  Performance measurements
  memory     Git operations interface
  audit      Compliance reporting

Options:
  -s, --storage <PATH>  Storage directory [default: ./advisor_memory/data]
  -v, --verbose         Show detailed operations

Examples:
  cargo run --example enhanced_demo         # Complete enhanced demonstration
  cargo run -- enhanced --verbose          # Enhanced interactive session
  cargo run -- --storage /tmp/data advise  # Original advisor
```

---

## 🎯 Use Cases Demonstrated

### 1. Personalized Client Advisory
- Different recommendation styles for conservative vs aggressive clients
- Communication adaptation based on client preferences
- Risk tolerance evolution tracking

### 2. Deep Market Research
- Multi-source data analysis with cross-validation
- Historical pattern recognition
- AI-enhanced market insights

### 3. Portfolio Management
- Multi-asset portfolio analysis
- Rebalancing recommendations
- Risk-adjusted performance optimization

### 4. Compliance Automation
- Real-time regulatory compliance checking
- Automated violation detection
- Comprehensive audit trail generation

### 5. Continuous Learning
- Recommendation outcome tracking
- Behavioral pattern learning
- Workflow optimization based on success rates

---

## 🏗️ Technical Architecture

### Design Patterns
- **Memory-Driven Architecture**: All decisions informed by stored knowledge
- **Workflow Orchestration**: Complex analysis pipelines
- **Behavioral Adaptation**: Learning from user interactions
- **Compliance-First Design**: Regulatory requirements built into the system

### Key Technologies
- **ProllyTree Storage**: Content-addressed storage with Merkle proofs
- **ProllyTree Agent Memory**: Advanced memory abstraction layer
- **Rig Framework**: AI-powered analysis and reasoning
- **Git Integration**: Native git operations for versioning
- **Async Rust**: High-performance concurrent processing

### Security Features
- **Input Validation**: Regex patterns block SQL injection
- **Anomaly Detection**: Statistical analysis of data patterns
- **Rate Limiting**: Prevents abuse and DOS attempts
- **Audit Trail**: Cryptographically signed operation log

---

## 📈 Performance and Analytics

### Memory Statistics
- Total memories stored across all types
- Storage utilization and performance metrics
- Memory access patterns and optimization opportunities
- Cross-reference relationship mapping

### Learning Analytics
- Recommendation accuracy tracking over time
- Client satisfaction correlation analysis
- Behavioral pattern recognition success rates
- Workflow efficiency improvements

### Compliance Reporting
- Automated compliance violation detection
- Risk assessment accuracy validation
- Audit trail completeness verification
- Regulatory reporting automation

---

## 🔧 Configuration

### Environment Variables
```bash
export OPENAI_API_KEY="your-api-key"          # Enable AI-powered analysis
export ADVISOR_STORAGE="/path/to/storage"     # Custom storage location
export ADVISOR_VERBOSE="true"                 # Enable verbose logging
```

### Customization Points
- **Analysis Modules**: Add new specialized analysis components
- **Memory Schema**: Extend with domain-specific data structures
- **Workflow Steps**: Create custom analysis workflows
- **Personalization Rules**: Add new behavioral pattern recognition
- **Compliance Rules**: Configure regulatory compliance checking

---

## 📚 Educational Value

This comprehensive financial advisor demonstrates:

1. **Versioned Memory Systems**: Git-like storage with temporal queries
2. **Agent Memory Architecture**: Complete implementation of all four memory types
3. **Complex Workflow Orchestration**: Multi-step analysis with memory context
4. **AI Integration Patterns**: Memory-driven prompt engineering
5. **Behavioral Learning**: Client adaptation and outcome-based improvement
6. **Compliance Automation**: Regulatory checking with procedural memory
7. **Security Best Practices**: Input validation and audit trails
8. **Real-World Application**: Practical financial advisory scenarios

---

## ⚠️ Important Notes

### Data Simulation
The system uses realistic market data simulation:
- Popular stocks (AAPL, MSFT, GOOGL, etc.) have accurate characteristics
- Prices vary ±1% between sources to simulate real discrepancies  
- Network latency is simulated based on API tier
- All data includes proper timestamps and source attribution

### Without OpenAI API
The system gracefully falls back to rule-based analysis:
- Uses P/E ratios and sector analysis
- Adjusts confidence based on risk tolerance
- Provides detailed reasoning without AI enhancement
- All core features remain functional

---

## 📄 License

Part of the ProllyTree project. See main repository for license terms.

## ⚠️ Disclaimer

This is a demonstration system for educational purposes. Not for actual investment decisions.