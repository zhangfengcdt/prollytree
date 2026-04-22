# Installation

ProllyTree ships as a **Rust crate**, a **Python package**, and a **CLI binary** (`git-prolly`). Pick whichever entry point matches how you plan to use it — the underlying library is the same.

## Requirements

- **Rust**: 1.75+ (edition 2021) — install via [rustup](https://rustup.rs).
- **Python**: 3.8+ if you want the Python bindings.
- **Git**: required for the `git` feature and the `git-prolly` CLI.
- **C toolchain**: needed only if you enable the `rocksdb_storage` feature.

## Rust crate

Add the dependency to `Cargo.toml`:

```toml
[dependencies]
prollytree = "0.3.3-beta"
```

Opt in to Git-backed versioning, SQL, or RocksDB via feature flags:

```toml
[dependencies.prollytree]
version = "0.3.3-beta"
features = ["git", "sql", "rocksdb_storage"]
```

| Feature | What it enables |
|---|---|
| `git` | Git-backed versioned storage, branching, merge, `git-prolly` CLI |
| `sql` | GlueSQL query interface, `git-prolly sql …` |
| `rocksdb_storage` | RocksDB `NodeStorage` backend |
| `python` | PyO3 extension module (only used by the Python build) |
| `tracing` | `tracing` instrumentation |
| `digest_base64` | Base64 encoding for digests (on by default) |

The `git` and `sql` features are enabled by default.

## `git-prolly` CLI

Install the CLI directly from crates.io:

```bash
cargo install prollytree --features git
```

Or build from source:

```bash
git clone https://github.com/zhangfengcdt/prollytree.git
cd prollytree
cargo build --release --features "git sql" --bin git-prolly
# Binary is at ./target/release/git-prolly
```

Verify:

```bash
git-prolly --help
```

See the [CLI reference](cli.md) for every subcommand.

## Python bindings

Install from PyPI:

```bash
pip install prollytree
```

Or build from source with [maturin](https://www.maturin.rs/):

```bash
git clone https://github.com/zhangfengcdt/prollytree.git
cd prollytree
pip install maturin

# Build with the python + sql feature set
./python/build_python.sh --with-sql --install

# Or with every feature
./python/build_python.sh --all-features --install
```

Verify:

```python
import prollytree
print(prollytree.__version__)
```

See the [Python API reference](api/python.md) and the [Python examples](examples/python.md) for end-to-end usage.

## Building the docs locally

The site is built with [mkdocs-material](https://squidfunk.github.io/mkdocs-material/). From a repo checkout:

```bash
pip install -r docs/requirements.txt

# Build the static site into ./site
mkdocs build

# Start the live-reload dev server
mkdocs serve

# Serve on a specific port
mkdocs serve -a 0.0.0.0:8080
```

The build is driven by [`mkdocs.yml`](https://github.com/zhangfengcdt/prollytree/blob/main/mkdocs.yml) at the repository root. Every page under `docs/` is a plain Markdown file you can edit and preview live.

## Troubleshooting

- **`cargo install` fails compiling `rocksdb`.** Either drop the `rocksdb_storage` feature, or install the system C/C++ toolchain (`build-essential` on Debian/Ubuntu, `xcode-select --install` on macOS).
- **Python import errors after `maturin build`.** Use a clean virtualenv: `python -m venv .venv && source .venv/bin/activate`, then re-run the install script.
- **`git-prolly` complains about missing git user config.** Run `git config user.name "…"` and `git config user.email "…"` in the repo before committing.

For anything else, open an [issue](https://github.com/zhangfengcdt/prollytree/issues) or check the [FAQ](faq.md).
