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

# Script to run ProllyTree benchmarks

echo "🚀 Running ProllyTree Benchmarks"
echo "================================"

echo ""
echo "📊 1. Running Core Tree Benchmarks..."
cargo bench --bench prollytree_bench --quiet -- --quick

echo ""
echo "📊 2. Running SQL Benchmarks..."
cargo bench --bench sql_bench --features sql --quiet -- --quick

echo ""
echo "📊 3. Running Git-Prolly Integration Benchmarks..."
cargo bench --bench git_prolly_bench --features "git sql" --quiet -- --quick

echo ""
echo "✅ All benchmarks completed!"
echo ""
echo "📈 To view detailed results, run:"
echo "   cargo bench --bench <benchmark_name>"
echo ""
echo "📊 Available benchmarks:"
echo "   - prollytree_bench: Core tree operations"
echo "   - sql_bench: SQL operations (requires --features sql)"
echo "   - git_prolly_bench: Git integration (requires --features git,sql)"
