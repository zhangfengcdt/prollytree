# Releasing ProllyTree

This document describes how to cut a release. The process is driven by the
[`Release` workflow](.github/workflows/release.yml), which is triggered
manually via GitHub Actions `workflow_dispatch`.

A release produces, in order:

1. A Rust crate published to [crates.io](https://crates.io/crates/prollytree)
2. Python wheels + source distribution published to
   [PyPI](https://pypi.org/project/prollytree/)
3. A GitHub Release tagged `v<version>` with the wheels and sdist attached

Each step is independently toggleable, and the whole pipeline has a dry-run
mode that validates packaging without publishing.

## Prerequisites

Configured once per repository:

- **`CARGO_REGISTRY_TOKEN`** — repository secret with a crates.io API token
  scoped to publish `prollytree`.
- **`pypi` GitHub Environment** — used for PyPI trusted publishing (OIDC). The
  project must be registered on PyPI with this repo + the `publish-python`
  job + the `pypi` environment as a trusted publisher. No PyPI API token is
  stored; the job mints short-lived credentials via `id-token: write`.
- **Branch protection / permissions** — whoever runs the workflow needs
  permission to dispatch workflows and (for the final step) `contents: write`
  to create the tag and GitHub Release.

## Versioning

- The release version is read from the `version` field of
  [`Cargo.toml`](Cargo.toml) by the `validate` job. There is no version input
  on the workflow — whatever is committed on the release branch is what ships.
- Suffixes `alpha`, `beta`, or `rc` in the version string cause the GitHub
  Release to be marked as a pre-release automatically.
- The Python package version is kept in sync via `pyproject.toml` /
  `maturin`, which reads from `Cargo.toml`.

## Release branch convention

The `validate` job rejects any branch whose name does not start with
`release/` or `release-`. Typical names:

- `release/0.3.3`
- `release-0.4.0-rc1`

The workflow refuses to run on `main` or feature branches.

## Step-by-step

### 1. Prepare the release branch

From an up-to-date `main`:

```bash
git checkout -b release/<version>

# Bump version in Cargo.toml (drop any -beta/-snapshot suffix if shipping stable)
$EDITOR Cargo.toml

# Regenerate Cargo.lock so `cargo check --locked` passes in CI
cargo check --all-features

# Optional: update CHANGELOG.md. The release-notes step will extract the
# section matching `## [<version>]` and include it in the GitHub Release body.
$EDITOR CHANGELOG.md

git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "Release v<version>"
git push -u origin release/<version>
```

Open a PR from `release/<version>` → `main` so CI runs (tests, clippy,
pre-commit, docs, and the Python wheel build) against the exact commit you
intend to ship. Do **not** merge the PR yet — the release workflow runs on
the release branch itself.

### 2. Dry run (recommended)

From the GitHub **Actions → Release** page:

- **Branch:** `release/<version>`
- **publish_rust:** `true`
- **publish_python:** `true`
- **dry_run:** `true`

This runs `cargo publish --dry-run --all-features`, builds all wheels, runs
`twine check`, and uploads to TestPyPI. No tag or GitHub Release is created
(`create-release` is gated on `dry_run == 'false'`).

Fix any packaging issues on the release branch and repeat until the dry run
is green.

### 3. Publish for real

Re-run the workflow with the same branch and `dry_run: false`. The jobs run
in this order:

| Job | Runs on | What it does |
| --- | --- | --- |
| `validate` | ubuntu | Checks branch name, extracts version from `Cargo.toml` |
| `publish-rust` | ubuntu | `cargo check/test --all-features`, then `cargo publish --all-features` |
| `build-python-wheels` | ubuntu-latest (x86_64, aarch64 via QEMU), windows-latest (x64), macos-14 (aarch64) | `maturin build --release --features python,git,sql` per target |
| `publish-python` | ubuntu | Downloads all wheel artifacts, builds sdist, publishes via PyPI trusted publishing |
| `create-release` | ubuntu | Creates git tag `v<version>`, generates release notes, attaches `dist/*.whl` and `dist/*.tar.gz` |

The macOS wheel is built for Apple Silicon only; Intel Macs pick it up via
Rosetta. Linux aarch64 wheels are cross-built under QEMU, so they take
noticeably longer than the x86_64 job.

### 4. Post-release

- Merge the release PR into `main`.
- Bump `Cargo.toml` to the next development version (e.g. `0.3.4-beta`) on
  `main` in a follow-up commit.
- Verify the tag, GitHub Release, crates.io page, and PyPI page all show
  the new version.

## Publishing only one ecosystem

Set `publish_rust: false` or `publish_python: false` on dispatch. The
`create-release` job is gated on the selected jobs either succeeding or
being skipped, so it still runs and tags the release as long as whatever you
asked for succeeded.

## Rollback

Neither crates.io nor PyPI allow re-publishing a version that already exists.
If a bad release ships:

- **crates.io:** `cargo yank --version <version> prollytree`
  (yanking prevents new dependents from resolving it but does not delete it).
- **PyPI:** delete the release via the PyPI UI if you catch it quickly,
  otherwise yank.
- Cut a new patch release (`<version>+1`) with the fix. Do not attempt to
  reuse the same version number.

## Troubleshooting

- **`validate` fails with "Not a release branch"** — you dispatched from
  `main` or a feature branch. Re-run from a `release/*` branch.
- **`cargo publish` fails on a transitive path dependency** — all workspace
  crates that `prollytree` depends on must already be on crates.io at a
  compatible version. Publish them first, then re-run.
- **PyPI publish fails with an OIDC / trusted-publisher error** — the `pypi`
  environment, the repository, the workflow filename (`release.yml`), and
  the job name (`publish-python`) must all match the trusted-publisher
  configuration on PyPI exactly. Re-check the PyPI project settings.
- **aarch64 Linux wheel build hangs or times out** — QEMU emulation is slow;
  re-run the job. If it consistently fails, the Rust build may be OOMing
  under emulation and the wheel matrix entry needs attention.
- **`create-release` is skipped** — check that `dry_run` was `false` and at
  least one of `publish-rust` / `publish-python` succeeded (the job is gated
  on `success || skipped` for each).
