# Deployment Setup

This document describes the one-time GitHub and external-service configuration
required for the `Release` workflow (`.github/workflows/release.yml`) to publish
to crates.io, PyPI, and GitHub Releases.

## Overview

The release workflow is **manually dispatched** via `workflow_dispatch` and runs
only on branches named `release/*` or `release-*`. It has three boolean inputs:

| Input | Default | Effect |
|---|---|---|
| `publish_rust` | `true` | Publish the crate to crates.io |
| `publish_python` | `true` | Build wheels + publish to PyPI |
| `dry_run` | `false` | Validate only (no publishing, no tag, no GitHub Release) |

On a successful non-dry-run, the workflow:

1. Publishes `prollytree` to **crates.io** (`cargo publish --all-features`)
2. Builds Python wheels on Linux (x86_64, aarch64), Windows (x64), and macOS (arm64)
3. Publishes wheels + sdist to **PyPI** via OIDC trusted publishing
4. Creates a GitHub tag `v<version>` and a GitHub Release with auto-generated notes

## One-time setup

### 1. crates.io API token (repository secret)

The `publish-rust` job reads `CARGO_REGISTRY_TOKEN` from repository secrets.

1. Sign in at https://crates.io/me → **API Tokens** → **New Token**.
2. Configure the token:
   - **Name**: e.g. `prollytree-gha`
   - **Scopes**: `publish-update` only (do not enable `publish-new` or `yank` unless needed)
   - **Crates**: scope to `prollytree`
   - **Expiration**: 30–90 days (set a calendar reminder to rotate)
3. Click **Create** and copy the token immediately — it is shown only once. A
   valid modern token is ~40+ characters with no whitespace.
4. In the GitHub repo → **Settings** → **Secrets and variables** → **Actions**
   → **New repository secret**:
   - **Name**: `CARGO_REGISTRY_TOKEN` (exact case)
   - **Secret**: paste the token (no leading/trailing whitespace)

Must be a **repository** secret, not an environment secret — `publish-rust`
does not declare an `environment:` and therefore cannot see environment secrets.

To verify the token format is well-formed before saving to GitHub, `cargo login
<token>` locally will reject a malformed token.

### 2. PyPI trusted publishing (OIDC, no secret)

PyPI does not use a long-lived API token. Instead, the `publish-python` job
mints a short-lived GitHub OIDC token (enabled by `permissions: id-token:
write` and `environment: pypi`), which PyPI exchanges for a short-lived upload
token.

1. Sign in at https://pypi.org/.
2. If `prollytree` already exists on PyPI: go to
   https://pypi.org/manage/project/prollytree/settings/publishing/ and click
   **Add a new publisher**.
   If not: go to https://pypi.org/manage/account/publishing/ and add a
   **pending publisher**.
3. In the **GitHub** tab, enter **exactly**:

   | Field | Value |
   |---|---|
   | Owner | `zhangfengcdt` |
   | Repository name | `prollytree` |
   | Workflow name | `release.yml` (filename only — no `.github/workflows/` prefix) |
   | Environment name | `pypi` |

4. Click **Add**.

If any field mismatches by even one character, PyPI returns `invalid-publisher`
at publish time. The error page includes the exact OIDC claims that were
presented — compare them side-by-side with the PyPI configuration.

### 3. GitHub `pypi` environment

The `publish-python` job declares `environment: pypi`. The environment must
exist in the repo or the job will fail.

1. Repo → **Settings** → **Environments** (left sidebar) → **New environment**.
2. Name: `pypi` (exact match, lowercase).
3. Recommended protection:
   - **Deployment branches and tags** → **Selected branches and tags** → add
     pattern `release/*` so only release branches can deploy to PyPI.
   - Optionally add **Required reviewers** to gate each publish on manual
     approval.
4. Click **Save protection rules**.

No environment **secrets** are needed — OIDC replaces the token.

## Releasing

### Pre-release checklist

Before dispatching, make sure these are in sync on the release branch:

