ProllyTree Python Documentation
================================

Welcome to the ProllyTree Python bindings documentation. ProllyTree is a probabilistic tree data structure that combines B-trees and Merkle trees to provide efficient data access with cryptographic verification.

.. toctree::
   :maxdepth: 2
   :caption: Contents:

   quickstart
   api
   examples
   advanced

Key Features
------------

* **Probabilistic Balancing**: Uses content-based hashing for automatic tree balancing
* **Merkle Tree Properties**: Provides cryptographic verification of data integrity
* **Efficient Storage**: Optimized for both memory and disk storage
* **Version Control**: Git-like versioned key-value storage with commit history
* **SQL Support**: Query your data using SQL with GlueSQL integration
* **Agent Memory**: Advanced memory system for AI agents with semantic and episodic memory

Quick Example
-------------

.. code-block:: python

   from prollytree import ProllyTree

   # Create a new tree
   tree = ProllyTree()

   # Insert some data
   tree.insert(b"key1", b"value1")
   tree.insert(b"key2", b"value2")

   # Find data
   value = tree.find(b"key1")
   print(value)  # b"value1"

Installation
------------

Install ProllyTree using pip:

.. code-block:: bash

   pip install prollytree

Or build from source:

.. code-block:: bash

   git clone https://github.com/zhangfengcdt/prollytree
   cd prollytree
   ./python/build_python.sh --all-features --install

Indices and tables
==================

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`
