# ProllyTree Versioned Memory for AI Agents

This example demonstrates how to use ProllyTree as a versioned memory backend for AI agents using the Rig framework. It showcases time-travel debugging, memory branching, and complete audit trails for reproducible AI behavior.

## Features

- **Versioned Memory**: Every interaction creates a new version, enabling rollback to any previous state
- **Memory Types**: Short-term (conversation), long-term (facts), and episodic (experiences) memory
- **Memory Branching**: Experiment with different agent behaviors without affecting the main memory
- **Audit Trails**: Track every decision and memory access for debugging and compliance
- **Rig Integration**: Seamless integration with Rig's LLM completion API

## Prerequisites

1. Rust (latest stable version)
2. OpenAI API key
3. ProllyTree library (included as local dependency)

## Setup

1. Set your OpenAI API key:
   ```bash
   export OPENAI_API_KEY="your-api-key-here"
   ```
   
   Or create a `.env` file:
   ```
   OPENAI_API_KEY=your-api-key-here
   ```

2. Build the project:
   ```bash
   cd examples/rig_versioned_memory
   cargo build
   ```

## Running the Demo

### Interactive Chat Mode (Default)
```bash
cargo run
```

### Custom Storage Location
```bash
# Use custom storage directory
cargo run -- --storage ./my_agent_memory

# Use absolute path
cargo run -- --storage /tmp/agent_data

# Short form
cargo run -- -s ./custom_location
```

### Specific Demos

1. **Memory Learning & Rollback**:
   ```bash
   cargo run -- learning
   cargo run -- --storage ./custom_path learning
   ```
   Shows how the agent learns preferences and can rollback to previous states.

2. **Memory Branching**:
   ```bash
   cargo run -- branching
   ```
   Demonstrates experimental memory branches for safe behavior testing.

3. **Audit Trail**:
   ```bash
   cargo run -- audit
   ```
   Shows decision tracking and memory access logging.

4. **Episodic Learning**:
   ```bash
   cargo run -- episodic
   ```
   Demonstrates learning from experiences and outcomes.

5. **Run All Demos**:
   ```bash
   cargo run -- all
   ```

## Interactive Mode Commands

- `/quit` - Exit interactive mode
- `/new` - Start a new conversation (clears session memory)
- `/version` - Show current memory version
- `/learn <concept> <fact>` - Teach the agent a new fact

## Architecture

### Memory Types

1. **Short-term Memory**: Current conversation context
   - Stores user inputs and agent responses
   - Session-based storage
   - Used for maintaining conversation flow

2. **Long-term Memory**: Learned facts and preferences
   - Persistent across sessions
   - Concept-based organization
   - Access count tracking for relevance

3. **Episodic Memory**: Past experiences and outcomes
   - Records actions and their results
   - Includes reward signals for reinforcement
   - Used for learning from experience

### Key Components

- `VersionedMemoryStore`: Core storage backend using ProllyTree
- `VersionedAgent`: Rig-based agent with memory integration
- `Memory`: Data structure for storing memories with metadata
- `MemoryContext`: Retrieved memories for context building

## Example Usage

```rust
// Initialize agent with versioned memory
let mut agent = VersionedAgent::new(api_key, "./agent_memory").await?;

// Process a message (automatically stores in memory)
let (response, version) = agent.process_message("Hello!").await?;

// Learn a fact
agent.learn_fact("user_preference", "Likes concise responses").await?;

// Create a memory branch for experimentation
agent.create_memory_branch("experiment_1").await?;

// Rollback to a previous version
agent.rollback_to_version(&version).await?;
```

## Memory Storage

### Storage Location
By default, the agent stores memory in `./demo_agent_memory/`. You can customize this with:
```bash
cargo run -- --storage /path/to/your/storage
```

### Storage Structure
The storage directory contains:
- `.git/` - Git repository for version control
- `.git-prolly/` - ProllyTree metadata and configuration
- SQL database files with the following tables:
  - `short_term_memory`: Conversation history
  - `long_term_memory`: Learned facts and knowledge
  - `episodic_memory`: Experiences and outcomes
  - `memory_links`: Relationships between memories

### Storage Options
- **Relative paths**: `./my_memory`, `../shared_memory`
- **Absolute paths**: `/tmp/agent_data`, `/Users/name/agents/memory`
- **Different agents**: Use different storage paths for separate agent instances

## Benefits

1. **Reproducibility**: Replay agent behavior from any historical state
2. **Debugging**: Complete audit trail of decisions and memory access
3. **Experimentation**: Safe testing with memory branches
4. **Compliance**: Maintain required audit logs and data lineage
5. **Learning**: Agents can learn and improve from experiences

## Future Enhancements

- Embedding-based semantic search
- Distributed memory sharing between agents
- Memory compression for old conversations
- Advanced attention mechanisms for memory retrieval