# Examples

End-to-end worked examples. Each one is runnable and exercises a different part of the library.

- **[Basic Tree](basic.md)** — in-memory tree, reads/writes, Merkle proof.
- **[Versioned Store](versioning.md)** — branching, merging, three-way conflict resolution.
- **[SQL Queries](sql.md)** — GlueSQL against a versioned store, including branch-scoped reads.
- **[Python Bindings](python.md)** — the same workflows from Python, plus LangMem integration.

If you're looking for something shorter, the [Quickstart](../quickstart.md) fits on one screen.

Runnable Rust examples also live in [`examples/`](https://github.com/zhangfengcdt/prollytree/tree/main/examples) in the repo — you can run any of them with `cargo run --example <name>`:

```bash
cargo run --example proof
cargo run --example versioning
cargo run --example sql --features sql
cargo run --example storage --features rocksdb_storage
cargo run --example worktree --features "git sql"
```
