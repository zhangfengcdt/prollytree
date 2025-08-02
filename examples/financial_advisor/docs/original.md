# Original Financial Advisor - Detailed Guide

A secure, auditable AI financial advisor with git-like versioned memory, temporal queries, and complete audit trails.

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

## Command Reference

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

## Quick Start

```bash
# 1. Setup storage with git
mkdir -p /tmp/advisor && cd /tmp/advisor && git init

# 2. Set OpenAI API key (optional, for AI reasoning)
export OPENAI_API_KEY="your-api-key"

# 3. Run the basic advisor
cargo run -- --storage /tmp/advisor/data advise
```

## Security Features
- **Input Validation**: Regex patterns block SQL injection
- **Anomaly Detection**: Statistical analysis of data patterns
- **Rate Limiting**: Prevents abuse and DOS attempts
- **Audit Trail**: Cryptographically signed operation log

## Data Simulation Notes
The system uses realistic market data simulation:
- Popular stocks (AAPL, MSFT, GOOGL, etc.) have accurate characteristics
- Prices vary ±1% between sources to simulate real discrepancies
- Network latency is simulated based on API tier
- All data includes proper timestamps and source attribution

## Without OpenAI API
The system gracefully falls back to rule-based analysis:
- Uses P/E ratios and sector analysis
- Adjusts confidence based on risk tolerance
- Provides detailed reasoning without AI enhancement
- All core features remain functional

## Configuration

### Environment Variables
```bash
export OPENAI_API_KEY="your-api-key"          # Enable AI-powered analysis
export ADVISOR_STORAGE="/path/to/storage"     # Custom storage location
export ADVISOR_VERBOSE="true"                 # Enable verbose logging
```

### Customization Points
- **Security Rules**: Modify injection detection patterns
- **Data Sources**: Add new simulated market data providers
- **Validation Logic**: Customize cross-source validation rules
- **Branch Strategies**: Create custom investment strategy branches
