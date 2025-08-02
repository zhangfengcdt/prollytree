#!/bin/bash

# Script to build ProllyTree and run the LangGraph example

echo "🔧 Building ProllyTree Python bindings..."
echo "This may take a few minutes on first build..."

# Change to project root
cd ../..

# Build the Python bindings
if ./python/build_python.sh --install; then
    echo "✅ ProllyTree built successfully!"

    # Change back to examples directory
    cd python/examples

    # Check if OPENAI_API_KEY is set
    if [ -z "$OPENAI_API_KEY" ]; then
        echo "⚠️  Warning: OPENAI_API_KEY is not set."
        echo "   The example will use mock LLM responses."
        echo "   To use real OpenAI, run: export OPENAI_API_KEY='your-key'"
    fi

    echo ""
    echo "🚀 Running ProllyTree memory demo..."
    python langgraph_memory_example.py
else
    echo "❌ Build failed. Please check the error messages above."
    echo ""
    echo "Common issues:"
    echo "1. Make sure you have Rust installed: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "2. Install maturin: pip install maturin"
    echo "3. Make sure you're using Python 3.8 or higher"
    exit 1
fi
