# Agent Memory System

This document describes the Agent Memory System implemented for the ProllyTree project, which provides a comprehensive memory framework for AI agents with different types of memory and persistence.

## Overview

The Agent Memory System implements different types of memory inspired by human cognitive psychology:

- **Short-Term Memory**: Session/thread-scoped memories with automatic expiration
- **Semantic Memory**: Long-term facts and concepts about entities
- **Episodic Memory**: Past experiences and interactions
- **Procedural Memory**: Rules, procedures, and decision-making guidelines

## Architecture

### Core Components

1. **Types** (`src/agent/types.rs`)
   - Memory data structures and enums
   - Namespace organization for hierarchical memory
   - Query and filter types

2. **Traits** (`src/agent/traits.rs`)
   - Abstract interfaces for memory operations
   - Embedding generation and search capabilities
   - Lifecycle management interfaces

3. **Persistence** (`src/agent/simple_persistence.rs`)
   - Prolly tree-based in-memory persistence
   - Uses `ProllyTree<32, InMemoryNodeStorage<32>>` for robust storage
   - Thread-safe async operations with Arc<RwLock>

4. **Store** (`src/agent/store.rs`)
   - Base memory store implementation
   - Handles serialization/deserialization
   - Manages memory validation and access

5. **Memory Types**:
   - **Short-Term** (`src/agent/short_term.rs`): Conversation history, working memory
   - **Long-Term** (`src/agent/long_term.rs`): Semantic, episodic, and procedural stores

6. **Search** (`src/agent/search.rs`)
   - Memory search and retrieval capabilities
   - Mock embedding generation
   - Distance calculation utilities

7. **Lifecycle** (`src/agent/lifecycle.rs`)
   - Memory consolidation and archival
   - Cleanup and optimization
   - Event broadcasting

## Key Features

### Memory Namespace Organization

Memories are organized hierarchically using namespaces:
```
/memory/agents/{agent_id}/{memory_type}/{sub_namespace}
```

For example:
- `/memory/agents/agent_001/ShortTerm/thread_123`
- `/memory/agents/agent_001/Semantic/person/john_doe`
- `/memory/agents/agent_001/Episodic/2025-01`

### Memory Types and Use Cases

#### Short-Term Memory
- **Conversation History**: Tracks dialogue between user and agent
- **Working Memory**: Temporary state and calculations
- **Session Context**: Current session information
- **Automatic Expiration**: TTL-based cleanup

#### Semantic Memory
- **Entity Facts**: Store facts about people, places, concepts
- **Relationships**: Model connections between entities
- **Knowledge Base**: Persistent factual information

#### Episodic Memory
- **Interactions**: Record past conversations and outcomes
- **Experiences**: Learn from past events
- **Time-Indexed**: Organized by temporal buckets

#### Procedural Memory
- **Rules**: Conditional logic for decision making
- **Procedures**: Step-by-step instructions
- **Priority System**: Ordered execution of rules

### Search and Retrieval

- **Text Search**: Full-text search across memory content
- **Semantic Search**: Embedding-based similarity search (mock implementation)
- **Temporal Search**: Time-based memory retrieval
- **Tag-based Search**: Boolean logic with tags

### Memory Lifecycle Management

- **Consolidation**: Merge similar memories
- **Archival**: Move old memories to archive namespace
- **Pruning**: Remove low-value memories
- **Event System**: Track memory operations

## Usage Example

```rust
use prollytree::agent::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize memory system
    let mut memory_system = AgentMemorySystem::init(
        "/tmp/agent",
        "agent_001".to_string(),
        Some(Box::new(MockEmbeddingGenerator)),
    )?;

    // Store conversation
    memory_system.short_term.store_conversation_turn(
        "thread_123",
        "user",
        "Hello, how are you?",
        None,
    ).await?;

    // Store facts
    memory_system.semantic.store_fact(
        "person",
        "alice",
        json!({"role": "developer", "experience": "5 years"}),
        0.9,
        "user_input",
    ).await?;

    // Store procedures
    memory_system.procedural.store_procedure(
        "coding",
        "debug_rust_error",
        "How to debug Rust compilation errors",
        vec![
            json!({"step": 1, "action": "Read error message carefully"}),
            json!({"step": 2, "action": "Check variable types"}),
        ],
        None,
        5,
    ).await?;

    // Create checkpoint
    memory_system.checkpoint("Session complete").await?;

    Ok(())
}
```

