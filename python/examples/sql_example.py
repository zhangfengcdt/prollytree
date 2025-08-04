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
Example demonstrating SQL functionality in ProllyTree

This example shows how to:
1. Create tables
2. Insert data
3. Query data with different output formats
4. Perform joins and aggregations
5. Commit changes with version control
"""

import tempfile
import json
from prollytree import ProllySQLStore


def main():
    # Create a temporary directory for the example
    with tempfile.TemporaryDirectory() as temp_dir:
        print(f"📂 Creating SQL store in: {temp_dir}\n")

        # Initialize git repository first
        import subprocess
        subprocess.run(["git", "init"], cwd=temp_dir, capture_output=True)
        subprocess.run(["git", "config", "user.name", "Test User"], cwd=temp_dir, capture_output=True)
        subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=temp_dir, capture_output=True)

        # Create a subdirectory for the SQL store
        import os
        store_dir = os.path.join(temp_dir, "data")
        os.makedirs(store_dir, exist_ok=True)

        # Initialize a new SQL store
        store = ProllySQLStore(store_dir)

        # ========================================
        # 1. Create Tables
        # ========================================
        print("📋 Creating tables...")

        # Create users table
        store.create_table(
            "users",
            [
                ("id", "INTEGER"),
                ("name", "TEXT"),
                ("email", "TEXT"),
                ("age", "INTEGER")
            ]
        )
        print("✅ Created 'users' table")

        # Create posts table using raw SQL
        store.execute("""
            CREATE TABLE posts (
                id INTEGER,
                user_id INTEGER,
                title TEXT,
                content TEXT,
                created_at TEXT
            )
        """)
        print("✅ Created 'posts' table\n")

        # ========================================
        # 2. Insert Data
        # ========================================
        print("📝 Inserting data...")

        # Insert users using the insert method
        store.insert("users", [
            [1, "Alice Johnson", "alice@example.com", 28],
            [2, "Bob Smith", "bob@example.com", 35],
            [3, "Charlie Brown", "charlie@example.com", 42],
            [4, "Diana Prince", "diana@example.com", 31]
        ])
        print("✅ Inserted 4 users")

        # Insert posts using raw SQL
        store.execute("""
            INSERT INTO posts VALUES
            (1, 1, 'Getting Started with ProllyTree', 'ProllyTree is amazing!', '2024-01-01'),
            (2, 1, 'SQL Support in ProllyTree', 'You can run SQL queries!', '2024-01-02'),
            (3, 2, 'My First Post', 'Hello, world!', '2024-01-03'),
            (4, 3, 'Data Structures', 'Merkle trees are cool', '2024-01-04'),
            (5, 2, 'Another Update', 'More content here', '2024-01-05')
        """)
        print("✅ Inserted 5 posts\n")

        # ========================================
        # 3. Query Data - Different Formats
        # ========================================
        print("🔍 Querying data in different formats:\n")

        # Dict format (default)
        print("📊 Dict format (default):")
        users = store.select("users", columns=["name", "email"])
        for user in users[:2]:  # Show first 2
            print(f"  - {user['name']}: {user['email']}")

        # Tuples format
        print("\n📊 Tuples format:")
        labels, rows = store.execute("SELECT name, age FROM users", format="tuples")
        print(f"  Columns: {labels}")
        print(f"  First row: {rows[0]}")

        # JSON format
        print("\n📊 JSON format:")
        json_result = store.execute("SELECT * FROM users WHERE age > 30", format="json")
        data = json.loads(json_result)
        print(f"  Users over 30: {len(data)}")
        print(f"  First user: {data[0]['name']} (age {data[0]['age']})")

        # CSV format
        print("\n📊 CSV format:")
        csv_result = store.execute("SELECT id, name FROM users", format="csv")
        print("  CSV output (first 3 lines):")
        for line in csv_result.strip().split("\n")[:3]:
            print(f"    {line}")
        print()

        # ========================================
        # 4. Complex Queries
        # ========================================
        print("🔄 Performing complex queries:\n")

        # JOIN query
        print("📍 Users with their posts (JOIN):")
        result = store.execute("""
            SELECT u.name, p.title, p.created_at
            FROM users u
            JOIN posts p ON u.id = p.user_id
            ORDER BY p.created_at
        """)
        for row in result[:3]:  # Show first 3
            print(f"  - {row['name']}: '{row['title']}' ({row['created_at']})")

        # Aggregation query
        print("\n📊 Post count per user (GROUP BY):")
        result = store.execute("""
            SELECT u.name, COUNT(p.id) as post_count
            FROM users u
            LEFT JOIN posts p ON u.id = p.user_id
            GROUP BY u.id, u.name
            ORDER BY post_count DESC
        """)
        for row in result:
            print(f"  - {row['name']}: {row['post_count']} posts")

        # Subquery (without DISTINCT since GlueSQL doesn't support it)
        print("\n🔎 Users with posts (subquery):")
        result = store.execute("""
            SELECT name, email
            FROM users
            WHERE id IN (SELECT user_id FROM posts)
        """)
        for row in result:
            print(f"  - {row['name']} ({row['email']})")

        # ========================================
        # 5. Updates and Deletes
        # ========================================
        print("\n✏️ Updating and deleting data:")

        # Update user age
        result = store.execute("UPDATE users SET age = age + 1 WHERE name = 'Alice Johnson'")
        print(f"  Updated {result['count']} user(s)")

        # Delete old posts
        result = store.execute("DELETE FROM posts WHERE created_at < '2024-01-03'")
        print(f"  Deleted {result['count']} old post(s)")

        # ========================================
        # 6. Execute Multiple Queries
        # ========================================
        print("\n🚀 Executing multiple queries:")

        queries = [
            "CREATE TABLE tags (id INTEGER, name TEXT)",
            "INSERT INTO tags VALUES (1, 'technology'), (2, 'tutorial')",
            "SELECT COUNT(*) as count FROM tags"
        ]

        results = store.execute_many(queries)
        print(f"  Executed {len(results)} queries")
        print(f"  Last query result: {results[-1]}")

        # ========================================
        # 7. Commit Changes
        # ========================================
        print("\n💾 Committing changes:")
        commit_id = store.commit("Added users, posts, and tags with sample data")
        print(f"  Committed with ID: {commit_id}")

        # ========================================
        # Final Statistics
        # ========================================
        print("\n📈 Final statistics:")

        stats = store.execute("""
            SELECT
                (SELECT COUNT(*) FROM users) as user_count,
                (SELECT COUNT(*) FROM posts) as post_count,
                (SELECT COUNT(*) FROM tags) as tag_count
        """)

        stat = stats[0]
        print(f"  Total users: {stat['user_count']}")
        print(f"  Total posts: {stat['post_count']}")
        print(f"  Total tags: {stat['tag_count']}")

        print("\n✨ Example completed successfully!")


if __name__ == "__main__":
    main()
