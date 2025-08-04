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
    context_quality_score: float
    enhancement_iterations: int
    max_iterations: int
    context_sufficiency: str  # "sufficient" | "needs_enhancement" | "poor"
    detailed_context: Dict[str, Any]
    final_response: str


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

    def enhanced_semantic_search(self, query: str, user_id: Optional[str] = None,
                                top_k: int = 5, expand_context: bool = True) -> List[Tuple[str, float, Dict]]:
        """Enhanced semantic search with context expansion."""
        # Start with basic semantic search
        initial_results = self.semantic_search(query, user_id, top_k)

        if not expand_context:
            return initial_results

        # Extract related entities and expand search
        expanded_results = list(initial_results)
        related_entities = set()

        for key, similarity, data in initial_results:
            if 'entities' in data:
                related_entities.update(data['entities'])

        # Search for related entities
        for entity in related_entities:
            entity_results = self.semantic_search(entity, user_id, top_k=2)
            for result in entity_results:
                if result not in expanded_results:
                    expanded_results.append(result)

        # Re-sort and limit
        expanded_results.sort(key=lambda x: x[1], reverse=True)
        return expanded_results[:top_k * 2]  # Return more results for enhanced context

    def get_contextual_threads(self, user_id: str, query: str) -> List[Dict[str, Any]]:
        """Get conversation threads related to the query."""
        # Find all conversation events for the user
        events = self.get_user_events(user_id, limit=50)

        # Filter events related to the query using simple keyword matching
        related_events = []
        query_words = set(query.lower().split())

        for event in events:
            event_words = set(event.get('content', '').lower().split())
            # Calculate word overlap
            overlap = len(query_words.intersection(event_words))
            if overlap > 0:
                event['relevance_score'] = overlap / len(query_words)
                related_events.append(event)

        # Sort by relevance
        related_events.sort(key=lambda x: x.get('relevance_score', 0), reverse=True)
        return related_events[:10]

    def assess_context_quality(self, context_data: Dict[str, Any], query: str) -> Dict[str, Any]:
        """Assess the quality and completeness of retrieved context."""
        quality_score = 0.0
        assessment = {
            'score': 0.0,
            'completeness': 'low',
            'relevance': 'low',
            'freshness': 'low',
            'suggestions': []
        }

        # Check semantic results quality
        semantic_results = context_data.get('semantic_results', [])
        if semantic_results:
            avg_similarity = sum(result[1] for result in semantic_results) / len(semantic_results)
            quality_score += min(avg_similarity * 100, 40)  # Max 40 points

            if avg_similarity > 0.7:
                assessment['relevance'] = 'high'
            elif avg_similarity > 0.3:
                assessment['relevance'] = 'medium'

        # Check entity context depth
        entity_contexts = context_data.get('entity_contexts', {})
        if entity_contexts:
            total_versions = sum(ctx.get('version_count', 0) for ctx in entity_contexts.values())
            quality_score += min(total_versions * 5, 30)  # Max 30 points

            if total_versions > 10:
                assessment['completeness'] = 'high'
            elif total_versions > 3:
                assessment['completeness'] = 'medium'

        # Check recent events
        recent_events = context_data.get('recent_events', [])
        if recent_events:
            quality_score += min(len(recent_events) * 5, 20)  # Max 20 points
            assessment['freshness'] = 'high' if len(recent_events) > 3 else 'medium'

        # Check user profile completeness
        user_profile = context_data.get('user_profile', {})
        if user_profile:
            profile_fields = ['name', 'preferences', 'interests', 'context']
            filled_fields = sum(1 for field in profile_fields if user_profile.get(field))
            quality_score += filled_fields * 2.5  # Max 10 points

        assessment['score'] = quality_score

        # Generate improvement suggestions
        if assessment['relevance'] == 'low':
            assessment['suggestions'].append('Expand semantic search with related terms')
        if assessment['completeness'] == 'low':
            assessment['suggestions'].append('Retrieve more historical context for entities')
        if assessment['freshness'] == 'low':
            assessment['suggestions'].append('Include more recent conversation history')

        return assessment


# ============================================================================
# Enhanced LangGraph Workflow Nodes with Loops and Intelligence
# ============================================================================

