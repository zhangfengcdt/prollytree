# Git-Integrated ProllyTree

This document demonstrates the complete end-user experience for using ProllyTree with Git integration.

## 🚀 Installation

### Option 1: Install from crates.io (Recommended)
```bash
cargo install prollytree --features git
```

### Option 2: Build from source
```bash
git clone https://github.com/username/prollytree.git
cd prollytree
cargo build --release --features git
sudo cp target/release/git-prolly /usr/local/bin/
```

### Verification
```bash
git prolly --help
# Should show: KV-aware Git operations for ProllyTree
```

## 📋 Complete Usage Examples

### 1. Initialize a New KV Store
```bash
# Create a new directory for your KV store
mkdir my-kv-store
cd my-kv-store

# Initialize git-backed KV store
git prolly init
# Output:
# Initializing ProllyTree KV store in "."...
# ✓ Initialized empty ProllyTree KV store
# ✓ Git repository initialized
# ✓ Ready to use!
```

### 2. Basic Key-Value Operations
```bash
# Set some key-value pairs
git prolly set user:123 "John Doe"
# Output: ✓ Staged: user:123 = "John Doe"
#         (Use 'git prolly commit' to save changes)

git prolly set user:456 "Jane Smith"
git prolly set config:theme "dark"
git prolly set config:language "en"

# Get a value
git prolly get user:123
# Output: John Doe

# List all keys
git prolly list
# Output:
# config:language
# config:theme
# user:123
# user:456

# List keys with values
git prolly list --values
# Output:
# config:language = "en"
# config:theme = "dark"
# user:123 = "John Doe"
# user:456 = "Jane Smith"
```

### 3. Staging and Committing
```bash
# Check staging status
git prolly status
# Output:
# Staged changes:
#   added: config:language
#   added: config:theme
#   added: user:123
#   added: user:456

# Commit changes
git prolly commit -m "Initial user data and configuration"
# Output:
# ✓ Committed: a1b2c3d4e5f6...
#   Message: Initial user data and configuration
#   Changes: 4 operations
#     + config:language
#     + config:theme
#     + user:123
#     + user:456
```

### 4. Modifying Data
```bash
# Update existing values
git prolly set user:123 "John A. Doe"
git prolly set config:theme "light"

# Add new data
git prolly set user:789 "Bob Wilson"

# Delete data
git prolly delete config:language

# Check status
git prolly status
# Output:
# Staged changes:
#   modified: user:123
#   modified: config:theme
#   added: user:789
#   deleted: config:language

# Commit changes
git prolly commit -m "Update user names and theme, add new user"
```

### 5. Branching and Merging
```bash
# Create a new branch
git prolly branch feature/preferences
git prolly checkout feature/preferences

# Make changes on feature branch
git prolly set pref:123:notifications "enabled"
git prolly set pref:123:theme "auto"
git prolly commit -m "Add user preference system"

# Switch back to main
git prolly checkout main

# Make different changes on main
git prolly set user:999 "Alice Green"
git prolly commit -m "Add user 999"

# Merge feature branch
git prolly merge feature/preferences
# Output:
# Merging branch 'feature/preferences'...
# ✓ Three-way merge completed
#   Merge commit: f1e2d3c4b5a6...
```

### 6. Viewing History and Diffs
```bash
# View commit history
git prolly log
# Output:
# f1e2d3c4 - 2024-01-15 10:30:00 - Merge branch 'feature/preferences'
# b5a6c7d8 - 2024-01-15 10:25:00 - Add user 999
# e9f0a1b2 - 2024-01-15 10:20:00 - Add user preference system
# c3d4e5f6 - 2024-01-15 10:15:00 - Update user names and theme, add new user
# a1b2c3d4 - 2024-01-15 10:10:00 - Initial user data and configuration

# View history with change summary
git prolly log --kv-summary
# Output:
# f1e2d3c4 - 2024-01-15 10:30:00 - Merge branch 'feature/preferences' (+0 ~0 -0)
# b5a6c7d8 - 2024-01-15 10:25:00 - Add user 999 (+1 ~0 -0)
# e9f0a1b2 - 2024-01-15 10:20:00 - Add user preference system (+2 ~0 -0)
# c3d4e5f6 - 2024-01-15 10:15:00 - Update user names and theme, add new user (+1 ~2 -1)
# a1b2c3d4 - 2024-01-15 10:10:00 - Initial user data and configuration (+4 ~0 -0)

# Diff between commits
git prolly diff a1b2c3d4 c3d4e5f6
# Output:
# Key-Value Changes (a1b2c3d4 -> c3d4e5f6):
#   + user:789 = "Bob Wilson"
#   ~ user:123 = "John Doe" -> "John A. Doe"
#   ~ config:theme = "dark" -> "light"
#   - config:language = "en"

# Detailed diff
git prolly diff a1b2c3d4 c3d4e5f6 --format=detailed
# Output:
# Detailed Key-Value Changes (a1b2c3d4 -> c3d4e5f6):
# ═══════════════════════════════════════
# 
# Key: user:789
#   Status: Added
#   Value: "Bob Wilson"
# 
# Key: user:123
#   Status: Modified
#   Old Value: "John Doe"
#   New Value: "John A. Doe"
# 
# Key: config:theme
#   Status: Modified
#   Old Value: "dark"
#   New Value: "light"
# 
# Key: config:language
#   Status: Removed
#   Previous Value: "en"
```

