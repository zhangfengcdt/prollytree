"""
ProllyTree - Python bindings for the Rust ProllyTree implementation

A Prolly Tree is a hybrid data structure that combines B-trees and Merkle trees
to provide efficient data access with verifiable integrity.
"""

from .prollytree import ProllyTree, TreeConfig

__version__ = "0.2.1"
__all__ = ["ProllyTree", "TreeConfig"]