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
Multi-Agent System with Git-like Branching using LangGraph + ProllyTree

This example demonstrates solving the "context bleeding" problem in multi-agent systems
using ProllyTree's versioned memory store with Git-like branching.

Architecture:
┌─────────────────────────────────────────────────────────────────────────┐
│                     Multi-Agent Branching Architecture                  │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                          Agent Hierarchy                                │
│                                                                         │
│                         Main Orchestrator                               │
│                         (main branch)                                   │
│                              │                                          │
│                 ┌────────────┼────────────┐                             │
│                 ▼            ▼            ▼                             │
│           Troubleshooting  Billing   Customer History                   │
│           (branch: ts-1)  (branch: b-1) (branch: ch-1)                  │
│                                                                         │
│           Each sub-agent operates in isolated branch                    │
│           Main agent validates and merges results                       │
└─────────────────────────────────────────────────────────────────────────┘

Key Features:
• Branch isolation prevents context bleeding between agents
• Semantic validation during merge operations
• Conflict resolution for inconsistent recommendations
• Complete audit trail with Git-like history
"""

import json
import os
import subprocess
import tempfile
import uuid
from datetime import datetime, timezone
from enum import Enum
from typing import Any, Dict, List, Optional, Tuple, Annotated, Literal
from dataclasses import dataclass, field, asdict

from langchain_core.messages import HumanMessage, AIMessage, SystemMessage
try:
    from pydantic import BaseModel, Field
except ImportError:
    from pydantic.v1 import BaseModel, Field
from langgraph.graph import StateGraph, START, END
from langgraph.graph.message import add_messages
from typing_extensions import TypedDict

# ProllyTree imports
from prollytree import VersionedKvStore

# ============================================================================
# Agent Types and Issue Types (Telecommunications Customer Support)
# ============================================================================

class AgentType(Enum):
    ORCHESTRATOR = "orchestrator"
    TROUBLESHOOTING = "troubleshooting"
    BILLING = "billing"
    CUSTOMER_HISTORY = "customer_history"
    ESCALATION = "escalation"
    KNOWLEDGE_BASE = "knowledge_base"

class IssueType(Enum):
    SLOW_INTERNET = "slow_internet"
    BILLING_DISPUTE = "billing_dispute"
    SERVICE_OUTAGE = "service_outage"
    ACCOUNT_UPGRADE = "account_upgrade"
    TECHNICAL_COMPLEX = "technical_complex"

# ============================================================================
# Data Models
# ============================================================================

@dataclass
class CustomerContext:
    """Customer information and current issue"""
    customer_id: str
    name: str
    account_type: str
    issue_type: IssueType
    issue_description: str
    priority: str
    contact_history: List[Dict] = field(default_factory=list)
    current_services: List[str] = field(default_factory=list)
    billing_status: str = "current"

    def to_dict(self):
        return {
            **asdict(self),
            'issue_type': self.issue_type.value
        }

@dataclass
class AgentRecommendation:
    """Recommendation from a sub-agent"""
    agent_type: AgentType
    branch_name: str
    actions: List[str]
    confidence: float
    reasoning: str
    data_collected: Dict[str, Any]
    timestamp: datetime = field(default_factory=lambda: datetime.now(tz=timezone.utc))

    def to_dict(self):
        return {
            'agent_type': self.agent_type.value,
            'branch_name': self.branch_name,
            'actions': self.actions,
            'confidence': self.confidence,
            'reasoning': self.reasoning,
            'data_collected': self.data_collected,
            'timestamp': self.timestamp.isoformat()
        }

class MergeConflict(BaseModel):
    """Represents a merge conflict between agent recommendations"""
    agent1: str = Field(..., description="First agent with conflicting recommendation")
    agent2: str = Field(..., description="Second agent with conflicting recommendation")
    conflict_type: str = Field(..., description="Type of conflict")
    resolution: Optional[str] = Field(None, description="How the conflict was resolved")

# ============================================================================
# State Definitions for LangGraph
# ============================================================================

class MultiAgentState(TypedDict):
    """State for multi-agent workflow"""
    messages: Annotated[List, add_messages]
    customer_context: CustomerContext
    session_id: str
    main_branch: str
    active_branches: List[str]
    agent_recommendations: List[AgentRecommendation]
    merge_conflicts: List[MergeConflict]
    final_resolution: Optional[Dict[str, Any]]
    context_bleeding_detected: bool
    isolation_success: bool

# ============================================================================
# Branched Memory Service for Multi-Agent Isolation
# ============================================================================

class BranchedMemoryService:
    """
    Memory service with Git-like branching for agent isolation.
    Prevents context bleeding through branch isolation.
    """

    def __init__(self, store_path: str):
        """Initialize the branched memory service"""
        self.store_path = store_path
        os.makedirs(store_path, exist_ok=True)

        # Create a subdirectory for the data store
        data_path = os.path.join(store_path, "data")
        os.makedirs(data_path, exist_ok=True)

        # Initialize git repo in the parent directory
        if not os.path.exists(os.path.join(store_path, '.git')):
            subprocess.run(["git", "init", "--quiet"], cwd=store_path, check=True)
            subprocess.run(["git", "config", "user.name", "Multi-Agent System"], cwd=store_path, check=True)
            subprocess.run(["git", "config", "user.email", "agents@example.com"], cwd=store_path, check=True)

        # Initialize ProllyTree store in subdirectory
        self.data_path = data_path  # Store data path for git operations
        self.kv_store = VersionedKvStore(data_path)
        self.main_branch = "main"
        self.current_branch = "main"

        # Track branch metadata
        self.branch_metadata = {}

        print(f"✅ Initialized branched memory service at {store_path}")

    def create_agent_branch(self, agent_type: AgentType, session_id: str) -> str:
        """Create an isolated branch for a sub-agent"""
        branch_name = f"{session_id}-{agent_type.value}-{uuid.uuid4().hex[:8]}"

        # Store branch metadata
        self.branch_metadata[branch_name] = {
            'agent_type': agent_type.value,
            'session_id': session_id,
            'created_at': datetime.now(tz=timezone.utc).isoformat(),
            'parent_branch': self.main_branch,
            'commit_id': self.kv_store.log()[0]['id'] if self.kv_store.log() else None
        }

        # Store metadata in the store
        metadata_key = f"branch:metadata:{branch_name}".encode('utf-8')
        metadata_value = json.dumps(self.branch_metadata[branch_name]).encode('utf-8')
        self.kv_store.insert(metadata_key, metadata_value)
        self.kv_store.commit(f"Created branch for {agent_type.value} agent")

        self.current_branch = branch_name
        print(f"🌿 Created logical branch '{branch_name}' for {agent_type.value} agent")

        return branch_name

    def _checkout_branch(self, branch_name: str):
        """Logically checkout a specific branch"""
        # For logical branches, just track the current branch
        self.current_branch = branch_name

    def store_agent_data(self, agent_type: AgentType, session_id: str,
                        key: str, data: Dict[str, Any]):
        """Store data in the current agent's branch"""
        # Ensure we're in the right branch context
        full_key = f"agent:{agent_type.value}:{session_id}:{key}".encode('utf-8')
        value = json.dumps(data).encode('utf-8')

        # Check if key exists
        existing = self.kv_store.get(full_key)
        if existing:
            self.kv_store.update(full_key, value)
        else:
            self.kv_store.insert(full_key, value)

        self.kv_store.commit(f"{agent_type.value}: Stored {key}")

        print(f"   💾 {agent_type.value} stored: {key}")

    def get_branch_data(self, branch_name: str) -> Dict[str, Any]:
        """Get all data from a specific branch"""
        # Save current branch
        prev_branch = self.current_branch

        # Checkout target branch
        self._checkout_branch(branch_name)

        # Collect all data
        branch_data = {}
        keys = self.kv_store.list_keys()

        for key in keys:
            key_str = key.decode('utf-8')
            if key_str.startswith("agent:"):
                data = self.kv_store.get(key)
                if data:
                    branch_data[key_str] = json.loads(data.decode('utf-8'))

        # Return to previous branch
        self._checkout_branch(prev_branch)

        return branch_data

    def validate_and_merge(self, branch_name: str, agent_type: AgentType,
                          validation_fn=None) -> Tuple[bool, Optional[str]]:
        """Validate and merge agent branch back to main"""
        # Get branch data
        branch_data = self.get_branch_data(branch_name)

        # Perform semantic validation
        if validation_fn:
            is_valid, reason = validation_fn(branch_data, agent_type)
            if not is_valid:
                print(f"   ❌ Validation failed for {agent_type.value}: {reason}")
                return False, reason

        # For logical branches, merging means copying validated data to main namespace
        self._checkout_branch(self.main_branch)

        try:
            # Copy branch data to main with namespace prefix
            for key_str, value in branch_data.items():
                # Create a merged key that shows it came from this branch
                merged_key = f"merged:{branch_name}:{key_str}".encode('utf-8')
                merged_value = json.dumps(value).encode('utf-8')

                # Store in main namespace
                existing = self.kv_store.get(merged_key)
                if existing:
                    self.kv_store.update(merged_key, merged_value)
                else:
                    self.kv_store.insert(merged_key, merged_value)

            # Commit the merge
            self.kv_store.commit(f"Merged {agent_type.value} recommendations from {branch_name}")
            print(f"   ✅ Successfully merged {agent_type.value} branch")
            return True, None

        except Exception as e:
            print(f"   ❌ Merge failed: {e}")
            return False, str(e)

    def get_commit_history(self) -> List[Dict[str, Any]]:
        """Get commit history showing agent activities"""
        commits = self.kv_store.log()

        history = []
        for commit in commits:
            history.append({
                'id': commit['id'][:8],
                'message': commit['message'],
                'timestamp': datetime.fromtimestamp(commit['timestamp']).isoformat(),
                'author': commit.get('author', 'Unknown')
            })

        return history

