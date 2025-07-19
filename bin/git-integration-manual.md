# Git Integration Manual

This document explains how to use **standard Git commands** with git-prolly repositories. While git-prolly provides KV-aware commands, it creates standard Git repositories that work seamlessly with all Git tools and workflows.

## 🎯 Purpose

git-prolly repositories are **standard Git repositories** with:
- Normal Git objects (commits, trees, blobs)
- Standard Git history and branching
- Full compatibility with Git remotes
- Support for all Git tools (CLI, GUI, web interfaces)

This manual shows how to leverage Git's full ecosystem alongside git-prolly's KV operations.

## 🚀 Prerequisites

```bash
# Install git-prolly
cargo install prollytree --features git

# Verify Git is installed
git --version
```

## 📋 Setup and Basic Integration

### 1. Initialize and Create Content
```bash
# Initialize a git-prolly repository
mkdir my-kv-store && cd my-kv-store
git-prolly init

# Add some KV data
git-prolly set user:123 "John Doe"
git-prolly set config:theme "dark"
git-prolly commit -m "Initial data"

# Verify it's a standard Git repository
ls -la
# Output: Shows .git directory - this is a normal Git repo!
```

## 🔧 Standard Git Commands

### 2. Git History and Logs
```bash
# Standard Git log commands work perfectly
git log --oneline
# Output:
# a1b2c3d4 Initial data
# f1e2d3c4 Initial commit

# More detailed log
git log --stat
# Output:
# commit a1b2c3d4e5f6...
# Author: git-prolly <git-prolly@example.com>
# Date: Mon Jan 15 10:30:00 2024 +0000
#
#     Initial data
#
#  prolly_tree_root | Bin 0 -> 1024 bytes
#  1 file changed, 0 insertions(+), 0 deletions(-)

# Pretty format
git log --pretty=format:"%h %an %ar %s"
# Output:
# a1b2c3d4 git-prolly 2 hours ago Initial data
# f1e2d3c4 git-prolly 2 hours ago Initial commit

# Show commit details
git show HEAD
# Output: Shows the full commit with file changes
```

### 3. Git Branching
```bash
# Create branches using standard Git
git branch feature/user-prefs
git checkout feature/user-prefs
# or
git checkout -b feature/notifications

# Make changes with git-prolly
git-prolly set pref:123:notifications "enabled"
git-prolly commit -m "Add notification preferences"

# Switch back to main
git checkout main

# List branches
git branch -v
# Output:
# * main                 a1b2c3d4 Initial data
#   feature/user-prefs   b2c3d4e5 Add notification preferences
#   feature/notifications c3d4e5f6 Add notification preferences

# Merge using standard Git
git merge feature/user-prefs
# Output: Standard Git merge output
```

### 4. Git Remotes and Collaboration
```bash
# Add remote repository
git remote add origin https://github.com/username/my-kv-store.git

# Push to remote
git push -u origin main
# Output: Standard Git push output

# Clone existing git-prolly repository
git clone https://github.com/username/my-kv-store.git
cd my-kv-store

# Verify it's a git-prolly repository
git-prolly list
# Output: Shows all keys from the repository

# Pull updates
git pull origin main
# Output: Standard Git pull output

# Push/pull works with all Git hosting services:
# GitHub, GitLab, Bitbucket, etc.
```

### 5. Git Status and Diffs
```bash
# Standard Git status
git status
# Output:
# On branch main
# nothing to commit, working tree clean

# After making changes with git-prolly
git-prolly set user:456 "Jane Smith"

# Git shows the file changes
git status
# Output:
# On branch main
# Changes not staged for commit:
#   modified: prolly_tree_root

# Standard Git diff
git diff
# Output: Shows binary diff of the ProllyTree data

# Compare branches
git diff main feature/user-prefs
# Output: Shows differences between branches
```

