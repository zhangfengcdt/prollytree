"""Type stubs for ProllyTree Python bindings"""

from typing import Optional, Dict, List, Tuple, Union

class TreeConfig:
    """Configuration for ProllyTree"""

    def __init__(
        self,
        base: int = 4,
        modulus: int = 64,
        min_chunk_size: int = 1,
        max_chunk_size: int = 4096,
        pattern: int = 0
    ) -> None: ...

class ProllyTree:
    """A probabilistic tree for efficient storage and retrieval of ordered data"""

    def __init__(
        self,
        storage_type: str = "memory",
        path: Optional[str] = None,
        config: Optional[TreeConfig] = None
    ) -> None:
        """
        Create a new ProllyTree instance.

        Args:
            storage_type: Type of storage to use ("memory" or "file")
            path: Path for file storage (required if storage_type is "file")
            config: Tree configuration (uses defaults if not provided)
        """
        ...

    def insert(self, key: bytes, value: bytes) -> None:
        """Insert a key-value pair into the tree"""
        ...

    def insert_batch(self, items: List[Tuple[bytes, bytes]]) -> None:
        """Insert multiple key-value pairs in a single batch operation"""
        ...

    def find(self, key: bytes) -> Optional[bytes]:
        """Find and return the value associated with a key"""
        ...

    def update(self, key: bytes, value: bytes) -> None:
        """Update the value for an existing key"""
        ...

    def delete(self, key: bytes) -> None:
        """Delete a key from the tree"""
        ...

    def delete_batch(self, keys: List[bytes]) -> None:
        """Delete multiple keys in a single batch operation"""
        ...

    def size(self) -> int:
        """Return the number of key-value pairs in the tree"""
        ...

    def depth(self) -> int:
        """Return the depth of the tree"""
        ...

    def get_root_hash(self) -> bytes:
        """Get the root hash of the tree"""
        ...

    def stats(self) -> Dict[str, int]:
        """Get statistics about the tree structure"""
        ...

    def generate_proof(self, key: bytes) -> bytes:
        """Generate a Merkle proof for a key"""
        ...

    def verify_proof(
        self,
        proof: bytes,
        key: bytes,
        expected_value: Optional[bytes] = None
    ) -> bool:
        """Verify a Merkle proof for a key"""
        ...

    def diff(self, other: "ProllyTree") -> Dict[str, Union[Dict[bytes, bytes], Dict[bytes, Dict[str, bytes]]]]:
        """
        Compare two trees and return the differences from this tree to other.

        Note: Currently returns empty results due to implementation limitations.
        The diff operation works at the tree structure level, but probabilistic trees
        can have different structures for the same logical data.

        Returns a dictionary with:
        - "added": Dict of keys/values present in other but not in this tree
        - "removed": Dict of keys/values present in this tree but not in other
        - "modified": Dict of keys with different values (maps to {"old": value_in_self, "new": value_in_other})
        """
        ...

    def traverse(self) -> str:
        """Return a string representation of the tree structure"""
        ...

    def save_config(self) -> None:
        """Save the tree configuration to storage"""
        ...

class MemoryType:
    """Enum representing different types of memory in the agent system"""
    ShortTerm: "MemoryType"
    Semantic: "MemoryType"
    Episodic: "MemoryType"
    Procedural: "MemoryType"

    def __str__(self) -> str: ...