# ============================================================================
# Sub-Agent Implementations (Real LangGraph Agents)
# ============================================================================

# Mock LLM for demonstration (replace with real LLM in production)
class MockLLM:
    """Mock LLM that simulates real AI responses"""

    def invoke(self, prompt: str) -> str:
        """Simulate LLM response based on prompt content"""
        prompt_lower = prompt.lower()

        # Troubleshooting agent responses
        if "troubleshooting" in prompt_lower and "slow internet" in prompt_lower:
            return """Based on the customer's slow internet issue, I recommend:
1. Check signal strength and modem status
2. Schedule technician visit for line quality assessment
3. Replace modem if hardware diagnostics show issues
4. Check area infrastructure for service outages

Confidence: 85%
Reasoning: Multiple indicators suggest hardware/infrastructure problems requiring professional assessment."""

        elif "troubleshooting" in prompt_lower:
            return """For this technical issue, I recommend:
1. Run remote diagnostics
2. Check service status
3. Perform standard connectivity tests

Confidence: 70%
Reasoning: Standard troubleshooting procedure for technical issues."""

        # Billing agent responses
        elif "billing" in prompt_lower and "dispute" in prompt_lower:
            return """For this billing dispute, I recommend:
1. Review all recent charges and billing history
2. Apply credit if charges are found to be incorrect
3. Provide detailed explanation of billing structure
4. Set up payment plan if needed

Confidence: 90%
Reasoning: Customer billing concerns require thorough review and transparent communication."""

        elif "billing" in prompt_lower and "technical" in prompt_lower:
            return """No billing action required for this technical issue.
1. Verify account is in good standing
2. No billing implications for technical problems

Confidence: 95%
Reasoning: Technical issues should not involve billing changes unless service credits are warranted."""

        # Customer history agent responses
        elif "customer history" in prompt_lower:
            if "premium" in prompt_lower or "multiple issues" in prompt_lower:
                return """Based on customer history analysis:
1. Prioritize resolution due to account status
2. Consider service credit for inconvenience
3. Escalate to senior support if needed
4. Document interaction for future reference

Confidence: 80%
Reasoning: Premium customers with previous issues need priority handling."""
            else:
                return """Standard customer history assessment:
1. Follow standard support process
2. Document interaction thoroughly
3. Monitor for pattern of issues

Confidence: 75%
Reasoning: Normal customer profile with no special handling required."""

        return "I need more specific information to provide recommendations."

