# Git-Prolly SQL Manual

## Overview

The `git prolly sql` command enables SQL query capabilities on top of ProllyTree's versioned key-value store. This integration combines the power of relational databases with Git's version control, allowing you to query your data using standard SQL while maintaining full version history.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Command Options](#command-options)
3. [Basic Usage](#basic-usage)
4. [SQL Operations](#sql-operations)
5. [Output Formats](#output-formats)
6. [Interactive Mode](#interactive-mode)
7. [Advanced Examples](#advanced-examples)
8. [Git Integration](#git-integration)
9. [Best Practices](#best-practices)
10. [Troubleshooting](#troubleshooting)

## Getting Started

### Prerequisites

1. Ensure you have initialized a git-prolly repository:
```bash
git prolly init
```

2. Verify the SQL feature is enabled (it's enabled by default):
```bash
git prolly sql --help
```

### Quick Start

```bash
# Create a table
git prolly sql "CREATE TABLE users (id INTEGER, name TEXT, email TEXT)"

# Insert data
git prolly sql "INSERT INTO users VALUES (1, 'Alice', 'alice@example.com')"

# Query data
git prolly sql "SELECT * FROM users"

# Commit your changes
git prolly commit -m "Added users table with initial data"
```

## Command Options

```
git prolly sql [OPTIONS] [QUERY]

Arguments:
  [QUERY]  SQL query to execute

Options:
  -f, --file <FILE>      Execute query from file
  -o, --format <FORMAT>  Output format (table, json, csv)
  -i, --interactive      Start interactive SQL shell
      --verbose          Show detailed error messages
  -b, --branch <BRANCH>  Execute against specific branch or commit (SELECT queries only, requires clean status)
  -h, --help             Print help
```

## Basic Usage

### 1. Direct Query Execution

Execute a single SQL query directly from the command line:

```bash
git prolly sql "SELECT * FROM users WHERE age > 25"
```

### 2. File-based Execution

Execute SQL commands from a file:

```bash
# Create a SQL file
cat > schema.sql << EOF
CREATE TABLE products (
    id INTEGER,
    name TEXT,
    price INTEGER,
    category TEXT
);

INSERT INTO products VALUES 
    (1, 'Laptop', 1200, 'Electronics'),
    (2, 'Book', 25, 'Education');
EOF

# Execute the file
git prolly sql -f schema.sql
```

### 3. Interactive Mode

Start an interactive SQL shell:

```bash
git prolly sql -i
```

In interactive mode:
- Type SQL queries and press Enter to execute
- Type `help` for available commands
- Type `exit` or `quit` to leave the shell

### 4. Historical Data Querying

Query data from specific branches or commits using the `-b` parameter:

```bash
# Query data from main branch
git prolly sql -b main "SELECT * FROM users"

# Query data from a specific commit
git prolly sql -b a1b2c3d4 "SELECT COUNT(*) FROM products"

# Query data from a feature branch
git prolly sql -b feature/new-schema "SELECT * FROM categories"
```

**Important Requirements:**
- Only `SELECT` statements are allowed when using `-b` parameter
- Your working directory must have clean status (no uncommitted staging changes)
- The branch/commit will be temporarily checked out and restored after execution

**Example with staging changes:**
```bash
# This will be blocked if you have uncommitted changes
git prolly set user:123 "John Doe"  # Creates staging changes
git prolly sql -b main "SELECT * FROM users"
# Error: Cannot use -b/--branch parameter with uncommitted staging changes

# Commit your changes first
git prolly commit -m "Add new user"
git prolly sql -b main "SELECT * FROM users"  # Now works
```

## SQL Operations

### Supported SQL Features

#### DDL (Data Definition Language)
- `CREATE TABLE` - Create new tables
- `DROP TABLE` - Remove tables (coming soon)

#### DML (Data Manipulation Language)
- `INSERT` - Add new rows
- `SELECT` - Query data
- `UPDATE` - Modify existing rows
- `DELETE` - Remove rows

#### Query Features
- `WHERE` clauses
- `ORDER BY` sorting
- `JOIN` operations (INNER, LEFT, RIGHT)
- `GROUP BY` aggregation
- Aggregate functions: `COUNT()`, `AVG()`, `MAX()`, `MIN()`
- `LIMIT` for result limiting

### Examples

#### Creating Tables

```sql
-- Simple table
CREATE TABLE users (
    id INTEGER,
    name TEXT,
    email TEXT,
    created_at TEXT
);

-- Table with more data types
CREATE TABLE products (
    id INTEGER,
    name TEXT,
    price INTEGER,
    in_stock BOOLEAN,
    description TEXT
);
```

#### Inserting Data

```sql
-- Single row insert
INSERT INTO users (id, name, email) 
VALUES (1, 'Alice Johnson', 'alice@example.com');

-- Multiple row insert
INSERT INTO users (id, name, email) VALUES 
    (2, 'Bob Smith', 'bob@example.com'),
    (3, 'Charlie Brown', 'charlie@example.com');

-- Insert without specifying columns (must match table structure)
INSERT INTO products VALUES 
    (1, 'Laptop', 1200, true, 'High-performance laptop');
```

#### Querying Data

```sql
-- Select all columns
SELECT * FROM users;

-- Select specific columns
SELECT name, email FROM users;

-- With WHERE clause
SELECT * FROM users WHERE id > 1;
SELECT * FROM products WHERE price < 100 AND in_stock = true;

-- With ORDER BY
SELECT * FROM users ORDER BY name ASC;
SELECT * FROM products ORDER BY price DESC;

-- With LIMIT
SELECT * FROM products ORDER BY price DESC LIMIT 5;
```

#### Joins

```sql
-- Create related tables
CREATE TABLE orders (
    id INTEGER,
    user_id INTEGER,
    product_id INTEGER,
    quantity INTEGER,
    order_date TEXT
);

-- Inner join
SELECT u.name, p.name as product, o.quantity
FROM users u
JOIN orders o ON u.id = o.user_id
JOIN products p ON o.product_id = p.id;

-- Left join (shows all users, even without orders)
SELECT u.name, COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.name;
```

#### Aggregation

```sql
-- Count rows
SELECT COUNT(*) FROM users;

-- Group by with aggregation
SELECT category, COUNT(*) as product_count, AVG(price) as avg_price
FROM products
GROUP BY category;

-- Having clause
SELECT user_id, COUNT(*) as order_count
FROM orders
GROUP BY user_id
HAVING COUNT(*) > 5;
```

#### Updates and Deletes

```sql
-- Update single row
UPDATE users SET email = 'newemail@example.com' WHERE id = 1;

-- Update multiple rows
UPDATE products SET price = price * 1.1 WHERE category = 'Electronics';

-- Delete rows
DELETE FROM orders WHERE order_date < '2023-01-01';
```

## Output Formats

### 1. Table Format (Default)

```bash
git prolly sql "SELECT * FROM users"
```

Output:
```
│ id     │ name              │ email                    │
├────────┼───────────────────┼──────────────────────────┤
│ I64(1) │ Str("Alice")      │ Str("alice@example.com") │
│ I64(2) │ Str("Bob")        │ Str("bob@example.com")   │
```

### 2. JSON Format

```bash
git prolly sql -o json "SELECT * FROM users"
```

Output:
```json
[
  {
    "id": 1,
    "name": "Alice",
    "email": "alice@example.com"
  },
  {
    "id": 2,
    "name": "Bob",
    "email": "bob@example.com"
  }
]
```

### 3. CSV Format

```bash
git prolly sql -o csv "SELECT * FROM users"
```

Output:
```csv
id,name,email
I64(1),Str("Alice"),Str("alice@example.com")
I64(2),Str("Bob"),Str("bob@example.com")
```

## Interactive Mode

The interactive SQL shell provides a convenient environment for exploring your data:

```bash
git prolly sql -i
```

```
🌟 ProllyTree SQL Interactive Shell
====================================
Type 'exit' or 'quit' to exit
Type 'help' for available commands

prolly-sql> CREATE TABLE test (id INTEGER, value TEXT);
✓ Table created successfully

prolly-sql> INSERT INTO test VALUES (1, 'Hello'), (2, 'World');
✓ Inserted 2 rows

prolly-sql> SELECT * FROM test;
│ id     │ value         │
├────────┼───────────────┤
│ I64(1) │ Str("Hello")  │
│ I64(2) │ Str("World")  │

prolly-sql> exit
Goodbye!
```

### Interactive Mode with Historical Data

Use interactive mode to explore historical data:

```bash
# Start interactive mode against a specific branch
git prolly sql -b feature/analytics -i
```

```
🌟 ProllyTree SQL Interactive Shell
====================================
Executing against branch/commit: feature/analytics
⚠️  Only SELECT statements are allowed in this mode
Type 'exit' or 'quit' to exit
Type 'help' for available commands

prolly-sql> SELECT COUNT(*) FROM new_analytics_table;
│ COUNT(*) │
├──────────┤
│ I64(150) │

prolly-sql> SELECT * FROM products WHERE price > 1000;
│ id     │ name                │ price     │
├────────┼─────────────────────┼───────────┤
│ I64(1) │ Str("Gaming PC")    │ I64(1500) │
│ I64(2) │ Str("MacBook Pro")  │ I64(2000) │

prolly-sql> INSERT INTO products VALUES (3, 'iPad', 800);
Error: Only SELECT statements are allowed when using -b/--branch parameter
       Historical commits/branches are read-only for data integrity

prolly-sql> exit
Goodbye!
Restored to original branch: main
```

## Advanced Examples

### 1. Historical Data Analysis

Compare data across different points in time:

```bash
# Query current data
git prolly sql "SELECT COUNT(*) as current_users FROM users"

# Query data from last week's commit
git prolly sql -b 7d1a2b3c "SELECT COUNT(*) as users_last_week FROM users"

# Compare product prices between branches
git prolly sql -b main "SELECT name, price FROM products WHERE category = 'Electronics'"
git prolly sql -b feature/price-update "SELECT name, price FROM products WHERE category = 'Electronics'"

# Analyze data growth over time
git prolly sql -b v1.0 "SELECT COUNT(*) as v1_orders FROM orders"
git prolly sql -b v2.0 "SELECT COUNT(*) as v2_orders FROM orders"
```

### 2. Complex Data Analysis

```sql
-- Create sales data
CREATE TABLE sales (
    id INTEGER,
    product_id INTEGER,
    customer_id INTEGER,
    quantity INTEGER,
    price INTEGER,
    sale_date TEXT
);

-- Top selling products
SELECT p.name, SUM(s.quantity) as total_sold, SUM(s.quantity * s.price) as revenue
FROM sales s
JOIN products p ON s.product_id = p.id
GROUP BY p.id, p.name
ORDER BY revenue DESC
LIMIT 10;

-- Customer purchase history
SELECT c.name, COUNT(DISTINCT s.id) as purchase_count, 
       SUM(s.quantity * s.price) as total_spent
FROM customers c
JOIN sales s ON c.id = s.customer_id
GROUP BY c.id, c.name
HAVING total_spent > 1000;
```

### 2. Data Migration Script

```bash
# Create migration script
cat > migrate_v2.sql << EOF
-- Add new columns to existing table
CREATE TABLE users_new (
    id INTEGER,
    name TEXT,
    email TEXT,
    created_at TEXT,
    updated_at TEXT,
    status TEXT
);

-- Copy existing data
INSERT INTO users_new (id, name, email)
SELECT id, name, email FROM users;

-- Update new fields
UPDATE users_new SET 
    created_at = '2024-01-01',
    updated_at = '2024-01-01',
    status = 'active';

-- Verify migration
SELECT COUNT(*) as migrated_count FROM users_new;
EOF

# Execute migration
git prolly sql -f migrate_v2.sql
```

### 3. Reporting and Analytics

```bash
# Generate daily report
git prolly sql -o json "
SELECT 
    DATE(order_date) as date,
    COUNT(*) as orders,
    SUM(quantity * price) as revenue
FROM orders
WHERE order_date >= '2024-01-01'
GROUP BY DATE(order_date)
ORDER BY date
" > daily_revenue.json

# Export customer list
git prolly sql -o csv "
SELECT id, name, email, created_at
FROM users
WHERE status = 'active'
ORDER BY name
" > active_customers.csv
```

## Git Integration

### Version Control for Your Data

All SQL operations are stored in ProllyTree and can be versioned with Git:

```bash
# Make changes
git prolly sql "INSERT INTO users VALUES (4, 'David', 'david@example.com')"
git prolly sql "UPDATE products SET price = 1100 WHERE id = 1"

# Check status
git prolly status

# Commit changes
git prolly commit -m "Added new user David and updated laptop price"

# View history
git prolly show HEAD

# Diff between commits
git prolly diff HEAD~1 HEAD
```

### Working with Branches

```bash
# Create a new branch for experimental changes
git checkout -b feature/new-schema

# Make schema changes
git prolly sql "CREATE TABLE categories (id INTEGER, name TEXT)"
git prolly sql "ALTER TABLE products ADD COLUMN category_id INTEGER"

# Commit on feature branch
git prolly commit -m "Added categories support"

# Switch back to main
git checkout main

# The new tables don't exist on main branch
git prolly sql "SELECT * FROM categories"  # Error: table not found

# Query the new schema without switching branches
git prolly sql -b feature/new-schema "SELECT * FROM categories"

# Merge when ready
git merge feature/new-schema
```

### Cross-Branch Data Comparison

Compare data between branches without switching contexts:

```bash
# Compare user counts between branches
echo "Main branch users:"
git prolly sql -b main "SELECT COUNT(*) FROM users"

echo "Feature branch users:"
git prolly sql -b feature/user-management "SELECT COUNT(*) FROM users"

# Generate reports from different branches
git prolly sql -b production -o json "SELECT * FROM daily_metrics WHERE date = '2024-01-15'" > prod_metrics.json
git prolly sql -b staging -o json "SELECT * FROM daily_metrics WHERE date = '2024-01-15'" > staging_metrics.json

# Compare table schemas between versions
git prolly sql -b v1.0 "SELECT name FROM sqlite_master WHERE type='table'"
git prolly sql -b v2.0 "SELECT name FROM sqlite_master WHERE type='table'"
```

## Best Practices

### 1. Schema Design

- Use meaningful table and column names
- Define appropriate data types for columns
- Consider adding indexes for frequently queried columns (when supported)

### 2. Query Optimization

- Use specific column names instead of `SELECT *` when possible
- Add appropriate WHERE clauses to limit result sets
- Use JOINs efficiently - avoid cartesian products

### 3. Data Integrity

- Commit your changes regularly with meaningful messages
- Test queries in interactive mode before running in scripts
- Use transactions for multi-step operations (when supported)

### 4. Migration Strategy

```bash
# Always backup before major changes
git prolly sql "SELECT * FROM important_table" -o json > backup.json

# Test migrations on a branch first
git checkout -b migration-test
# ... run migration ...
# If successful, merge to main
```

### 5. Historical Data Querying

- **Commit changes before using `-b`**: Always commit your staging changes before querying historical data
- **Use for read-only analysis**: The `-b` parameter is perfect for generating reports without affecting current work
- **Branch-specific schemas**: Use `-b` to query data from branches with different table structures
- **Performance**: Historical queries access committed data, so they may be slower than current branch queries

```bash
# Good practice: commit first
git prolly commit -m "Save current work"
git prolly sql -b production "SELECT * FROM metrics"

# Avoid: Don't leave uncommitted changes
git prolly set user:new "data"  # Uncommitted change
git prolly sql -b main "SELECT * FROM users"  # Will be blocked
```

## Troubleshooting

### Common Issues

1. **"Failed to open dataset"**
   - Ensure you're in a git-prolly initialized directory
   - Run `git prolly init` if needed

2. **"Table not found"**
   - Check table name spelling
   - Ensure the table was created and changes were committed
   - Use `git prolly list` to see all keys (tables are stored with `:__schema__` suffix)

3. **Query errors**
   - Use `--verbose` flag for detailed error messages
   - Check SQL syntax - the parser is strict about formatting
   - Ensure column names match exactly (case-sensitive)

4. **"Cannot use -b/--branch parameter with uncommitted staging changes"**
   - Check staging status with `git prolly status`
   - Commit your changes first: `git prolly commit -m "Save changes"`
   - Or discard changes if not needed

5. **"Only SELECT statements are allowed when using -b/--branch parameter"**
   - Historical data is read-only for safety
   - Use regular `git prolly sql` (without `-b`) for data modifications
   - Switch to the target branch if you need to make changes there

6. **"Failed to checkout branch/commit"**
   - Verify the branch/commit exists: `git branch -a` or `git log --oneline`
   - Check branch name spelling (case-sensitive)
   - Ensure you have access to the specified commit

### Performance Tips

1. **Large Result Sets**: Use LIMIT to restrict output
   ```bash
   git prolly sql "SELECT * FROM large_table LIMIT 100"
   ```

2. **Complex Queries**: Break them into steps
   ```bash
   # Create intermediate results
   git prolly sql "CREATE TABLE temp_results AS SELECT ..."
   ```

3. **Bulk Operations**: Use file-based execution
   ```bash
   # More efficient than individual commands
   git prolly sql -f bulk_insert.sql
   ```

### Debug Mode

Use verbose mode for debugging:

```bash
git prolly sql --verbose "SELECT * FROM users"
```

This will show:
- Detailed error messages
- Query execution time
- Additional context for troubleshooting

## Limitations

Current limitations (may be addressed in future versions):

1. **No ALTER TABLE** - Create new table and migrate data instead
2. **Limited aggregate functions** - SUM() may have issues with empty groups
3. **No DISTINCT in some contexts** - Use GROUP BY as workaround
4. **No subqueries in some positions** - Use JOINs or temporary tables
5. **Case-sensitive identifiers** - Table and column names are case-sensitive

## Conclusion

The `git prolly sql` command brings the power of SQL to ProllyTree's versioned storage, enabling complex data analysis while maintaining Git's version control benefits. This unique combination allows you to:

- Query your data with familiar SQL syntax
- Version control your data changes
- Branch and merge data schemas
- Track data history over time
- Export data in multiple formats

For more examples and advanced usage, see the `examples/sql_example.rs` file in the repository.