class AgentMemorySystem:
    """Comprehensive memory system for AI agents"""

    def __init__(self, path: str, agent_id: str) -> None:
        """
        Initialize a new agent memory system.

        Args:
            path: Directory path for memory storage
            agent_id: Unique identifier for the agent
        """
        ...

    @staticmethod
    def open(path: str, agent_id: str) -> "AgentMemorySystem":
        """
        Open an existing agent memory system.

        Args:
            path: Directory path where memory is stored
            agent_id: Unique identifier for the agent
        """
        ...

    def store_conversation_turn(
        self,
        thread_id: str,
        role: str,
        content: str,
        metadata: Optional[Dict[str, str]] = None
    ) -> str:
        """
        Store a conversation turn in short-term memory.

        Args:
            thread_id: Conversation thread identifier
            role: Role of the speaker (e.g., "user", "assistant")
            content: The message content
            metadata: Optional metadata dictionary

        Returns:
            Unique ID of the stored memory
        """
        ...

    def get_conversation_history(
        self,
        thread_id: str,
        limit: Optional[int] = None
    ) -> List[Dict[str, Union[str, float]]]:
        """
        Retrieve conversation history for a thread.

        Args:
            thread_id: Conversation thread identifier
            limit: Maximum number of messages to retrieve

        Returns:
            List of message dictionaries with id, content, and created_at fields
        """
        ...

    def store_fact(
        self,
        entity_type: str,
        entity_id: str,
        facts: str,  # JSON string
        confidence: float,
        source: str
    ) -> str:
        """
        Store a fact in semantic memory.

        Args:
            entity_type: Type of entity (e.g., "person", "place")
            entity_id: Unique identifier for the entity
            facts: JSON string containing the facts
            confidence: Confidence score (0.0 to 1.0)
            source: Source of the information

        Returns:
            Unique ID of the stored fact
        """
        ...

    def get_entity_facts(
        self,
        entity_type: str,
        entity_id: str
    ) -> List[Dict[str, Union[str, float]]]:
        """
        Retrieve facts about an entity.

        Args:
            entity_type: Type of entity
            entity_id: Unique identifier for the entity

        Returns:
            List of fact dictionaries
        """
        ...

    def store_procedure(
        self,
        category: str,
        name: str,
        description: str,
        steps: List[str],  # List of JSON strings
        prerequisites: Optional[List[str]] = None,
        priority: int = 1
    ) -> str:
        """
        Store a procedure in procedural memory.

        Args:
            category: Category of the procedure
            name: Name of the procedure
            description: Description of what the procedure does
            steps: List of JSON strings describing each step
            prerequisites: Optional list of prerequisites
            priority: Priority level (default: 1)

        Returns:
            Unique ID of the stored procedure
        """
        ...

    def get_procedures_by_category(
        self,
        category: str
    ) -> List[Dict[str, str]]:
        """
        Retrieve procedures by category.

        Args:
            category: Category to search for

        Returns:
            List of procedure dictionaries
        """
        ...

    def checkpoint(self, message: str) -> str:
        """
        Create a memory checkpoint.

        Args:
            message: Commit message for the checkpoint

        Returns:
            Checkpoint ID
        """
        ...

    def optimize(self) -> Dict[str, int]:
        """
        Optimize the memory system by cleaning up and consolidating memories.

        Returns:
            Dictionary with optimization statistics
        """
        ...

class StorageBackend:
    """Enum representing different storage backend types"""
    InMemory: "StorageBackend"
    File: "StorageBackend"
    Git: "StorageBackend"

    def __str__(self) -> str: ...

class MergeConflict:
    """Represents a merge conflict between branches"""

    @property
    def key(self) -> bytes:
        """The key that has a conflict"""
        ...

    @property
    def base_value(self) -> Optional[bytes]:
        """The value in the common base commit"""
        ...

    @property
    def source_value(self) -> Optional[bytes]:
        """The value in the source branch"""
        ...

    @property
    def destination_value(self) -> Optional[bytes]:
        """The value in the destination branch"""
        ...

class ConflictResolution:
    """Enum representing different conflict resolution strategies"""
    IgnoreAll: "ConflictResolution"
    TakeSource: "ConflictResolution"
    TakeDestination: "ConflictResolution"

