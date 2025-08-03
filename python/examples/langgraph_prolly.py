#!/usr/bin/env python3

# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""
LangGraph + ProllyTree Integration with Persistent Memory Workflow

This example demonstrates a complete LangGraph agent workflow using ProllyTree
as the versioned memory backend, featuring:
- Cross-thread persistent memory using scratchpad tools
- State graph workflow with LLM and tool nodes
- Git-like versioning of all memory operations
"""

import os
import tempfile
import json
import base64
from datetime import datetime
from typing import Any, Dict, List, Optional, Tuple, TypedDict, Annotated
from prollytree import VersionedKvStore

# LangGraph and LangChain imports
from langgraph.checkpoint.base import BaseCheckpointSaver, Checkpoint
from langgraph.store.base import BaseStore
from langgraph.graph import StateGraph, START, END
from langgraph.graph.message import add_messages
from langchain_core.messages import HumanMessage, AIMessage, ToolMessage
from langchain_core.tools import tool

# Import OpenAI LLM
from langchain_openai import ChatOpenAI


# State definition for the agent workflow
class ScratchpadState(TypedDict):
    """State definition for the scratchpad agent workflow."""
    messages: Annotated[List, add_messages]


# Mock LLM for fallback when OpenAI key is not available
class MockLLM:
    """Mock LLM that simulates tool calls for demonstration."""

    def invoke(self, messages):
        """Mock LLM that generates tool calls based on message content."""
        last_message = messages[-1]

        # If the last message is a ToolMessage, respond normally without tools
        if isinstance(last_message, ToolMessage):
            return AIMessage(content="Task completed successfully!")

        # Only generate tool calls for HumanMessage
        if isinstance(last_message, HumanMessage):
            content = last_message.content.lower()

            if "write" in content and "scratchpad" in content:
                # Extract content to write
                notes = content.split(":")[-1].strip() if ":" in content else content.replace("write to scratchpad", "").strip()
                return AIMessage(
                    content="I'll write that to the scratchpad for you.",
                    tool_calls=[{
                        "name": "WriteToScratchpad",
                        "args": {"notes": notes},
                        "id": f"call_{datetime.now().timestamp()}"
                    }]
                )
            elif "read" in content and "scratchpad" in content:
                return AIMessage(
                    content="Let me read from the scratchpad.",
                    tool_calls=[{
                        "name": "ReadFromScratchpad",
                        "args": {},
                        "id": f"call_{datetime.now().timestamp()}"
                    }]
                )
            elif "search" in content:
                # Mock search query
                query = content.replace("search", "").strip()
                return AIMessage(
                    content=f"I'll search for: {query}",
                    tool_calls=[{
                        "name": "tavily_search",
                        "args": {"query": query},
                        "id": f"call_{datetime.now().timestamp()}"
                    }]
                )

        return AIMessage(content="I understand. How can I help you with writing to or reading from the scratchpad, or searching for information?")


# Create LLM with tools (OpenAI or fallback to mock)
def create_llm_with_tools():
    """Create LLM bound with tools. Uses OpenAI if API key is available, otherwise falls back to mock."""
    import os

    # Check if OpenAI API key is available and valid
    api_key = os.getenv("OPENAI_API_KEY")
    if api_key and api_key != "mock" and not api_key.startswith("test") and api_key.startswith("sk-"):
        try:
            llm = ChatOpenAI(
                model="gpt-4o-mini",  # Use the more affordable mini model
                temperature=0.1,
                max_tokens=150
            )

            # Bind tools to the LLM
            tools = [WriteToScratchpad, ReadFromScratchpad, tavily_search]
            print("🤖 Using OpenAI GPT-4o-mini")
            return llm.bind_tools(tools)
        except Exception as e:
            print(f"⚠️  OpenAI initialization failed: {e}")
            print("🔄 Falling back to mock LLM")
    else:
        if api_key in ["mock", "test"] or (api_key and api_key.startswith("test")):
            print("🔄 Using mock LLM for demonstration (mock/test API key detected)")
        else:
            print("⚠️  No valid OpenAI API key found (OPENAI_API_KEY environment variable)")
            print("🔄 Using mock LLM for demonstration")

    # Fallback to mock LLM
    return MockLLM()


# Mock tools for demonstration
@tool
def WriteToScratchpad(notes: str) -> str:
    """Write notes to the persistent scratchpad."""
    class Result:
        def __init__(self, notes):
            self.notes = notes
    return Result(notes)


@tool
def ReadFromScratchpad() -> str:
    """Read notes from the persistent scratchpad."""
    return "Reading from scratchpad..."


@tool
def tavily_search(query: str) -> str:
    """Mock search tool that returns search results."""
    return f"Mock search results for: {query}. Found relevant information about the topic."


# Tools lookup dictionary
tools_by_name = {
    "WriteToScratchpad": WriteToScratchpad,
    "ReadFromScratchpad": ReadFromScratchpad,
    "tavily_search": tavily_search
}

# Global namespace for cross-thread memory
namespace = ("global", "scratchpad")


class ProllyVersionedMemoryStore(BaseStore):
    """ProllyTree-backed versioned memory store for LangGraph."""

    def __init__(self, store_path: str):
        """Initialize with a ProllyTree versioned KV store."""
        super().__init__()

        # Create a subdirectory for the store (not in git root)
        store_subdir = os.path.join(store_path, "data")
        os.makedirs(store_subdir, exist_ok=True)

        # Initialize git repo in parent if needed
        if not os.path.exists(os.path.join(store_path, '.git')):
            os.system(f"cd {store_path} && git init --quiet")

        self.kv_store = VersionedKvStore(store_subdir)
        print(f"✅ Initialized ProllyTree store at {store_subdir}")

    def batch(self, ops: List[Tuple]) -> List[Any]:
        """Batch operations - required by BaseStore."""
        results = []
        for op in ops:
            if len(op) == 2:
                method, args = op
                result = getattr(self, method)(*args)
                results.append(result)
        return results

    def abatch(self, ops: List[Tuple]) -> List[Any]:
        """Async batch operations - synchronous implementation."""
        return self.batch(ops)

    def _encode_value(self, value: Any) -> bytes:
        """Encode any value to bytes for storage."""
        if isinstance(value, bytes):
            return value
        elif isinstance(value, str):
            return value.encode('utf-8')
        else:
            # Use JSON with base64 for complex objects
            json_str = json.dumps(value, default=lambda x: {
                '__type': 'bytes',
                'data': base64.b64encode(x).decode() if isinstance(x, bytes) else str(x)
            })
            return json_str.encode('utf-8')

    def _decode_value(self, data: bytes) -> Any:
        """Decode bytes from storage back to original type."""
        if not data:
            return None

        try:
            # Try to decode as JSON first
            json_str = data.decode('utf-8')
            obj = json.loads(json_str)

            # Handle special types
            if isinstance(obj, dict) and '__type' in obj:
                if obj['__type'] == 'bytes':
                    return base64.b64decode(obj['data'])
            return obj
        except (json.JSONDecodeError, UnicodeDecodeError):
            # Return as string if not JSON
            try:
                return data.decode('utf-8')
            except UnicodeDecodeError:
                return data

    def search(self, namespace: tuple, *, filter: Optional[dict] = None, limit: int = 10) -> List[tuple]:
        """Search for items in a namespace."""
        prefix = ":".join(namespace) + ":"
        results = []

        # Use list_keys() to get all keys
        try:
            keys = self.kv_store.list_keys()
            count = 0
            for key in keys:
                if count >= limit:
                    break

                key_str = key.decode('utf-8')
                if key_str.startswith(prefix):
                    value = self.kv_store.get(key)
                    decoded_value = self._decode_value(value)

                    # Apply filter if provided
                    if filter:
                        # Simple filter matching
                        if not all(decoded_value.get(k) == v for k, v in filter.items() if isinstance(decoded_value, dict)):
                            continue

                    # Extract item key from full key
                    item_key = key_str[len(prefix):]
                    results.append((namespace, item_key, decoded_value))
                    count += 1
        except AttributeError:
            # If list_keys not available, return empty
            pass

        return results

    def put(self, namespace: tuple, key: str, value: dict) -> None:
        """Store a value in a namespace."""
        full_key = ":".join(namespace) + ":" + key
        key_bytes = full_key.encode('utf-8')
        value_bytes = self._encode_value(value)

        # Check if key exists to decide between insert/update
        existing = self.kv_store.get(key_bytes)
        if existing:
            self.kv_store.update(key_bytes, value_bytes)
            print(f"   📝 Updated: {full_key}")
        else:
            self.kv_store.insert(key_bytes, value_bytes)
            print(f"   ➕ Inserted: {full_key}")

    def get(self, namespace: tuple, key: str) -> Optional[dict]:
        """Retrieve a value from a namespace."""
        full_key = ":".join(namespace) + ":" + key
        key_bytes = full_key.encode('utf-8')
        data = self.kv_store.get(key_bytes)
        return self._decode_value(data) if data else None

    def delete(self, namespace: tuple, key: str) -> None:
        """Delete a key from a namespace."""
        full_key = ":".join(namespace) + ":" + key
        key_bytes = full_key.encode('utf-8')
        self.kv_store.delete(key_bytes)
        print(f"   ❌ Deleted: {full_key}")

    def commit(self, message: str) -> str:
        """Create a Git-like commit of current state."""
        commit_id = self.kv_store.commit(message)
        print(f"   💾 Committed: {commit_id[:8]} - {message}")
        return commit_id

    def history(self, limit: int = 10) -> List[Dict]:
        """Get commit history."""
        return self.kv_store.log()[-limit:]


class ProllyVersionedMemorySaver(BaseCheckpointSaver):
    """Checkpoint saver using ProllyTree for versioned persistence."""

    def __init__(self, store: ProllyVersionedMemoryStore):
        """Initialize with a ProllyTree store."""
        super().__init__()
        self.store = store

    def put(self, config: dict, checkpoint: Checkpoint, metadata: dict, new_versions: dict = None) -> dict:
        """Save a checkpoint."""
        thread_id = config.get("configurable", {}).get("thread_id", "default")
        checkpoint_ns = config.get("configurable", {}).get("checkpoint_ns", "")
        checkpoint_id = checkpoint.id if hasattr(checkpoint, 'id') else str(datetime.now().timestamp())

        # Convert checkpoint to dict if it's an object
        checkpoint_dict = checkpoint.__dict__ if hasattr(checkpoint, '__dict__') else dict(checkpoint)

        # Store checkpoint
        self.store.put(
            ("checkpoints", thread_id),
            checkpoint_id,
            checkpoint_dict
        )

        # Store metadata
        self.store.put(
            ("metadata", thread_id),
            checkpoint_id,
            metadata
        )

        # Commit with descriptive message
        self.store.commit(f"Checkpoint {checkpoint_id[:8]} for thread {thread_id}")

        return {
            "configurable": {
                "thread_id": thread_id,
                "checkpoint_ns": checkpoint_ns,
                "checkpoint_id": checkpoint_id
            }
        }

    def get_tuple(self, config: dict) -> Optional[tuple]:
        """Get a checkpoint tuple."""
        # Note: For this demo, we return None to let LangGraph start fresh each time
        # while still saving all checkpoints via put(). This demonstrates the
        # checkpointer interface without complex state restoration logic.
        # In production, you would implement full checkpoint retrieval here.
        return None

    def list(self, config: Optional[dict] = None, *, filter: Optional[dict] = None, before: Optional[dict] = None, limit: int = 10) -> List[Tuple[dict, Checkpoint, dict]]:
        """List checkpoints."""
        thread_id = config.get("configurable", {}).get("thread_id", "default") if config else None

        if thread_id:
            checkpoints = self.store.search(("checkpoints", thread_id), limit=limit)
        else:
            # Get all checkpoints
            checkpoints = []

        results = []
        for namespace, checkpoint_id, checkpoint_data in checkpoints:
            thread_id = namespace[1] if len(namespace) > 1 else "default"
            metadata = self.store.get(("metadata", thread_id), checkpoint_id) or {}
            config = {
                "configurable": {
                    "thread_id": thread_id,
                    "checkpoint_id": checkpoint_id
                }
            }
            if isinstance(checkpoint_data, dict):
                # Extract fields from checkpoint data for Checkpoint constructor
                checkpoint = Checkpoint(
                    id=checkpoint_data.get('id', checkpoint_id),
                    ts=checkpoint_data.get('ts', ''),
                    channel_values=checkpoint_data.get('channel_values', {}),
                    v=checkpoint_data.get('v', 1)
                )
            else:
                checkpoint = Checkpoint() if not checkpoint_data else checkpoint_data
            results.append((config, checkpoint, metadata))

        return results

    def put_writes(self, config: dict, writes: list, task_id: str) -> None:
        """Store pending writes for a checkpoint."""
        thread_id = config.get("configurable", {}).get("thread_id", "default")
        self.store.put(
            ("writes", thread_id),
            task_id,
            {"writes": writes, "timestamp": datetime.now().isoformat()}
        )
        self.store.commit(f"Pending writes for task {task_id[:8]}")


# Workflow node functions
def llm_call(state: ScratchpadState) -> dict:
    """LLM node that processes messages and potentially generates tool calls."""
    messages = state["messages"]
    llm = create_llm_with_tools()
    response = llm.invoke(messages)
    return {"messages": [response]}


def tool_node_persistent(state: ScratchpadState, store: ProllyVersionedMemoryStore) -> dict:
    """Execute tool calls with persistent memory storage across threads.

    This version of the tool node uses ProllyTree's persistent store to
    maintain scratchpad data across different conversation threads, enabling
    true long-term memory functionality.

    Args:
        state: Current conversation state with tool calls
        store: Persistent store for cross-thread memory

    Returns:
        Dictionary with tool results
    """
    result = []
    last_message = state["messages"][-1]

    if not hasattr(last_message, 'tool_calls') or not last_message.tool_calls:
        return {"messages": []}

    for tool_call in last_message.tool_calls:
        tool = tools_by_name[tool_call["name"]]
        observation = tool.invoke(tool_call["args"])

        if tool_call["name"] == "WriteToScratchpad":
            # Save to persistent store for cross-thread access
            notes = observation.notes
            result.append(ToolMessage(content=f"✅ Wrote to scratchpad: {notes}", tool_call_id=tool_call["id"]))
            store.put(namespace, "scratchpad", {"scratchpad": notes})
            store.commit(f"Scratchpad updated: {notes[:50]}...")

        elif tool_call["name"] == "ReadFromScratchpad":
            # Retrieve from persistent store across threads
            stored_data = store.get(namespace, "scratchpad")
            notes = stored_data["scratchpad"] if stored_data else "No notes found"
            result.append(ToolMessage(content=f"📖 Notes from scratchpad: {notes}", tool_call_id=tool_call["id"]))

        elif tool_call["name"] == "tavily_search":
            # Write search tool observation to messages
            result.append(ToolMessage(content=f"🔍 {observation}", tool_call_id=tool_call["id"]))

    return {"messages": result}


def should_continue(state: ScratchpadState) -> str:
    """Determine whether to continue to tool node or end."""
    last_message = state["messages"][-1]
    if hasattr(last_message, 'tool_calls') and last_message.tool_calls:
        return "tool_node"
    return END


def create_persistent_memory_workflow(store: ProllyVersionedMemoryStore, checkpointer: ProllyVersionedMemorySaver = None):
    """Create a LangGraph workflow with persistent memory using ProllyTree.

    Args:
        store: ProllyVersionedMemoryStore for cross-thread memory persistence
        checkpointer: Optional ProllyVersionedMemorySaver for conversation state checkpointing

    Returns:
        Compiled LangGraph agent with ProllyTree backend(s)
    """

    # Build persistent memory workflow
    agent_builder = StateGraph(ScratchpadState)

    # Add nodes
    agent_builder.add_node("llm_call", llm_call)
    agent_builder.add_node("tool_node", lambda state: tool_node_persistent(state, store))

    # Define workflow edges
    agent_builder.add_edge(START, "llm_call")
    agent_builder.add_conditional_edges("llm_call", should_continue, {"tool_node": "tool_node", END: END})
    agent_builder.add_edge("tool_node", "llm_call")

    # Compile with both store and checkpointer
    if checkpointer:
        agent = agent_builder.compile(store=store, checkpointer=checkpointer)
        print("✅ Using ProllyTree for both memory store AND checkpoint persistence")
    else:
        agent = agent_builder.compile(store=store)
        print("✅ Using ProllyTree for memory store only")

    return agent


def demonstrate_persistent_memory_workflow():
    """Demonstrate the complete persistent memory workflow."""
    print("\n=== LangGraph + ProllyTree Persistent Memory Workflow ===\n")

    with tempfile.TemporaryDirectory() as tmpdir:
        store_path = os.path.join(tmpdir, "langgraph_memory")

        # Initialize store and checkpointer
        store = ProllyVersionedMemoryStore(store_path)
        checkpointer = ProllyVersionedMemorySaver(store)

        # Create the agent workflow with both store and checkpointer
        agent = create_persistent_memory_workflow(store, checkpointer)

        print("🎯 === Thread 1: Writing to scratchpad ===")
        config1 = {"configurable": {"thread_id": "research_session_1"}}

        # Thread 1: Write research findings to scratchpad
        state1 = agent.invoke({
            "messages": [HumanMessage(content="Write to scratchpad: Commonwealth Fusion Systems raised $84M Series A in 2024 for fusion energy research")]
        }, config1)

        print("📝 Thread 1 Messages:")
        for msg in state1['messages']:
            if isinstance(msg, HumanMessage):
                print(f"   👤 Human: {msg.content}")
            elif isinstance(msg, AIMessage):
                print(f"   🤖 AI: {msg.content}")
            elif isinstance(msg, ToolMessage):
                print(f"   🛠️  Tool: {msg.content}")

        print("\n🔄 === Thread 2: Reading from scratchpad ===")
        config2 = {"configurable": {"thread_id": "analysis_session_2"}}

        # Thread 2: Read from scratchpad (different thread, same memory)
        state2 = agent.invoke({
            "messages": [HumanMessage(content="Read from scratchpad")]
        }, config2)

        print("📖 Thread 2 Messages:")
        for msg in state2['messages']:
            if isinstance(msg, HumanMessage):
                print(f"   👤 Human: {msg.content}")
            elif isinstance(msg, AIMessage):
                print(f"   🤖 AI: {msg.content}")
            elif isinstance(msg, ToolMessage):
                print(f"   🛠️  Tool: {msg.content}")

        print("\n🔄 === Thread 1: Continuing research ===")
        # Thread 1: Add more information
        state1_cont = agent.invoke({
            "messages": [HumanMessage(content="Write to scratchpad: Founded by MIT scientists, targeting 2032 for first fusion power plant")]
        }, config1)

        print("📝 Thread 1 Continuation:")
        for msg in state1_cont['messages'][-3:]:  # Show last 3 messages
            if isinstance(msg, HumanMessage):
                print(f"   👤 Human: {msg.content}")
            elif isinstance(msg, AIMessage):
                print(f"   🤖 AI: {msg.content}")
            elif isinstance(msg, ToolMessage):
                print(f"   🛠️  Tool: {msg.content}")

        print("\n🔄 === Thread 3: New user reading latest research ===")
        config3 = {"configurable": {"thread_id": "review_session_3"}}

        # Thread 3: Different user reading latest research
        state3 = agent.invoke({
            "messages": [HumanMessage(content="Read from scratchpad")]
        }, config3)

        print("📖 Thread 3 Messages:")
        for msg in state3['messages']:
            if isinstance(msg, HumanMessage):
                print(f"   👤 Human: {msg.content}")
            elif isinstance(msg, AIMessage):
                print(f"   🤖 AI: {msg.content}")
            elif isinstance(msg, ToolMessage):
                print(f"   🛠️  Tool: {msg.content}")

        print("\n📚 Git-like commit history:")
        for commit in store.history(10):
            timestamp = datetime.fromtimestamp(commit['timestamp'])
            print(f"   {commit['id'][:8]} - {commit['message']} ({timestamp.strftime('%H:%M:%S')})")

        print("\n🔄 ProllyVersionedMemorySaver API Demonstration:")
        # Demonstrate the checkpointer API directly
        from langgraph.checkpoint.base import Checkpoint

        # Create a sample checkpoint
        sample_checkpoint = Checkpoint(
            id="demo_checkpoint_1",
            ts="2024-01-01T00:00:00Z",
            channel_values={"messages": state1["messages"]},
            v=1
        )

        # Save checkpoint for Thread 1
        checkpointer.put(
            config=config1,
            checkpoint=sample_checkpoint,
            metadata={"thread": "research_session_1", "step": "final"}
        )
        print("   • ✅ Saved checkpoint for Thread 1 using ProllyVersionedMemorySaver")

        # Create another checkpoint for Thread 2
        sample_checkpoint2 = Checkpoint(
            id="demo_checkpoint_2",
            ts="2024-01-01T00:01:00Z",
            channel_values={"messages": state2["messages"]},
            v=1
        )

        checkpointer.put(
            config=config2,
            checkpoint=sample_checkpoint2,
            metadata={"thread": "analysis_session_2", "step": "final"}
        )
        print("   • ✅ Saved checkpoint for Thread 2 using ProllyVersionedMemorySaver")

        # List all checkpoints
        all_checkpoints = checkpointer.list(limit=10)
        print(f"   • 📚 Total checkpoints saved: {len(all_checkpoints)}")

        # Retrieve a specific checkpoint
        retrieved = checkpointer.get_tuple(config1)
        if retrieved:
            print("   • ✅ Successfully retrieved checkpoint from ProllyTree")
            print("   • All checkpoints are versioned with Git-like commits in ProllyTree")

        print("\n📊 Thread summary:")
        print(f"   • Thread 1: {len(state1['messages'])} initial messages + {len(state1_cont['messages'])} continuation messages")
        print(f"   • Thread 2: {len(state2['messages'])} messages")
        print(f"   • Thread 3: {len(state3['messages'])} messages")
        print("   • All threads share the same persistent scratchpad memory")
        print("   • Each thread's conversation state is checkpointed in ProllyTree")


def main():
    """Run the LangGraph + ProllyTree demo."""
    print("=" * 70)
    print("   LangGraph + ProllyTree: Versioned Memory for AI Agents")
    print("=" * 70)

    try:
        demonstrate_persistent_memory_workflow()

        print("\n" + "=" * 80)
        print("✅ Demo Complete! Key Features Demonstrated:")
        print("   • Cross-thread persistent memory using scratchpad tools")
        print("   • StateGraph workflow with LLM and tool nodes")
        print("   • ProllyTree as both BaseStore and BaseCheckpointSaver")
        print("   • Conversation state checkpointing with versioned persistence")
        print("   • Git-like commits for complete audit trail")
        print("   • Real LangGraph agent workflow with memory continuity")
        print("   • OpenAI integration with automatic fallback to mock LLM")
        print("=" * 80)

    except ImportError as e:
        print(f"\n❌ Error: {e}")
        print("\nPlease install LangGraph dependencies:")
        print("  pip install langgraph langchain-core")
        print("\nMake sure ProllyTree is also installed:")
        print("  cd ../.. && ./python/build_python.sh --install")


if __name__ == "__main__":
    main()
