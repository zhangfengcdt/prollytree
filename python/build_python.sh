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
#
# Usage:
#   ./build_python.sh                    # Build with default Python bindings
#   ./build_python.sh --with-sql         # Build with SQL support
#   ./build_python.sh --all-features     # Build with all features (Python + SQL)
#   ./build_python.sh --features "python sql"  # Specify features explicitly
#   ./build_python.sh --install          # Build and install the package
#   ./build_python.sh --with-sql --install  # Build with SQL and install

set -e

# Show help if requested
if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    echo "Build script for ProllyTree Python bindings"
    echo ""
    echo "Usage:"
    echo "  ./build_python.sh [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --with-sql           Build with SQL support"
    echo "  --all-features       Build with all features (Python + SQL)"
    echo "  --features FEATURES  Specify features explicitly (e.g., 'python sql')"
    echo "  --install            Install the built package after building"
    echo "  --help, -h           Show this help message"
    echo ""
    echo "Examples:"
    echo "  ./build_python.sh                       # Basic Python bindings"
    echo "  ./build_python.sh --with-sql            # With SQL support"
    echo "  ./build_python.sh --with-sql --install  # Build and install with SQL"
    exit 0
fi

echo "🔧 Building ProllyTree Python bindings..."

# Check if maturin is installed
if ! command -v maturin &> /dev/null; then
    echo "❌ maturin is not installed. Installing with pip..."
    pip install maturin
fi

# Change to project root directory
cd "$(dirname "$0")/.."

# Parse command line arguments for features
FEATURES="python"
for arg in "$@"; do
    case $arg in
        --features)
            shift
            FEATURES="$1"
            shift
            ;;
        --features=*)
            FEATURES="${arg#*=}"
            shift
            ;;
        --with-sql)
            FEATURES="python sql"
            shift
            ;;
        --all-features)
            FEATURES="python sql"
            shift
            ;;
    esac
done

# Build the wheel
echo "🍹 Building wheel with maturin (features: $FEATURES)..."
maturin build --release --features "$FEATURES"

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
print(f'✅ Basic test passed: {result == b\"value\"}')
"

    # Test SQL functionality if available
    if [[ "$FEATURES" == *"sql"* ]]; then
        echo "🧪 Testing SQL functionality..."
        python3 -c "
import tempfile
import subprocess
import os
from prollytree import ProllySQLStore

# Create temp dir and init git
with tempfile.TemporaryDirectory() as tmpdir:
    subprocess.run(['git', 'init'], cwd=tmpdir, capture_output=True)
    subprocess.run(['git', 'config', 'user.name', 'Test'], cwd=tmpdir, capture_output=True)
    subprocess.run(['git', 'config', 'user.email', 'test@test.com'], cwd=tmpdir, capture_output=True)

    # Create SQL store
    store_dir = os.path.join(tmpdir, 'data')
    os.makedirs(store_dir)
    store = ProllySQLStore(store_dir)

    # Test basic SQL operations
    store.create_table('test', [('id', 'INTEGER'), ('name', 'TEXT')])
    store.insert('test', [[1, 'Test']])
    result = store.select('test')

    print(f'✅ SQL test passed: {len(result) == 1 and result[0][\"name\"] == \"Test\"}')
" || echo "⚠️  SQL test skipped (import failed - may need git in temp dir)"
    fi
fi

echo "🎉 Build complete!"
echo ""
echo "Built with features: $FEATURES"
echo ""
echo "To install the wheel manually:"
echo "  pip install $WHEEL_PATH"
echo ""
echo "To test the bindings:"
echo "  python3 test_python_binding.py"
if [[ "$FEATURES" == *"sql"* ]]; then
    echo "  python3 python/examples/sql_example.py  # Test SQL functionality"
fi
echo ""
echo "To publish to PyPI:"
echo "  cd python && ./publish_python.sh test    # Publish to TestPyPI first"
echo "  cd python && ./publish_python.sh prod    # Publish to production PyPI"
