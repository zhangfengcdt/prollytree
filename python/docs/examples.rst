Examples
========

This page contains comprehensive examples showing how to use ProllyTree in different scenarios.

Basic Tree Operations
---------------------

Simple Key-Value Store
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from prollytree import ProllyTree

   def example_basic_kv_store():
       """Basic key-value store example"""
       tree = ProllyTree()

       # Store user data
       users = {
           "alice": {"name": "Alice Smith", "age": 30},
           "bob": {"name": "Bob Jones", "age": 25},
           "charlie": {"name": "Charlie Brown", "age": 35}
       }

       # Insert users
       import json
       for user_id, user_data in users.items():
           key = f"user:{user_id}".encode('utf-8')
           value = json.dumps(user_data).encode('utf-8')
           tree.insert(key, value)

       # Retrieve a user
       alice_data = tree.find(b"user:alice")
       if alice_data:
           alice = json.loads(alice_data.decode('utf-8'))
           print(f"Alice: {alice['name']}, age {alice['age']}")

       return tree

Working with Different Data Types
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   import json
   import pickle
   from datetime import datetime

   def example_data_types():
       """Examples of storing different data types"""
       tree = ProllyTree()

       # String data
       tree.insert(b"string_key", "Hello, World!".encode('utf-8'))

       # JSON data
       data = {"name": "Alice", "scores": [95, 87, 92]}
       tree.insert(b"json_key", json.dumps(data).encode('utf-8'))

       # Binary data (using pickle)
       complex_data = {
           "timestamp": datetime.now(),
           "nested": {"list": [1, 2, 3], "dict": {"a": 1}}
       }
       tree.insert(b"pickle_key", pickle.dumps(complex_data))

       # Retrieve and decode
       string_val = tree.find(b"string_key").decode('utf-8')
       json_val = json.loads(tree.find(b"json_key").decode('utf-8'))
       pickle_val = pickle.loads(tree.find(b"pickle_key"))

       print(f"String: {string_val}")
       print(f"JSON: {json_val}")
       print(f"Pickle: {pickle_val}")

Versioned Storage Examples
--------------------------

Document Version Control
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from prollytree import VersionedKvStore
   import json
   from datetime import datetime

   def example_document_versioning():
       """Example of version controlling documents"""
       store = VersionedKvStore("./document_store")

       # Create initial document
       doc = {
           "title": "My Document",
           "content": "Initial content",
           "author": "Alice",
           "created": datetime.now().isoformat()
       }

       store.insert(b"doc:readme", json.dumps(doc).encode('utf-8'))
       commit1 = store.commit("Initial document creation")
       print(f"Initial commit: {commit1[:8]}")

       # Update document
       doc["content"] = "Updated content with more details"
       doc["modified"] = datetime.now().isoformat()
       store.update(b"doc:readme", json.dumps(doc).encode('utf-8'))
       commit2 = store.commit("Add more content details")
       print(f"Update commit: {commit2[:8]}")

       # View commit history
       print("\\nCommit History:")
       commits = store.log()
       for commit in commits:
           timestamp = datetime.fromtimestamp(commit['timestamp'])
           print(f"  {commit['id'][:8]} - {commit['message']} ({timestamp})")

       return store

Configuration Management
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   def example_config_management():
       """Example of managing application configuration with versions"""
       store = VersionedKvStore("./config_store")

       # Production config
       prod_config = {
           "database": {
               "host": "prod-db.example.com",
               "port": 5432,
               "ssl": True
           },
           "api": {
               "rate_limit": 1000,
               "timeout": 30
           }
       }

       store.insert(b"config:production", json.dumps(prod_config).encode('utf-8'))
       store.commit("Initial production configuration")

       # Development config
       dev_config = prod_config.copy()
       dev_config["database"]["host"] = "localhost"
       dev_config["database"]["ssl"] = False
       dev_config["api"]["rate_limit"] = 10000  # Higher for dev

       store.insert(b"config:development", json.dumps(dev_config).encode('utf-8'))
       store.commit("Add development configuration")

       # Update production config
       prod_config["api"]["rate_limit"] = 2000  # Increase rate limit
       store.update(b"config:production", json.dumps(prod_config).encode('utf-8'))
       store.commit("Increase production rate limit")

       # Retrieve current configs
       current_prod = json.loads(store.get(b"config:production").decode('utf-8'))
       print(f"Production rate limit: {current_prod['api']['rate_limit']}")

       return store

SQL Query Examples
------------------

