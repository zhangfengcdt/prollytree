# Financial Advisory AI with Versioned Memory

A demonstration of an AI-powered financial advisory system using ProllyTree for versioned memory management. This example showcases how to build a secure, auditable AI agent that maintains consistent memory across time and can handle complex financial recommendations with full traceability.

## Features

- 🤖 **AI-Powered Recommendations**: Uses OpenAI's API to generate intelligent investment advice
- 📊 **Multi-Source Data Validation**: Cross-validates market data from multiple sources
- 🔒 **Security Monitoring**: Detects and prevents injection attacks and anomalies
- 📚 **Versioned Memory**: Uses ProllyTree to maintain git-like versioned storage of all data
- 🕐 **Temporal Queries**: Query recommendations and data as they existed at any point in time
- 🌿 **Smart Branch Management**: Git-style branch operations with validation and external tool sync
- 🏦 **Real-time UI**: Live branch display that updates with external git operations
- 📝 **Audit Trail**: Complete audit logs for compliance and debugging
- 🎯 **Risk-Aware**: Adapts recommendations based on client risk tolerance
- 👤 **Persistent Profiles**: Client profiles automatically saved per branch

## Prerequisites

- Rust (latest stable version)
- Git (for memory versioning)
- OpenAI API key (optional, for AI-enhanced reasoning)

## Quick Start

### 1. Initialize Storage Directory

First, create a directory with git repository for the advisor's memory:

```bash
# Create a directory for the advisor's memory
mkdir -p /tmp/advisor
cd /tmp/advisor

# Initialize git repository (required for versioned memory)
git init

# Return to the project directory
cd /path/to/prollytree
```

### 2. Set Environment Variables (Optional)

For AI-enhanced recommendations, set your OpenAI API key:

```bash
export OPENAI_API_KEY="your-api-key-here"
```

### 3. Run the Financial Advisor

```bash
# Basic usage with temporary storage
cargo run --example financial_advisor -- --storage /tmp/advisor/data advise

# Or use the shorter form
cargo run -- --storage /tmp/advisor/data advise
```

## Usage

### Interactive Commands

Once the advisor is running, you can use these commands:

#### Core Operations
- `recommend <SYMBOL>` - Get AI-powered recommendation for a stock symbol (e.g., `recommend AAPL`)
- `profile` - Show current client profile
- `risk <LEVEL>` - Set risk tolerance (`conservative`, `moderate`, or `aggressive`)

#### History and Analysis
- `history` - Show recent recommendations
- `history <commit>` - Show recommendations at a specific git commit
- `history --branch <name>` - Show recommendations from a specific branch
- `memory` - Show memory system status and statistics
- `audit` - Show complete audit trail

#### Branch Management
- `branch <NAME>` - Create and switch to a new memory branch
- `switch <NAME>` - Switch to an existing branch
- `list-branches` - Show all available branches with visual indicators
- `branch-info` - List branches in git-style format (like `git branch`)

#### Advanced Features
- `visualize` - Show memory tree visualization
- `test-inject <TEXT>` - Test security monitoring (try malicious inputs)

#### Other Commands
- `help` - Show all available commands
- `exit` or `quit` - Exit the advisor

### Example Session

```bash
🏦 [main] recommend AAPL
📊 Recommendation Generated
Symbol: AAPL
Action: BUY
Confidence: 52.0%
Reasoning: Analysis of AAPL at $177.89 with P/E ratio 28.4...

🏦 [main] risk aggressive
✅ Risk tolerance set to: Aggressive

🏦 [main] recommend AAPL
📊 Recommendation Generated
Symbol: AAPL
Action: BUY
Confidence: 60.0%
(Notice higher confidence for aggressive risk tolerance)

🏦 [main] branch test-strategy
🌿 Creating memory branch: test-strategy
✅ Branch 'test-strategy' created successfully
🔀 Switched to branch 'test-strategy'

🏦 [test-strategy] recommend MSFT
📊 Recommendation Generated
Symbol: MSFT
Action: BUY
Confidence: 58.0%

🏦 [test-strategy] list-branches
🌳 Available Branches
━━━━━━━━━━━━━━━━━━━━━━━━━
  ○ main
  ● test-strategy (current)

🏦 [test-strategy] switch main
🔀 Switching to branch: main
✅ Switched to branch 'main'

🏦 [main] history
📜 Recent Recommendations
📊 Recommendation #1
  Symbol: AAPL
  Action: BUY
  Confidence: 60.0%
  ...
📊 Recommendation #2
  Symbol: AAPL
  Action: BUY
  Confidence: 52.0%
  ...

🏦 [main] memory
🧠 Memory Status
✅ Memory validation: ACTIVE
🛡️ Security monitoring: ENABLED
📝 Audit trail: ENABLED
🌿 Current branch: main
📊 Total commits: 15
💡 Recommendations: 2
```