def initialize_workflow_node(state: MemoryState) -> Dict:
    """Initialize workflow state with default values."""
    print("\n🚀 Initializing enhanced memory workflow...")

    return {
        "context_quality_score": 0.0,
        "enhancement_iterations": 0,
        "max_iterations": 3,
        "context_sufficiency": "needs_assessment",
        "detailed_context": {},
        "final_response": ""
    }


def extract_memories_node(state: MemoryState, service: HybridMemoryService) -> Dict:
    """Extract memories from conversation with enhanced tracking."""
    messages = state["messages"]
    user_id = state["user_id"]
    thread_id = state["thread_id"]
    iteration = state.get("enhancement_iterations", 0)

    print(f"\n📝 [Iteration {iteration + 1}] Extracting memories for user {user_id}...")

    # Extract and store memories
    extracted = service.extract_and_store(messages, user_id, thread_id)

    # Flatten extracted memories
    all_memories = []
    for memories in extracted.values():
        all_memories.extend(memories)

    print(f"   ✅ Extracted {len(all_memories)} memories")

    return {
        "extracted_memories": all_memories,
        "messages": [AIMessage(content=f"Extracted {len(all_memories)} memories (iteration {iteration + 1})")]
    }


def enhanced_semantic_search_node(state: MemoryState, service: HybridMemoryService) -> Dict:
    """Perform enhanced semantic search with context expansion."""
    messages = state["messages"]
    user_id = state["user_id"]
    iteration = state.get("enhancement_iterations", 0)

    # Get last human message as query
    query = None
    for msg in reversed(messages):
        if isinstance(msg, HumanMessage):
            query = msg.content
            break

    if not query:
        return {"semantic_results": []}

    print(f"\n🔍 [Iteration {iteration + 1}] Enhanced semantic search for: {query[:50]}...")

    # Use enhanced search with context expansion
    expand_context = iteration > 0  # Expand context in subsequent iterations
    results = service.enhanced_semantic_search(query, user_id, top_k=5, expand_context=expand_context)

    # Format results
    semantic_results = []
    for key, similarity, data in results:
        print(f"   📄 Found (similarity: {similarity:.2f}): {key}")
        semantic_results.append((key, similarity))

    # Get contextual threads
    contextual_threads = service.get_contextual_threads(user_id, query)
    if contextual_threads:
        print(f"   🧵 Found {len(contextual_threads)} related conversation threads")

    return {
        "semantic_results": semantic_results,
        "contextual_threads": contextual_threads,
        "messages": [AIMessage(content=f"Enhanced search found {len(semantic_results)} memories + {len(contextual_threads)} threads")]
    }


def deep_entity_analysis_node(state: MemoryState, service: HybridMemoryService) -> Dict:
    """Perform deep analysis of entities with version tracking."""
    extracted_memories = state.get("extracted_memories", [])
    semantic_results = state.get("semantic_results", [])
    iteration = state.get("enhancement_iterations", 0)

    print(f"\n🔬 [Iteration {iteration + 1}] Deep entity analysis...")

    entity_contexts = {}
    detailed_context = state.get("detailed_context", {})

    # Extract entity references from memories
    entities = set()
    for memory in extracted_memories:
        if hasattr(memory, 'entities'):
            entities.update(memory.entities)

    # Also extract entities from semantic search results
    for result in semantic_results:
        if len(result) == 2:
            key, similarity = result
            # Try to get data from vector store
            if key in service.vector_store:
                _, data = service.vector_store[key]
                if 'entities' in data:
                    entities.update(data['entities'])
        elif len(result) == 3:
            key, similarity, data = result
            if 'entities' in data:
                entities.update(data['entities'])

    # Get detailed context for each entity
    for entity in list(entities)[:5]:  # Increased limit for deeper analysis
        history = service.get_entity_history(entity)

        # Get more detailed entity information in later iterations
        if iteration > 0:
            # Simulate getting richer entity context
            additional_context = {
                'related_topics': [f"topic_{i}" for i in range(min(3, len(history)))],
                'interaction_frequency': len(history),
                'last_interaction': history[0]['timestamp'] if history else None
            }
        else:
            additional_context = {}

        entity_contexts[entity] = {
            'history': history,
            'version_count': len(history),
            'additional_context': additional_context
        }
        print(f"   📊 Entity {entity}: {len(history)} versions" +
              (f" + enhanced context" if additional_context else ""))

    # Store detailed context for quality assessment
    detailed_context.update({
        'entity_contexts': entity_contexts,
        'semantic_results': semantic_results,
        'contextual_threads': state.get("contextual_threads", [])
    })

    return {
        "entity_contexts": entity_contexts,
        "detailed_context": detailed_context,
        "messages": [AIMessage(content=f"Deep analysis: {len(entity_contexts)} entities analyzed")]
    }


