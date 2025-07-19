#!/bin/bash

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

# Script to manually publish ProllyTree Python package to PyPI
# Usage: ./publish_python.sh [test|prod]

set -e

ENVIRONMENT=${1:-test}

echo "🔧 Building ProllyTree Python package for publication..."

# Change to project root directory
cd "$(dirname "$0")/.."

# Check if maturin is installed
if ! command -v maturin &> /dev/null; then
    echo "❌ maturin is not installed. Installing with pip..."
    pip install maturin
fi

# Clean previous builds
echo "🧹 Cleaning previous builds..."
rm -rf target/wheels/* dist/* build/*

# Build wheels for multiple platforms (if on CI) or current platform
echo "🔨 Building wheels..."
if [ "$CI" = "true" ]; then
    # CI environment - build for multiple platforms
    maturin build --release --features python --find-interpreter
else
    # Local environment - build for current platform
    maturin build --release --features python
fi

# Build source distribution
echo "📦 Building source distribution..."
maturin sdist

# Check the built packages
echo "📋 Built packages:"
ls -la target/wheels/
ls -la dist/ 2>/dev/null || echo "No dist/ directory found"

# Determine target registry
if [ "$ENVIRONMENT" = "test" ]; then
    REGISTRY_ARG="--repository testpypi"
    REGISTRY_NAME="TestPyPI"
    REGISTRY_URL="https://test.pypi.org/project/prollytree/"
else
    REGISTRY_ARG=""
    REGISTRY_NAME="PyPI"
    REGISTRY_URL="https://pypi.org/project/prollytree/"
fi

echo ""
echo "🚀 Ready to publish to $REGISTRY_NAME"
echo "📍 Wheels will be uploaded from: target/wheels/"

# Confirm publication
read -p "Do you want to proceed with publication to $REGISTRY_NAME? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "❌ Publication cancelled."
    exit 1
fi

# Check if API token is set
if [ "$ENVIRONMENT" = "test" ]; then
    if [ -z "$MATURIN_PYPI_TOKEN" ] && [ -z "$TEST_PYPI_API_TOKEN" ]; then
        echo "❌ No TestPyPI API token found."
        echo "   Set MATURIN_PYPI_TOKEN or TEST_PYPI_API_TOKEN environment variable"
        echo "   Get token from: https://test.pypi.org/manage/account/token/"
        exit 1
    fi
else
    if [ -z "$MATURIN_PYPI_TOKEN" ] && [ -z "$PYPI_API_TOKEN" ]; then
        echo "❌ No PyPI API token found."
        echo "   Set MATURIN_PYPI_TOKEN or PYPI_API_TOKEN environment variable"
        echo "   Get token from: https://pypi.org/manage/account/token/"
        exit 1
    fi
fi

# Publish
echo "📤 Publishing to $REGISTRY_NAME..."
if [ "$ENVIRONMENT" = "test" ]; then
    maturin upload $REGISTRY_ARG target/wheels/* --non-interactive --skip-existing
else
    maturin upload target/wheels/* --non-interactive --skip-existing
fi

echo ""
echo "✅ Successfully published to $REGISTRY_NAME!"
echo "🔗 View at: $REGISTRY_URL"
echo ""
echo "📥 Install with:"
if [ "$ENVIRONMENT" = "test" ]; then
    echo "   pip install --index-url https://test.pypi.org/simple/ prollytree"
else
    echo "   pip install prollytree"
fi