### 7. Show Specific Commits
```bash
# Show a specific commit
git prolly show c3d4e5f6
# Output:
# Commit: c3d4e5f6 - Update user names and theme, add new user
# Author: Developer
# Date: 2024-01-15 10:15:00
# 
# Key-Value Changes:
#   + user:789 = "Bob Wilson"
#   ~ user:123 = "John Doe" -> "John A. Doe"
#   ~ config:theme = "dark" -> "light"
#   - config:language = "en"

# Show only keys
git prolly show HEAD --keys-only
# Output:
# Keys at commit HEAD:
#   config:theme
#   pref:123:notifications
#   pref:123:theme
#   user:123
#   user:456
#   user:789
#   user:999
```

### 8. Repository Statistics
```bash
# Show repository stats
git prolly stats
# Output:
# ProllyTree Statistics for HEAD:
# ═══════════════════════════════════
# Total Keys: 7
# Current Branch: main
# Total Commits: 5
# Latest Commit: 2024-01-15 10:30:00

# Stats for specific commit
git prolly stats c3d4e5f6
# Output:
# ProllyTree Statistics for c3d4e5f6:
# ═══════════════════════════════════
# Total Keys: 5
# Current Branch: main
# Total Commits: 3
# Latest Commit: 2024-01-15 10:15:00
```

### 9. Reverting Changes
```bash
# Revert a specific commit
git prolly revert c3d4e5f6
# Output:
# ✓ Reverted commit: c3d4e5f6

# This creates a new commit that undoes the changes
git prolly log --kv-summary | head -2
# Output:
# g7h8i9j0 - 2024-01-15 10:35:00 - Revert "Update user names and theme, add new user" (+1 ~2 -1)
# f1e2d3c4 - 2024-01-15 10:30:00 - Merge branch 'feature/preferences' (+0 ~0 -0)
```

### 10. Working with Standard Git
```bash
# All standard Git commands work alongside git-prolly
git log --oneline
# Output:
# g7h8i9j0 Revert "Update user names and theme, add new user"
# f1e2d3c4 Merge branch 'feature/preferences'
# b5a6c7d8 Add user 999
# e9f0a1b2 Add user preference system
# c3d4e5f6 Update user names and theme, add new user
# a1b2c3d4 Initial user data and configuration

git branch -a
# Output:
# * main
#   feature/preferences

git remote add origin https://github.com/username/my-kv-store.git
git push -u origin main
# Output:
# Counting objects: 12, done.
# Delta compression using up to 8 threads.
# Compressing objects: 100% (8/8), done.
# Writing objects: 100% (12/12), 1.89 KiB | 0 bytes/s, done.
# Total 12 (delta 2), reused 0 (delta 0)
# To https://github.com/username/my-kv-store.git
#  * [new branch]      main -> main
```

## 🎯 Key Benefits

### For Developers
- **Familiar workflow**: Uses standard Git commands with KV-aware enhancements
- **Full version control**: Complete history of all key-value changes
- **Branching and merging**: Parallel development with conflict resolution
- **Remote collaboration**: Push/pull/clone like any Git repository

### For Operations
- **Audit trail**: Every change is tracked with author, timestamp, and message
- **Rollback capability**: Revert any change or set of changes
- **Conflict resolution**: Automatic and manual merge conflict handling
- **Backup and sync**: Standard Git remote operations

### Technical Advantages
- **Efficient storage**: ProllyTree's probabilistic chunking reduces storage overhead
- **Content addressing**: Hash-based verification ensures data integrity
- **Scalable**: Handles large datasets with efficient tree operations
- **Interoperable**: Works with existing Git tools and workflows

## 🔧 Advanced Features

### JSON Output for Automation
```bash
git prolly diff main feature/preferences --format=json
# Output:
# {
#   "from": "main",
#   "to": "feature/preferences",
#   "changes": [
#     {
#       "key": "pref:123:notifications",
#       "operation": "added",
#       "value": "enabled"
#     },
#     {
#       "key": "pref:123:theme",
#       "operation": "added",
#       "value": "auto"
#     }
#   ]
# }
```

### Key Pattern Filtering
```bash
# Filter by key pattern
git prolly diff main feature/preferences --keys="user:*"
# Output:
# Key-Value Changes (user:* keys only):
# (no changes to user:* keys)

git prolly list --values | grep "^user:"
# Output:
# user:123 = "John A. Doe"
# user:456 = "Jane Smith"
# user:789 = "Bob Wilson"
# user:999 = "Alice Green"
```

This demonstrates a complete git-integrated versioned key-value store that provides developers with familiar Git workflows while offering powerful KV-specific operations and insights.