def assess_context_quality_node(state: MemoryState, service: HybridMemoryService) -> Dict:
    """Assess the quality of retrieved context and decide if enhancement is needed."""
    detailed_context = state.get("detailed_context", {})
    iteration = state.get("enhancement_iterations", 0)
    max_iterations = state.get("max_iterations", 3)

    # Get query for assessment
    query = None
    for msg in reversed(state["messages"]):
        if isinstance(msg, HumanMessage):
            query = msg.content
            break

    print(f"\n📊 [Iteration {iteration + 1}] Assessing context quality...")

    # Add user profile and recent events to context
    user_id = state["user_id"]
    detailed_context['user_profile'] = service.get_user_profile(user_id) or {}
    detailed_context['recent_events'] = service.get_user_events(user_id, limit=10)

    # Assess context quality
    assessment = service.assess_context_quality(detailed_context, query or "")
    quality_score = assessment['score']

    print(f"   📈 Context Quality Score: {quality_score:.1f}/100")
    print(f"   📋 Relevance: {assessment['relevance']}, Completeness: {assessment['completeness']}, Freshness: {assessment['freshness']}")

    # Determine if we need more enhancement
    if quality_score >= 70:
        context_sufficiency = "sufficient"
        print("   ✅ Context quality is sufficient for response generation")
    elif iteration >= max_iterations - 1:
        context_sufficiency = "sufficient"  # Force completion after max iterations
        print("   ⏰ Maximum iterations reached, proceeding with available context")
    else:
        context_sufficiency = "needs_enhancement"
        print("   🔄 Context needs enhancement, will iterate")
        if assessment['suggestions']:
            print(f"   💡 Suggestions: {', '.join(assessment['suggestions'])}")

    return {
        "context_quality_score": quality_score,
        "context_sufficiency": context_sufficiency,
        "detailed_context": detailed_context,
        "quality_assessment": assessment,
        "messages": [AIMessage(content=f"Quality assessment: {quality_score:.1f}/100 - {context_sufficiency}")]
    }


def context_enhancement_loop_node(state: MemoryState) -> Dict:
    """Increment iteration counter for enhancement loop."""
    current_iteration = state.get("enhancement_iterations", 0)
    new_iteration = current_iteration + 1

    print(f"\n🔄 Context enhancement loop: Starting iteration {new_iteration}")

    return {
        "enhancement_iterations": new_iteration,
        "messages": [AIMessage(content=f"Enhancement iteration {new_iteration} started")]
    }


