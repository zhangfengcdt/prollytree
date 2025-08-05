#!/bin/bash

# Build script for ProllyTree Python documentation
# This script builds the Python bindings with SQL features and generates Sphinx documentation

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYTHON_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$PYTHON_DIR")"

echo "Building ProllyTree Python documentation..."
echo "Project root: $PROJECT_ROOT"
echo "Python dir: $PYTHON_DIR"
echo "Docs dir: $SCRIPT_DIR"

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check for required tools
if ! command_exists sphinx-build; then
    echo "Error: sphinx-build not found. Please install Sphinx:"
    echo "pip install sphinx sphinx_rtd_theme sphinx-autodoc-typehints myst-parser"
    exit 1
fi

# Change to project root
cd "$PROJECT_ROOT"

# Build Python bindings with SQL features
echo "Building Python bindings with SQL features..."
./python/build_python.sh --with-sql --install

# Change to docs directory
cd "$SCRIPT_DIR"

# Clean previous build
echo "Cleaning previous documentation build..."
rm -rf _build

# Build documentation
echo "Building Sphinx documentation..."
sphinx-build -b html . _build/html

# Check if build was successful
if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Documentation built successfully!"
    echo "📖 Documentation available at: file://$SCRIPT_DIR/_build/html/index.html"
    echo ""
    echo "To view the documentation:"
    echo "  open $SCRIPT_DIR/_build/html/index.html"
    echo ""
    echo "To serve locally:"
    echo "  cd $SCRIPT_DIR/_build/html && python -m http.server 8000"
    echo "  Then visit: http://localhost:8000"
else
    echo "❌ Documentation build failed!"
    exit 1
fi