## Implementation Status

### Completed ✅
- Core type definitions and interfaces
- **Prolly tree-based persistence layer** with `ProllyTree<32, InMemoryNodeStorage<32>>`
- All four memory types (Short-term, Semantic, Episodic, Procedural)
- Basic search functionality
- Memory lifecycle management
- Working demo example
- Thread-safe async operations
- Tree statistics and range queries
- Commit tracking with sequential IDs
- **Rig framework integration** with AI-powered responses and intelligent fallback
- **Memory-contextual AI** that uses stored knowledge for better responses

### Planned 🚧
- Real embedding generation (currently uses mock)
- Advanced semantic search
- Memory conflict resolution
- Performance optimizations
- Git-based prolly tree persistence for durability
- Multi-agent memory sharing through Rig

### Known Limitations
- Mock embedding generation
- Limited semantic search capabilities
- No conflict resolution for concurrent updates
- In-memory storage (data doesn't persist across restarts)

## Design Decisions

1. **Hierarchical Namespaces**: Enables efficient organization and querying
2. **Trait-based Architecture**: Allows for different storage backends
3. **Async/Await**: Modern Rust async patterns throughout
4. **Event System**: Enables monitoring and debugging
5. **Type Safety**: Strong typing for memory operations
6. **Extensible Design**: Easy to add new memory types or features

## Prolly Tree Integration Details

The memory system now uses prolly trees for storage with the following features:

### Storage Architecture
- **Tree Structure**: `ProllyTree<32, InMemoryNodeStorage<32>>`
- **Namespace Prefixes**: Organized hierarchically with agent ID and memory type
- **Thread Safety**: `Arc<RwLock<>>` for concurrent access
- **Commit Tracking**: Sequential commit IDs (prolly_commit_00000001, etc.)

### Advanced Features
- **Tree Statistics**: `tree_stats()` provides key count and size metrics
- **Range Queries**: `range_query()` for efficient range-based retrieval
- **Direct Tree Access**: `with_tree()` for advanced operations
- **Git-like Operations**: Branch, checkout, merge simulation for future git integration

### Performance Benefits
- **Balanced Tree Structure**: O(log n) operations for most queries
- **Content Addressing**: Efficient deduplication and integrity checking
- **Probabilistic Balancing**: Maintains performance under various workloads
- **Memory Efficient**: Shared storage for duplicate content

## Future Enhancements

1. **Git-based Persistence**: Replace in-memory with durable git-based storage
2. **Real Embedding Models**: Integration with actual embedding services
3. **Conflict Resolution**: Handle concurrent memory updates
4. **Performance Metrics**: Track memory system performance
5. **Memory Compression**: Efficient storage of large memories
6. **Distributed Memory**: Support for multi-agent memory sharing

## Running the Demos

### Basic Memory System Demo

To see the core memory system in action:

```bash
cargo run --example agent_memory_demo
```

This demonstrates:
- All four memory types with prolly tree storage
- Conversation tracking and fact storage
- Episode recording and procedure management
- Tree statistics and checkpoint creation
- System optimization and cleanup

### Rig Framework Integration Demo

To see the memory system integrated with Rig framework for AI-powered agents:

```bash
# With OpenAI API key (AI-powered responses)
OPENAI_API_KEY=your_key_here cargo run --example agent_rig_demo --features="git sql rig"

# Without API key (memory-based fallback responses)
cargo run --example agent_rig_demo --features="git sql rig"
```

This demonstrates:
- 🤖 **Rig framework integration** for AI-powered responses
- 🧠 **Memory-contextual AI** using conversation history and stored knowledge
- 🔄 **Intelligent fallback** to memory-based responses when AI is unavailable
- 📚 **Contextual learning** from interactions stored in episodic memory
- ⚙️ **Procedural knowledge updates** based on conversation patterns
- 📊 **Real-time memory statistics** and checkpoint management

## Testing

The memory system includes comprehensive unit tests for each component, including prolly tree persistence tests. Run tests with:

```bash
cargo test agent
```

This will run all tests including:
- Basic prolly tree operations (save, load, delete)
- Key listing and range queries
- Tree statistics and checkpoints
- Memory lifecycle operations

## Contributing

The memory system is designed to be modular and extensible. Key areas for contribution:

1. Better persistence backends
2. Advanced search algorithms
3. Memory optimization strategies
4. Integration with ML/AI frameworks
5. Performance benchmarks