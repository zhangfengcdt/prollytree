# git-prolly User Manual

A git-integrated versioned key-value store built on ProllyTree.

## Overview

`git-prolly` is a command-line tool that provides a versioned key-value store with full Git integration. It combines the efficient operations of ProllyTree with Git's proven version control capabilities, allowing you to store, version, and collaborate on key-value data using familiar Git workflows.

## Installation

### From crates.io (Recommended)
```bash
cargo install prollytree --features git
```

### From source
```bash
git clone https://github.com/zhangfengcdt/prollytree.git
cd prollytree
cargo build --release --features git
sudo cp target/release/git-prolly /usr/local/bin/
```

### Verification
```bash
git-prolly --help
# Should show: KV-aware Git operations for ProllyTree
```

## Quick Start

```bash
# 1. Initialize a new KV store
mkdir my-kv-store && cd my-kv-store
git-prolly init

# 2. Add some data
git-prolly set user:123 "John Doe"
git-prolly set config:theme "dark"

# 3. Check status
git-prolly status

# 4. Commit changes
git-prolly commit -m "Initial data"

# 5. View history
git-prolly log
```

## Commands Reference

### Repository Management

#### `git-prolly init`
Initialize a new git-prolly repository in the current directory.

**Usage:**
```bash
git-prolly init
```

**Example:**
```bash
mkdir my-project && cd my-project
git-prolly init
# Output: ✓ Initialized empty ProllyTree KV store
#         ✓ Git repository initialized
```

### Key-Value Operations

#### `git-prolly set <key> <value>`
Set a key-value pair (stages the change).

**Usage:**
```bash
git-prolly set <key> <value>
```

**Examples:**
```bash
git-prolly set user:123 "John Doe"
git-prolly set config:theme "dark"
git-prolly set "complex key" "value with spaces"
```

#### `git-prolly get <key>`
Retrieve a value by key.

**Usage:**
```bash
git-prolly get <key>
```

**Examples:**
```bash
git-prolly get user:123
# Output: John Doe

git-prolly get nonexistent:key
# Output: Key not found
```

#### `git-prolly delete <key>`
Delete a key-value pair (stages the change).

**Usage:**
```bash
git-prolly delete <key>
```

**Examples:**
```bash
git-prolly delete user:123
git-prolly delete config:theme
```

#### `git-prolly list [--values]`
List all keys, optionally with their values.

**Usage:**
```bash
git-prolly list [--values]
```

**Examples:**
```bash
# List just keys
git-prolly list
# Output: config:theme
#         user:123
#         user:456

# List keys with values
git-prolly list --values
# Output: config:theme = "dark"
#         user:123 = "John Doe"
#         user:456 = "Jane Smith"
```

### Version Control

#### `git-prolly status`
Show the current status of staged changes.

**Usage:**
```bash
git-prolly status
```

**Example:**
```bash
git-prolly status
# Output: Staged changes:
#           added: config:theme
#           modified: user:123
#           deleted: user:456
```

#### `git-prolly commit -m <message>`
Commit staged changes with a message.

**Usage:**
```bash
git-prolly commit -m "<message>"
```

**Examples:**
```bash
git-prolly commit -m "Add initial user configuration"
git-prolly commit -m "Update theme settings"
```

#### `git-prolly log [--kv-summary]`
Show commit history.

**Usage:**
```bash
git-prolly log [--kv-summary]
```

**Examples:**
```bash
# Basic log
git-prolly log
# Output: f1e2d3c4 - 2024-01-15 10:30:00 - Add initial user configuration
#         a1b2c3d4 - 2024-01-15 10:25:00 - Initial commit

# Log with key-value change summary
git-prolly log --kv-summary
# Output: f1e2d3c4 - 2024-01-15 10:30:00 - Add initial user configuration (+2 ~1 -0)
#         a1b2c3d4 - 2024-01-15 10:25:00 - Initial commit (+4 ~0 -0)
```

### Branching and Merging

#### `git-prolly branch <branch-name>`
Create a new branch.

**Usage:**
```bash
git-prolly branch <branch-name>
```

**Examples:**
```bash
git-prolly branch feature/user-preferences
git-prolly branch hotfix/theme-bug
```

#### `git-prolly checkout <branch-name>`
Switch to a different branch.

**Usage:**
```bash
git-prolly checkout <branch-name>
```

