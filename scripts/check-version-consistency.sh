#!/usr/bin/env bash
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# Verifies that every file carrying a hardcoded ProllyTree version agrees
# with [package].version in Cargo.toml. Exits 0 on consistency, 1 on drift.
#
# Run from anywhere — the script cds into the repo root.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CANONICAL=$(awk '/^\[package\]/{p=1;next} p && /^version/{gsub(/"/,"",$3); print $3; exit}' Cargo.toml)

if [[ -z "$CANONICAL" ]]; then
    echo "ERROR: could not read [package].version from Cargo.toml" >&2
    exit 1
fi

echo "Canonical version (Cargo.toml): $CANONICAL"
echo

FAIL=0

report() {
    local status="$1" label="$2" found="$3"
    if [[ "$status" == ok ]]; then
        printf "  ok     %-55s %s\n" "$label" "$found"
    else
        printf "  DRIFT  %-55s expected %s, got %s\n" "$label" "$CANONICAL" "${found:-<not found>}"
        FAIL=1
    fi
}

check() {
    local label="$1" found="$2"
    if [[ "$found" == "$CANONICAL" ]]; then
        report ok "$label" "$found"
    else
        report drift "$label" "$found"
    fi
}

# Cargo.toml [package].version (canonical itself — still report for visibility)
check "Cargo.toml [package]" "$CANONICAL"

# Cargo.lock — the prollytree entry
v=$(awk '/^name = "prollytree"$/{getline; gsub(/"/,"",$3); print $3; exit}' Cargo.lock)
check "Cargo.lock [[package]] prollytree" "$v"

# pyproject.toml [project].version
v=$(awk '/^\[project\]/{p=1;next} p && /^version/{gsub(/"/,"",$3); print $3; exit}' pyproject.toml)
check "pyproject.toml [project]" "$v"

# python/prollytree/__init__.py __version__
v=$(grep -E '^__version__' python/prollytree/__init__.py | sed -E 's/.*"([^"]+)".*/\1/')
check "python/prollytree/__init__.py __version__" "$v"

# python/docs/conf.py — Sphinx release + version (both single-quoted)
v=$(grep -E "^release *=" python/docs/conf.py | sed -E "s/.*'([^']+)'.*/\1/")
check "python/docs/conf.py release" "$v"
v=$(grep -E "^version *=" python/docs/conf.py | sed -E "s/.*'([^']+)'.*/\1/")
check "python/docs/conf.py version" "$v"

# src/bin/prolly-ui.rs — clap #[command(version = "...")]
v=$(grep -E 'command\(version *= *"' src/bin/prolly-ui.rs | sed -E 's/.*"([^"]+)".*/\1/')
check "src/bin/prolly-ui.rs #[command(version)]" "$v"

# src/lib.rs — rustdoc code example: prollytree = "..."
v=$(grep -E 'prollytree *= *"' src/lib.rs | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
check "src/lib.rs rustdoc example" "$v"

# README.md — every install snippet must pin the same version.
# Collect every distinct quoted version-looking string on install-snippet lines.
readme_versions=()
while IFS= read -r line; do
    readme_versions+=("$line")
done < <(
    grep -E '^(prollytree *=|version *= *")' README.md \
        | grep -oE '"[0-9][^"]*"' \
        | tr -d '"' \
        | sort -u
)

if [[ ${#readme_versions[@]} -eq 0 ]]; then
    check "README.md install snippets" ""
elif [[ ${#readme_versions[@]} -eq 1 && "${readme_versions[0]}" == "$CANONICAL" ]]; then
    check "README.md install snippets" "${readme_versions[0]}"
else
    for v in "${readme_versions[@]}"; do
        check "README.md install snippet" "$v"
    done
fi

echo
if [[ $FAIL -eq 0 ]]; then
    echo "All version strings match $CANONICAL."
    exit 0
else
    echo "Version drift detected. Bump every DRIFT entry to $CANONICAL." >&2
    echo "See RELEASING.md for the full list of version-bearing files." >&2
    exit 1
fi
