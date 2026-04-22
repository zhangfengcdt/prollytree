# Versioned Store

Branch, commit, diff, merge — the full Git-like workflow on a key-value store.

## Rust

```rust
use prollytree::git::versioned_store::StoreFactory;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // data/ must be inside a git repo (init one first if needed).
    let mut store = StoreFactory::git::<32, _>("data")?;

    // Seed.
    store.insert(b"user:alice".to_vec(), b"Alice".to_vec())?;
    store.insert(b"user:bob".to_vec(),   b"Bob".to_vec())?;
    store.commit("seed users")?;

    // Work on a feature branch.
    store.create_branch("rename-alice")?;
    store.update(b"user:alice".to_vec(), b"Alice Smith".to_vec())?;
    store.commit("rename alice")?;

    // Back to main and merge.
    store.checkout("main")?;
    let merge = store.merge("rename-alice")?;
    println!("merge commit = {}", merge);

    // Inspect.
    for c in store.log()?.iter().take(5) {
        println!("{} {}", &c.id[..8], c.message);
    }
    Ok(())
}
```

Also see [`examples/versioning.rs`](https://github.com/zhangfengcdt/prollytree/blob/main/examples/versioning.rs).

## Python

```python
from prollytree import VersionedKvStore, ConflictResolution

store = VersionedKvStore("./store")

store.insert(b"user:alice", b"Alice")
store.insert(b"user:bob",   b"Bob")
store.commit("seed")

store.create_branch("rename")
store.update(b"user:alice", b"Alice Smith")
store.commit("rename alice")

store.checkout("main")

# Probe first.
ok, conflicts = store.try_merge("rename")
print("mergeable:", ok, "conflicts:", conflicts)

# Apply.
merge = store.merge("rename", ConflictResolution.TakeSource)
print("merge commit:", merge[:8])

for c in store.log():
    print(c["id"][:8], c["message"])
```

## Handling conflicts

Two branches change the same key:

```python
store.checkout("main")
store.insert(b"config:theme", b"light")
store.commit("light theme")

store.create_branch("darkmode")
store.update(b"config:theme", b"dark")
store.commit("dark theme")

store.checkout("main")
store.update(b"config:theme", b"solarized")
store.commit("solarized")

ok, conflicts = store.try_merge("darkmode")
# ok == False, conflicts == [MergeConflict(key=b"config:theme", ...)]

for c in conflicts:
    print(c.key, "base:", c.base_value, "src:", c.source_value, "dst:", c.destination_value)

# Resolve by taking the source.
store.merge("darkmode", ConflictResolution.TakeSource)
```

See [Theory → Versioning & Merge](../theory/versioning.md) for the algorithm and what the built-in resolvers do.

## Time travel

Read from a historical commit by asking `VersionedKvStore` for the tree at that ref — or, more ergonomically, from the CLI:

```bash
git-prolly keys-at v1.0 --values
git-prolly sql -b v1.0 "SELECT COUNT(*) FROM users"
git-prolly diff v1.0 v2.0
```

See the [CLI reference](../cli.md).

## Worktrees for concurrent work

If you need multiple writers against independent branches of the same store, use the worktree manager:

```python
# (pseudo-code — see the worktree example for the exact API)
wt = store.create_worktree("feature-x")
wt.insert(b"x", b"value")
wt.commit("work on x")
# main branch is unaffected.
```

Runnable example: [`examples/worktree.rs`](https://github.com/zhangfengcdt/prollytree/blob/main/examples/worktree.rs) (`cargo run --example worktree --features "git sql"`).