## Command Line Options

```bash
cargo run -- [OPTIONS] <COMMAND>

Commands:
  advise     Start interactive advisory session
  visualize  Visualize memory evolution  
  attack     Run attack simulations
  benchmark  Run performance benchmarks
  memory     Git memory operations
  examples   Show integration examples
  audit      Audit memory for compliance

Options:
  -s, --storage <PATH>  Path to store agent memory [default: ./advisor_memory/data]
  -h, --help           Print help
```

## Architecture

### Memory System
- **ProllyTree Storage**: Git-like versioned storage for all data
- **Multi-table Schema**: Separate tables for recommendations, market data, client profiles
- **Cross-validation**: Data integrity through hash validation and cross-references
- **Temporal Queries**: Query data as it existed at any commit or branch

### Security Features
- **Input Sanitization**: Prevents SQL injection and other attacks
- **Anomaly Detection**: Monitors for suspicious patterns in data
- **Attack Simulation**: Built-in testing for security vulnerabilities
- **Audit Logging**: Complete trail of all operations

### AI Integration
- **Market Analysis**: Real-time analysis of market conditions
- **Risk Assessment**: Adapts to client risk tolerance
- **Reasoning Generation**: Explains the logic behind recommendations
- **Multi-source Validation**: Cross-checks data from multiple financial sources

## Advanced Usage

### Branch Management

Create and manage branches for different scenarios:

```bash
# Create and switch to a new branch
🏦 [main] branch conservative-strategy
🌿 Creating memory branch: conservative-strategy
✅ Branch 'conservative-strategy' created successfully
🔀 Switched to branch 'conservative-strategy'

🏦 [conservative-strategy] risk conservative
✅ Risk tolerance set to: Conservative

🏦 [conservative-strategy] recommend MSFT
# Generate recommendations for conservative strategy

# List all available branches
🏦 [conservative-strategy] list-branches
🌳 Available Branches
━━━━━━━━━━━━━━━━━━━━━━━━━
  ○ main
  ● conservative-strategy (current)

# Switch back to main branch
🏦 [conservative-strategy] switch main
🔀 Switching to branch: main
✅ Switched to branch 'main'

🏦 [main] history --branch conservative-strategy
# Compare recommendations from different branch

# Git-style branch listing
🏦 [main] branch-info
* main
  conservative-strategy
```

#### Branch Validation

The system prevents common branching mistakes:

```bash
# Try to create existing branch
🏦 [main] branch main
⚠️ Branch 'main' already exists!
💡 Use 'switch main' to switch to the existing branch

# Try to switch to non-existent branch  
🏦 [main] switch nonexistent
❌ Branch 'nonexistent' does not exist!
💡 Use 'branch nonexistent' to create a new branch
```

### Temporal Analysis

Analyze how recommendations changed over time:

```bash
# Get commit history
🏦> memory

# Query specific time points  
🏦> history abc1234  # Recommendations at specific commit
🏦> history def5678  # Compare with different commit
```

### Security Testing

Test the system's security:

```bash
🏦> test-inject "'; DROP TABLE recommendations; --"
🛡️ Security Alert: Potential SQL injection detected and blocked

🏦> test-inject "unusual market manipulation data"
🚨 Anomaly detected in data pattern
```

## Troubleshooting## License

This example is part of the ProllyTree project and follows the same license terms.

## Contributing

Contributions are welcome! Please see the main project's contributing guidelines.

## Disclaimer

This is a demonstration system for educational purposes. Do not use for actual financial decisions without proper validation and compliance review.