**Examples:**
```bash
git-prolly checkout feature/user-preferences
git-prolly checkout main
```

#### `git-prolly merge <branch-name>`
Merge a branch into the current branch.

**Usage:**
```bash
git-prolly merge <branch-name>
```

**Examples:**
```bash
git-prolly merge feature/user-preferences
# Output: Merging branch 'feature/user-preferences'...
#         ✓ Three-way merge completed
#         Merge commit: f1e2d3c4b5a6...
```

### Diff and History

#### `git-prolly diff <from> <to>`
Show differences between two commits or branches.

**Usage:**
```bash
git-prolly diff <from> <to> [--format=<format>]
```

**Options:**
- `--format=detailed`: Show detailed diff information
- `--format=json`: Output in JSON format
- `--keys=<pattern>`: Filter by key pattern

**Examples:**
```bash
# Basic diff
git-prolly diff main feature/preferences
# Output: Key-Value Changes (main -> feature/preferences):
#           + pref:123:notifications = "enabled"
#           ~ user:123 = "John Doe" -> "John A. Doe"
#           - config:language = "en"

# Detailed diff
git-prolly diff main feature/preferences --format=detailed
# Output: Detailed Key-Value Changes (main -> feature/preferences):
#         ═══════════════════════════════════════
#         
#         Key: pref:123:notifications
#           Status: Added
#           Value: "enabled"
#         
#         Key: user:123
#           Status: Modified
#           Old Value: "John Doe"
#           New Value: "John A. Doe"

# JSON output
git-prolly diff main feature/preferences --format=json
# Output: {
#           "from": "main",
#           "to": "feature/preferences",
#           "changes": [
#             {
#               "key": "pref:123:notifications",
#               "operation": "added",
#               "value": "enabled"
#             }
#           ]
#         }
```

#### `git-prolly show <commit> [--keys-only]`
Show detailed information about a specific commit.

**Usage:**
```bash
git-prolly show <commit> [--keys-only]
```

**Examples:**
```bash
# Show commit details
git-prolly show HEAD
# Output: Commit: f1e2d3c4 - Add user preferences
#         Author: Developer
#         Date: 2024-01-15 10:30:00
#         
#         Key-Value Changes:
#           + pref:123:notifications = "enabled"
#           ~ user:123 = "John Doe" -> "John A. Doe"

# Show only keys
git-prolly show HEAD --keys-only
# Output: Keys at commit HEAD:
#           config:theme
#           user:123
#           user:456
```

### Advanced Operations

#### `git-prolly revert <commit>`
Revert changes from a specific commit.

**Usage:**
```bash
git-prolly revert <commit>
```

**Examples:**
```bash
git-prolly revert f1e2d3c4
# Output: ✓ Reverted commit: f1e2d3c4
#         Created revert commit: g7h8i9j0
```

#### `git-prolly stats [<commit>]`
Show repository statistics.

**Usage:**
```bash
git-prolly stats [<commit>]
```

**Examples:**
```bash
# Current stats
git-prolly stats
# Output: ProllyTree Statistics for HEAD:
#         ═══════════════════════════════════
#         Total Keys: 7
#         Current Branch: main
#         Total Commits: 5
#         Latest Commit: 2024-01-15 10:30:00

# Stats for specific commit
git-prolly stats c3d4e5f6
# Output: ProllyTree Statistics for c3d4e5f6:
#         ═══════════════════════════════════
#         Total Keys: 5
#         Current Branch: main
#         Total Commits: 3
#         Latest Commit: 2024-01-15 10:15:00
```

## Workflows

### Basic Workflow

1. **Initialize**: Create a new repository
2. **Add Data**: Set key-value pairs
3. **Stage**: Changes are automatically staged
4. **Commit**: Save changes with a message
5. **Repeat**: Continue adding and committing

```bash
git-prolly init
git-prolly set user:123 "John Doe"
git-prolly set config:theme "dark"
git-prolly status
git-prolly commit -m "Initial setup"
```

### Branching Workflow

1. **Create Branch**: For new features
2. **Switch**: Work on the branch
3. **Develop**: Make changes
4. **Merge**: Integrate back to main

```bash
git-prolly branch feature/new-users
git-prolly checkout feature/new-users
git-prolly set user:456 "Jane Smith"
git-prolly commit -m "Add new user"
git-prolly checkout main
git-prolly merge feature/new-users
```

