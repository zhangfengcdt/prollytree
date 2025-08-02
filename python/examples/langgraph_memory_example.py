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
LangGraph + ProllyTree Integration

This example demonstrates using ProllyTree as a versioned memory backend
for LangGraph applications, enabling Git-like versioning of AI agent memory.
"""

import os
import tempfile
import json
import base64
from datetime import datetime
from typing import Any, Dict, List, Optional, Tuple
from prollytree import VersionedKvStore

from langgraph.checkpoint.base import BaseCheckpointSaver, Checkpoint
from langgraph.store.base import BaseStore


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

    def put(self, config: dict, checkpoint: Checkpoint, metadata: dict) -> dict:
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

    def get_tuple(self, config: dict) -> Optional[Tuple[Checkpoint, dict, dict]]:
        """Get a checkpoint tuple."""
        thread_id = config.get("configurable", {}).get("thread_id", "default")
        checkpoint_id = config.get("configurable", {}).get("checkpoint_id")

        if not checkpoint_id:
            # Get latest checkpoint for thread
            checkpoints = self.store.search(("checkpoints", thread_id), limit=100)
            if not checkpoints:
                return None
            # Get the last one (most recent)
            _, checkpoint_id, checkpoint_data = checkpoints[-1]
        else:
            checkpoint_data = self.store.get(("checkpoints", thread_id), checkpoint_id)
            if not checkpoint_data:
                return None

        metadata = self.store.get(("metadata", thread_id), checkpoint_id) or {}

        config = {
            "configurable": {
                "thread_id": thread_id,
                "checkpoint_id": checkpoint_id
            }
        }

        # Create Checkpoint object from data
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

        return checkpoint, metadata, config

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


def demonstrate_langgraph_integration():
    """Demonstrate LangGraph + ProllyTree integration."""
    print("\n=== LangGraph + ProllyTree Integration Demo ===\n")

    with tempfile.TemporaryDirectory() as tmpdir:
        store_path = os.path.join(tmpdir, "langgraph_memory")

        # Initialize store and saver
        store = ProllyVersionedMemoryStore(store_path)
        saver = ProllyVersionedMemorySaver(store)

        # Simulate conversation threads
        print("📝 Thread 1: Initial conversation")
        checkpoint1 = Checkpoint(
            id="ckpt_001",
            ts="2025-01-15T10:00:00",
            channel_values={
                "messages": ["Hello", "How can I help you today?"],
                "context": {"topic": "greeting", "user": "alice"}
            }
        )
        config1 = {"configurable": {"thread_id": "conv_alice_001"}}
        metadata1 = {"user": "alice", "session": "morning", "timestamp": datetime.now().isoformat()}

        saver.put(config1, checkpoint1, metadata1)

        print("\n💻 Thread 1: Continuing conversation")
        checkpoint2 = Checkpoint(
            id="ckpt_002",
            ts="2025-01-15T10:05:00",
            channel_values={
                "messages": ["Hello", "How can I help you today?", "Tell me about AI", "AI is fascinating..."],
                "context": {"topic": "ai_discussion", "user": "alice"}
            }
        )
        metadata2 = {"user": "alice", "session": "morning", "timestamp": datetime.now().isoformat()}

        saver.put(config1, checkpoint2, metadata2)

        print("\n🔀 Thread 2: Different user session")
        checkpoint3 = Checkpoint(
            id="ckpt_003",
            ts="2025-01-15T11:00:00",
            channel_values={
                "messages": ["Hi there", "Welcome! What would you like to know?"],
                "context": {"topic": "greeting", "user": "bob"}
            }
        )
        config2 = {"configurable": {"thread_id": "conv_bob_001"}}
        metadata3 = {"user": "bob", "session": "afternoon", "timestamp": datetime.now().isoformat()}

        saver.put(config2, checkpoint3, metadata3)

        # Demonstrate cross-thread memory
        print("\n🔍 Cross-thread shared memory:")
        store.put(("shared", "global"), "current_model", {"name": "gpt-4", "temperature": 0.7})
        store.put(("shared", "global"), "system_prompt", {"content": "You are a helpful assistant"})
        store.commit("Shared configuration updated")

        # Thread 1 reads shared memory
        print("\n📖 Thread 1 reading shared configuration:")
        model_config = store.get(("shared", "global"), "current_model")
        system_prompt = store.get(("shared", "global"), "system_prompt")
        print(f"   Model: {model_config}")
        print(f"   System: {system_prompt}")

        # Retrieve checkpoints
        print("\n🔄 Retrieving latest checkpoint for Thread 1:")
        result = saver.get_tuple(config1)
        if result:
            checkpoint, metadata, config = result
            print(f"   Checkpoint ID: {checkpoint['id']}")
            print(f"   Messages: {len(checkpoint['channel_values'].get('messages', []))} messages")
            print(f"   Context: {checkpoint['channel_values'].get('context', {})}")

        # List all checkpoints
        print("\n📋 All checkpoints:")
        all_checkpoints = saver.list(limit=10)
        for config, checkpoint, metadata in all_checkpoints:
            thread = config["configurable"]["thread_id"]
            user = metadata.get("user", "unknown")
            print(f"   - Thread {thread}: {checkpoint['id']} (user: {user})")

        # Show commit history
        print("\n📚 Git-like commit history:")
        for commit in store.history(10):
            timestamp = datetime.fromtimestamp(commit['timestamp'])
            print(f"   {commit['id'][:8]} - {commit['message']} ({timestamp.strftime('%H:%M:%S')})")


def main():
    """Run the LangGraph + ProllyTree demo."""
    print("=" * 70)
    print("   LangGraph + ProllyTree: Versioned Memory for AI Agents")
    print("=" * 70)

    try:
        demonstrate_langgraph_integration()

        print("\n" + "=" * 70)
        print("✅ Demo Complete! Key Features:")
        print("   • Versioned checkpoint storage with Git-like commits")
        print("   • Cross-thread shared memory for global state")
        print("   • Complete audit trail of all memory operations")
        print("   • Content-addressed storage with deduplication")
        print("   • Native LangGraph BaseStore and BaseCheckpointSaver integration")
        print("=" * 70)

    except ImportError as e:
        print(f"\n❌ Error: {e}")
        print("\nPlease install LangGraph dependencies:")
        print("  pip install langgraph langchain-core")
        print("\nMake sure ProllyTree is also installed:")
        print("  cd ../.. && ./python/build_python.sh --install")


if __name__ == "__main__":
    main()