### 6. Git Tags
```bash
# Tag specific versions
git tag -a v1.0 -m "First stable version"
git tag -a v1.1 -m "Added user preferences"

# List tags
git tag -l
# Output:
# v1.0
# v1.1

# Show tag details
git show v1.0
# Output: Shows the tagged commit and its changes

# Push tags
git push origin --tags
```

### 7. Git Stash
```bash
# Make uncommitted changes
git-prolly set temp:data "temporary value"

# Stash changes
git stash push -m "Temporary work in progress"
# Output: Saved working directory and index state

# Work on something else
git checkout feature/other-work
# ... do work ...
git checkout main

# Restore stashed changes
git stash pop
# Output: Restored changes

# List stashes
git stash list
# Output: Shows all stashes
```

## 🌐 Working with Git Hosting Services

### 8. GitHub Integration
```bash
# GitHub workflow
git clone https://github.com/username/my-kv-store.git
cd my-kv-store

# Create feature branch
git checkout -b feature/add-users

# Make changes
git-prolly set user:789 "Bob Wilson"
git-prolly set user:101 "Alice Johnson"
git-prolly commit -m "Add new users"

# Push feature branch
git push origin feature/add-users

# Create pull request on GitHub web interface
# Merge via GitHub UI
# Pull merged changes
git checkout main
git pull origin main
```

### 9. GitLab, Bitbucket, etc.
```bash
# Works with any Git hosting service
git remote add gitlab https://gitlab.com/username/my-kv-store.git
git push gitlab main

# Azure DevOps
git remote add azure https://dev.azure.com/org/project/_git/my-kv-store
git push azure main

# Self-hosted Git
git remote add company https://git.company.com/team/my-kv-store.git
git push company main
```

## 🛠️ Git Tools and GUI Integration

### 10. Git GUI Tools
```bash
# All Git GUI tools work with git-prolly repositories:

# GitKraken
# - Open repository: File → Open Repo
# - All commits, branches, and merges visible

# SourceTree
# - Clone/open repository normally
# - Full history and branch visualization

# VS Code Git extension
# - Open folder in VS Code
# - Git tab shows all standard Git operations

# GitHub Desktop
# - Clone repository from GitHub
# - Standard commit/push/pull workflow
```

### 11. Git Hooks
```bash
# Standard Git hooks work normally
cd .git/hooks

# Pre-commit hook example
cat > pre-commit << 'EOF'
#!/bin/bash
# Validate KV data before commit
if git-prolly list | grep -q "test:"; then
    echo "Error: Test keys found in production commit"
    exit 1
fi
EOF

chmod +x pre-commit

# Post-commit hook example
cat > post-commit << 'EOF'
#!/bin/bash
# Log KV statistics after each commit
echo "$(date): $(git-prolly stats --brief)" >> kv-stats.log
EOF

chmod +x post-commit
```

### 12. Git Aliases
```bash
# Add convenient aliases
git config --global alias.kv-log "log --oneline --decorate"
git config --global alias.kv-status "status --porcelain"
git config --global alias.kv-diff "diff --stat"

# Use aliases
git kv-log
git kv-status
git kv-diff
```

## 🔍 Advanced Git Integration

### 13. Git Bisect
```bash
# Find when a key was introduced/changed
git bisect start
git bisect bad HEAD
git bisect good v1.0

# Git will checkout commits for testing
# At each commit, check the KV state
git-prolly get problematic:key

# Tell git if the commit is good or bad
git bisect good  # or git bisect bad

# Git finds the exact commit that introduced the issue
```

### 14. Git Rebase
```bash
# Clean up history before merging
git checkout feature/user-management
git rebase main

# Interactive rebase to clean up commits
git rebase -i HEAD~3
# Edit commit messages, squash commits, etc.

# Force push after rebase (if needed)
git push --force-with-lease origin feature/user-management
```

