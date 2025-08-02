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
Simplified LangGraph + ProllyTree Integration Demo

This example shows how to use ProllyTree as a versioned memory backend
for LangGraph applications, enabling Git-like versioning of AI agent memory.
"""

import os
import tempfile
import json
import base64
from datetime import datetime
from typing import Any, Dict, List, Optional, Tuple
from prollytree import VersionedKvStore

# Try to import LangGraph, fall back to mock if not available
try:
    from langgraph.checkpoint.base import BaseCheckpointSaver, Checkpoint
    from langgraph.store.base import BaseStore
    from langgraph.errors import InvalidUpdateError
    LANGGRAPH_AVAILABLE = True
except ImportError:
    LANGGRAPH_AVAILABLE = False
    print("⚠️  LangGraph not installed. Using mock interfaces.")

    # Mock interfaces for demonstration
    class Checkpoint:
        def __init__(self, **kwargs):
            self.__dict__.update(kwargs)

    class BaseStore:
        def __init__(self):
            pass

        def search(self, *args, **kwargs):
            return []

        def put(self, *args, **kwargs):
            pass

        def get(self, *args, **kwargs):
            return {}

    class BaseCheckpointSaver:
        def __init__(self):
            pass


class SimpleProllyStore(BaseStore):
    """Simplified ProllyTree-backed store for LangGraph."""

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
        """Async batch operations - just calls batch for now."""
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

    def search(self, prefix: str) -> List[Tuple[str, Any]]:
        """Search for items by prefix."""
        results = []
        # Use list_keys() method which is available
        try:
            keys = self.kv_store.list_keys()
            for key in keys:
                key_str = key.decode('utf-8')
                if key_str.startswith(prefix):
                    value = self.kv_store.get(key)
                    decoded_value = self._decode_value(value)
                    results.append((key_str, decoded_value))
        except AttributeError:
            # Fallback if list_keys not available
            pass
        return results

    def put(self, key: str, value: Any) -> None:
        """Store a value with a specific key."""
        key_bytes = key.encode('utf-8')
        value_bytes = self._encode_value(value)

        # Check if key exists to decide between insert/update
        existing = self.kv_store.get(key_bytes)
        if existing:
            self.kv_store.update(key_bytes, value_bytes)
            print(f"   📝 Updated: {key}")
        else:
            self.kv_store.insert(key_bytes, value_bytes)
            print(f"   ➕ Inserted: {key}")

    def get(self, key: str) -> Optional[Any]:
        """Retrieve a value by key."""
        key_bytes = key.encode('utf-8')
        data = self.kv_store.get(key_bytes)
        return self._decode_value(data) if data else None

    def delete(self, key: str) -> None:
        """Delete a key from the store."""
        key_bytes = key.encode('utf-8')
        self.kv_store.delete(key_bytes)
        print(f"   ❌ Deleted: {key}")

    def commit(self, message: str) -> str:
        """Create a Git-like commit of current state."""
        commit_id = self.kv_store.commit(message)
        print(f"   💾 Committed: {commit_id[:8]} - {message}")
        return commit_id

    def history(self, limit: int = 10) -> List[Dict]:
        """Get commit history."""
        return self.kv_store.log()[-limit:]


class SimpleProllyCheckpointSaver(BaseCheckpointSaver):
    """Simplified checkpoint saver using ProllyTree."""

    def __init__(self, store: SimpleProllyStore):
        """Initialize with a ProllyTree store."""
        super().__init__()
        self.store = store

    def put(self, config: dict, checkpoint: dict, metadata: dict) -> dict:
        """Save a checkpoint."""
        thread_id = config.get("configurable", {}).get("thread_id", "default")
        checkpoint_id = checkpoint.get("id", str(datetime.now().timestamp()))

        # Store checkpoint
        checkpoint_key = f"checkpoint:{thread_id}:{checkpoint_id}"
        self.store.put(checkpoint_key, checkpoint)

        # Store metadata
        metadata_key = f"metadata:{thread_id}:{checkpoint_id}"
        self.store.put(metadata_key, metadata)

        # Commit with descriptive message
        self.store.commit(f"Checkpoint {checkpoint_id[:8]} for thread {thread_id}")

        return {"configurable": {"thread_id": thread_id, "checkpoint_id": checkpoint_id}}

    def get_tuple(self, config: dict) -> Optional[Tuple]:
        """Get a checkpoint tuple."""
        thread_id = config.get("configurable", {}).get("thread_id", "default")
        checkpoint_id = config.get("configurable", {}).get("checkpoint_id")

        if not checkpoint_id:
            # Get latest checkpoint for thread
            checkpoints = self.store.search(f"checkpoint:{thread_id}:")
            if not checkpoints:
                return None
            checkpoint_key, checkpoint = checkpoints[-1]
            checkpoint_id = checkpoint_key.split(":")[-1]
        else:
            checkpoint_key = f"checkpoint:{thread_id}:{checkpoint_id}"
            checkpoint = self.store.get(checkpoint_key)
            if not checkpoint:
                return None

        metadata_key = f"metadata:{thread_id}:{checkpoint_id}"
        metadata = self.store.get(metadata_key) or {}

        config = {"configurable": {"thread_id": thread_id, "checkpoint_id": checkpoint_id}}

        # Return proper tuple format
        return Checkpoint(**checkpoint), metadata, config

    def list(self, config: dict) -> List[Tuple]:
        """List all checkpoints for a thread."""
        thread_id = config.get("configurable", {}).get("thread_id", "default")
        checkpoints = self.store.search(f"checkpoint:{thread_id}:")

        results = []
        for checkpoint_key, checkpoint in checkpoints:
            checkpoint_id = checkpoint_key.split(":")[-1]
            metadata_key = f"metadata:{thread_id}:{checkpoint_id}"
            metadata = self.store.get(metadata_key) or {}
            config = {"configurable": {"thread_id": thread_id, "checkpoint_id": checkpoint_id}}
            results.append((Checkpoint(**checkpoint), metadata, config))

        return results


def demo_memory_persistence():
    """Demonstrate cross-thread memory persistence."""
    print("\n=== Cross-Thread Memory Persistence ===\n")

    with tempfile.TemporaryDirectory() as tmpdir:
        store_path = os.path.join(tmpdir, "agent_memory")
        store = SimpleProllyStore(store_path)

        # Thread 1: Planning session
        print("📝 Thread 1 (Planning):")
        store.put("project:goals", ["Build AI agent", "Add memory", "Deploy"])
        store.put("project:status", "planning")
        store.commit("Thread 1: Initial planning session")

        # Thread 2: Development session
        print("\n💻 Thread 2 (Development):")
        goals = store.get("project:goals")
        print(f"   Retrieved goals: {goals}")
        store.put("project:status", "in_progress")
        store.put("dev:current_task", "Implementing memory layer")
        store.commit("Thread 2: Started development")

        # Thread 3: Review session
        print("\n🔍 Thread 3 (Review):")
        status = store.get("project:status")
        current_task = store.get("dev:current_task")
        print(f"   Project status: {status}")
        print(f"   Current task: {current_task}")
        store.put("review:feedback", "Memory implementation looks good")
        store.commit("Thread 3: Code review completed")

        # Show history
        print("\n📚 Commit History:")
        for commit in store.history(5):
            timestamp = datetime.fromtimestamp(commit['timestamp'])
            print(f"   {commit['id'][:8]} - {commit['message']} ({timestamp.strftime('%H:%M:%S')})")


def demo_checkpoint_saving():
    """Demonstrate checkpoint saving and retrieval."""
    print("\n=== Checkpoint Management ===\n")

    with tempfile.TemporaryDirectory() as tmpdir:
        store_path = os.path.join(tmpdir, "checkpoints")
        store = SimpleProllyStore(store_path)
        saver = SimpleProllyCheckpointSaver(store)

        # Save checkpoint for thread 1
        print("💾 Saving checkpoints:")
        checkpoint1 = {
            "id": "ckpt_001",
            "state": {"messages": ["Hello", "How can I help?"], "context": "greeting"}
        }
        config1 = {"configurable": {"thread_id": "conv_123"}}
        metadata1 = {"user": "alice", "timestamp": datetime.now().isoformat()}

        saver.put(config1, checkpoint1, metadata1)

        # Save another checkpoint
        checkpoint2 = {
            "id": "ckpt_002",
            "state": {"messages": ["Hello", "How can I help?", "Tell me about AI"], "context": "ai_discussion"}
        }
        metadata2 = {"user": "alice", "timestamp": datetime.now().isoformat()}

        saver.put(config1, checkpoint2, metadata2)

        # Retrieve latest checkpoint
        print("\n🔍 Retrieving latest checkpoint:")
        result = saver.get_tuple(config1)
        if result:
            checkpoint, metadata, config = result
            print(f"   Checkpoint ID: {checkpoint.id if hasattr(checkpoint, 'id') else 'N/A'}")
            print(f"   State: {checkpoint.state if hasattr(checkpoint, 'state') else 'N/A'}")

        # List all checkpoints
        print("\n📋 All checkpoints for thread:")
        checkpoints = saver.list(config1)
        for ckpt, meta, cfg in checkpoints:
            print(f"   - {ckpt.id if hasattr(ckpt, 'id') else 'Unknown'}: {meta.get('timestamp', 'N/A')}")


def main():
    """Run all demonstrations."""
    print("=" * 60)
    print("   Simplified LangGraph + ProllyTree Integration Demo")
    print("=" * 60)

    demo_memory_persistence()
    demo_checkpoint_saving()

    print("\n" + "=" * 60)
    print("✅ Demo Complete! Key Features Demonstrated:")
    print("   • Cross-thread memory persistence")
    print("   • Git-like versioning of all changes")
    print("   • Checkpoint management for conversations")
    print("   • Complete audit trail with commit history")
    print("=" * 60)


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"\n❌ Error: {e}")
        print("\nTroubleshooting:")
        print("1. Ensure ProllyTree is installed:")
        print("   pip install ../../target/wheels/prollytree-*.whl")
        print("2. For full LangGraph integration, install:")
        print("   pip install langgraph langchain")
