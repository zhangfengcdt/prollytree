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

# Build script for ProllyTree Python bindings

set -e

echo "🔧 Building ProllyTree Python bindings..."

# Check if maturin is installed
if ! command -v maturin &> /dev/null; then
    echo "❌ maturin is not installed. Installing with pip..."
    pip install maturin
fi

# Change to project root directory
cd "$(dirname "$0")/.."

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
echo ""
echo "To publish to PyPI:"
echo "  cd python && ./publish_python.sh test    # Publish to TestPyPI first"
echo "  cd python && ./publish_python.sh prod    # Publish to production PyPI"
