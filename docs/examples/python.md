# Python Bindings

End-to-end Python examples. For the reference surface, see the [Python API](../api/python.md).

## Everyday tree

```python
from prollytree import ProllyTree

tree = ProllyTree()
tree.insert(b"hello", b"world")
tree.insert(b"foo", b"bar")

print(tree.find(b"hello"))           # b"world"

tree.update(b"hello", b"Hello!")
tree.delete(b"foo")

# Batch.
tree.insert_batch([(b"k1", b"v1"), (b"k2", b"v2")])
```

## Persistent tree

```python
tree = ProllyTree(storage_type="file", path="/tmp/prolly_data")
tree.insert(b"alpha", b"1")
# Data survives a restart.
```

## Versioned store with branch/merge

```python
from prollytree import VersionedKvStore, ConflictResolution

store = VersionedKvStore("./my_store")
store.insert(b"config:theme", b"light")
store.insert(b"config:lang",  b"en")
store.commit("initial config")

store.create_branch("darkmode")
store.update(b"config:theme", b"dark")
store.insert(b"config:animations", b"enabled")
store.commit("dark + animations")

store.checkout("main")
store.update(b"config:lang", b"fr")
store.commit("french")

ok, conflicts = store.try_merge("darkmode")
assert ok  # no overlapping keys

merge = store.merge("darkmode", ConflictResolution.TakeSource)
print("merge:", merge[:8])
```

## SQL

```python
from prollytree import ProllySQLStore

sql = ProllySQLStore("./sql_store")
sql.execute("CREATE TABLE users (id INTEGER, name TEXT, email TEXT)")
sql.execute("INSERT INTO users VALUES (1, 'Alice', 'alice@example.com'),"
            "                          (2, 'Bob',   'bob@example.com')")

for row in sql.execute("SELECT id, name FROM users ORDER BY id"):
    print(row)
```

See [SQL example](sql.md) for analytics-style queries.

## Document versioning

```python
from prollytree import VersionedKvStore
import json, datetime

store = VersionedKvStore("./docs_store")

doc = {"title": "My Document",
       "content": "Initial content",
       "author": "Alice",
       "created": datetime.datetime.utcnow().isoformat()}

store.insert(b"doc:readme", json.dumps(doc).encode())
store.commit("initial")

# Update.
doc["content"] = "Updated with more details"
doc["modified"] = datetime.datetime.utcnow().isoformat()
store.update(b"doc:readme", json.dumps(doc).encode())
store.commit("expand")

for c in store.log():
    print(c["id"][:8], c["message"])
```

## Configuration management

```python
prod = {
    "database": {"host": "prod-db", "port": 5432, "ssl": True},
    "api":      {"rate_limit": 1000, "timeout": 30},
}

dev = json.loads(json.dumps(prod))   # copy
dev["database"]["host"] = "localhost"
dev["database"]["ssl"]  = False
dev["api"]["rate_limit"] = 10_000

store.insert(b"config:production",  json.dumps(prod).encode())
store.insert(b"config:development", json.dumps(dev).encode())
store.commit("seed configs")
```

## Batch-insert benchmark

```python
import time
from prollytree import ProllyTree

tree = ProllyTree()
start = time.time()
for i in range(1000):
    tree.insert(f"single:{i:04d}".encode(), f"v{i}".encode())
print("single:", time.time() - start, "s")

tree = ProllyTree()
start = time.time()
tree.insert_batch([(f"batch:{i:04d}".encode(), f"v{i}".encode()) for i in range(1000)])
print("batch:", time.time() - start, "s")
```