### Collaboration Workflow

Since git-prolly uses standard Git underneath, you can use normal Git commands for remote operations:

```bash
# Set up remote
git remote add origin https://github.com/username/my-kv-store.git

# Push changes
git push -u origin main

# Pull changes
git pull origin main

# Clone existing repository
git clone https://github.com/username/my-kv-store.git
cd my-kv-store
git-prolly status  # Works with existing git-prolly repositories
```

## Key Features

### Git Integration
- **Standard Git**: Works with existing Git tools and workflows
- **Remote Sync**: Push/pull/clone like any Git repository
- **Branching**: Full branching and merging support
- **History**: Complete audit trail of all changes

### Efficient Storage
- **ProllyTree**: Probabilistic tree structure for efficient operations
- **Content Addressing**: Hash-based verification ensures data integrity
- **Incremental**: Only stores changes, not full snapshots

### Developer Friendly
- **Familiar Commands**: Git-like interface for easy adoption
- **JSON Output**: Machine-readable output for automation
- **Pattern Matching**: Filter operations by key patterns

## Best Practices

### Key Naming
- Use namespaces: `user:123`, `config:theme`, `cache:session:abc`
- Be consistent: Use the same delimiter throughout
- Avoid special characters: Stick to alphanumeric and common symbols

### Commit Messages
- Be descriptive: "Add user preferences system"
- Use present tense: "Add" not "Added"
- Reference context: "Fix theme loading for mobile users"

### Branching Strategy
- **main**: Stable, production-ready data
- **feature/***: New features or major changes
- **hotfix/***: Critical fixes
- **dev**: Development integration

### Data Organization
```bash
# Good: Organized by domain
git-prolly set user:123:name "John Doe"
git-prolly set user:123:email "john@example.com"
git-prolly set config:app:theme "dark"
git-prolly set config:app:language "en"

# Avoid: Flat structure
git-prolly set john_name "John Doe"
git-prolly set john_email "john@example.com"
```

## Troubleshooting

### Common Issues

#### "Repository not found"
```bash
# Make sure you're in a git-prolly repository
git-prolly init

# Or check if you're in the right directory
ls -la  # Should show .git folder
```

#### "Key not found"
```bash
# Check if key exists
git-prolly list | grep "mykey"

# Check staged changes
git-prolly status
```

#### "Merge conflicts"
```bash
# View conflicting changes
git-prolly status

# Resolve manually by setting new values
git-prolly set conflicting:key "resolved value"
git-prolly commit -m "Resolve merge conflict"
```

### Performance Tips

- **Batch operations**: Group related changes in single commits
- **Regular commits**: Don't let staging area grow too large
- **Prune old data**: Use `git-prolly delete` for unused keys

## Integration with Standard Git

git-prolly repositories are standard Git repositories. You can:

- Use `git log` to see commit history
- Use `git branch` to manage branches
- Use `git remote` for remote repositories
- Use `git diff` to see file-level changes
- Use any Git GUI tool

## Examples

### Configuration Management
```bash
# Application settings
git-prolly set app:version "2.1.0"
git-prolly set app:debug "false"
git-prolly set app:port "8080"

# Database configuration
git-prolly set db:host "localhost"
git-prolly set db:port "5432"
git-prolly set db:name "myapp"

git-prolly commit -m "Update application configuration"
```

### User Management
```bash
# User profiles
git-prolly set user:123:name "John Doe"
git-prolly set user:123:role "admin"
git-prolly set user:456:name "Jane Smith"
git-prolly set user:456:role "user"

# Permissions
git-prolly set perm:admin:read "true"
git-prolly set perm:admin:write "true"
git-prolly set perm:user:read "true"
git-prolly set perm:user:write "false"

git-prolly commit -m "Set up user system"
```

### Feature Flags
```bash
# Feature toggles
git-prolly set feature:new_ui "true"
git-prolly set feature:beta_search "false"
git-prolly set feature:mobile_app "true"

# Environment-specific
git-prolly set env:prod:debug "false"
git-prolly set env:staging:debug "true"

git-prolly commit -m "Update feature flags"
```

## Support

For issues, questions, or contributions:
- GitHub: https://github.com/zhangfengcdt/prollytree
- Documentation: https://docs.rs/prollytree
- Issues: https://github.com/zhangfengcdt/prollytree/issues

## License

Licensed under the Apache License, Version 2.0.