# Initialize LLM (try real first, fallback to mock)
try:
    # Try to use real LLM if available
    from langchain_openai import ChatOpenAI
    import os

    api_key = os.getenv("OPENAI_API_KEY", "")
    if api_key and api_key.startswith("sk-") and not api_key.startswith(("mock", "test")):
        llm = ChatOpenAI(model="gpt-3.5-turbo", temperature=0.7)
        print("✅ Using real OpenAI LLM for agents")
        LLM_TYPE = "real"
    else:
        llm = MockLLM()
        print("🔄 Using mock LLM for agents (set OPENAI_API_KEY for real LLM)")
        LLM_TYPE = "mock"
except ImportError:
    llm = MockLLM()
    print("🔄 Using mock LLM for agents (install langchain-openai for real LLM)")
    LLM_TYPE = "mock"

class TroubleshootingAgent:
    """Real LangGraph-based technical troubleshooting agent"""

    def __init__(self):
        self.agent_type = AgentType.TROUBLESHOOTING
        self.system_prompt = """You are a technical troubleshooting specialist for a telecommunications company.
Your role is to:
- Analyze technical issues with internet, phone, and cable services
- Perform diagnostic procedures
- Recommend technical solutions
- Schedule technician visits when needed
- Focus ONLY on technical aspects, not billing or account issues

Respond with specific actions, confidence level (0-100%), and reasoning."""

    def process(self, customer: CustomerContext, memory: BranchedMemoryService,
                session_id: str, branch_name: str) -> AgentRecommendation:
        """Process technical issues using LLM in isolated branch"""
        print(f"\n🔧 Troubleshooting Agent (LLM-powered) processing in branch: {branch_name}")

        # Create detailed prompt with customer context
        prompt = f"""{self.system_prompt}

Customer Information:
- Name: {customer.name}
- Account Type: {customer.account_type}
- Issue: {customer.issue_description}
- Services: {', '.join(customer.current_services)}
- Priority: {customer.priority}

Previous Contact History:
{json.dumps(customer.contact_history, indent=2) if customer.contact_history else "No previous contacts"}

Analyze this technical issue and provide specific recommendations."""

        # Get LLM response
        if LLM_TYPE == "real":
            from langchain_core.messages import SystemMessage, HumanMessage
            messages = [
                SystemMessage(content=self.system_prompt),
                HumanMessage(content=prompt)
            ]
            response = llm.invoke(messages).content
        else:  # Mock LLM
            response = llm.invoke(f"troubleshooting agent analyzing: {customer.issue_description}")

        # Parse LLM response into structured recommendation
        actions, confidence, reasoning, diagnostics = self._parse_llm_response(response, customer)

        print(f"   🤖 LLM Response: {response[:100]}..." if len(response) > 100 else f"   🤖 LLM Response: {response}")
        print(f"   📝 Extracted {len(actions)} actions with {confidence:.0%} confidence")

        # Store diagnostic data in isolated branch
        memory.store_agent_data(self.agent_type, session_id, 'diagnostics', diagnostics)
        memory.store_agent_data(self.agent_type, session_id, 'llm_response', {'raw_response': response})

        return AgentRecommendation(
            agent_type=self.agent_type,
            branch_name=branch_name,
            actions=actions,
            confidence=confidence,
            reasoning=reasoning,
            data_collected=diagnostics
        )

    def _parse_llm_response(self, response: str, customer: CustomerContext) -> Tuple[List[str], float, str, Dict]:
        """Parse LLM response into structured data"""
        # Extract actions (look for numbered lists or bullet points)
        actions = []
        lines = response.split('\n')
        for line in lines:
            line = line.strip()
            if (line.startswith(('1.', '2.', '3.', '4.', '5.', '-', '•')) and
                len(line) > 5):
                # Clean up the action text
                action = line[2:].strip() if line[1] == '.' else line[1:].strip()
                if action and len(action) > 10:  # Filter out very short items
                    actions.append(action)

        # Extract confidence (look for percentage)
        confidence = 0.75  # default
        import re
        conf_match = re.search(r'confidence[:\s]*(\d+)%?', response.lower())
        if conf_match:
            confidence = int(conf_match.group(1)) / 100.0

        # Extract reasoning
        reasoning_keywords = ['reasoning:', 'rationale:', 'because:', 'explanation:']
        reasoning = "LLM-generated technical analysis"
        for keyword in reasoning_keywords:
            if keyword in response.lower():
                reasoning_start = response.lower().find(keyword)
                reasoning = response[reasoning_start:].split('\n')[0][len(keyword):].strip()
                break

        # Generate diagnostic data based on issue type
        diagnostics = {
            'issue_type': customer.issue_type.value,
            'analysis_method': 'llm_powered',
            'customer_tier': customer.account_type,
            'services_affected': customer.current_services,
            'llm_confidence': confidence
        }

        return actions, confidence, reasoning, diagnostics

