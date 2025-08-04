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
Tests for ProllyTree SQL functionality
"""

import pytest
import tempfile
import shutil
import os
from prollytree import ProllySQLStore


class TestProllySQLStore:
    """Test suite for SQL functionality in ProllyTree"""

    @pytest.fixture
    def temp_store(self):
        """Create a temporary SQL store for testing"""
        temp_dir = tempfile.mkdtemp()
        store = ProllySQLStore(temp_dir)
        yield store
        shutil.rmtree(temp_dir)

    def test_create_table(self, temp_store):
        """Test creating a table"""
        result = temp_store.create_table(
            "users",
            [("id", "INTEGER"), ("name", "TEXT"), ("email", "TEXT")]
        )
        assert result["type"] == "create"
        assert result["success"] == True

    def test_insert_data(self, temp_store):
        """Test inserting data into a table"""
        # Create table first
        temp_store.create_table(
            "users",
            [("id", "INTEGER"), ("name", "TEXT"), ("email", "TEXT")]
        )

        # Insert data
        result = temp_store.insert("users", [
            [1, "Alice", "alice@example.com"],
            [2, "Bob", "bob@example.com"]
        ])
        assert result["type"] == "insert"
        assert result["count"] == 2

    def test_select_data(self, temp_store):
        """Test selecting data from a table"""
        # Setup
        temp_store.create_table(
            "users",
            [("id", "INTEGER"), ("name", "TEXT"), ("email", "TEXT")]
        )
        temp_store.insert("users", [
            [1, "Alice", "alice@example.com"],
            [2, "Bob", "bob@example.com"]
        ])

        # Select all
        result = temp_store.select("users")
        assert len(result) == 2
        assert result[0]["name"] == "Alice"
        assert result[1]["name"] == "Bob"

        # Select specific columns
        result = temp_store.select("users", columns=["name", "email"])
        assert len(result) == 2
        assert "id" not in result[0]
        assert "name" in result[0]
        assert "email" in result[0]

        # Select with WHERE clause
        result = temp_store.select("users", where_clause="id = 1")
        assert len(result) == 1
        assert result[0]["name"] == "Alice"

    def test_execute_raw_sql(self, temp_store):
        """Test executing raw SQL queries"""
        # Create table using raw SQL
        result = temp_store.execute(
            "CREATE TABLE products (id INTEGER, name TEXT, price FLOAT)"
        )
        assert result["type"] == "create"

        # Insert using raw SQL
        result = temp_store.execute(
            "INSERT INTO products VALUES (1, 'Widget', 9.99), (2, 'Gadget', 19.99)"
        )
        assert result["type"] == "insert"
        assert result["count"] == 2

        # Select using raw SQL
        result = temp_store.execute("SELECT * FROM products WHERE price < 15")
        assert len(result) == 1
        assert result[0]["name"] == "Widget"

    def test_output_formats(self, temp_store):
        """Test different output formats"""
        # Setup
        temp_store.create_table("test", [("id", "INTEGER"), ("value", "TEXT")])
        temp_store.insert("test", [[1, "one"], [2, "two"]])

        # Test dict format (default)
        result = temp_store.execute("SELECT * FROM test", format="dict")
        assert isinstance(result, list)
        assert isinstance(result[0], dict)

        # Test tuples format
        labels, rows = temp_store.execute("SELECT * FROM test", format="tuples")
        assert labels == ["id", "value"]
        assert len(rows) == 2
        assert rows[0] == [1, "one"]

        # Test JSON format
        result = temp_store.execute("SELECT * FROM test", format="json")
        assert isinstance(result, str)
        import json
        data = json.loads(result)
        assert len(data) == 2
        assert data[0]["id"] == 1

        # Test CSV format
        result = temp_store.execute("SELECT * FROM test", format="csv")
        assert isinstance(result, str)
        lines = result.strip().split("\n")
        assert lines[0] == "id,value"
        assert lines[1] == "1,\"one\""

    def test_execute_many(self, temp_store):
        """Test executing multiple queries"""
        queries = [
            "CREATE TABLE test1 (id INTEGER, name TEXT)",
            "CREATE TABLE test2 (id INTEGER, value FLOAT)",
            "INSERT INTO test1 VALUES (1, 'first')",
            "INSERT INTO test2 VALUES (1, 3.14)"
        ]

        results = temp_store.execute_many(queries)
        assert len(results) == 4
        assert results[0]["type"] == "create"
        assert results[1]["type"] == "create"
        assert results[2]["type"] == "insert"
        assert results[3]["type"] == "insert"

    def test_complex_queries(self, temp_store):
        """Test more complex SQL operations"""
        # Create tables
        temp_store.execute("""
            CREATE TABLE customers (
                id INTEGER,
                name TEXT,
                country TEXT
            )
        """)

        temp_store.execute("""
            CREATE TABLE orders (
                id INTEGER,
                customer_id INTEGER,
                amount FLOAT,
                date TEXT
            )
        """)

        # Insert data
        temp_store.execute("""
            INSERT INTO customers VALUES
            (1, 'Alice', 'USA'),
            (2, 'Bob', 'UK'),
            (3, 'Charlie', 'USA')
        """)

        temp_store.execute("""
            INSERT INTO orders VALUES
            (1, 1, 100.0, '2024-01-01'),
            (2, 1, 200.0, '2024-01-02'),
            (3, 2, 150.0, '2024-01-03')
        """)

        # Test JOIN
        result = temp_store.execute("""
            SELECT c.name, o.amount
            FROM customers c
            JOIN orders o ON c.id = o.customer_id
            WHERE c.country = 'USA'
        """)
        assert len(result) == 2
        assert all(r["name"] == "Alice" for r in result)

        # Test aggregation
        result = temp_store.execute("""
            SELECT customer_id, SUM(amount) as total
            FROM orders
            GROUP BY customer_id
        """)
        assert len(result) == 2
        alice_total = next(r for r in result if r["customer_id"] == 1)
        assert alice_total["total"] == 300.0

    def test_update_and_delete(self, temp_store):
        """Test UPDATE and DELETE operations"""
        # Setup
        temp_store.create_table("items", [("id", "INTEGER"), ("name", "TEXT"), ("quantity", "INTEGER")])
        temp_store.insert("items", [
            [1, "Item1", 10],
            [2, "Item2", 20],
            [3, "Item3", 30]
        ])

        # Test UPDATE
        result = temp_store.execute("UPDATE items SET quantity = 25 WHERE id = 2")
        assert result["type"] == "update"
        assert result["count"] == 1

        # Verify update
        result = temp_store.execute("SELECT * FROM items WHERE id = 2")
        assert result[0]["quantity"] == 25

        # Test DELETE
        result = temp_store.execute("DELETE FROM items WHERE quantity < 20")
        assert result["type"] == "delete"
        assert result["count"] == 1

        # Verify deletion
        result = temp_store.execute("SELECT * FROM items")
        assert len(result) == 2
        assert all(r["quantity"] >= 20 for r in result)


class TestProllySQLStoreStaticMethods:
    """Test static methods of ProllySQLStore"""

    def test_open_existing_store(self):
        """Test opening an existing SQL store"""
        temp_dir = tempfile.mkdtemp()
        try:
            # Create and populate a store
            store1 = ProllySQLStore(temp_dir)
            store1.create_table("test", [("id", "INTEGER"), ("value", "TEXT")])
            store1.insert("test", [[1, "test"]])
            del store1

            # Open the existing store
            store2 = ProllySQLStore.open(temp_dir)
            result = store2.execute("SELECT * FROM test")
            assert len(result) == 1
            assert result[0]["value"] == "test"
        finally:
            shutil.rmtree(temp_dir)
