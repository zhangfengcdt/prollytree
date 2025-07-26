# Financial Advisory AI with Versioned Memory

A secure, auditable AI financial advisor demonstrating ProllyTree's versioned memory capabilities with git-like branching, temporal queries, and complete audit trails.

## Quick Start

```bash
# 1. Setup storage with git
mkdir -p /tmp/advisor && cd /tmp/advisor && git init

# 2. Set OpenAI API key (optional, for AI reasoning)
export OPENAI_API_KEY="your-api-key"

# 3. Run the advisor
cargo run -- --storage /tmp/advisor/data advise
```

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

Without OpenAI API (fallback):
```
📊 Recommendation Generated  
Symbol: AAPL
Action: HOLD
Confidence: 52.0%
Reasoning: AAPL shows strong fundamentals with a P/E ratio of 28.4...
```

### 5. Memory Storage
Every recommendation is stored with:
- Full audit trail
- Validation results
- Cross-reference hashes
- Git commit for time-travel queries

## Key Commands

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

## Example Workflow

```bash
# Start with conservative strategy
🏦 [main] risk conservative
🏦 [main] recommend MSFT
📊 Action: HOLD, Confidence: 45% (conservative approach)

# Try aggressive strategy on new branch  
🏦 [main] branch aggressive-growth
🏦 [aggressive-growth] risk aggressive  
🏦 [aggressive-growth] recommend MSFT
📊 Action: BUY, Confidence: 78% (growth opportunity identified)

# Compare branches
🏦 [aggressive-growth] visualize
├── ◆ main (conservative MSFT: HOLD)
└── ● aggressive-growth (current) 
    └── Aggressive MSFT: BUY recommendation

# Time travel to see past recommendations
🏦 [aggressive-growth] switch main
🏦 [main] history abc1234
📊 Viewing recommendations as of 2024-01-15...
```

## Architecture Highlights

- **ProllyTree Storage**: Content-addressed storage with Merkle proofs
- **Git Integration**: Native git operations for versioning
- **Multi-Table Schema**: Separate tables for recommendations, market data, profiles
- **Async Processing**: Concurrent data fetching and validation
- **Security Layers**: Input sanitization, anomaly detection, audit logging

## Advanced Options

```bash
# Command line interface
cargo run -- [OPTIONS] <COMMAND>

Commands:
  advise     Interactive advisory session  
  visualize  Memory tree visualization
  attack     Security testing suite
  benchmark  Performance measurements
  memory     Git operations interface
  audit      Compliance reporting

Options:
  -s, --storage <PATH>  Storage directory [default: ./advisor_memory/data]
  -v, --verbose         Show detailed operations
```

## Technical Notes

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

### Security Features
- **Input Validation**: Regex patterns block SQL injection
- **Anomaly Detection**: Statistical analysis of data patterns
- **Rate Limiting**: Prevents abuse and DOS attempts
- **Audit Trail**: Cryptographically signed operation log

## License

Part of the ProllyTree project. See main repository for license terms.

## Disclaimer

This is a demonstration system for educational purposes. Not for actual investment decisions.