- `Cargo.toml` `version`
- `pyproject.toml` `version`
- `python/prollytree/__init__.py` `__version__`
- `python/docs/conf.py` `release` and `version`
- `README.md` install examples
- `src/lib.rs` doc example
- `src/bin/prolly-ui.rs` `#[command(version = ...)]`
- `Cargo.lock` (`cargo check` to regenerate)

Local validation:

```bash
cargo fmt --all -- --check
cargo clippy --all --all-features -- -D warnings
cargo check --all-features
cargo test --all-features        # runs in release workflow; Linux CI covers it
cargo publish --dry-run --features "git,sql" --allow-dirty   # macOS cannot run --all-features locally due to pyo3 libpython linkage
```

### Dispatching

1. Create a branch named `release/v<version>` (workflow rejects anything else).
2. Commit the version bump.
3. Push the branch.
4. Either via GitHub UI (**Actions** → **Release** → **Run workflow**) or CLI:

   ```bash
   # Dry run first (publishes wheels to TestPyPI, does not create tag or GitHub Release)
   gh workflow run release.yml --ref release/v<version> \
     -f publish_rust=true -f publish_python=true -f dry_run=true
   gh run watch

   # Production release
   gh workflow run release.yml --ref release/v<version> \
     -f publish_rust=true -f publish_python=true -f dry_run=false
   gh run watch
   ```

5. After success, merge the release branch back to `main`.

### Post-release verification

```bash
curl -s https://crates.io/api/v1/crates/prollytree | jq '.crate.max_version'
curl -s https://pypi.org/pypi/prollytree/json | jq '.info.version'
gh release view v<version>
```

## Troubleshooting

### `please provide a non-empty token` (publish-rust)

`CARGO_REGISTRY_TOKEN` secret is missing or empty. See [§1](#1-cratesio-api-token-repository-secret).
Must be a repository secret, not an environment secret.

### `The given API token does not match the format used by crates.io`

The token was pasted with corruption (trailing newline, partial copy) or is a
pre-2020-07-14 token. Regenerate on crates.io, verify with `cargo login`, and
replace the secret.

### `invalid-publisher: valid token, but no corresponding publisher` (publish-python)

OIDC succeeded but PyPI's trusted publisher config doesn't match the claims.
Compare the claims shown in the error message to the PyPI form field-by-field.
Most common cause: entering the workflow **path** (`.github/workflows/release.yml`)
instead of the **filename** (`release.yml`).

### macOS wheel build stuck at `Waiting for a runner to pick up this job...`

GitHub retired the free `macos-13` (Intel) runner image in early 2026. Either
drop the x86_64 macOS entry (arm64 wheels run on Intel Macs under Rosetta),
switch to the paid `macos-13-large` runner, or build a `universal2-apple-darwin`
wheel on `macos-14`.

### `error: the package 'prollytree' does not contain this feature: <name>`

A feature referenced in the workflow `--features` list was removed from
`Cargo.toml`. Update `release.yml` to match the current feature set in
`Cargo.toml`.

### maturin fails with `Couldn't find any python interpreters`

Pass `-i python3.11` (or another interpreter present on the runner) to the
maturin args. Combined with `pyo3` `abi3-py38` feature, one wheel per platform
covers all Python 3.8+.

### Tests fail with missing imports under `cargo test --all-features`

A test references a type gated on a feature (e.g. `rocksdb_storage`) without
importing it. Add a `#[cfg(feature = "<name>")] use ...;` in the test module.
Consider adding a CI matrix row that enables the feature so the failure is
caught on PRs rather than during release.

## Rollback

| Target | Command / action | Limitations |
|---|---|---|
| crates.io | `cargo yank --version <v>` | Cannot delete; yanked versions are permanently unusable for new `cargo publish` of the same version |
| PyPI | Delete release via web UI | Permanent — version number can never be reused |
| GitHub Release | `gh release delete v<v> --cleanup-tag` | Safe to recreate |

If a half-published release must be recovered, generally the right move is to
bump to the next patch version (`0.X.Y+1`) rather than attempting to re-publish
the same number.
