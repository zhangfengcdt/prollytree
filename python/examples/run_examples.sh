#!/bin/bash

# Script to build ProllyTree and run Python examples
# Usage: ./run_examples.sh [example_name]
# If no example name is provided, all examples will be run

# Define available examples using a case statement instead of associative array
# This is more portable across different shells
get_example_file() {
    case "$1" in
        "basic") echo "basic_usage.py" ;;
        "sql") echo "sql_example.py" ;;
        "langgraph") echo "langgraph_example.py" ;;
        "chronological") echo "langgraph_chronological.py" ;;
        "merge") echo "merge_example.py" ;;
        *) echo "" ;;
    esac
}

# Check if a specific example was requested
REQUESTED_EXAMPLE=$1

# Function to show usage
show_usage() {
    echo "Usage: $0 [example_name]"
    echo ""
    echo "Available examples:"
    echo "  basic         - Basic memory usage example"
    echo "  sql           - SQL query example"
    echo "  langgraph     - LangGraph memory example"
    echo "  chronological - LangGraph chronological memory example"
    echo "  merge         - Branch merging with conflict resolution example"
    echo ""
    echo "If no example name is provided, all examples will be run."
}

# Validate requested example if provided
if [ ! -z "$REQUESTED_EXAMPLE" ]; then
    if [ "$REQUESTED_EXAMPLE" == "--help" ] || [ "$REQUESTED_EXAMPLE" == "-h" ]; then
        show_usage
        exit 0
    fi

    EXAMPLE_FILE=$(get_example_file "$REQUESTED_EXAMPLE")
    if [ -z "$EXAMPLE_FILE" ]; then
        echo "❌ Error: Unknown example '$REQUESTED_EXAMPLE'"
        echo ""
        show_usage
        exit 1
    fi
fi

if [ -z "$REQUESTED_EXAMPLE" ]; then
    echo "🔧 Building ProllyTree Python bindings for all examples..."
else
    echo "🔧 Building ProllyTree Python bindings for '$REQUESTED_EXAMPLE' example..."
fi
echo "This may take a few minutes on first build..."

# Change to project root
cd ../..

# Build the Python bindings
if ./python/build_python.sh --all-features --install; then
    echo "✅ ProllyTree built successfully!"

    # Change back to examples directory
    cd python/examples

    # Check if OPENAI_API_KEY is set
    if [ -z "$OPENAI_API_KEY" ]; then
        echo "⚠️  Warning: OPENAI_API_KEY is not set."
        echo "   The example will use mock LLM responses."
        echo "   To use real OpenAI, run: export OPENAI_API_KEY='your-key'"
    fi

    # Function to run a single example
    run_example() {
        local name=$1
        local file=$2
        echo ""
        echo "🚀 Running $name example..."
        python "$file"
    }

    # Run examples based on request
    if [ -z "$REQUESTED_EXAMPLE" ]; then
        # Run all examples
        echo ""
        echo "📚 Running all examples..."
        run_example "basic memory usage" "basic_usage.py"
        run_example "SQL" "sql_example.py"
        run_example "LangGraph memory" "langgraph_example.py"
        run_example "LangGraph chronological memory" "langgraph_chronological.py"
        run_example "merge with conflict resolution" "merge_example.py"
    else
        # Run only the requested example
        EXAMPLE_FILE=$(get_example_file "$REQUESTED_EXAMPLE")
        run_example "$REQUESTED_EXAMPLE" "$EXAMPLE_FILE"
    fi
else
    echo "❌ Build failed. Please check the error messages above."
    echo ""
    echo "Common issues:"
    echo "1. Make sure you have Rust installed: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "2. Install maturin: pip install maturin"
    echo "3. Make sure you're using Python 3.8 or higher"
    exit 1
fi