.. code-block:: python

   from prollytree import ProllySQLStore

   def example_sql_analytics():
       """Example using SQL for data analytics"""
       sql_store = ProllySQLStore("./analytics_store")

       # Create tables
       sql_store.execute("""
           CREATE TABLE users (
               id INTEGER,
               name TEXT,
               email TEXT,
               signup_date TEXT,
               plan TEXT
           )
       """)

       sql_store.execute("""
           CREATE TABLE events (
               id INTEGER,
               user_id INTEGER,
               event_type TEXT,
               timestamp TEXT,
               metadata TEXT
           )
       """)

       # Insert sample data
       users_data = [
           (1, 'Alice Smith', 'alice@example.com', '2023-01-15', 'premium'),
           (2, 'Bob Jones', 'bob@example.com', '2023-02-01', 'basic'),
           (3, 'Charlie Brown', 'charlie@example.com', '2023-02-15', 'premium'),
       ]

       for user in users_data:
           sql_store.execute(
               "INSERT INTO users (id, name, email, signup_date, plan) VALUES (?, ?, ?, ?, ?)",
               user
           )

       # Analytics queries
       print("=== User Analytics ===")

       # Premium users
       premium_users = sql_store.execute(
           "SELECT name, email FROM users WHERE plan = 'premium'"
       )
       print(f"Premium users: {len(premium_users)}")
       for user in premium_users:
           print(f"  - {user[0]} ({user[1]})")

       return sql_store

Agent Memory System Examples
----------------------------

.. code-block:: python

   from prollytree import AgentMemorySystem, MemoryType

   def example_ai_agent_memory():
       """Example of using ProllyTree for AI agent memory"""
       memory = AgentMemorySystem("./agent_memory")

       # Store semantic knowledge
       semantic_memories = [
           ("The Eiffel Tower is in Paris, France", {"topic": "landmarks", "city": "Paris"}),
           ("Python is a programming language", {"topic": "programming", "language": "Python"}),
           ("Machine learning is a subset of AI", {"topic": "AI", "domain": "machine learning"}),
       ]

       for content, metadata in semantic_memories:
           memory.store_memory(MemoryType.Semantic, content, metadata)

       # Store episodic memories (experiences)
       episodic_memories = [
           ("User asked about French landmarks", {"user": "alice", "timestamp": "2023-03-01T10:00:00Z"}),
           ("Helped user debug Python code", {"user": "bob", "timestamp": "2023-03-01T11:00:00Z"}),
           ("Explained ML concepts to student", {"user": "charlie", "timestamp": "2023-03-01T12:00:00Z"}),
       ]

       for content, metadata in episodic_memories:
           memory.store_memory(MemoryType.Episodic, content, metadata)

       # Retrieve relevant memories
       print("=== Memory Retrieval ===")

       # Query semantic memory
       paris_memories = memory.retrieve_memories(
           MemoryType.Semantic,
           query="Paris landmarks",
           limit=3
       )
       print("Semantic memories about Paris:")
       for mem in paris_memories:
           print(f"  - {mem['content']}")

       return memory

Performance Examples
--------------------

.. code-block:: python

   def example_batch_operations():
       """Example showing efficient batch operations"""
       tree = ProllyTree()

       # Generate test data
       import time

       # Single inserts (slower)
       start_time = time.time()
       for i in range(1000):
           key = f"single:{i:04d}".encode('utf-8')
           value = f"value_{i}".encode('utf-8')
           tree.insert(key, value)
       single_time = time.time() - start_time

       # Batch insert (faster)
       start_time = time.time()
       batch_data = []
       for i in range(1000):
           key = f"batch:{i:04d}".encode('utf-8')
           value = f"value_{i}".encode('utf-8')
           batch_data.append((key, value))

       tree.insert_batch(batch_data)
       batch_time = time.time() - start_time

       print(f"Single inserts: {single_time:.3f}s")
       print(f"Batch insert: {batch_time:.3f}s")
       print(f"Speedup: {single_time/batch_time:.1f}x")

       return tree

Running Examples
----------------

.. code-block:: python

   if __name__ == "__main__":
       # Run examples
       print("=== Basic Key-Value Store ===")
       example_basic_kv_store()

       print("\\n=== Data Types ===")
       example_data_types()

       print("\\n=== Document Versioning ===")
       example_document_versioning()

       print("\\n=== SQL Analytics ===")
       example_sql_analytics()

       print("\\n=== AI Agent Memory ===")
       example_ai_agent_memory()

       print("\\n=== Performance ===")
       example_batch_operations()