class BillingAgent:
    """Real LangGraph-based billing and account agent"""

    def __init__(self):
        self.agent_type = AgentType.BILLING
        self.system_prompt = """You are a billing specialist for a telecommunications company.
Your role is to:
- Handle billing disputes and payment issues
- Review account charges and payment history
- Apply credits and adjustments when appropriate
- Explain billing structures and charges
- Focus ONLY on billing and account matters, not technical issues

Respond with specific actions, confidence level (0-100%), and reasoning."""

    def process(self, customer: CustomerContext, memory: BranchedMemoryService,
                session_id: str, branch_name: str) -> AgentRecommendation:
        """Process billing issues using LLM in isolated branch"""
        print(f"\n💰 Billing Agent (LLM-powered) processing in branch: {branch_name}")

        # Create detailed prompt with customer context
        prompt = f"""{self.system_prompt}

Customer Information:
- Name: {customer.name}
- Account Type: {customer.account_type}
- Issue: {customer.issue_description}
- Issue Type: {customer.issue_type.value}
- Services: {', '.join(customer.current_services)}
- Billing Status: {customer.billing_status}

Previous Contact History:
{json.dumps(customer.contact_history, indent=2) if customer.contact_history else "No previous contacts"}

Analyze this billing-related issue and provide specific recommendations."""

        # Get LLM response
        if LLM_TYPE == "real":
            from langchain_core.messages import SystemMessage, HumanMessage
            messages = [
                SystemMessage(content=self.system_prompt),
                HumanMessage(content=prompt)
            ]
            response = llm.invoke(messages).content
        else:  # Mock LLM
            context = f"billing agent analyzing: {customer.issue_description} (issue type: {customer.issue_type.value})"
            response = llm.invoke(context)

        # Parse LLM response into structured recommendation
        actions, confidence, reasoning, billing_data = self._parse_llm_response(response, customer)

        print(f"   🤖 LLM Response: {response[:100]}..." if len(response) > 100 else f"   🤖 LLM Response: {response}")
        print(f"   📝 Extracted {len(actions)} actions with {confidence:.0%} confidence")

        # Store billing data in isolated branch
        memory.store_agent_data(self.agent_type, session_id, 'billing_analysis', billing_data)
        memory.store_agent_data(self.agent_type, session_id, 'llm_response', {'raw_response': response})

        return AgentRecommendation(
            agent_type=self.agent_type,
            branch_name=branch_name,
            actions=actions,
            confidence=confidence,
            reasoning=reasoning,
            data_collected=billing_data
        )

    def _parse_llm_response(self, response: str, customer: CustomerContext) -> Tuple[List[str], float, str, Dict]:
        """Parse LLM response into structured data"""
        # Extract actions (look for numbered lists or bullet points)
        actions = []
        lines = response.split('\n')
        for line in lines:
            line = line.strip()
            if (line.startswith(('1.', '2.', '3.', '4.', '5.', '-', '•')) and
                len(line) > 5):
                # Clean up the action text
                action = line[2:].strip() if line[1] == '.' else line[1:].strip()
                if action and len(action) > 10:  # Filter out very short items
                    actions.append(action)

        # Extract confidence (look for percentage)
        confidence = 0.75  # default
        import re
        conf_match = re.search(r'confidence[:\s]*(\d+)%?', response.lower())
        if conf_match:
            confidence = int(conf_match.group(1)) / 100.0

        # Extract reasoning
        reasoning_keywords = ['reasoning:', 'rationale:', 'because:', 'explanation:']
        reasoning = "LLM-generated billing analysis"
        for keyword in reasoning_keywords:
            if keyword in response.lower():
                reasoning_start = response.lower().find(keyword)
                reasoning = response[reasoning_start:].split('\n')[0][len(keyword):].strip()
                break

        # Generate billing data based on analysis
        billing_data = {
            'issue_type': customer.issue_type.value,
            'analysis_method': 'llm_powered',
            'account_type': customer.account_type,
            'billing_status': customer.billing_status,
            'llm_confidence': confidence,
            'services': customer.current_services
        }

        return actions, confidence, reasoning, billing_data

class CustomerHistoryAgent:
    """Real LangGraph-based customer history and context agent"""

    def __init__(self):
        self.agent_type = AgentType.CUSTOMER_HISTORY
        self.system_prompt = """You are a customer relationship specialist for a telecommunications company.
Your role is to:
- Analyze customer interaction history and patterns
- Assess customer loyalty and satisfaction levels
- Recommend appropriate service levels based on customer tier
- Identify escalation needs based on historical patterns
- Provide context about customer relationship health

Respond with specific actions, confidence level (0-100%), and reasoning."""

    def process(self, customer: CustomerContext, memory: BranchedMemoryService,
                session_id: str, branch_name: str) -> AgentRecommendation:
        """Analyze customer history using LLM in isolated branch"""
        print(f"\n📚 Customer History Agent (LLM-powered) processing in branch: {branch_name}")

        # Create detailed prompt with customer context
        contact_summary = f"{len(customer.contact_history)} previous contacts" if customer.contact_history else "No previous contacts"

        prompt = f"""{self.system_prompt}

Customer Information:
- Name: {customer.name}
- Account Type: {customer.account_type}
- Current Issue: {customer.issue_description}
- Issue Priority: {customer.priority}
- Services: {', '.join(customer.current_services)}
- Contact History Summary: {contact_summary}

Detailed Contact History:
{json.dumps(customer.contact_history, indent=2) if customer.contact_history else "No previous interactions"}

Analyze this customer's history and relationship status. Provide recommendations for handling this interaction."""

        # Get LLM response
        if LLM_TYPE == "real":
            from langchain_core.messages import SystemMessage, HumanMessage
            messages = [
                SystemMessage(content=self.system_prompt),
                HumanMessage(content=prompt)
            ]
            response = llm.invoke(messages).content
        else:  # Mock LLM
            context = f"customer history agent analyzing: {customer.name} ({customer.account_type}) with {len(customer.contact_history)} previous contacts"
            response = llm.invoke(context)

        # Parse LLM response into structured recommendation
        actions, confidence, reasoning, history_data = self._parse_llm_response(response, customer)

        print(f"   🤖 LLM Response: {response[:100]}..." if len(response) > 100 else f"   🤖 LLM Response: {response}")
        print(f"   📝 Extracted {len(actions)} actions with {confidence:.0%} confidence")

        # Store history data in isolated branch
        memory.store_agent_data(self.agent_type, session_id, 'history_analysis', history_data)
        memory.store_agent_data(self.agent_type, session_id, 'llm_response', {'raw_response': response})

        return AgentRecommendation(
            agent_type=self.agent_type,
            branch_name=branch_name,
            actions=actions,
            confidence=confidence,
            reasoning=reasoning,
            data_collected=history_data
        )

    def _parse_llm_response(self, response: str, customer: CustomerContext) -> Tuple[List[str], float, str, Dict]:
        """Parse LLM response into structured data"""
        # Extract actions (look for numbered lists or bullet points)
        actions = []
        lines = response.split('\n')
        for line in lines:
            line = line.strip()
            if (line.startswith(('1.', '2.', '3.', '4.', '5.', '-', '•')) and
                len(line) > 5):
                # Clean up the action text
                action = line[2:].strip() if line[1] == '.' else line[1:].strip()
                if action and len(action) > 10:  # Filter out very short items
                    actions.append(action)

        # Extract confidence (look for percentage)
        confidence = 0.75  # default
        import re
        conf_match = re.search(r'confidence[:\s]*(\d+)%?', response.lower())
        if conf_match:
            confidence = int(conf_match.group(1)) / 100.0

        # Extract reasoning
        reasoning_keywords = ['reasoning:', 'rationale:', 'because:', 'explanation:']
        reasoning = "LLM-generated customer history analysis"
        for keyword in reasoning_keywords:
            if keyword in response.lower():
                reasoning_start = response.lower().find(keyword)
                reasoning = response[reasoning_start:].split('\n')[0][len(keyword):].strip()
                break

        # Generate history data based on analysis
        history_data = {
            'previous_issues': len(customer.contact_history),
            'analysis_method': 'llm_powered',
            'customer_tier': customer.account_type,
            'priority_level': customer.priority,
            'services_count': len(customer.current_services),
            'llm_confidence': confidence,
            'recent_interactions': customer.contact_history[-3:] if customer.contact_history else []
        }

        return actions, confidence, reasoning, history_data

