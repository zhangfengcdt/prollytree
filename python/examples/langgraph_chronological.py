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
Complete LangGraph + ProllyTree Integration: Production-Ready Memory System

This example demonstrates a production-ready memory system that combines:
1. Structured memory extraction using LangGraph's patterns
2. Vector embeddings for semantic search (mock or real)
3. ProllyTree for git-like version control
4. Entity tracking with complete history
5. Hybrid retrieval combining semantic and versioned data

Architecture:
┌─────────────────────────────────────────────────────────────────────────┐
│                     Production Memory System Architecture                │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                          Memory Processing Pipeline                      │
│                                                                         │
│  User Message → Extract Memories → Generate Embeddings → Store Both     │
│                         ↓                    ↓              ↓          │
│                  (structured data)    (vector search)  (version control)│
└─────────────────────────────────────────────────────────────────────────┘

Key Components:
• MemoryConfig: Defines extraction schemas (patch/insert modes)
• HybridMemoryService: Combines vector and versioned storage
• MemoryGraph: LangGraph workflow for memory processing
• Entity tracking with complete audit trail
"""

import os
import sys
import json
import uuid
import tempfile
import hashlib
import subprocess
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional, Tuple, TypedDict, Annotated, Literal
from dataclasses import dataclass, asdict
import numpy as np

# ProllyTree imports
from prollytree import VersionedKvStore

# LangGraph and LangChain imports
from langgraph.graph import StateGraph, START, END
from langgraph.graph.message import add_messages
from langgraph.constants import Send
from langchain_core.messages import HumanMessage, AIMessage, SystemMessage
from langchain_core.pydantic_v1 import BaseModel, Field
from typing_extensions import TypedDict

# For embeddings
try:
    from langchain_openai import OpenAIEmbeddings
    OPENAI_AVAILABLE = True
except ImportError:
    OPENAI_AVAILABLE = False

# For diagram visualization
try:
    from IPython.display import display, Image
    IPYTHON_AVAILABLE = True
except ImportError:
    IPYTHON_AVAILABLE = False


# ============================================================================
# Schema Definitions (following langgraph-memory patterns)
# ============================================================================

class UserProfile(BaseModel):
    """User profile schema for patch-based memory."""
    name: Optional[str] = Field(None, description="User's name")
    preferences: Dict[str, Any] = Field(default_factory=dict, description="User preferences")
    interests: List[str] = Field(default_factory=list, description="User interests")
    context: Dict[str, Any] = Field(default_factory=dict, description="Additional context")


class ConversationEvent(BaseModel):
    """Event schema for insert-based memory."""
    event_type: str = Field(..., description="Type of event (query, fact, preference)")
    content: str = Field(..., description="Event content")
    entities: List[str] = Field(default_factory=list, description="Referenced entities")
    timestamp: str = Field(default_factory=lambda: datetime.now(tz=timezone.utc).isoformat())
    metadata: Dict[str, Any] = Field(default_factory=dict, description="Additional metadata")


class FunctionSchema(TypedDict):
    """Function schema for memory extraction."""
    name: str
    description: str
    parameters: dict


class MemoryConfig(TypedDict, total=False):
    """Configuration for memory extraction."""
    function: FunctionSchema
    system_prompt: Optional[str]
    update_mode: Literal["patch", "insert"]


# ============================================================================
# State Definitions
# ============================================================================

class MemoryState(TypedDict):
    """State for memory processing workflow."""
    messages: Annotated[List, add_messages]
    user_id: str
    thread_id: str
    extracted_memories: List[BaseModel]
    semantic_results: List[Tuple[str, float]]
    entity_contexts: Dict[str, Any]


class SingleExtractorState(MemoryState):
    """State for single memory extractor."""
    function_name: str
    responses: List[BaseModel]
    user_state: Optional[Dict[str, Any]]


# ============================================================================
# Mock Components (replace with real implementations)
# ============================================================================

class MockEmbeddings:
    """Mock embeddings for demonstration."""

    def embed_text(self, text: str) -> List[float]:
        """Generate a mock embedding vector."""
        hash_obj = hashlib.md5(text.encode())
        hash_hex = hash_obj.hexdigest()
        np.random.seed(int(hash_hex[:8], 16))
        return np.random.randn(384).tolist()

    def embed_documents(self, texts: List[str]) -> List[List[float]]:
        """Embed multiple documents."""
        return [self.embed_text(text) for text in texts]

    def similarity(self, vec1: List[float], vec2: List[float]) -> float:
        """Calculate cosine similarity."""
        vec1 = np.array(vec1)
        vec2 = np.array(vec2)
        return float(np.dot(vec1, vec2) / (np.linalg.norm(vec1) * np.linalg.norm(vec2)))


class MockLLM:
    """Mock LLM for memory extraction."""

    def extract_memories(self, messages: List, schema: FunctionSchema) -> List[BaseModel]:
        """Extract memories based on schema."""
        # Mock extraction logic
        results = []

        for msg in messages:
            if isinstance(msg, HumanMessage):
                content = msg.content.lower()

                if schema["name"] == "UserProfile":
                    # Extract user profile information
                    profile = UserProfile(
                        name="User" if "i" in content else None,
                        preferences={"communication": "detailed"} if "prefer" in content else {},
                        interests=["technology"] if "tech" in content else [],
                        context={"last_topic": content[:50]}
                    )
                    results.append(profile)

                elif schema["name"] == "ConversationEvent":
                    # Extract conversation events
                    event_type = "query" if "?" in content else "fact"
                    entities = []

                    # Simple entity extraction
                    if "product" in content:
                        entities.append("product:general")
                    if "user" in content or "customer" in content:
                        entities.append("user:mentioned")

                    event = ConversationEvent(
                        event_type=event_type,
                        content=content[:200],
                        entities=entities
                    )
                    results.append(event)

        return results


# ============================================================================
# Hybrid Memory Service
# ============================================================================

class HybridMemoryService:
    """
    Production-ready memory service combining vector search and version control.

    This service implements the patterns from langgraph-memory with ProllyTree backend.
    """

    def __init__(self, store_path: str):
        """Initialize the hybrid memory service."""
        # Create subdirectory for ProllyTree
        self.store_path = store_path
        store_subdir = os.path.join(store_path, "memory_data")
        os.makedirs(store_subdir, exist_ok=True)

        # Initialize git repo if needed
        if not os.path.exists(os.path.join(store_path, '.git')):
            subprocess.run(["git", "init", "--quiet"], cwd=store_path, check=True)
            subprocess.run(["git", "config", "user.name", "Memory Service"], cwd=store_path, check=True)
            subprocess.run(["git", "config", "user.email", "memory@example.com"], cwd=store_path, check=True)

        # Initialize ProllyTree store
        self.kv_store = VersionedKvStore(store_subdir)

        # Initialize embeddings (use OpenAI if available)
        api_key = os.getenv("OPENAI_API_KEY", "")
        if OPENAI_AVAILABLE and api_key and api_key.startswith("sk-") and not api_key.startswith(("mock", "test")):
            try:
                # Test the embeddings with a simple call
                test_embeddings = OpenAIEmbeddings(model="text-embedding-3-small")
                test_embeddings.embed_query("test")  # Test the connection
                self.embeddings = test_embeddings
                print("✅ Using OpenAI embeddings (text-embedding-3-small)")
            except Exception as e:
                print(f"⚠️  OpenAI embeddings failed: {e}")
                print("🔄 Falling back to mock embeddings")
                self.embeddings = MockEmbeddings()
        else:
            self.embeddings = MockEmbeddings()
            if api_key in ["mock", "test"] or api_key.startswith("test"):
                print("🔄 Using mock embeddings (mock/test API key detected)")
            else:
                print("🔄 Using mock embeddings (no valid OpenAI API key)")

        # Initialize LLM
        self.llm = MockLLM()

        # In-memory vector store (replace with Pinecone/Weaviate in production)
        self.vector_store: Dict[str, Tuple[List[float], Dict[str, Any]]] = {}

        # Memory configurations
        self.memory_configs = self._create_memory_configs()

        print(f"✅ Initialized hybrid memory service at {store_subdir}")

    def _create_memory_configs(self) -> Dict[str, MemoryConfig]:
        """Create memory extraction configurations."""
        return {
            "user_profile": MemoryConfig(
                function=FunctionSchema(
                    name="UserProfile",
                    description="Extract user profile information",
                    parameters={
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"},
                            "preferences": {"type": "object"},
                            "interests": {"type": "array", "items": {"type": "string"}}
                        }
                    }
                ),
                system_prompt="Extract user profile information from the conversation",
                update_mode="patch"
            ),
            "conversation_events": MemoryConfig(
                function=FunctionSchema(
                    name="ConversationEvent",
                    description="Extract conversation events",
                    parameters={
                        "type": "object",
                        "properties": {
                            "event_type": {"type": "string"},
                            "content": {"type": "string"},
                            "entities": {"type": "array", "items": {"type": "string"}}
                        },
                        "required": ["event_type", "content"]
                    }
                ),
                system_prompt="Extract important events from the conversation",
                update_mode="insert"
            )
        }

    def extract_and_store(self, messages: List, user_id: str, thread_id: str) -> Dict[str, List[BaseModel]]:
        """Extract and store memories from messages."""
        extracted = {}

        for config_name, config in self.memory_configs.items():
            # Extract memories using schema
            memories = self.llm.extract_memories(messages, config["function"])
            extracted[config_name] = memories

            if config["update_mode"] == "patch":
                # Patch mode: update single document
                self._store_patch_memory(memories, user_id, config_name)
            else:
                # Insert mode: add new documents
                self._store_insert_memories(memories, user_id, config_name)

        return extracted

    def _store_patch_memory(self, memories: List[BaseModel], user_id: str, config_name: str):
        """Store memories in patch mode (single document update)."""
        if not memories:
            return

        # Use the last memory as the current state
        memory = memories[-1]
        key = f"patch:{user_id}:{config_name}".encode('utf-8')
        value = memory.json().encode('utf-8')

        # Check if exists
        existing = self.kv_store.get(key)
        if existing:
            self.kv_store.update(key, value)
            print(f"   📝 Updated {config_name} for {user_id}")
        else:
            self.kv_store.insert(key, value)
            print(f"   ➕ Created {config_name} for {user_id}")

        # Store in vector store
        memory_text = memory.json()
        if hasattr(self.embeddings, 'embed_query'):
            # OpenAI embeddings
            embedding = self.embeddings.embed_query(memory_text)
        else:
            # Mock embeddings
            embedding = self.embeddings.embed_text(memory_text)
        self.vector_store[f"patch:{user_id}:{config_name}"] = (embedding, memory.dict())

        # Commit
        self.kv_store.commit(f"Updated {config_name} for user {user_id}")

    def _store_insert_memories(self, memories: List[BaseModel], user_id: str, config_name: str):
        """Store memories in insert mode (append new documents)."""
        for memory in memories:
            # Generate unique ID
            memory_id = str(uuid.uuid4())[:8]
            key = f"insert:{user_id}:{config_name}:{memory_id}".encode('utf-8')
            value = memory.json().encode('utf-8')

            # Insert into ProllyTree
            self.kv_store.insert(key, value)
            print(f"   ➕ Inserted {config_name} event {memory_id}")

            # Store in vector store
            memory_text = memory.json()
            if hasattr(self.embeddings, 'embed_query'):
                # OpenAI embeddings
                embedding = self.embeddings.embed_query(memory_text)
            else:
                # Mock embeddings
                embedding = self.embeddings.embed_text(memory_text)
            self.vector_store[f"insert:{user_id}:{config_name}:{memory_id}"] = (embedding, memory.dict())

        if memories:
            self.kv_store.commit(f"Added {len(memories)} {config_name} events for user {user_id}")

    def semantic_search(self, query: str, user_id: Optional[str] = None, top_k: int = 5) -> List[Tuple[str, float, Dict]]:
        """Search for semantically similar memories."""
        # Generate query embedding
        if hasattr(self.embeddings, 'embed_query'):
            # OpenAI embeddings
            query_embedding = self.embeddings.embed_query(query)
        else:
            # Mock embeddings
            query_embedding = self.embeddings.embed_text(query)

        results = []
        for key, (embedding, data) in self.vector_store.items():
            # Filter by user if specified
            if user_id and f":{user_id}:" not in key:
                continue

            # Calculate similarity
            if hasattr(self.embeddings, 'similarity'):
                # Mock embeddings have similarity method
                similarity = self.embeddings.similarity(query_embedding, embedding)
            else:
                # Calculate cosine similarity for OpenAI embeddings
                query_vec = np.array(query_embedding)
                embed_vec = np.array(embedding)
                similarity = float(np.dot(query_vec, embed_vec) / (np.linalg.norm(query_vec) * np.linalg.norm(embed_vec)))
            results.append((key, similarity, data))

        # Sort by similarity
        results.sort(key=lambda x: x[1], reverse=True)
        return results[:top_k]

    def get_entity_history(self, entity_key: str) -> List[Dict[str, Any]]:
        """Get detailed version history for a specific entity."""
        try:
            # Get commits that specifically affected this entity key
            key_bytes = entity_key.encode('utf-8') if isinstance(entity_key, str) else entity_key
            key_commits = self.kv_store.get_commits_for_key(key_bytes)

            history = []
            for commit in key_commits:
                history.append({
                    'commit_id': commit['id'][:8],
                    'full_commit_id': commit['id'],
                    'timestamp': datetime.fromtimestamp(commit['timestamp']).isoformat(),
                    'message': commit['message'],
                    'author': commit['author'],
                    'committer': commit['committer']
                })

            return history
        except Exception as e:
            print(f"⚠️  Error getting detailed entity history for {entity_key}: {e}")
            # Fallback to general commit history
            commits = self.kv_store.log()

            history = []
            for commit in commits:
                history.append({
                    'commit_id': commit['id'][:8],
                    'timestamp': datetime.fromtimestamp(commit['timestamp']).isoformat(),
                    'message': commit['message']
                })

            return history

    def get_user_profile(self, user_id: str) -> Optional[Dict[str, Any]]:
        """Get current user profile."""
        key = f"patch:{user_id}:user_profile".encode('utf-8')
        data = self.kv_store.get(key)
        if data:
            return json.loads(data.decode('utf-8'))
        return None

    def get_user_events(self, user_id: str, limit: int = 10) -> List[Dict[str, Any]]:
        """Get recent events for a user."""
        events = []
        keys = self.kv_store.list_keys()

        for key in keys:
            key_str = key.decode('utf-8')
            if f"insert:{user_id}:conversation_events:" in key_str:
                data = self.kv_store.get(key)
                if data:
                    event = json.loads(data.decode('utf-8'))
                    events.append(event)

        # Sort by timestamp
        events.sort(key=lambda x: x.get('timestamp', ''), reverse=True)
        return events[:limit]


# ============================================================================
# LangGraph Workflow Nodes
# ============================================================================

def extract_memories_node(state: MemoryState, service: HybridMemoryService) -> Dict:
    """Extract memories from conversation."""
    messages = state["messages"]
    user_id = state["user_id"]
    thread_id = state["thread_id"]

    print(f"\n📝 Extracting memories for user {user_id}...")

    # Extract and store memories
    extracted = service.extract_and_store(messages, user_id, thread_id)

    # Flatten extracted memories
    all_memories = []
    for memories in extracted.values():
        all_memories.extend(memories)

    return {
        "extracted_memories": all_memories,
        "messages": [AIMessage(content=f"Extracted {len(all_memories)} memories")]
    }


def semantic_search_node(state: MemoryState, service: HybridMemoryService) -> Dict:
    """Perform semantic search for relevant memories."""
    messages = state["messages"]
    user_id = state["user_id"]

    # Get last human message as query
    query = None
    for msg in reversed(messages):
        if isinstance(msg, HumanMessage):
            query = msg.content
            break

    if not query:
        return {"semantic_results": []}

    print(f"\n🔍 Searching memories for: {query[:50]}...")

    # Perform semantic search
    results = service.semantic_search(query, user_id, top_k=3)

    # Format results
    semantic_results = []
    for key, similarity, data in results:
        print(f"   📄 Found (similarity: {similarity:.2f}): {key}")
        semantic_results.append((key, similarity))

    return {
        "semantic_results": semantic_results,
        "messages": [AIMessage(content=f"Found {len(semantic_results)} relevant memories")]
    }


def entity_lookup_node(state: MemoryState, service: HybridMemoryService) -> Dict:
    """Perform deep lookup for specific entities."""
    extracted_memories = state.get("extracted_memories", [])
    semantic_results = state.get("semantic_results", [])

    print("\n🔬 Performing entity deep dive...")

    entity_contexts = {}

    # Extract entity references from memories
    entities = set()
    for memory in extracted_memories:
        if hasattr(memory, 'entities'):
            entities.update(memory.entities)

    # Get context for each entity
    for entity in list(entities)[:3]:  # Limit to 3 entities for demo
        history = service.get_entity_history(entity)
        entity_contexts[entity] = {
            'history': history,
            'version_count': len(history)
        }
        print(f"   📊 Entity {entity}: {len(history)} versions")

    return {
        "entity_contexts": entity_contexts,
        "messages": [AIMessage(content=f"Retrieved context for {len(entity_contexts)} entities")]
    }


def generate_response_node(state: MemoryState, service: HybridMemoryService) -> Dict:
    """Generate final response with memory context."""
    user_id = state["user_id"]
    extracted_memories = state.get("extracted_memories", [])
    semantic_results = state.get("semantic_results", [])
    entity_contexts = state.get("entity_contexts", {})

    print("\n💬 Generating response with memory context...")

    response_parts = []

    # Add user profile if available
    profile = service.get_user_profile(user_id)
    if profile:
        response_parts.append(f"User Profile: {profile.get('name', 'Unknown')}")
        if profile.get('preferences'):
            response_parts.append(f"Preferences: {json.dumps(profile['preferences'])}")

    # Add recent events
    events = service.get_user_events(user_id, limit=3)
    if events:
        response_parts.append(f"\nRecent Events ({len(events)}):")
        for event in events:
            response_parts.append(f"  • {event.get('event_type', 'unknown')}: {event.get('content', '')[:50]}...")

    # Add semantic search results
    if semantic_results:
        response_parts.append(f"\nRelevant Memories ({len(semantic_results)}):")
        for key, similarity in semantic_results[:2]:
            response_parts.append(f"  • {key} (similarity: {similarity:.2f})")

    # Add entity context
    if entity_contexts:
        response_parts.append(f"\nEntity History:")
        for entity, context in entity_contexts.items():
            response_parts.append(f"  • {entity}: {context['version_count']} versions")

    response = "\n".join(response_parts) if response_parts else "No memory context available."

    return {"messages": [AIMessage(content=response)]}


# ============================================================================
# Workflow Visualization
# ============================================================================

def display_workflow_diagram(workflow):
    """Display the LangGraph workflow diagram using built-in visualization."""
    print("🎨 Generating workflow diagram...")

    try:
        # Generate the diagram bytes using LangGraph's built-in Mermaid rendering
        diagram_bytes = workflow.get_graph(xray=True).draw_mermaid_png()

        # Save to file for viewing
        temp_file = '/tmp/langgraph_workflow_diagram.png'
        with open(temp_file, 'wb') as f:
            f.write(diagram_bytes)
        print(f"💾 Diagram saved to: {temp_file}")

        # Try to display inline if in a Jupyter environment
        if IPYTHON_AVAILABLE:
            try:
                # Check if we're in a Jupyter notebook environment
                from IPython import get_ipython
                if get_ipython() is not None and get_ipython().__class__.__name__ == 'ZMQInteractiveShell':
                    display(Image(diagram_bytes))
                    print("📊 Workflow diagram displayed inline!")
                else:
                    print("📊 Workflow diagram generated (view at the file path above)")
                    print("   💡 For inline display, run in a Jupyter notebook")
            except Exception:
                print("📊 Workflow diagram generated (view at the file path above)")
        else:
            print("📊 Workflow diagram generated (view at the file path above)")
            print("   💡 Install IPython for enhanced display: pip install ipython")

        print("✅ LangGraph built-in diagram generation successful!")
        return temp_file

    except Exception as e:
        print(f"⚠️  Could not generate diagram: {e}")
        print("   This may require additional dependencies for Mermaid rendering")
        print("   Try: pip install pygraphviz or check LangGraph documentation")

    return None


# ============================================================================
# Create Memory Workflow
# ============================================================================

def create_memory_workflow(service: HybridMemoryService):
    """Create the complete memory processing workflow."""

    # Build the graph
    builder = StateGraph(MemoryState)

    # Add nodes with service injection
    builder.add_node("extract_memories", lambda state: extract_memories_node(state, service))
    builder.add_node("semantic_search", lambda state: semantic_search_node(state, service))
    builder.add_node("entity_lookup", lambda state: entity_lookup_node(state, service))
    builder.add_node("generate_response", lambda state: generate_response_node(state, service))

    # Define the flow
    builder.add_edge(START, "extract_memories")
    builder.add_edge("extract_memories", "semantic_search")
    builder.add_edge("semantic_search", "entity_lookup")
    builder.add_edge("entity_lookup", "generate_response")
    builder.add_edge("generate_response", END)

    return builder.compile()


# ============================================================================
# Demonstration
# ============================================================================

def demonstrate_complete_system():
    """Demonstrate the complete memory system."""

    print("\n" + "=" * 80)
    print("   🚀 Complete LangGraph + ProllyTree Memory System")
    print("=" * 80)

    with tempfile.TemporaryDirectory() as tmpdir:
        store_path = os.path.join(tmpdir, "memory_system")
        service = HybridMemoryService(store_path)
        workflow = create_memory_workflow(service)

        # Generate and display workflow diagram
        print("\n📊 Displaying workflow visualization...")
        display_workflow_diagram(workflow)
        print("🚀 Proceeding with demonstration...")

        # User 1: Initial conversation
        print("\n👤 User: alice - Initial Conversation")
        print("-" * 40)

        state1 = workflow.invoke({
            "messages": [HumanMessage(content="I prefer detailed technical explanations and I'm interested in AI and quantum computing")],
            "user_id": "alice",
            "thread_id": "thread_001"
        })

        print("\n🤖 System Response:")
        for msg in state1["messages"][-1:]:
            if isinstance(msg, AIMessage):
                print(msg.content)

        # User 1: Follow-up with product question
        print("\n👤 User: alice - Product Question")
        print("-" * 40)

        state2 = workflow.invoke({
            "messages": [HumanMessage(content="What product options do you have for quantum computing research?")],
            "user_id": "alice",
            "thread_id": "thread_002"
        })

        print("\n🤖 System Response:")
        for msg in state2["messages"][-1:]:
            if isinstance(msg, AIMessage):
                print(msg.content)

        # User 2: Different user
        print("\n👤 User: bob - New User")
        print("-" * 40)

        state3 = workflow.invoke({
            "messages": [HumanMessage(content="I need help with machine learning deployment")],
            "user_id": "bob",
            "thread_id": "thread_003"
        })

        print("\n🤖 System Response:")
        for msg in state3["messages"][-1:]:
            if isinstance(msg, AIMessage):
                print(msg.content)

        # User 1: Return with semantic query
        print("\n👤 User: alice - Semantic Query")
        print("-" * 40)

        state4 = workflow.invoke({
            "messages": [HumanMessage(content="Tell me about quantum technologies")],
            "user_id": "alice",
            "thread_id": "thread_004"
        })

        print("\n🤖 System Response:")
        for msg in state4["messages"][-1:]:
            if isinstance(msg, AIMessage):
                print(msg.content)

        # Show git-like history
        print("\n📚 Git-like Commit History:")
        print("-" * 40)

        commits = service.kv_store.log()
        for commit in commits[-5:]:
            timestamp = datetime.fromtimestamp(commit['timestamp'])
            print(f"   {commit['id'][:8]} - {commit['message'][:60]} ({timestamp.strftime('%H:%M:%S')})")

        # Show memory statistics
        print("\n📊 Memory System Statistics:")
        print("-" * 40)

        # Count memories by type
        patch_count = sum(1 for k in service.vector_store.keys() if k.startswith("patch:"))
        insert_count = sum(1 for k in service.vector_store.keys() if k.startswith("insert:"))

        print(f"   • Patch memories (profiles): {patch_count}")
        print(f"   • Insert memories (events): {insert_count}")
        print(f"   • Total vector embeddings: {len(service.vector_store)}")
        print(f"   • Git commits: {len(commits)}")

        # Show user profiles
        print("\n👥 User Profiles:")
        print("-" * 40)

        for user_id in ["alice", "bob"]:
            profile = service.get_user_profile(user_id)
            if profile:
                print(f"   • {user_id}: {json.dumps(profile, indent=2)[:100]}...")
            else:
                print(f"   • {user_id}: No profile yet")


def main():
    """Run the complete demonstration."""

    print("=" * 80)
    print("   Complete LangGraph + ProllyTree Integration")
    print("=" * 80)
    print("\nThis demo shows:")
    print("  • Structured memory extraction (patch and insert modes)")
    print("  • Vector embeddings for semantic search")
    print("  • Git-like version control with ProllyTree")
    print("  • Entity tracking with complete history")
    print("  • Hybrid retrieval combining all approaches")

    try:
        demonstrate_complete_system()

        print("\n" + "=" * 80)
        print("✅ Demo Complete! Production-Ready Features:")
        print("   • Structured extraction with schemas")
        print("   • Patch mode for user profiles")
        print("   • Insert mode for event streams")
        print("   • Semantic search with embeddings")
        print("   • Version control for all changes")
        print("   • Entity tracking and history")
        print("   • Complete audit trail")
        print("=" * 80)

    except ImportError as e:
        print(f"\n❌ Error: {e}")
        print("\nPlease install required dependencies:")
        print("  pip install langgraph langchain-core numpy")
        print("\nFor real embeddings:")
        print("  pip install langchain-openai")


if __name__ == "__main__":
    main()