def generate_enhanced_response_node(state: MemoryState, service: HybridMemoryService) -> Dict:
    """Generate comprehensive response with all enhanced context."""
    user_id = state["user_id"]
    detailed_context = state.get("detailed_context", {})
    quality_assessment = state.get("quality_assessment", {})
    iteration = state.get("enhancement_iterations", 0)

    print(f"\n💬 [Final] Generating enhanced response with {iteration} iterations of context...")

    response_parts = []

    # Header with context quality information
    quality_score = state.get("context_quality_score", 0)
    response_parts.append(f"🎯 Enhanced Response (Context Quality: {quality_score:.1f}/100, {iteration} iterations)")
    response_parts.append("=" * 60)

    # User profile with more detail
    user_profile = detailed_context.get('user_profile', {})
    if user_profile:
        response_parts.append(f"\n👤 User Profile ({user_id}):")
        response_parts.append(f"   Name: {user_profile.get('name', 'Unknown')}")
        if user_profile.get('preferences'):
            response_parts.append(f"   Preferences: {json.dumps(user_profile['preferences'])}")
        if user_profile.get('interests'):
            response_parts.append(f"   Interests: {', '.join(user_profile['interests'])}")

    # Enhanced semantic results
    semantic_results = detailed_context.get('semantic_results', [])
    if semantic_results:
        response_parts.append(f"\n🔍 Semantic Search Results ({len(semantic_results)} found):")
        for i, (key, similarity) in enumerate(semantic_results[:5], 1):
            response_parts.append(f"   {i}. {key} (similarity: {similarity:.3f})")

    # Enhanced entity analysis
    entity_contexts = detailed_context.get('entity_contexts', {})
    if entity_contexts:
        response_parts.append(f"\n🏷️  Entity Analysis ({len(entity_contexts)} entities):")
        for entity, context in entity_contexts.items():
            additional = context.get('additional_context', {})
            versions = context['version_count']
            freq = additional.get('interaction_frequency', 0)
            response_parts.append(f"   • {entity}: {versions} versions" +
                                (f", {freq} interactions" if freq else ""))

    # Contextual conversation threads
    contextual_threads = detailed_context.get('contextual_threads', [])
    if contextual_threads:
        response_parts.append(f"\n🧵 Related Conversation Threads ({len(contextual_threads)} found):")
        for i, thread in enumerate(contextual_threads[:3], 1):
            relevance = thread.get('relevance_score', 0)
            content = thread.get('content', '')[:60]
            response_parts.append(f"   {i}. {content}... (relevance: {relevance:.2f})")

    # Recent events with enhanced detail
    recent_events = detailed_context.get('recent_events', [])
    if recent_events:
        response_parts.append(f"\n📅 Recent Events ({len(recent_events)} events):")
        for i, event in enumerate(recent_events[:5], 1):
            event_type = event.get('event_type', 'unknown')
            content = event.get('content', '')[:50]
            timestamp = event.get('timestamp', '')[:19]  # Remove microseconds
            response_parts.append(f"   {i}. [{event_type}] {content}... ({timestamp})")

    # Quality assessment summary
    if quality_assessment:
        response_parts.append(f"\n📊 Context Quality Assessment:")
        response_parts.append(f"   Overall Score: {quality_assessment.get('score', 0):.1f}/100")
        response_parts.append(f"   Relevance: {quality_assessment.get('relevance', 'unknown')}")
        response_parts.append(f"   Completeness: {quality_assessment.get('completeness', 'unknown')}")
        response_parts.append(f"   Freshness: {quality_assessment.get('freshness', 'unknown')}")

    response = "\n".join(response_parts) if response_parts else "No enhanced context available."

    return {
        "final_response": response,
        "messages": [AIMessage(content=response)]
    }


def should_enhance_context(state: MemoryState) -> str:
    """Decision function to determine if context enhancement is needed."""
    context_sufficiency = state.get("context_sufficiency", "needs_assessment")

    if context_sufficiency == "sufficient":
        return "generate_response"
    else:
        return "enhance_context"


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

def create_enhanced_memory_workflow(service: HybridMemoryService):
    """Create the enhanced memory processing workflow with loops and intelligence."""

    # Build the graph
    builder = StateGraph(MemoryState)

    # Add nodes with service injection
    builder.add_node("initialize", initialize_workflow_node)
    builder.add_node("extract_memories", lambda state: extract_memories_node(state, service))
    builder.add_node("enhanced_semantic_search", lambda state: enhanced_semantic_search_node(state, service))
    builder.add_node("deep_entity_analysis", lambda state: deep_entity_analysis_node(state, service))
    builder.add_node("assess_context_quality", lambda state: assess_context_quality_node(state, service))
    builder.add_node("context_enhancement_loop", context_enhancement_loop_node)
    builder.add_node("generate_enhanced_response", lambda state: generate_enhanced_response_node(state, service))

    # Define the enhanced flow with loops
    builder.add_edge(START, "initialize")
    builder.add_edge("initialize", "extract_memories")
    builder.add_edge("extract_memories", "enhanced_semantic_search")
    builder.add_edge("enhanced_semantic_search", "deep_entity_analysis")
    builder.add_edge("deep_entity_analysis", "assess_context_quality")

    # Conditional edge: decide whether to enhance context or generate response
    builder.add_conditional_edges(
        "assess_context_quality",
        should_enhance_context,
        {
            "enhance_context": "context_enhancement_loop",
            "generate_response": "generate_enhanced_response"
        }
    )

    # Loop back for context enhancement
    builder.add_edge("context_enhancement_loop", "enhanced_semantic_search")

    # Final edge
    builder.add_edge("generate_enhanced_response", END)

    return builder.compile()


