# Financial Advisory AI with Agent Memory

A comprehensive financial advisory system demonstrating ProllyTree's capabilities - from basic versioned memory to sophisticated agent memory with behavioral learning.

## 🚀 Two Implementation Levels

### Original Financial Advisor
Secure, auditable AI financial advisor with git-like versioned memory and complete audit trails.

**Key Features:**
- Git-like versioned storage with branches and time travel
- AI-powered recommendations with fallback to rules
- Security monitoring with injection detection
- Multi-source data validation

### Enhanced Financial Advisor
Advanced system with full agent memory integration, multi-step workflows, and behavioral learning.

**Key Enhancements:**
- Complete 4-type agent memory system (semantic, episodic, procedural, short-term)
- Multi-step workflow orchestration with context
- Behavioral pattern learning and adaptation
- Deep personalization and compliance automation

---

## 📋 Quick Start

### Basic Financial Advisor

```bash
# Setup and run
mkdir -p /tmp/advisor && cd /tmp/advisor && git init
export OPENAI_API_KEY="your-api-key"  # optional
cargo run -- --storage /tmp/advisor/data advise
```

### Enhanced Financial Advisor

```bash
# Interactive session
cargo run -- enhanced --verbose

# Complete demonstration
cargo run --example enhanced_demo

# With AI integration
OPENAI_API_KEY="your-key" cargo run -- enhanced --verbose
```

---

## 🎯 What Each Version Demonstrates

### Original Version
- **Versioned Memory**: Git-like storage with temporal queries
- **Security First**: Input validation, anomaly detection, audit trails
- **AI Integration**: OpenAI-powered analysis with graceful fallbacks
- **Real-world Simulation**: Multi-source market data with realistic delays

### Enhanced Version
- **Agent Memory Architecture**: All four memory types working together
- **Complex Workflows**: Multi-step analysis with memory context
- **Behavioral Learning**: Client adaptation based on interaction history
- **Compliance Automation**: Procedural memory for regulatory rules
- **Personalization Engine**: Dynamic communication and risk adaptation

---

## 📚 Documentation

### Detailed Guides
- **[Original Guide](docs/original.md)** - Complete walkthrough of the basic advisor
- **[Enhanced Guide](docs/enhanced.md)** - Advanced features and memory integration
- **[Architecture](docs/architecture.md)** - System design and evolution comparison
- **[Memory Architecture](MEMORY_ARCHITECTURE.md)** - Agent memory system diagrams

### Key Commands

**Original Advisor:**
```bash
recommend AAPL    # Get AI recommendation
profile           # View/edit client profile
branch strategy1  # Create investment strategy branch
history           # View recommendation history
audit             # Complete operation audit
```

**Enhanced Advisor:**
```bash
client sarah_retired      # Set current client
recommend AAPL           # Full workflow recommendation
research GOOGL           # Deep market research
stats                    # Memory system statistics
optimize                 # Optimize memory performance
```

---

## 🛠️ Technical Architecture

### Memory System Evolution

**Original:** Simple versioned storage
```
User Input ──► Validation ──► AI Analysis ──► Git Storage
```

**Enhanced:** Multi-type agent memory
```
User Input ──► Workflow Engine ──► Analysis Modules ──► Agent Memory
                       │                    │              │
                   Market Research    ┌─────────────┐  ┌─────────┐
                   Risk Analysis      │ Semantic    │  │Episodic │
                   Compliance    ──►  │ Facts       │  │Episodes │
                   Personalization    │ Knowledge   │  │Learning │
                                     └─────────────┘  └─────────┘
                                           │              │
                                     ┌─────────────┐  ┌─────────┐
                                     │ Procedural  │  │Short-   │
                                     │ Workflows   │  │term     │
                                     │ Rules       │  │Context  │
                                     └─────────────┘  └─────────┘
```

### Key Technologies
- **ProllyTree Storage**: Content-addressed storage with Merkle proofs
- **Agent Memory System**: Advanced memory abstraction with 4 specialized types
- **Rig Framework**: AI-powered analysis and reasoning
- **Git Integration**: Native version control for auditability
- **Async Rust**: High-performance concurrent processing

---

## 🔧 Command Reference

```bash
# Build and test
cargo build --all
cargo test

# Run different modes
cargo run -- advise                    # Original interactive mode
cargo run -- enhanced                  # Enhanced interactive mode
cargo run -- visualize                 # Memory visualization
cargo run -- benchmark                 # Performance testing

# Examples
cargo run --example enhanced_demo      # Complete enhanced demonstration
cargo run --example memory_demo        # Memory system showcase

# Options
--storage <PATH>     # Custom storage directory
--verbose           # Detailed operation logging
```

---

## ⚠️ Important Notes

### Data & Security
- Uses realistic simulated market data (not real trading data)
- All security features are for demonstration purposes
- Complete audit trails for all operations
- Git-based storage ensures data integrity

### AI Integration
- Works with or without OpenAI API key
- Graceful fallback to rule-based analysis
- Memory-contextual prompts in enhanced version
- Configurable AI model selection

### Educational Value
This project demonstrates:
1. **Versioned Memory Systems** - Git-like storage with temporal queries
2. **Agent Memory Architecture** - Complete 4-type memory implementation
3. **Complex Workflow Orchestration** - Multi-step analysis with context
4. **Behavioral Learning** - Client adaptation and outcome-based improvement
5. **Security Best Practices** - Input validation and comprehensive auditing
6. **Real-World Application** - Practical financial advisory scenarios

---

## 📄 License & Disclaimer

Part of the ProllyTree project. See main repository for license terms.

**⚠️ This is a demonstration system for educational purposes. Not for actual investment decisions.**
