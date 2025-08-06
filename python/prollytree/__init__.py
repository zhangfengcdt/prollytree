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
ProllyTree - Python bindings for the Rust ProllyTree implementation

A Prolly Tree is a hybrid data structure that combines B-trees and Merkle trees
to provide efficient data access with verifiable integrity.
"""

from .prollytree import (
    ProllyTree,
    TreeConfig,
    AgentMemorySystem,
    MemoryType,
    VersionedKvStore,
    StorageBackend,
    MergeConflict,
    ConflictResolution
)

# Try to import SQL functionality if available
sql_available = False
try:
    from .prollytree import ProllySQLStore
    sql_available = True
except ImportError:
    pass

# Try to import Git functionality if available
git_available = False
try:
    from .prollytree import WorktreeManager, WorktreeVersionedKvStore
    git_available = True
except ImportError:
    pass

# Build __all__ based on available features
__all__ = [
    "ProllyTree",
    "TreeConfig",
    "AgentMemorySystem",
    "MemoryType",
    "VersionedKvStore",
    "StorageBackend",
    "MergeConflict",
    "ConflictResolution"
]

if sql_available:
    __all__.append("ProllySQLStore")

if git_available:
    __all__.extend(["WorktreeManager", "WorktreeVersionedKvStore"])

__version__ = "0.2.1"