# Keep the old simple workflow for comparison
def create_memory_workflow(service: HybridMemoryService):
    """Create the simple memory processing workflow (for comparison)."""

    # Build the graph
    builder = StateGraph(MemoryState)

    # Add nodes with service injection
    builder.add_node("extract_memories", lambda state: extract_memories_node(state, service))
    builder.add_node("enhanced_semantic_search", lambda state: enhanced_semantic_search_node(state, service))
    builder.add_node("deep_entity_analysis", lambda state: deep_entity_analysis_node(state, service))
    builder.add_node("generate_enhanced_response", lambda state: generate_enhanced_response_node(state, service))

    # Define the simple flow
    builder.add_edge(START, "extract_memories")
    builder.add_edge("extract_memories", "enhanced_semantic_search")
    builder.add_edge("enhanced_semantic_search", "deep_entity_analysis")
    builder.add_edge("deep_entity_analysis", "generate_enhanced_response")
    builder.add_edge("generate_enhanced_response", END)

    return builder.compile()


# ============================================================================
# Demonstration
# ============================================================================

def demonstrate_enhanced_system():
    """Demonstrate the enhanced memory system with loops and intelligence."""

    print("\n" + "=" * 80)
    print("   🚀 Enhanced LangGraph + ProllyTree Memory System with Loops")
    print("=" * 80)

    with tempfile.TemporaryDirectory() as tmpdir:
        store_path = os.path.join(tmpdir, "memory_system")
        service = HybridMemoryService(store_path)
        enhanced_workflow = create_enhanced_memory_workflow(service)

        # Generate and display workflow diagram
        print("\n📊 Displaying enhanced workflow visualization...")
        display_workflow_diagram(enhanced_workflow)
        print("🚀 Proceeding with enhanced demonstration...")

        # Demo 1: Complex query that will trigger multiple enhancement iterations
        print("\n" + "=" * 60)
        print("👤 User: alice - Complex Technical Query (Multi-iteration)")
        print("=" * 60)

        complex_state = enhanced_workflow.invoke({
            "messages": [HumanMessage(content="I need comprehensive information about quantum computing applications in machine learning, specifically for optimization problems. Please provide detailed technical analysis.")],
            "user_id": "alice",
            "thread_id": "thread_complex_001"
        })

        print("\n🤖 Enhanced System Response:")
        for msg in complex_state["messages"][-1:]:
            if isinstance(msg, AIMessage):
                print(msg.content)

        print(f"\n📈 Final Statistics for Complex Query:")
        print(f"   • Quality Score: {complex_state.get('context_quality_score', 0):.1f}/100")
        print(f"   • Enhancement Iterations: {complex_state.get('enhancement_iterations', 0)}")
        print(f"   • Context Sufficiency: {complex_state.get('context_sufficiency', 'unknown')}")

        # Demo 2: Follow-up question that should use the enhanced context
        print("\n" + "=" * 60)
        print("👤 User: alice - Follow-up Question")
        print("=" * 60)

        followup_state = enhanced_workflow.invoke({
            "messages": [HumanMessage(content="What are the specific hardware requirements for running quantum ML algorithms?")],
            "user_id": "alice",
            "thread_id": "thread_followup_002"
        })

        print("\n🤖 Enhanced System Response:")
        for msg in followup_state["messages"][-1:]:
            if isinstance(msg, AIMessage):
                print(msg.content)

        # Demo 3: Different user with simpler query (should require fewer iterations)
        print("\n" + "=" * 60)
        print("👤 User: bob - Simple Query (Fewer iterations expected)")
        print("=" * 60)

        simple_state = enhanced_workflow.invoke({
            "messages": [HumanMessage(content="How do I get started with machine learning?")],
            "user_id": "bob",
            "thread_id": "thread_simple_003"
        })

        print("\n🤖 Enhanced System Response:")
        for msg in simple_state["messages"][-1:]:
            if isinstance(msg, AIMessage):
                print(msg.content)

        print(f"\n📈 Final Statistics for Simple Query:")
        print(f"   • Quality Score: {simple_state.get('context_quality_score', 0):.1f}/100")
        print(f"   • Enhancement Iterations: {simple_state.get('enhancement_iterations', 0)}")

        # Demo 4: Return to alice with related query (should have rich context)
        print("\n" + "=" * 60)
        print("👤 User: alice - Related Query (Rich Context Expected)")
        print("=" * 60)

        related_state = enhanced_workflow.invoke({
            "messages": [HumanMessage(content="Based on our previous discussions about quantum computing, what's the current state of quantum machine learning research?")],
            "user_id": "alice",
            "thread_id": "thread_related_004"
        })

        print("\n🤖 Enhanced System Response:")
        for msg in related_state["messages"][-1:]:
            if isinstance(msg, AIMessage):
                print(msg.content)

        # Show comprehensive system analytics
        print("\n" + "=" * 60)
        print("📊 Enhanced Memory System Analytics")
        print("=" * 60)

        # Git history
        commits = service.kv_store.log()
        print(f"\n📚 Git-like Commit History ({len(commits)} total commits):")
        for commit in commits[-8:]:  # Show more commits
            timestamp = datetime.fromtimestamp(commit['timestamp'])
            print(f"   {commit['id'][:8]} - {commit['message'][:70]} ({timestamp.strftime('%H:%M:%S')})")

        # Memory statistics
        print(f"\n📊 Memory Store Statistics:")
        patch_count = sum(1 for k in service.vector_store.keys() if k.startswith("patch:"))
        insert_count = sum(1 for k in service.vector_store.keys() if k.startswith("insert:"))
        print(f"   • Patch memories (profiles): {patch_count}")
        print(f"   • Insert memories (events): {insert_count}")
        print(f"   • Total vector embeddings: {len(service.vector_store)}")
        print(f"   • Git commits: {len(commits)}")

        # User profiles with more detail
        print(f"\n👥 User Profile Analysis:")
        for user_id in ["alice", "bob"]:
            profile = service.get_user_profile(user_id)
            events = service.get_user_events(user_id, limit=5)
            if profile:
                print(f"   • {user_id}:")
                print(f"     - Profile: {json.dumps(profile, indent=6)[:150]}...")
                print(f"     - Recent events: {len(events)}")
                if events:
                    for i, event in enumerate(events[:2], 1):
                        print(f"       {i}. {event.get('event_type', 'unknown')}: {event.get('content', '')[:40]}...")
            else:
                print(f"   • {user_id}: No profile data")

        # Enhancement statistics comparison
        print(f"\n🔄 Enhancement Loop Statistics:")
        print(f"   • Complex query iterations: {complex_state.get('enhancement_iterations', 0)}")
        print(f"   • Simple query iterations: {simple_state.get('enhancement_iterations', 0)}")
        print(f"   • Related query iterations: {related_state.get('enhancement_iterations', 0)}")
        print(f"   • Average quality improvement: Demonstrated through iterative context enhancement")


