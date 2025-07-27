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

1. **Types** (`src/agent_memory/types.rs`)
   - Memory data structures and enums
   - Namespace organization for hierarchical memory
   - Query and filter types

2. **Traits** (`src/agent_memory/traits.rs`)
   - Abstract interfaces for memory operations
   - Embedding generation and search capabilities
   - Lifecycle management interfaces

3. **Persistence** (`src/agent_memory/simple_persistence.rs`)
   - Simple in-memory persistence for demonstration
   - Designed to be replaced with prolly tree persistence
   - Thread-safe async operations

4. **Store** (`src/agent_memory/store.rs`)
   - Base memory store implementation
   - Handles serialization/deserialization
   - Manages memory validation and access

5. **Memory Types**:
   - **Short-Term** (`src/agent_memory/short_term.rs`): Conversation history, working memory
   - **Long-Term** (`src/agent_memory/long_term.rs`): Semantic, episodic, and procedural stores

6. **Search** (`src/agent_memory/search.rs`)
   - Memory search and retrieval capabilities
   - Mock embedding generation
   - Distance calculation utilities

7. **Lifecycle** (`src/agent_memory/lifecycle.rs`)
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
use prollytree::agent_memory::*;

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
- Simple persistence layer
- All four memory types (Short-term, Semantic, Episodic, Procedural)
- Basic search functionality
- Memory lifecycle management
- Working demo example

### Planned 🚧
- Full prolly tree persistence integration (blocked by Send/Sync issues)
- Real embedding generation (currently uses mock)
- Advanced semantic search
- Memory conflict resolution
- Performance optimizations

### Known Limitations
- Uses simple in-memory persistence instead of prolly tree
- Mock embedding generation
- Limited semantic search capabilities
- No conflict resolution for concurrent updates

## Design Decisions

1. **Hierarchical Namespaces**: Enables efficient organization and querying
2. **Trait-based Architecture**: Allows for different storage backends
3. **Async/Await**: Modern Rust async patterns throughout
4. **Event System**: Enables monitoring and debugging
5. **Type Safety**: Strong typing for memory operations
6. **Extensible Design**: Easy to add new memory types or features

## Future Enhancements

1. **True Prolly Tree Integration**: Once Send/Sync issues are resolved
2. **Real Embedding Models**: Integration with actual embedding services
3. **Conflict Resolution**: Handle concurrent memory updates
4. **Performance Metrics**: Track memory system performance
5. **Memory Compression**: Efficient storage of large memories
6. **Distributed Memory**: Support for multi-agent memory sharing

## Running the Demo

To see the memory system in action:

```bash
cargo run --example agent_memory_demo
```

This demonstrates all four memory types, search capabilities, and system operations.

## Testing

The memory system includes comprehensive unit tests for each component. Run tests with:

```bash
cargo test agent
```

## Contributing

The memory system is designed to be modular and extensible. Key areas for contribution:

1. Better persistence backends
2. Advanced search algorithms
3. Memory optimization strategies
4. Integration with ML/AI frameworks
5. Performance benchmarks