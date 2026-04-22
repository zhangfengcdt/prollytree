# SQL Queries

GlueSQL against a ProllyTree store. Tables are persisted in the tree, so every write is versioned — `git-prolly commit`, `branch`, `merge` work exactly as with raw KV.

See [SQL Interface](../sql.md) for the full surface and limitations.

## CLI

```bash
git-prolly sql "CREATE TABLE users (id INTEGER, name TEXT, email TEXT)"
git-prolly sql "INSERT INTO users VALUES (1, 'Alice', 'alice@example.com'),
                                          (2, 'Bob',   'bob@example.com')"
git-prolly sql "SELECT * FROM users ORDER BY id"

# Commit the changes.
git-prolly commit -m "seed users"
```

### Read-only queries against history

```bash
git-prolly sql -b v1.0   "SELECT COUNT(*) FROM users"
git-prolly sql -b main   "SELECT * FROM users WHERE id = 42"
git-prolly sql -b feat   "SELECT name FROM users WHERE created_at > '2024-01-01'"
```

Write statements (`INSERT`/`UPDATE`/`DELETE`/`CREATE`) are rejected with `-b`. Switch branches if you want to modify.

### Output formats

```bash
git-prolly sql -o table "SELECT * FROM users"   # pretty (default)
git-prolly sql -o json  "SELECT * FROM users"   # machine readable
git-prolly sql -o csv   "SELECT * FROM users"   # spreadsheet-friendly
```

### File-driven workflow

```bash
cat > schema.sql <<'EOF'
CREATE TABLE products (
    id INTEGER, name TEXT, price INTEGER, category TEXT
);
INSERT INTO products VALUES
    (1, 'Laptop', 1200, 'Electronics'),
    (2, 'Book',     25, 'Education');
EOF

git-prolly sql -f schema.sql
git-prolly commit -m "initial schema"
```

### Interactive shell

```bash
git-prolly sql -i
# prolly-sql> CREATE TABLE foo (id INTEGER);
# prolly-sql> SELECT * FROM foo;
# prolly-sql> exit
```

## Python

```python
from prollytree import ProllySQLStore

sql = ProllySQLStore("./store")

sql.execute("""
    CREATE TABLE users (
        id INTEGER,
        name TEXT,
        email TEXT,
        signup_date TEXT,
        plan TEXT
    )
""")

sql.execute(
    "INSERT INTO users VALUES (?, ?, ?, ?, ?)",
    (1, "Alice", "alice@example.com", "2024-01-15", "premium"),
)

# Analytics.
rows = sql.execute("""
    SELECT plan, COUNT(*) AS cnt
    FROM users
    GROUP BY plan
    ORDER BY cnt DESC
""")
for r in rows:
    print(r)
```

## Combining SQL with versioning

SQL writes are ordinary tree writes — you commit them with the same `commit()` you use for raw KV:

```python
from prollytree import VersionedKvStore, ProllySQLStore

kv  = VersionedKvStore("./store")
sql = ProllySQLStore.from_store(kv)                # shared store

sql.execute("CREATE TABLE audit (actor TEXT, action TEXT, ts TEXT)")
sql.execute("INSERT INTO audit VALUES ('alice', 'login', '2024-01-15T09:00:00')")
kv.commit("initial audit")

# Now audit lives on main branch; branching off 'experiment' isolates writes.
kv.create_branch("experiment")
sql.execute("INSERT INTO audit VALUES ('alice', 'test', '2024-01-15T09:05:00')")
kv.commit("experimental entry")
```

This is the pattern behind branch-scoped data migrations and A/B testing — see [Versioning & Merge](../theory/versioning.md).