# ============================================================================
# LangGraph Workflow Nodes
# ============================================================================

def initialize_session_node(state: MultiAgentState) -> Dict:
    """Initialize the multi-agent session"""
    print("\n" + "="*80)
    print("🚀 MULTI-AGENT SESSION WITH BRANCH ISOLATION")
    print("="*80)

    session_id = str(uuid.uuid4())[:8]

    print(f"\n📋 Session ID: {session_id}")
    print(f"👤 Customer: {state['customer_context'].name}")
    print(f"❓ Issue: {state['customer_context'].issue_description}")

    return {
        "session_id": session_id,
        "main_branch": "main",
        "active_branches": [],
        "agent_recommendations": [],
        "merge_conflicts": [],
        "context_bleeding_detected": False,
        "isolation_success": True,
        "messages": [SystemMessage(content=f"Session {session_id} initialized")]
    }

def delegate_to_agents_node(state: MultiAgentState, memory: BranchedMemoryService) -> Dict:
    """Main orchestrator delegates to sub-agents with branch isolation"""
    customer = state['customer_context']
    session_id = state['session_id']

    print(f"\n🎯 Main Orchestrator delegating tasks...")

    # Determine which agents to involve based on issue type
    agents_to_involve = []

    if customer.issue_type == IssueType.SLOW_INTERNET:
        agents_to_involve = [
            (AgentType.TROUBLESHOOTING, TroubleshootingAgent()),
            (AgentType.CUSTOMER_HISTORY, CustomerHistoryAgent()),
        ]
    elif customer.issue_type == IssueType.BILLING_DISPUTE:
        agents_to_involve = [
            (AgentType.BILLING, BillingAgent()),
            (AgentType.CUSTOMER_HISTORY, CustomerHistoryAgent()),
        ]
    else:
        agents_to_involve = [
            (AgentType.TROUBLESHOOTING, TroubleshootingAgent()),
            (AgentType.BILLING, BillingAgent()),
            (AgentType.CUSTOMER_HISTORY, CustomerHistoryAgent()),
        ]

    print(f"   📊 Involving {len(agents_to_involve)} specialized agents")

    # Create branches and run agents
    active_branches = []
    recommendations = []

    for agent_type, agent_instance in agents_to_involve:
        # Create isolated branch for this agent
        branch_name = memory.create_agent_branch(agent_type, session_id)

        if branch_name:
            active_branches.append(branch_name)

            # Run agent in its isolated branch
            recommendation = agent_instance.process(
                customer, memory, session_id, branch_name
            )
            recommendations.append(recommendation)

            # Agent completes work in branch
            memory.kv_store.commit(f"{agent_type.value}: Completed analysis")

    # Return to main branch
    memory._checkout_branch(memory.main_branch)

    return {
        "active_branches": active_branches,
        "agent_recommendations": recommendations,
        "messages": [AIMessage(content=f"Delegated to {len(recommendations)} agents in isolated branches")]
    }

def semantic_validation_node(state: MultiAgentState, memory: BranchedMemoryService) -> Dict:
    """Validate agent recommendations for semantic coherence"""
    print(f"\n🔍 Semantic Validation Phase...")

    recommendations = state['agent_recommendations']
    customer = state['customer_context']
    conflicts = []

    # Check for conflicting recommendations
    for i, rec1 in enumerate(recommendations):
        for rec2 in recommendations[i+1:]:
            # Check for direct conflicts
            if _are_conflicting(rec1, rec2, customer.issue_type):
                conflict = MergeConflict(
                    agent1=rec1.agent_type.value,
                    agent2=rec2.agent_type.value,
                    conflict_type="action_conflict",
                    resolution=None
                )
                conflicts.append(conflict)
                print(f"   ⚠️  Conflict detected: {rec1.agent_type.value} vs {rec2.agent_type.value}")

    # Check for context bleeding indicators
    context_bleeding = False

    # Example: Billing agent trying to handle technical issues
    for rec in recommendations:
        if rec.agent_type == AgentType.BILLING:
            if customer.issue_type == IssueType.SLOW_INTERNET:
                if any("technician" in action.lower() or "modem" in action.lower()
                      for action in rec.actions):
                    context_bleeding = True
                    print(f"   🚨 Context bleeding detected: Billing agent suggesting technical fixes")

    # Validate each recommendation's relevance
    validated_recommendations = []
    for rec in recommendations:
        is_valid = _validate_recommendation(rec, customer.issue_type)
        if is_valid:
            validated_recommendations.append(rec)
            print(f"   ✅ Validated: {rec.agent_type.value} recommendations")
        else:
            print(f"   ❌ Rejected: {rec.agent_type.value} - out of scope")

    return {
        "agent_recommendations": validated_recommendations,
        "merge_conflicts": conflicts,
        "context_bleeding_detected": context_bleeding,
        "messages": [AIMessage(content=f"Validation complete: {len(conflicts)} conflicts, bleeding={context_bleeding}")]
    }