def main():
    """Run the enhanced demonstration with loops and intelligence."""

    print("=" * 80)
    print("   Enhanced LangGraph + ProllyTree Integration with Loops")
    print("=" * 80)
    print("\nThis enhanced demo demonstrates:")
    print("  🔄 Iterative context enhancement with quality assessment")
    print("  🧠 Intelligent loop control based on context sufficiency")
    print("  🎯 Multi-iteration retrieval for complex queries")
    print("  📊 Context quality scoring and improvement suggestions")
    print("  🔍 Enhanced semantic search with context expansion")
    print("  🏷️  Deep entity analysis with version tracking")
    print("  🧵 Contextual conversation thread retrieval")
    print("  ⚡ Adaptive workflow that improves response quality")

    print("\n🔄 Workflow Features:")
    print("  • START → Initialize → Extract → Search → Analyze → Assess Quality")
    print("  • IF context insufficient: Loop back for enhancement")
    print("  • IF context sufficient: Generate enhanced response")
    print("  • Maximum 3 iterations to prevent infinite loops")

    try:
        demonstrate_enhanced_system()

        print("\n" + "=" * 80)
        print("✅ Enhanced Demo Complete! Advanced Features Demonstrated:")
        print("   🔄 Iterative context enhancement loops")
        print("   🧠 Intelligent quality assessment")
        print("   📊 Context scoring and improvement")
        print("   🎯 Multi-iteration retrieval optimization")
        print("   🔍 Enhanced semantic search expansion")
        print("   🏷️  Deep entity version tracking")
        print("   🧵 Contextual thread analysis")
        print("   ⚡ Adaptive response generation")
        print("   📈 Quality-driven workflow decisions")
        print("   🔒 Loop control and termination")
        print("=" * 80)

    except ImportError as e:
        print(f"\n❌ Error: {e}")
        print("\nPlease install required dependencies:")
        print("  pip install langgraph langchain-core numpy")
        print("\nFor real embeddings:")
        print("  pip install langchain-openai")


if __name__ == "__main__":
    main()
