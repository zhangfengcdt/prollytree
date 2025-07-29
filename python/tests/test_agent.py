#!/usr/bin/env python3
"""Test script for ProllyTree Agent Memory System Python bindings."""

import json
from prollytree import AgentMemorySystem, MemoryType
import tempfile
import os

def test_agent_memory_system():
    """Test the agent memory system functionality."""
    
    # Create a temporary directory for the memory store
    with tempfile.TemporaryDirectory() as tmpdir:
        print(f"📁 Creating memory system in: {tmpdir}")
        
        # Initialize the agent memory system
        memory_system = AgentMemorySystem(tmpdir, "test_agent_001")
        print("✅ Agent memory system initialized")
        
        # Test 1: Store conversation turns
        print("\n🧪 Test 1: Short-term memory (conversation)")
        conv_id1 = memory_system.store_conversation_turn(
            "thread_123",
            "user",
            "Hello, how are you?",
            {"source": "chat", "session": "morning"}
        )
        print(f"   Stored user message: {conv_id1}")
        
        conv_id2 = memory_system.store_conversation_turn(
            "thread_123",
            "assistant",
            "I'm doing well, thank you for asking! How can I help you today?"
        )
        print(f"   Stored assistant message: {conv_id2}")
        
        # Retrieve conversation history
        history = memory_system.get_conversation_history("thread_123", limit=10)
        print(f"   Retrieved {len(history)} messages from conversation history")
        for msg in history:
            print(f"   - {msg['created_at']}: {json.loads(msg['content'])}")
        
        # Test 2: Store semantic facts
        print("\n🧪 Test 2: Semantic memory (facts)")
        fact_id = memory_system.store_fact(
            "person",
            "john_doe",
            json.dumps({
                "age": 30,
                "occupation": "software engineer",
                "location": "San Francisco"
            }),
            0.95,  # confidence
            "user_input"
        )
        print(f"   Stored fact about john_doe: {fact_id}")
        
        # Retrieve facts
        facts = memory_system.get_entity_facts("person", "john_doe")
        print(f"   Retrieved {len(facts)} facts about john_doe")
        for fact in facts:
            print(f"   - Confidence: {fact['confidence']}, Source: {fact['source']}")
            print(f"     Facts: {fact['facts']}")
        
        # Test 3: Store procedures
        print("\n🧪 Test 3: Procedural memory")
        proc_id = memory_system.store_procedure(
            "task_management",
            "create_project",
            "How to create a new project in the system",
            [
                json.dumps({"step": 1, "action": "Define project name and description"}),
                json.dumps({"step": 2, "action": "Set project timeline and milestones"}),
                json.dumps({"step": 3, "action": "Assign team members and roles"}),
                json.dumps({"step": 4, "action": "Initialize project repository"})
            ],
            ["admin_access", "project_creation_permission"],
            priority=2
        )
        print(f"   Stored procedure: {proc_id}")
        
        # Get procedures by category
        procedures = memory_system.get_procedures_by_category("task_management")
        print(f"   Retrieved {len(procedures)} procedures in task_management category")
        for proc in procedures:
            print(f"   - {proc['id']}: {proc['content']}")
        
        # Test 4: Create checkpoint
        print("\n🧪 Test 4: Memory checkpoint")
        checkpoint_id = memory_system.checkpoint("Initial test data loaded")
        print(f"   Created checkpoint: {checkpoint_id}")
        
        # Test 5: Optimize memory
        print("\n🧪 Test 5: Memory optimization")
        optimization_report = memory_system.optimize()
        print("   Optimization report:")
        for key, value in optimization_report.items():
            print(f"   - {key}: {value}")
        
        print("\n✅ All tests completed successfully!")
        
        # Test MemoryType enum
        print("\n🧪 Test 6: MemoryType enum")
        print(f"   MemoryType.ShortTerm: {MemoryType.ShortTerm}")
        print(f"   MemoryType.Semantic: {MemoryType.Semantic}")
        print(f"   MemoryType.Episodic: {MemoryType.Episodic}")
        print(f"   MemoryType.Procedural: {MemoryType.Procedural}")


if __name__ == "__main__":
    test_agent_memory_system()