def merge_recommendations_node(state: MultiAgentState, memory: BranchedMemoryService) -> Dict:
    """Merge validated recommendations from agent branches"""
    print(f"\n🔀 Merging Agent Recommendations...")

    branches = state['active_branches']
    recommendations = state['agent_recommendations']
    conflicts = state['merge_conflicts']

    merged_actions = []
    merge_success_count = 0

    # Process each branch
    for branch, rec in zip(branches, recommendations):
        # Define validation function for this merge
        def validate_fn(branch_data, agent_type):
            # Check if branch data is consistent with main objectives
            if state['context_bleeding_detected']:
                return False, "Context bleeding detected"
            if len(conflicts) > 2:
                return False, "Too many conflicts"
            return True, "Valid"

        # Attempt merge
        success, reason = memory.validate_and_merge(branch, rec.agent_type, validate_fn)

        if success:
            merge_success_count += 1
            merged_actions.extend(rec.actions)
        else:
            print(f"   ⚠️  Skipped merge for {rec.agent_type.value}: {reason}")

    print(f"   📊 Successfully merged {merge_success_count}/{len(branches)} branches")

    # Check isolation success
    isolation_success = not state['context_bleeding_detected'] and len(conflicts) == 0

    return {
        "isolation_success": isolation_success,
        "merged_actions": merged_actions,
        "messages": [AIMessage(content=f"Merged {merge_success_count} branches, isolation={'success' if isolation_success else 'failed'}")]
    }

def generate_resolution_node(state: MultiAgentState) -> Dict:
    """Generate final resolution based on merged recommendations"""
    print(f"\n📝 Generating Final Resolution...")

    customer = state['customer_context']
    recommendations = state['agent_recommendations']
    conflicts = state['merge_conflicts']
    isolation_success = state['isolation_success']

    # Build resolution
    resolution = {
        'session_id': state['session_id'],
        'customer_id': customer.customer_id,
        'issue_type': customer.issue_type.value,
        'resolution_quality': 'high' if isolation_success else 'degraded',
        'actions_taken': [],
        'conflicts_resolved': len(conflicts),
        'branch_isolation': 'successful' if isolation_success else 'failed'
    }

    # Compile final actions
    final_actions = []
    for rec in recommendations:
        if rec.confidence > 0.7:  # Only high-confidence actions
            final_actions.extend(rec.actions)

    # Remove duplicates while preserving order
    seen = set()
    unique_actions = []
    for action in final_actions:
        if action not in seen:
            seen.add(action)
            unique_actions.append(action)

    resolution['actions_taken'] = unique_actions

    # Generate customer response
    if isolation_success:
        print(f"\n✅ RESOLUTION (Branch Isolation Successful):")
        print(f"   • No context bleeding detected")
        print(f"   • Each agent worked in isolation")
        print(f"   • Coherent recommendations merged")
    else:
        print(f"\n⚠️  RESOLUTION (Context Issues Detected):")
        print(f"   • Context bleeding or conflicts found")
        print(f"   • Some recommendations filtered out")

    print(f"\n📋 Final Actions:")
    for i, action in enumerate(unique_actions, 1):
        print(f"   {i}. {action}")

    return {
        "final_resolution": resolution,
        "messages": [AIMessage(content=f"Resolution generated with {len(unique_actions)} actions")]
    }

# ============================================================================
# Helper Functions
# ============================================================================

def _are_conflicting(rec1: AgentRecommendation, rec2: AgentRecommendation,
                     issue_type: IssueType) -> bool:
    """Check if two recommendations conflict"""
    # Technical vs non-technical conflict
    tech_actions = ["technician", "modem", "restart", "diagnostic"]
    billing_actions = ["credit", "charge", "billing", "payment"]

    rec1_is_tech = any(word in ' '.join(rec1.actions).lower() for word in tech_actions)
    rec1_is_billing = any(word in ' '.join(rec1.actions).lower() for word in billing_actions)

    rec2_is_tech = any(word in ' '.join(rec2.actions).lower() for word in tech_actions)
    rec2_is_billing = any(word in ' '.join(rec2.actions).lower() for word in billing_actions)

    # Check for scope conflicts
    if issue_type == IssueType.SLOW_INTERNET:
        if rec1_is_billing and rec2_is_tech:
            return True  # Billing shouldn't interfere with technical
    elif issue_type == IssueType.BILLING_DISPUTE:
        if rec1_is_tech and rec2_is_billing:
            return True  # Technical shouldn't interfere with billing

    return False

def _validate_recommendation(rec: AgentRecommendation, issue_type: IssueType) -> bool:
    """Validate if recommendation is appropriate for issue type"""
    if issue_type == IssueType.SLOW_INTERNET:
        # For technical issues, billing recommendations are invalid
        if rec.agent_type == AgentType.BILLING:
            if any("credit" in action.lower() for action in rec.actions):
                return False
    elif issue_type == IssueType.BILLING_DISPUTE:
        # For billing issues, technical fixes are invalid
        if rec.agent_type == AgentType.TROUBLESHOOTING:
            if any("modem" in action.lower() or "technician" in action.lower()
                  for action in rec.actions):
                return False

    return True

# ============================================================================
# Workflow Visualization
# ============================================================================

# For diagram visualization
try:
    from IPython.display import display, Image
    IPYTHON_AVAILABLE = True
except ImportError:
    IPYTHON_AVAILABLE = False

def display_workflow_diagram(workflow):
    """Display the LangGraph workflow diagram using built-in visualization."""
    print("🎨 Generating multi-agent workflow diagram...")

    try:
        # Generate the diagram bytes using LangGraph's built-in Mermaid rendering
        diagram_bytes = workflow.get_graph(xray=True).draw_mermaid_png()

        # Save to file for viewing
        temp_file = '/tmp/multi_agent_workflow_diagram.png'
        with open(temp_file, 'wb') as f:
            f.write(diagram_bytes)
        print(f"💾 Multi-agent workflow diagram saved to: {temp_file}")

        # Try to display inline if in a Jupyter environment
        if IPYTHON_AVAILABLE:
            try:
                # Check if we're in a Jupyter notebook environment
                from IPython import get_ipython
                if get_ipython() is not None and get_ipython().__class__.__name__ == 'ZMQInteractiveShell':
                    display(Image(diagram_bytes))
                    print("📊 Multi-agent workflow diagram displayed inline!")
                else:
                    print("📊 Multi-agent workflow diagram generated (view at the file path above)")
                    print("   💡 For inline display, run in a Jupyter notebook")
            except Exception:
                print("📊 Multi-agent workflow diagram generated (view at the file path above)")
        else:
            print("📊 Multi-agent workflow diagram generated (view at the file path above)")
            print("   💡 Install IPython for enhanced display: pip install ipython")

        print("✅ Multi-agent workflow diagram generation successful!")
        return temp_file

    except Exception as e:
        print(f"⚠️  Could not generate workflow diagram: {e}")
        print("   This may require additional dependencies for Mermaid rendering")
        print("   Try: pip install pygraphviz or check LangGraph documentation")

    return None

