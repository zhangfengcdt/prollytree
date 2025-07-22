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