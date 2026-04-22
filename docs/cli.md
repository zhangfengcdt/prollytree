# `git-prolly` CLI

`git-prolly` is the shell interface to a ProllyTree versioned key-value store. It wraps the Rust `VersionedKvStore` + SQL layers and is designed to feel like Git — `init`, `set`/`get`, `commit`, `log`, `branch`, `checkout`, `merge`, `diff`, `history`, `sql`.

If you'd rather call the library directly, see the [Rust API](api/rust.md) or [Python API](api/python.md).

## Setup

Install the binary:

```bash
cargo install prollytree --features git
git-prolly --help
```

`git-prolly` operates *inside* a git repository. The typical layout is a repo with a dedicated data subdirectory:

```bash
mkdir my-project && cd my-project
git init
mkdir data && git-prolly init data
cd data
```

From here on, commands run from `./data` operate on the prolly store. Remote sync (`git push`, `git pull`) uses plain Git.

## Repository management

### `init`

Initialise a ProllyTree KV store in the current directory (or the given path).

```bash
git-prolly init [path]
```

## Key-value operations

### `set <key> <value>`

Stage a key/value pair. The change is persisted but uncommitted.

```bash
git-prolly set user:123 "John Doe"
git-prolly set "complex key" "value with spaces"
```

### `get <key>`

Retrieve a value.

```bash
git-prolly get user:123
# → John Doe
```

### `delete <key>`

Stage a deletion.

```bash
git-prolly delete user:123
```

### `list [--values] [--graph]`

List keys. `--values` prints each value; `--graph` prints the tree structure.

```bash
git-prolly list
git-prolly list --values
git-prolly list --graph
```

## Version control

### `status`

Show staged changes not yet committed.

```bash
git-prolly status
# Staged changes:
#   added: config:theme
#   modified: user:123
#   deleted: user:456
```

### `commit -m "<message>"`

Commit staged changes. Creates a real Git commit on the current branch.

```bash
git-prolly commit -m "Initial user data"
```

### `log [--limit N] [--kv-summary]`

Show commit history.

```bash
git-prolly log
git-prolly log --limit 5
git-prolly log --kv-summary
# f1e2d3c4 - 2024-01-15 10:30:00 - Initial data (+2 ~1 -0)
```

### `show <commit> [--keys-only]`

Detail view of a commit.

```bash
git-prolly show HEAD
git-prolly show HEAD --keys-only
```

### `revert <commit>`

Create a new commit that undoes the changes from `<commit>`.

```bash
git-prolly revert f1e2d3c4
```

## Branching and merging

Branching uses native Git branches; the prolly store tracks them.

```bash
git checkout -b feature/darkmode
git-prolly set config:theme dark
git-prolly commit -m "Enable dark mode"

git checkout main
git-prolly merge feature/darkmode
```

### `merge <branch>`

Three-way merge at the key-value level. See [Theory → Versioning & Merge](theory/versioning.md) for the algorithm. Conflicting keys are resolved using the default strategy; use the Rust/Python API for custom resolvers.

## Diff and history

### `diff <from> <to> [--format <fmt>] [--keys <pattern>]`

```bash
git-prolly diff main feature/darkmode
git-prolly diff main feature/darkmode --format=detailed
git-prolly diff main feature/darkmode --format=json
git-prolly diff main feature/darkmode --keys "config:*"
```

Formats: `compact` (default), `detailed`, `json`.

### `history <key> [--format <fmt>] [--limit N]`

Show every commit that touched `<key>`.

```bash
git-prolly history user:123
git-prolly history user:123 --format=detailed
git-prolly history user:123 --limit=5 --format=json
```

### `keys-at <ref> [--values] [--format <fmt>]`

List keys (optionally with values) that exist at a given commit or branch.

```bash
git-prolly keys-at HEAD
git-prolly keys-at v1.0 --values
git-prolly keys-at a1b2c3d4 --format=json
```

### `stats [<commit>]`

Summary statistics — total keys, commits, current branch.

```bash
git-prolly stats
git-prolly stats c3d4e5f6
```

## SQL

`git-prolly sql` runs GlueSQL queries against the store. See the [SQL Interface](sql.md) for the full surface.

```bash
git-prolly sql "CREATE TABLE users (id INTEGER, name TEXT)"
git-prolly sql "INSERT INTO users VALUES (1, 'Alice')"
git-prolly sql "SELECT * FROM users"
git-prolly sql -f schema.sql
git-prolly sql -i                                 # interactive shell
git-prolly sql -b v1.0 "SELECT COUNT(*) FROM users"   # read-only, historical
git-prolly sql -o json "SELECT * FROM users"
```

## Output formats

Where a subcommand accepts `--format`:

- `compact` / default — one-line per row.
- `detailed` — multi-line block per row.
- `json` — machine-readable; best shape for `jq`, CI, or programmatic callers.
- `csv` — SQL only.

Setting `MEMOIR_*`-style env vars isn't a thing here; pass `--format json` explicitly when scripting.

## Integration with plain Git

Because the store is a Git repository underneath, everything Git knows how to do still works:

```bash
# Push to a remote
git remote add origin git@github.com:you/mystore.git
git push -u origin main

# Clone elsewhere
git clone git@github.com:you/mystore.git
cd mystore && git-prolly list --values
```

You can also combine the two: use `git log`/`git blame` for the commit graph, and `git-prolly history <key>` for per-key history.

## Troubleshooting

- **"Repository not found."** Run `git-prolly init` in the directory, or check that you're inside the `data/` subdirectory of your git repo.
- **"Cannot use -b/--branch parameter with uncommitted staging changes."** Commit or discard staged changes before running a branch-scoped SQL query.
- **"Only SELECT statements are allowed when using -b/--branch."** Historical commits are read-only. Switch to the target branch first if you need to write.
- **Pre-commit / hook failures.** The store is a plain Git repo — the usual `git config`, hook setup, and `git reset` apply.

For a richer walkthrough see [Basic Usage](basic_usage.md) and the [Versioned Store example](examples/versioning.md).
