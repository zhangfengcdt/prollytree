# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project

ProllyTree is a Rust probabilistic B-tree with Merkle properties, plus Python bindings
and a `git-prolly` CLI for Git-versioned key-value storage.

## Layout

- `src/tree.rs`, `src/node.rs` — core probabilistic B-tree
- `src/storage.rs`, `src/rocksdb/`, `src/git/storage.rs` — pluggable storage backends
- `src/git/` — Git-backed versioned KV store, branching, merge, history
- `src/sql.rs` — GlueSQL query interface (`sql` feature)
- `src/agent/` — AI agent memory layer
- `src/python.rs` — PyO3 bindings (`python` feature)
- `src/git/git-prolly.rs` — `git-prolly` CLI binary

## Feature flags

| Flag | Purpose |
|------|---------|
| `git` | Git-backed versioned storage |
| `sql` | GlueSQL query interface |
| `rocksdb_storage` | RocksDB persistent backend |
| `python` | PyO3 bindings (build with `maturin`, not raw `cargo`) |
| `rig` | Rig framework integration |
| `tui` | Terminal UI |

Most work uses `--features "git sql"`.

## Commands

```bash
# Build & quality gate (run before claiming done)
cargo build --all
cargo fmt --all
cargo clippy --all

# Tests
cargo test --features "git sql"
cargo test --features "git sql" <name> -- --nocapture

# Python bindings (use a venv; needs maturin)
./python/build_python.sh --all-features --install
python -m pytest python/tests/

# Benchmarks
cargo bench --bench tree   # also: sql, git, storage
```

`git-prolly --help` is the source of truth for the CLI — don't duplicate it here.

## Project-specific gotchas

- **Tests need git identity**: many tests run `git init` and require
  `git config user.name` / `user.email` set in the temp repo.
- **Dataset subdirectory required**: `git-prolly init` must run in a *subdirectory* of
  the git repo, not at git root, or commits will accidentally stage all repo files.
- **Python feature can't be built with raw cargo**: `cargo build --all-features` fails
  to link PyO3 — use `./python/build_python.sh` instead.
- **Test cwd serialization**: tests that mutate `std::env::set_current_dir` must hold
  the global mutex via `cwd_lock()` / `CwdGuard` (see
  `src/git/versioned_store/tests.rs`).

## Hard rules

- **Never commit or push** without explicit user instruction (covers both local commits
  and remote pushes).
- **Never test in the project working directory** — use `/tmp` or `/var/tmp`.
- **Always add the Apache 2.0 license header** to new source files (Rust, Python):

  ```
  // Licensed under the Apache License, Version 2.0 (the "License");
  // you may not use this file except in compliance with the License.
  // You may obtain a copy of the License at
  //
  //     http://www.apache.org/licenses/LICENSE-2.0
  //
  // Unless required by applicable law or agreed to in writing, software
  // distributed under the License is distributed on an "AS IS" BASIS,
  // WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  // See the License for the specific language governing permissions and
  // limitations under the License.
  ```
- **Fix all errors and warnings introduced by your changes** before finishing
  (`cargo build --all`, `cargo fmt --all`, `cargo clippy --all`).