### 15. Git Submodules
```bash
# Include git-prolly repositories as submodules
git submodule add https://github.com/team/shared-config.git config
git submodule add https://github.com/team/user-data.git users

# Update submodules
git submodule update --remote

# Each submodule is a full git-prolly repository
cd config
git-prolly list
git-prolly set shared:setting "value"
git-prolly commit -m "Update shared configuration"
```

## 📊 Git Analytics and Reporting

### 16. Git Statistics
```bash
# Standard Git statistics work
git shortlog -sn
# Output: Shows commit counts by author

# Git log with custom format for KV analysis
git log --pretty=format:"%h %s" | grep -E "(add|update|delete)"
# Output: Shows KV-related commits

# Combine with git-prolly for detailed analysis
git log --oneline | while read commit msg; do
    echo "Commit $commit: $(git-prolly stats $commit --brief)"
done
```

### 17. Git Workflows
```bash
# GitFlow workflow
git flow init
git flow feature start user-profiles
git-prolly set user:template "default template"
git-prolly commit -m "Add user profile template"
git flow feature finish user-profiles

# GitHub Flow
git checkout -b feature/notifications
git-prolly set notif:default "enabled"
git-prolly commit -m "Add notification system"
git push origin feature/notifications
# Create pull request on GitHub
```

## 🎯 Best Practices

### 18. Combining git-prolly and Git
```bash
# Use git-prolly for KV operations
git-prolly set user:123 "John Doe"
git-prolly commit -m "Add user"

# Use Git for repository operations
git branch feature/enhancement
git checkout feature/enhancement
git merge main
git push origin feature/enhancement

# Use Git for collaboration
git pull origin main
git push origin main

# Use both for comprehensive workflows
git checkout -b feature/new-data
git-prolly set data:new "value"
git-prolly commit -m "Add new data"
git push origin feature/new-data
# Create pull request
# After merge:
git checkout main
git pull origin main
```

### 19. Repository Structure
```
my-kv-store/
├── .git/              # Standard Git repository
├── .gitignore         # Git ignore rules
├── README.md          # Project documentation
├── prolly_tree_root   # ProllyTree data (managed by git-prolly)
└── scripts/           # Utility scripts
    ├── backup.sh
    └── migrate.sh
```

## 🎉 Benefits of Git Integration

### For Developers
- **Familiar Tools**: Use existing Git knowledge and tools
- **IDE Integration**: Full support in VS Code, IntelliJ, etc.
- **Workflow Integration**: Works with existing Git workflows
- **Collaboration**: Standard pull requests, code reviews

### For Operations
- **Hosting**: Use any Git hosting service (GitHub, GitLab, etc.)
- **Backup**: Standard Git backup and replication
- **Monitoring**: Git-based monitoring and alerting
- **Compliance**: Audit trails through Git history

### For Organizations
- **No Vendor Lock-in**: Standard Git format
- **Existing Infrastructure**: Leverage current Git infrastructure
- **Training**: No new tools to learn
- **Integration**: Works with existing CI/CD pipelines

## 🔧 Troubleshooting

### Common Issues
```bash
# If git-prolly commands don't work in a cloned repo
git-prolly --help
# Make sure git-prolly is installed

# If Git operations seem slow
git gc
# Run Git garbage collection

# If merge conflicts occur
git status
# Shows conflicted files
# Resolve manually, then:
git add .
git commit
```

## 📚 Further Reading

- [Git Documentation](https://git-scm.com/doc)
- [Pro Git Book](https://git-scm.com/book)
- [GitHub Guides](https://guides.github.com)
- [git-prolly User Manual](./git-prolly-manual.md)

## 🎯 Summary

git-prolly creates **standard Git repositories** that work seamlessly with:
- All Git commands and tools
- Any Git hosting service
- Existing Git workflows
- Git GUI applications
- Git hooks and automation
- CI/CD pipelines

The key insight is that git-prolly enhances Git with KV-aware operations while maintaining full Git compatibility. You get the best of both worlds: powerful versioned key-value operations and the entire Git ecosystem.