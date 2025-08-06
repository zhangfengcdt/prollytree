Quickstart Guide
================

This guide will help you get started with ProllyTree Python bindings quickly.

Installation
------------

From PyPI (Recommended)
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   pip install prollytree

From Source
~~~~~~~~~~~

.. code-block:: bash

   git clone https://github.com/zhangfengcdt/prollytree
   cd prollytree
   ./python/build_python.sh --all-features --install

Basic Usage
-----------

Creating a Tree
~~~~~~~~~~~~~~~

.. code-block:: python

   from prollytree import ProllyTree, TreeConfig

   # Create with default settings
   tree = ProllyTree()

   # Or with custom configuration
   config = TreeConfig(base=4, modulus=64)
   tree = ProllyTree(config=config)

   # Use file storage instead of memory
   tree = ProllyTree(storage_type="file", path="/path/to/data")

Basic Operations
~~~~~~~~~~~~~~~~

.. code-block:: python

   # Insert data
   tree.insert(b"hello", b"world")
   tree.insert(b"foo", b"bar")

   # Find data
   value = tree.find(b"hello")
   print(value)  # b"world"

   # Update existing key
   tree.update(b"hello", b"updated world")

   # Delete a key
   tree.delete(b"foo")

   # Batch operations for efficiency
   items = [(b"key1", b"value1"), (b"key2", b"value2")]
   tree.insert_batch(items)

Working with Text
~~~~~~~~~~~~~~~~~

ProllyTree operates on bytes, but you can easily work with strings:

.. code-block:: python

   def str_to_bytes(s):
       return s.encode('utf-8')

   def bytes_to_str(b):
       return b.decode('utf-8') if b else None

   # Insert string data
   tree.insert(str_to_bytes("name"), str_to_bytes("Alice"))

   # Retrieve string data
   value = tree.find(str_to_bytes("name"))
   name = bytes_to_str(value)
   print(name)  # "Alice"

Versioned Storage
-----------------

ProllyTree provides Git-like versioned storage:

.. code-block:: python

   from prollytree import VersionedKvStore

   # Create a versioned store
   store = VersionedKvStore("/path/to/store")

   # Insert data
   store.insert(b"user:123", b"Alice")
   store.insert(b"user:456", b"Bob")

   # Commit changes
   commit_id = store.commit("Add initial users")

   # Update data
   store.update(b"user:123", b"Alice Smith")
   commit_id2 = store.commit("Update Alice's name")

   # View history
   commits = store.log()
   for commit in commits:
       print(f"{commit['id'][:8]} - {commit['message']}")

Branch Operations & Merging
----------------------------

ProllyTree supports Git-like branching and merging with conflict resolution:

.. code-block:: python

   from prollytree import VersionedKvStore, ConflictResolution

   store = VersionedKvStore("/path/to/store")

   # Set up initial data
   store.insert(b"config:theme", b"light")
   store.insert(b"config:lang", b"en")
   store.commit("Initial configuration")

   # Create and switch to feature branch
   store.create_branch("feature-dark-mode")

   # Make changes on feature branch
   store.update(b"config:theme", b"dark")
   store.insert(b"config:animations", b"enabled")
   store.commit("Add dark mode and animations")

   # Switch back to main branch
   store.checkout("main")

   # Make different changes on main
   store.update(b"config:lang", b"fr")
   store.commit("Change language to French")

   # Merge feature branch with conflict resolution
   try:
       # Attempt merge, taking source values on conflicts
       merge_commit = store.merge("feature-dark-mode", ConflictResolution.TakeSource)
       print(f"Merge successful: {merge_commit[:8]}")
   except Exception as e:
       print(f"Merge failed: {e}")

   # Check for conflicts without applying merge
   success, conflicts = store.try_merge("feature-dark-mode")
   if not success:
       print(f"Found {len(conflicts)} conflicts:")
       for conflict in conflicts:
           print(f"  Key: {conflict.key}")
           print(f"    Source: {conflict.source_value}")
           print(f"    Destination: {conflict.destination_value}")

SQL Queries
-----------

Query your data using SQL (requires building with SQL feature):

.. code-block:: python

   from prollytree import ProllySQLStore

   # Create SQL store
   sql_store = ProllySQLStore("/path/to/sql_store")

   # Create tables and insert data
   sql_store.execute("""
       CREATE TABLE users (
           id INTEGER,
           name TEXT,
           email TEXT
       )
   """)

   sql_store.execute("""
       INSERT INTO users (id, name, email) VALUES
       (1, 'Alice', 'alice@example.com'),
       (2, 'Bob', 'bob@example.com')
   """)

   # Query data
   results = sql_store.execute("SELECT * FROM users WHERE name = 'Alice'")
   print(results)

Agent Memory System
-------------------

For AI applications, ProllyTree provides an advanced memory system:

.. code-block:: python

   from prollytree import AgentMemorySystem, MemoryType

   # Create memory system
   memory = AgentMemorySystem("/path/to/memory")

   # Store different types of memories
   memory.store_memory(
       MemoryType.Semantic,
       "Paris is the capital of France",
       {"topic": "geography", "country": "France"}
   )

   memory.store_memory(
       MemoryType.Episodic,
       "User asked about French capitals at 2pm",
       {"timestamp": "2023-01-01T14:00:00Z", "user_id": "123"}
   )

   # Retrieve memories
   semantic_memories = memory.retrieve_memories(
       MemoryType.Semantic,
       query="French capital",
       limit=5
   )

Next Steps
----------

- Check out the :doc:`examples` for more detailed use cases
- Read the :doc:`api` for complete documentation
- See :doc:`advanced` for performance optimization and advanced features
