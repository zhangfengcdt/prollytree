Advanced Usage
==============

This guide covers advanced features and performance optimization techniques for ProllyTree.

Performance Optimization
-------------------------

Batch Operations
~~~~~~~~~~~~~~~~

For better performance when inserting many items, use batch operations:

.. code-block:: python

   from prollytree import ProllyTree

   tree = ProllyTree()

   # Instead of individual inserts
   for i in range(1000):
       tree.insert(f"key_{i}".encode(), f"value_{i}".encode())

   # Use batch insert (much faster)
   batch_data = [
       (f"key_{i}".encode(), f"value_{i}".encode())
       for i in range(1000)
   ]
   tree.insert_batch(batch_data)

Storage Backends
~~~~~~~~~~~~~~~~

Choose the appropriate storage backend for your use case:

.. code-block:: python

   from prollytree import ProllyTree, VersionedKvStore

   # In-memory (fastest, not persistent)
   tree = ProllyTree()

   # File-based storage (persistent)
   tree = ProllyTree(storage_type="file", path="/path/to/data")

   # Versioned storage with Git-like history
   store = VersionedKvStore("/path/to/versioned_data")

Tree Configuration
~~~~~~~~~~~~~~~~~~

Tune tree parameters for your workload:

.. code-block:: python

   from prollytree import ProllyTree, TreeConfig

   # Default configuration
   config = TreeConfig()

   # Custom configuration for specific workloads
   config = TreeConfig(
       base=8,        # Higher base for wider trees (good for read-heavy)
       modulus=128,   # Higher modulus for deeper trees (good for write-heavy)
   )

   tree = ProllyTree(config=config)

Concurrent Access
-----------------

Thread Safety
~~~~~~~~~~~~~

For multi-threaded applications:

.. code-block:: python

   import threading
   from prollytree import ProllyTree

   # Create a thread-safe tree
   tree = ProllyTree(thread_safe=True)

   def worker(thread_id):
       for i in range(100):
           key = f"thread_{thread_id}_key_{i}".encode()
           value = f"value_{i}".encode()
           tree.insert(key, value)

   # Start multiple threads
   threads = []
   for i in range(4):
       t = threading.Thread(target=worker, args=(i,))
       threads.append(t)
       t.start()

   for t in threads:
       t.join()

Memory Management
-----------------

LRU Cache
~~~~~~~~~

Enable LRU caching for read-heavy workloads:

.. code-block:: python

   from prollytree import ProllyTree, CacheConfig

   cache_config = CacheConfig(
       max_size=10000,  # Cache up to 10k nodes
       eviction_policy="lru"
   )

   tree = ProllyTree(cache_config=cache_config)

Memory Monitoring
~~~~~~~~~~~~~~~~~

Monitor memory usage:

.. code-block:: python

   tree = ProllyTree()

   # Insert data
   for i in range(10000):
       tree.insert(f"key_{i}".encode(), f"value_{i}".encode())

   # Get memory statistics
   stats = tree.get_memory_stats()
   print(f"Nodes in memory: {stats['node_count']}")
   print(f"Memory usage: {stats['memory_bytes']} bytes")
   print(f"Cache hit rate: {stats['cache_hit_rate']}%")

Data Serialization
-------------------

Custom Serialization
~~~~~~~~~~~~~~~~~~~~~

For complex data types:

.. code-block:: python

   import json
   import pickle
   from prollytree import ProllyTree

   tree = ProllyTree()

   # JSON serialization
   def store_json(tree, key, data):
       serialized = json.dumps(data).encode('utf-8')
       tree.insert(key.encode('utf-8'), serialized)

   def load_json(tree, key):
       data = tree.find(key.encode('utf-8'))
       return json.loads(data.decode('utf-8')) if data else None

   # Usage
   complex_data = {
       "user": "alice",
       "scores": [95, 87, 92],
       "metadata": {"premium": True, "last_login": "2023-01-01"}
   }

   store_json(tree, "user:alice", complex_data)
   retrieved = load_json(tree, "user:alice")

SQL Advanced Queries
---------------------

Complex Joins and Aggregations
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from prollytree import ProllySQLStore

   sql_store = ProllySQLStore("/path/to/sql_data")

   # Create tables
   sql_store.execute("""
       CREATE TABLE users (
           id INTEGER PRIMARY KEY,
           name TEXT,
           department_id INTEGER,
           salary REAL
       )
   """)

   sql_store.execute("""
       CREATE TABLE departments (
           id INTEGER PRIMARY KEY,
           name TEXT,
           budget REAL
       )
   """)

   # Complex aggregation query
   result = sql_store.execute("""
       SELECT
           d.name as department,
           COUNT(u.id) as employee_count,
           AVG(u.salary) as avg_salary,
           MAX(u.salary) as max_salary,
           SUM(u.salary) as total_salary
       FROM departments d
       LEFT JOIN users u ON d.id = u.department_id
       GROUP BY d.id, d.name
       HAVING COUNT(u.id) > 0
       ORDER BY avg_salary DESC
   """)

Error Handling and Debugging
-----------------------------

Exception Handling
~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from prollytree import ProllyTree, ProllyTreeError, StorageError

   try:
       tree = ProllyTree(storage_type="file", path="/invalid/path")
       tree.insert(b"key", b"value")
   except StorageError as e:
       print(f"Storage error: {e}")
   except ProllyTreeError as e:
       print(f"Tree operation error: {e}")
   except Exception as e:
       print(f"Unexpected error: {e}")

Debug Mode
~~~~~~~~~~

.. code-block:: python

   # Enable debug logging
   tree = ProllyTree(debug=True, log_level="DEBUG")

   # Validate tree structure
   is_valid = tree.validate()
   if not is_valid:
       print("Tree structure is corrupted!")

   # Get detailed statistics
   stats = tree.get_debug_stats()
   print(f"Tree height: {stats['height']}")
   print(f"Node distribution: {stats['node_distribution']}")
   print(f"Rebalancing events: {stats['rebalance_count']}")

Migration and Backup
---------------------

Data Export/Import
~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   # Export tree data
   tree.export_to_file("/path/to/backup.json", format="json")
   tree.export_to_file("/path/to/backup.bin", format="binary")

   # Import tree data
   new_tree = ProllyTree()
   new_tree.import_from_file("/path/to/backup.json", format="json")

This advanced guide covers performance optimization, concurrent access patterns, memory management, complex data operations, and debugging techniques for ProllyTree.
