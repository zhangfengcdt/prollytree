# Basic Tree

Create a tree, put some keys in, verify one of them with a Merkle proof. No Git, no SQL — just the core data structure.

## Rust

```rust
use prollytree::tree::{ProllyTree, Tree};
use prollytree::storage::InMemoryNodeStorage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tree = ProllyTree::new(
        InMemoryNodeStorage::<32>::new(),
        Default::default(),
    );

    // Batch insert — faster than one-at-a-time on big loads.
    tree.insert_batch(&[
        (b"user:alice".to_vec(),    b"Alice Johnson".to_vec()),
        (b"user:bob".to_vec(),      b"Bob Smith".to_vec()),
        (b"config:timeout".to_vec(), b"30".to_vec()),
    ]);

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

    // Root hash — a stable fingerprint of the KV set.
    println!("root = {:?}", tree.root_hash());
    Ok(())
}
```

Run it with the [`proof` example](https://github.com/zhangfengcdt/prollytree/blob/main/examples/proof.rs) in the repo:

```bash
cargo run --example proof
```

## Python

```python
from prollytree import ProllyTree

tree = ProllyTree()
tree.insert_batch([
    (b"user:alice",    b"Alice Johnson"),
    (b"user:bob",      b"Bob Smith"),
    (b"config:timeout", b"30"),
])

print("alice =", tree.find(b"user:alice"))

proof = tree.generate_proof(b"user:alice")
assert tree.verify(proof, b"user:alice", b"Alice Johnson")

print("root =", tree.root_hash().hex())
```

## Things to try

1. **Insert the same keys in a different order.** Verify that `root_hash()` is identical. This is the history-independence property — see [Prolly Trees](../theory/prolly_tree.md).
2. **Switch to a persistent backend.** Swap `InMemoryNodeStorage` for `FileNodeStorage` or `RocksDBNodeStorage` (Rust) or pass `storage_type="file", path=…` (Python). The tree code is the same.
3. **Generate an absence proof.** `verify(proof, missing_key, None)` should return `True` if `missing_key` isn't in the tree. See [Theory → Merkle Properties & Proofs](../theory/merkle.md).