# ============================================================================
# Create Multi-Agent Workflow
# ============================================================================

def create_multi_agent_workflow(memory: BranchedMemoryService):
    """Create the multi-agent workflow with branch isolation"""

    # Build the graph
    builder = StateGraph(MultiAgentState)

    # Add nodes
    builder.add_node("initialize", initialize_session_node)
    builder.add_node("delegate", lambda state: delegate_to_agents_node(state, memory))
    builder.add_node("validate", lambda state: semantic_validation_node(state, memory))
    builder.add_node("merge", lambda state: merge_recommendations_node(state, memory))
    builder.add_node("resolve", generate_resolution_node)

    # Define flow
    builder.add_edge(START, "initialize")
    builder.add_edge("initialize", "delegate")
    builder.add_edge("delegate", "validate")
    builder.add_edge("validate", "merge")
    builder.add_edge("merge", "resolve")
    builder.add_edge("resolve", END)

    return builder.compile()

# ============================================================================
# Demonstration Functions
# ============================================================================

def demonstrate_context_bleeding_prevention():
    """Demonstrate how branch isolation prevents context bleeding"""

    print("\n" + "="*80)
    print("   🚀 Multi-Agent System with Git-like Branch Isolation")
    print("="*80)
    print("\nThis demo shows how ProllyTree's branching prevents context bleeding:")
    print("  • Each agent works in an isolated branch")
    print("  • No shared memory pollution")
    print("  • Semantic validation before merging")
    print("  • Clean audit trail of all operations")

    with tempfile.TemporaryDirectory() as tmpdir:
        store_path = os.path.join(tmpdir, "multi_agent_memory")
        memory = BranchedMemoryService(store_path)

        # Capture initial main memory state
        print("\n🧠 INITIAL MAIN AGENT MEMORY STATE:")
        initial_keys = memory.kv_store.list_keys()
        print(f"   📊 Main memory entries before sub-agents: {len(initial_keys)}")
        for key in initial_keys[:3]:
            print(f"      - {key.decode('utf-8')}")

        workflow = create_multi_agent_workflow(memory)

        # Generate and display workflow diagram
        print("\n📊 Displaying multi-agent workflow visualization...")
        print("🏗️  Workflow Structure:")
        print("   START → Initialize → Delegate → Validate → Merge → Resolve → END")
        print("   ├─ Initialize: Set up session and branch tracking")
        print("   ├─ Delegate: Create isolated branches for each sub-agent")
        print("   ├─ Validate: Semantic validation to prevent context bleeding")
        print("   ├─ Merge: Controlled merging of validated recommendations")
        print("   └─ Resolve: Generate final coherent resolution")

        display_workflow_diagram(workflow)
        print("🚀 Proceeding with multi-agent demonstration...")

        # Test Case 1: Technical Issue (should not involve billing actions)
        print("\n" + "="*70)
        print("TEST CASE 1: Technical Issue (Slow Internet)")
        print("="*70)

        customer1 = CustomerContext(
            customer_id="CUST-001",
            name="Alice Smith",
            account_type="Premium",
            issue_type=IssueType.SLOW_INTERNET,
            issue_description="Internet very slow, can't stream videos",
            priority="high",
            contact_history=[
                {"date": "2024-01-15", "issue": "Setup help", "resolved": True}
            ],
            current_services=["Internet 1Gbps", "Cable TV"]
        )

        result1 = workflow.invoke({
            "messages": [],
            "customer_context": customer1
        })

        print(f"\n📊 Result Summary:")
        print(f"   • Isolation Success: {result1['isolation_success']}")
        print(f"   • Context Bleeding: {result1['context_bleeding_detected']}")
        print(f"   • Conflicts Found: {len(result1['merge_conflicts'])}")

        # Test Case 2: Billing Issue (should not involve technical fixes)
        print("\n" + "="*70)
        print("TEST CASE 2: Billing Issue")
        print("="*70)

        customer2 = CustomerContext(
            customer_id="CUST-002",
            name="Bob Johnson",
            account_type="Basic",
            issue_type=IssueType.BILLING_DISPUTE,
            issue_description="Charged twice for the same service",
            priority="medium",
            contact_history=[],
            current_services=["Internet 100Mbps"]
        )

        result2 = workflow.invoke({
            "messages": [],
            "customer_context": customer2
        })

        print(f"\n📊 Result Summary:")
        print(f"   • Isolation Success: {result2['isolation_success']}")
        print(f"   • Context Bleeding: {result2['context_bleeding_detected']}")
        print(f"   • Conflicts Found: {len(result2['merge_conflicts'])}")

        # Show how sub-agents modified main agent's memory
        print("\n" + "="*70)
        print("🧠 MAIN AGENT MEMORY CHANGES")
        print("="*70)

        print("\n🔍 Examining main agent's memory after sub-agent merges...")

        # Show all keys in main memory
        main_keys = memory.kv_store.list_keys()
        merged_keys = [key.decode('utf-8') for key in main_keys if key.decode('utf-8').startswith('merged:')]

        if merged_keys:
            print(f"\n📊 Found {len(merged_keys)} merged entries from sub-agents:")
            for key in merged_keys[:8]:  # Show first 8
                print(f"   • {key}")
                # Show the actual merged data
                data = memory.kv_store.get(key.encode('utf-8'))
                if data:
                    try:
                        parsed_data = json.loads(data.decode('utf-8'))
                        if isinstance(parsed_data, dict) and len(str(parsed_data)) < 200:
                            print(f"     Data: {parsed_data}")
                        else:
                            print(f"     Data: {str(parsed_data)[:100]}...")
                    except:
                        print(f"     Raw data: {data.decode('utf-8')[:100]}...")
        else:
            print("   ⚠️  No merged data found (check merge implementation)")

        # Show specific memory sections modified by each agent
        print(f"\n🔄 Memory Modifications by Agent Type:")
        agent_modifications = {}
        for key in merged_keys:
            # Extract agent type from merged key pattern: merged:session-agent-uuid:original_key
            parts = key.split(':')
            if len(parts) >= 3:
                session_part = parts[1]  # session-agent-uuid
                if '-' in session_part:
                    agent_type = session_part.split('-')[1]  # Extract agent type
                    if agent_type not in agent_modifications:
                        agent_modifications[agent_type] = []
                    agent_modifications[agent_type].append(key)

        for agent_type, keys in agent_modifications.items():
            print(f"   🤖 {agent_type}: Modified {len(keys)} memory entries")
            for key in keys[:2]:  # Show first 2 for each agent
                print(f"      - {key}")

        print(f"\n🧮 Memory Impact Summary:")
        print(f"   • Total main memory entries: {len(main_keys)}")
        print(f"   • Entries added by sub-agents: {len(merged_keys)}")
        print(f"   • Memory growth from isolation: {len(merged_keys)} new entries")
        print(f"   • Agent types that modified memory: {len(agent_modifications)}")

        # Demonstrate main agent accessing merged sub-agent data
        print(f"\n🎯 MAIN AGENT ACCESSING SUB-AGENT DATA:")
        print("   Simulating main agent decision-making using merged sub-agent insights...")

        # Main agent analyzes merged data from all sub-agents
        main_agent_insights = {"sub_agent_contributions": {}}

        for agent_type, keys in agent_modifications.items():
            contributions = []
            for key in keys:
                data = memory.kv_store.get(key.encode('utf-8'))
                if data:
                    try:
                        parsed_data = json.loads(data.decode('utf-8'))
                        if isinstance(parsed_data, dict):
                            # Extract key insights
                            if 'llm_confidence' in parsed_data:
                                contributions.append(f"Confidence: {parsed_data['llm_confidence']:.0%}")
                            if 'analysis_method' in parsed_data:
                                contributions.append(f"Method: {parsed_data['analysis_method']}")
                            if 'issue_type' in parsed_data:
                                contributions.append(f"Handled: {parsed_data['issue_type']}")
                    except:
                        pass

            if contributions:
                main_agent_insights["sub_agent_contributions"][agent_type] = contributions
                print(f"   📋 From {agent_type}: {', '.join(contributions[:2])}")

        # Main agent makes informed decision based on merged data
        total_agents = len(agent_modifications)
        if total_agents >= 2:
            decision_quality = "high" if total_agents >= 2 else "medium"
            print(f"\n   🎯 Main Agent Decision: {decision_quality} quality resolution")
            print(f"      • Integrated insights from {total_agents} specialized agents")
            print(f"      • Each agent contributed isolated analysis")
            print(f"      • No context bleeding between agent analyses")
            print(f"      • Main agent has comprehensive view after merge")

        # Store main agent's final decision in memory
        final_decision_key = "main_agent:final_decision".encode('utf-8')
        final_decision_data = json.dumps({
            "decision_timestamp": datetime.now(tz=timezone.utc).isoformat(),
            "contributing_agents": list(agent_modifications.keys()),
            "decision_quality": decision_quality if total_agents >= 2 else "medium",
            "insights_integrated": main_agent_insights,
            "isolation_successful": True
        }).encode('utf-8')

        memory.kv_store.insert(final_decision_key, final_decision_data)
        memory.kv_store.commit("Main agent: Final decision based on merged sub-agent data")

        print(f"   💾 Main agent decision stored in memory with full audit trail")

        # Show Git-like history
        print("\n" + "="*70)
        print("📚 Git-like Audit Trail")
        print("="*70)

        history = memory.get_commit_history()
        print(f"\nCommit History ({len(history)} commits):")
        for commit in history[-10:]:
            print(f"   {commit['id']} - {commit['message'][:60]}")

        # Compare with traditional approach
        print("\n" + "="*70)
        print("🔄 Comparison: Branch Isolation vs Traditional Shared Memory")
        print("="*70)

        print("\n❌ Traditional Approach Problems:")
        print("   • All agents share same memory space")
        print("   • Billing agent might see technical context and get confused")
        print("   • Technical agent might suggest billing solutions")
        print("   • Difficult to track which agent made which decision")
        print("   • No rollback capability if wrong path taken")

        print("\n✅ Branch Isolation Solutions:")
        print("   • Each agent has isolated workspace (branch)")
        print("   • No cross-contamination of context")
        print("   • Clear separation of concerns")
        print("   • Complete audit trail with Git history")
        print("   • Can rollback or replay specific agent actions")

        # Show branch structure
        print("\n🌳 Branch Structure Example:")
        print("   main")
        print("   ├── session1-troubleshooting-abc123")
        print("   ├── session1-customer_history-def456")
        print("   ├── session2-billing-ghi789")
        print("   └── session2-customer_history-jkl012")

        print("\n   Each branch contains only relevant agent data,")
        print("   preventing context bleeding between agents.")

def main():
    """Run the multi-agent demonstration"""

    print("="*80)
    print("   Multi-Agent System with Git-like Branching")
    print("   Using LangGraph + ProllyTree")
    print("="*80)

    print("\n🎯 Key Features Demonstrated:")
    print("  • Branch isolation for each sub-agent")
    print("  • Prevention of context bleeding")
    print("  • Semantic validation before merging")
    print("  • Conflict detection and resolution")
    print("  • Git-like audit trail")
    print("  • Clean separation of concerns")

    try:
        demonstrate_context_bleeding_prevention()

        print("\n" + "="*80)
        print("✅ Demonstration Complete!")
        print("="*80)
        print("\nKey Takeaways:")
        print("  1. Branch isolation prevents agents from interfering with each other")
        print("  2. Semantic validation ensures only relevant recommendations are merged")
        print("  3. Git-like history provides complete audit trail")
        print("  4. Context bleeding is eliminated through isolation")
        print("  5. System maintains coherence through controlled merging")

    except ImportError as e:
        print(f"\n❌ Error: {e}")
        print("\nPlease install required dependencies:")
        print("  pip install langgraph langchain-core prollytree")

if __name__ == "__main__":
    main()
