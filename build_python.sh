#!/bin/bash

# Build script for ProllyTree Python bindings

set -e

echo "🔧 Building ProllyTree Python bindings..."

# Check if maturin is installed
if ! command -v maturin &> /dev/null; then
    echo "❌ maturin is not installed. Installing with pip..."
    pip install maturin
fi

# Build the wheel
echo "🍹 Building wheel with maturin..."
maturin build --release --features python

# Find the built wheel
WHEEL_PATH=$(find target/wheels -name "prollytree-*.whl" | head -1)

if [ -z "$WHEEL_PATH" ]; then
    echo "❌ No wheel found in target/wheels/"
    exit 1
fi

echo "✅ Built wheel: $WHEEL_PATH"

# Optionally install the wheel
if [ "$1" = "--install" ]; then
    echo "📦 Installing wheel..."
    pip install "$WHEEL_PATH" --force-reinstall
    echo "✅ Installed ProllyTree Python bindings"
    
    # Run quick test
    echo "🧪 Running quick test..."
    python3 -c "
from prollytree import ProllyTree, TreeConfig
tree = ProllyTree()
tree.insert(b'test', b'value')
result = tree.find(b'test')
print(f'✅ Test passed: {result == b\"value\"}')
"
fi

echo "🎉 Build complete!"
echo ""
echo "To install the wheel manually:"
echo "  pip install $WHEEL_PATH"
echo ""
echo "To test the bindings:"
echo "  python3 test_python_binding.py"