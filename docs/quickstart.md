# Quickstart

This page gets you from zero to a working prolly tree in both Rust and Python, then shows the Git-backed versioned-KV workflow with the `git-prolly` CLI.

If you haven't installed ProllyTree yet, see [Installation](installation.md).

## Rust: an in-memory tree with a Merkle proof

```rust
use prollytree::tree::{ProllyTree, Tree};
use prollytree::storage::InMemoryNodeStorage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 32-byte hashes (SHA-256) is the common config.
    let storage = InMemoryNodeStorage::<32>::new();
    let mut tree = ProllyTree::new(storage, Default::default());

    tree.insert(b"user:alice".to_vec(), b"Alice Johnson".to_vec());
    tree.insert(b"user:bob".to_vec(), b"Bob Smith".to_vec());
    tree.insert(b"config:timeout".to_vec(), b"30".to_vec());

    // Point lookup.
    if let Some(node) = tree.find(b"user:alice") {
        for (i, k) in node.keys.iter().enumerate() {
            if k == b"user:alice" {
                println!("alice = {}", String::from_utf8(node.values[i].clone())?);
            }
        }
    }

    // Cryptographic inclusion proof.
    let proof = tree.generate_proof(b"user:alice");
    assert!(tree.verify(proof, b"user:alice", Some(b"Alice Johnson")));

    // Root hash is stable across any tree with the same key/value set,
    // regardless of insertion order.
    println!("root = {:?}", tree.root_hash());
    Ok(())
}
```

## Rust: Git-backed versioned store

Turn the tree into a branchable, committable key-value store. This uses the `git` feature.

```rust
use prollytree::git::versioned_store::StoreFactory;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize a Git-backed store in ./data (must be inside a git repo).
    let mut store = StoreFactory::git::<32, _>("data")?;

    store.insert(b"config/api_key".to_vec(), b"secret123".to_vec())?;
    store.insert(b"config/timeout".to_vec(), b"30".to_vec())?;
    let c1 = store.commit("Initial config")?;
    println!("commit = {}", c1);

    // Branch, edit, come back, merge.
    store.create_branch("experimental")?;
    store.update(b"config/timeout".to_vec(), b"60".to_vec())?;
    store.commit("bump timeout")?;

    store.checkout("main")?;
    store.merge("experimental")?; // three-way merge on the KV level
    Ok(())
}
```

More in [Versioned Store example](examples/versioning.md) and [Theory: Versioning & Merge](theory/versioning.md).

## Python: versioned KV with branching

```python
from prollytree import VersionedKvStore, ConflictResolution

store = VersionedKvStore("./my_store")

store.insert(b"user:alice", b"Alice")
store.insert(b"user:bob", b"Bob")
store.commit("seed users")

# Branch, change, merge.
store.create_branch("feature")
store.update(b"user:alice", b"Alice Smith")
store.commit("rename alice")

store.checkout("main")
merge_commit = store.merge("feature", ConflictResolution.TakeSource)
print("merged:", merge_commit[:8])

# Inspect history.
for c in store.log():
    print(c["id"][:8], c["message"])
```

See [Examples → Python bindings](examples/python.md) for the full surface including SQL and LangMem integration.

## CLI: `git-prolly` as a Git-like KV store

```bash
# 1. Create a repo and initialize a KV dataset inside it.
git init demo && cd demo
mkdir data && git-prolly init data
cd data

# 2. Put data, commit.
git-prolly set "user:alice" "Alice Johnson"
git-prolly set "user:bob" "Bob Smith"
git-prolly commit -m "seed users"

# 3. Branch, edit, merge.
git checkout -b feature/rename
git-prolly set "user:alice" "Alice J."
git-prolly commit -m "rename alice"
git checkout main
git-prolly merge feature/rename

# 4. Inspect.
git-prolly list --values
git-prolly log
git-prolly history "user:alice"
```

See the [CLI reference](cli.md) for every subcommand, and the [SQL Interface](sql.md) page for `git-prolly sql …` usage.

## What to read next

- **[Basic Usage](basic_usage.md)** — a longer walk through the API with commentary.
- **[Architecture](architecture.md)** — how the tree, storage, versioning, and SQL layers fit.
- **[Theory](theory/index.md)** — why a prolly tree is shaped the way it is.
- **[FAQ](faq.md)** — the questions people ask on day one.
