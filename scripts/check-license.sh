#!/bin/bash

# License header check script for Apache 2.0 license
# This script checks if Rust (.rs) and Python (.py) files contain the Apache 2.0 license header

set -e

# Apache 2.0 license header for Rust files (multi-line comment)
read -r -d '' RUST_LICENSE_HEADER << 'EOF' || true
/*
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/
EOF

# Apache 2.0 license header for Python files (single-line comments)
read -r -d '' PYTHON_LICENSE_HEADER << 'EOF' || true
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
EOF

EXIT_CODE=0

check_rust_license() {
    local file="$1"

    # Skip files in target directory (generated code)
    if [[ "$file" =~ target/ ]]; then
        return 0
    fi

    # Read first 14 lines of the file (license header length)
    local file_header
    file_header=$(head -14 "$file" 2>/dev/null || echo "")

    # Check if license header is present
    if [[ "$file_header" != "$RUST_LICENSE_HEADER"* ]]; then
        echo "❌ Missing or incorrect Apache 2.0 license header in: $file"
        echo "Expected header:"
        echo "$RUST_LICENSE_HEADER"
        echo ""
        echo "Found header:"
        echo "$file_header"
        echo ""
        return 1
    fi

    echo "✅ License header OK: $file"
    return 0
}

check_python_license() {
    local file="$1"

    # Read the entire file content
    local file_content
    file_content=$(cat "$file" 2>/dev/null || echo "")

    # Check if the license header is present anywhere in the file
    if [[ "$file_content" == *"$PYTHON_LICENSE_HEADER"* ]]; then
        echo "✅ License header OK: $file"
        return 0
    fi

    # If not found, show error
    echo "❌ Missing or incorrect Apache 2.0 license header in: $file"
    echo "Expected header:"
    echo "$PYTHON_LICENSE_HEADER"
    echo ""
    echo "Found header:"
    head -14 "$file" 2>/dev/null || echo ""
    echo ""
    return 1
}

# Process each file passed as argument
for file in "$@"; do
    if [[ ! -f "$file" ]]; then
        continue
    fi

    case "$file" in
        *.rs)
            if ! check_rust_license "$file"; then
                EXIT_CODE=1
            fi
            ;;
        *.py)
            if ! check_python_license "$file"; then
                EXIT_CODE=1
            fi
            ;;
        *)
            echo "⚠️  Skipping unsupported file type: $file"
            ;;
    esac
done

if [[ $EXIT_CODE -eq 0 ]]; then
    echo "🎉 All files have proper license headers!"
else
    echo ""
    echo "💡 To fix missing license headers, add the appropriate Apache 2.0 license header to the beginning of each file."
    echo "   For Rust files (.rs): Use /* */ multi-line comment format"
    echo "   For Python files (.py): Use # single-line comment format"
fi

exit $EXIT_CODE
