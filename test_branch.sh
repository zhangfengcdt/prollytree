#!/bin/bash

# Test script for the new -b option
echo "Testing git-prolly checkout -b option..."

# Build the project
echo "Building project..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

# Create a test directory
TEST_DIR="/tmp/test_prolly_branch"
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR"
cd "$TEST_DIR"

# Initialize git repo
git init
echo "test content" > test.txt
git add test.txt
git commit -m "Initial commit"

# Create a subdirectory for prolly
mkdir dataset
cd dataset

# Initialize git-prolly
echo "Initializing git-prolly..."
/Users/feng/github/prollytree/target/release/git-prolly init

# Test the checkout -b functionality
echo "Testing checkout -b..."
/Users/feng/github/prollytree/target/release/git-prolly checkout -b test-branch

# Check git branch
echo "Git branches:"
git branch

# Check prolly branch
echo "Prolly branch:"
/Users/feng/github/prollytree/target/release/git-prolly branch

# Switch back to main
echo "Switching back to main..."
/Users/feng/github/prollytree/target/release/git-prolly checkout main

# Check git branch again
echo "Git branches after switching back:"
git branch

echo "Test completed!"