class VersionedKvStore:
    """A versioned key-value store backed by Git and ProllyTree"""

    def __init__(self, path: str) -> None:
        """
        Initialize a new versioned key-value store.

        Args:
            path: Directory path for the store (must be within a git repository)
        """
        ...

    @staticmethod
    def open(path: str) -> "VersionedKvStore":
        """
        Open an existing versioned key-value store.

        Args:
            path: Directory path where the store is located
        """
        ...

    def insert(self, key: bytes, value: bytes) -> None:
        """
        Insert a key-value pair (stages the change).

        Args:
            key: The key as bytes
            value: The value as bytes
        """
        ...

    def get(self, key: bytes) -> Optional[bytes]:
        """
        Get a value by key.

        Args:
            key: The key to look up

        Returns:
            The value as bytes, or None if not found
        """
        ...

    def update(self, key: bytes, value: bytes) -> bool:
        """
        Update an existing key-value pair (stages the change).

        Args:
            key: The key to update
            value: The new value

        Returns:
            True if the key existed and was updated, False otherwise
        """
        ...

    def delete(self, key: bytes) -> bool:
        """
        Delete a key-value pair (stages the change).

        Args:
            key: The key to delete

        Returns:
            True if the key existed and was deleted, False otherwise
        """
        ...

    def list_keys(self) -> List[bytes]:
        """
        List all keys in the store (includes staged changes).

        Returns:
            List of keys as bytes
        """
        ...

    def status(self) -> List[Tuple[bytes, str]]:
        """
        Show current staging area status.

        Returns:
            List of tuples (key, status) where status is "added", "modified", or "deleted"
        """
        ...

    def commit(self, message: str) -> str:
        """
        Commit staged changes.

        Args:
            message: Commit message

        Returns:
            Commit hash as hex string
        """
        ...

    def branch(self, name: str) -> None:
        """
        Create a new branch.

        Args:
            name: Name of the new branch
        """
        ...

    def create_branch(self, name: str) -> None:
        """
        Create a new branch and switch to it.

        Args:
            name: Name of the new branch
        """
        ...

    def checkout(self, branch_or_commit: str) -> None:
        """
        Switch to a different branch or commit.

        Args:
            branch_or_commit: Branch name or commit hash
        """
        ...

    def current_branch(self) -> str:
        """
        Get the current branch name.

        Returns:
            Current branch name
        """
        ...

    def list_branches(self) -> List[str]:
        """
        List all branches in the repository.

        Returns:
            List of branch names
        """
        ...

    def log(self) -> List[Dict[str, Union[str, int]]]:
        """
        Get commit history.

        Returns:
            List of commit dictionaries with id, author, committer, message, and timestamp
        """
        ...

    def get_commits_for_key(self, key: bytes) -> List[Dict[str, Union[str, int]]]:
        """
        Get all commits that contain changes to a specific key.

        Args:
            key: The key to search for

        Returns:
            List of commit dictionaries with id, author, committer, message, and timestamp
        """
        ...

    def get_commit_history(self) -> List[Dict[str, Union[str, int]]]:
        """
        Get the commit history for the repository.

        Returns:
            List of commit dictionaries with id, author, committer, message, and timestamp
        """
        ...

    def merge(
        self,
        source_branch: str,
        conflict_resolution: Optional[ConflictResolution] = None
    ) -> str:
        """
        Merge another branch into the current branch.

        Args:
            source_branch: Name of the branch to merge from
            conflict_resolution: Strategy for resolving conflicts (default: IgnoreAll)

        Returns:
            The commit ID of the merge commit

        Raises:
            ValueError: If merge fails or has unresolved conflicts
        """
        ...

    def try_merge(self, source_branch: str) -> Tuple[bool, List[MergeConflict]]:
        """
        Attempt to merge another branch and return any conflicts.

        Args:
            source_branch: Name of the branch to merge from

        Returns:
            Tuple of (success, conflicts) where:
            - success: True if merge succeeded, False if there were conflicts
            - conflicts: List of MergeConflict objects if success is False
        """
        ...

    def storage_backend(self) -> StorageBackend:
        """
        Get the current storage backend type.

        Returns:
            Storage backend enum value
        """
        ...

    def generate_proof(self, key: bytes) -> bytes:
        """
        Generate a cryptographic proof for a key's existence and value in the versioned store.

        Args:
            key: The key to generate proof for

        Returns:
            Serialized proof as bytes
        """
        ...

    def verify_proof(
        self,
        proof: bytes,
        key: bytes,
        expected_value: Optional[bytes] = None
    ) -> bool:
        """
        Verify a cryptographic proof for a key-value pair in the versioned store.

        Args:
            proof: The serialized proof to verify
            key: The key that the proof claims to prove
            expected_value: Optional expected value to verify against

        Returns:
            True if the proof is valid, False otherwise
        """
        ...
