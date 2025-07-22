# Design of Development Workflows using Prollytree and Git

A comprehensive guide to integrating git-prolly into your development workflows, covering both separate repository and monorepo approaches.

## Table of Contents

1. [Overview](#overview)
2. [Repository Architecture Patterns](#repository-architecture-patterns)
3. [Separate Repository Workflow](#separate-repository-workflow)
4. [Monorepo Workflow](#monorepo-workflow)
5. [Cross-Branch Data Testing](#cross-branch-data-testing)
6. [Advanced Debugging Techniques](#advanced-debugging-techniques)
7. [Best Practices](#best-practices)
8. [Common Scenarios](#common-scenarios)

## Overview

git-prolly enables versioned key-value storage with full Git integration, allowing you to version your data alongside your code. This manual covers recommended workflows for different development scenarios.

### Key Benefits
- **Version Control**: Full history tracking for both code and data
- **Branching**: Separate data states for different features/environments
- **Collaboration**: Standard Git workflows for team development
- **Debugging**: Test code against different data states
- **Deployment**: Coordinate code and data deployments

## Repository Architecture Patterns

### Pattern 1: Separate Repositories
```
myapp/          (Main application repository)
├── .git/
├── src/
├── Cargo.toml
└── data/       (Git submodule → kv-data repo)

kv-data/        (Separate KV data repository)
├── .git/
├── prolly_tree_root
└── README.md
```

### Pattern 2: Monorepo (Single Repository)
```
myapp/          (Single repository)
├── .git/
├── src/        (Application code)
├── config/     (KV data store)
│   └── prolly_tree_root
├── user-data/  (Another KV store)
│   └── prolly_tree_root
└── Cargo.toml
```

## Separate Repository Workflow

### Setup

#### 1. Create KV Data Repository
```bash
# Create and initialize KV data repository
git clone --bare https://github.com/myteam/kv-data.git
cd kv-data
git-prolly init
git-prolly set config:app:name "MyApp"
git-prolly set config:app:version "1.0.0"
git-prolly commit -m "Initial configuration"
git push origin main
```

#### 2. Add KV Data as Submodule
```bash
# In your main application repository
git submodule add https://github.com/myteam/kv-data.git data
git commit -m "Add KV data submodule"
```

### Development Workflow

#### Feature Development
```bash
# Start new feature
git checkout -b feature/new-ui

# Update KV data for this feature
cd data
git checkout -b feature/new-ui-config
git-prolly set ui:theme "material"
git-prolly set ui:layout "grid"
git-prolly commit -m "Add new UI configuration"
git push origin feature/new-ui-config
cd ..

# Update submodule reference
git add data
git commit -m "Update KV data for new UI feature"
```

#### Environment-Specific Branches
```bash
# Production KV data
cd data
git checkout production
git-prolly set db:host "prod-db.example.com"
git-prolly set cache:ttl "3600"
git-prolly commit -m "Production configuration"
cd ..

# Staging KV data
cd data
git checkout staging
git-prolly set db:host "staging-db.example.com"
git-prolly set cache:ttl "300"
git-prolly commit -m "Staging configuration"
cd ..
```

### Using KV Data in Code
```rust
// src/main.rs
use prollytree::git::VersionedKvStore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open KV store from submodule
    let store = VersionedKvStore::open("./data")?;
    
    let app_name = store.get(b"config:app:name")?;
    let db_host = store.get(b"db:host")?;
    
    println!("Starting {} with database at {}", 
        String::from_utf8_lossy(&app_name.unwrap_or_default()),
        String::from_utf8_lossy(&db_host.unwrap_or_default())
    );
    
    Ok(())
}
```

### Deployment
```bash
# Deploy to production
git checkout main
cd data
git checkout production  # Use production KV data
cd ..
git add data
git commit -m "Deploy with production configuration"
git push origin main

# Deploy to staging
git checkout staging
cd data
git checkout staging     # Use staging KV data
cd ..
git add data
git commit -m "Deploy with staging configuration"
git push origin staging
```

## Monorepo Workflow

### Setup

#### 1. Initialize Monorepo
```bash
# Create project structure
mkdir myapp && cd myapp
git init

# Initialize KV stores
mkdir config && cd config
git-prolly init
git-prolly set app:name "MyApp"
git-prolly set app:version "1.0.0"
git-prolly commit -m "Initial app configuration"
cd ..

mkdir user-data && cd user-data
git-prolly init
git-prolly set schema:version "1"
git-prolly commit -m "Initial user data schema"
cd ..

# Add application code
mkdir src
echo 'fn main() { println!("Hello World"); }' > src/main.rs

# Commit everything
git add .
git commit -m "Initial project setup"
```

### Development Workflow

#### Feature Development
```bash
# Create feature branch
git checkout -b feature/user-profiles

# Update both code and KV data
echo 'fn create_user_profile() {}' >> src/lib.rs

cd config
git-prolly set features:user_profiles "true"
git-prolly set ui:profile_page "enabled"
git-prolly commit -m "Enable user profiles feature"
cd ..

cd user-data
git-prolly set schema:user_profile "name,email,created_at"
git-prolly commit -m "Add user profile schema"
cd ..

# Commit all changes together
git add .
git commit -m "Implement user profiles feature"
```

#### Environment-Specific Configurations
```bash
# Production configuration
git checkout main
cd config
git-prolly set db:host "prod-db.example.com"
git-prolly set features:beta_features "false"
git-prolly commit -m "Production settings"
cd ..
git add config/
git commit -m "Update production configuration"

# Staging configuration
git checkout -b staging
cd config
git-prolly set db:host "staging-db.example.com"
git-prolly set features:beta_features "true"
git-prolly commit -m "Staging settings"
cd ..
git add config/
git commit -m "Update staging configuration"
```

### Using Multiple KV Stores
```rust
// src/main.rs
use prollytree::git::VersionedKvStore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open multiple KV stores
    let config_store = VersionedKvStore::open("./config")?;
    let user_store = VersionedKvStore::open("./user-data")?;
    
    // Use configuration
    let app_name = config_store.get(b"app:name")?;
    let db_host = config_store.get(b"db:host")?;
    
    // Use user data schema
    let schema = user_store.get(b"schema:user_profile")?;
    
    println!("App: {} | DB: {} | Schema: {}", 
        String::from_utf8_lossy(&app_name.unwrap_or_default()),
        String::from_utf8_lossy(&db_host.unwrap_or_default()),
        String::from_utf8_lossy(&schema.unwrap_or_default())
    );
    
    Ok(())
}
```

## Cross-Branch Data Testing

### The Problem
You're working on a hotfix and need to test it against data from different branches/environments:
- Production data (stable)
- Staging data (recent changes)
- Production-sample data (subset for testing)

### Solution 1: Git Worktrees (Recommended)

```bash
# Create separate worktrees for different environments
git worktree add ../myapp-staging staging
git worktree add ../myapp-production production
git worktree add ../myapp-sample production-sample

# Test your hotfix against each environment
cd ../myapp-staging
cargo test -- --test-threads=1

cd ../myapp-production
cargo test -- --test-threads=1

cd ../myapp-sample
cargo test -- --test-threads=1

# Clean up when done
cd ../myapp
git worktree remove ../myapp-staging
git worktree remove ../myapp-production
git worktree remove ../myapp-sample
```

### Solution 2: KV Data Branch Switching

```bash
#!/bin/bash
# test_cross_branch.sh

BRANCHES=("staging" "production" "production-sample")
ORIGINAL_BRANCH=$(cd config && git-prolly current-branch)

echo "Testing hotfix against multiple data branches..."

for branch in "${BRANCHES[@]}"; do
    echo "========================================="
    echo "Testing against $branch data..."
    
    # Switch KV data to this branch
    cd config
    git-prolly checkout $branch
    cd ..
    
    # Run tests
    echo "Running tests with $branch data:"
    cargo test --test integration_tests
    
    if [ $? -eq 0 ]; then
        echo "✅ Tests PASSED with $branch data"
    else
        echo "❌ Tests FAILED with $branch data"
    fi
    
    echo ""
done

# Restore original branch
cd config
git-prolly checkout $ORIGINAL_BRANCH
cd ..

echo "Cross-branch testing complete!"
```

### Solution 3: Programmatic Testing

```rust
// tests/cross_branch_test.rs
use prollytree::git::VersionedKvStore;
use std::process::Command;

#[derive(Debug)]
struct TestResult {
    branch: String,
    passed: bool,
    details: String,
}

struct CrossBranchTester {
    config_path: String,
}

impl CrossBranchTester {
    fn new(config_path: &str) -> Self {
        Self {
            config_path: config_path.to_string(),
        }
    }
    
    fn test_against_branch(&self, branch: &str) -> Result<TestResult, Box<dyn std::error::Error>> {
        // Switch to the test branch
        let mut store = VersionedKvStore::open(&self.config_path)?;
        let current_branch = store.current_branch().to_string();
        
        store.checkout(branch)?;
        
        // Run your hotfix logic
        let result = self.run_hotfix_tests(&store);
        
        // Restore original branch
        store.checkout(&current_branch)?;
        
        Ok(TestResult {
            branch: branch.to_string(),
            passed: result.is_ok(),
            details: match result {
                Ok(msg) => msg,
                Err(e) => format!("Error: {}", e),
            },
        })
    }
    
    fn run_hotfix_tests(&self, store: &VersionedKvStore<32>) -> Result<String, Box<dyn std::error::Error>> {
        // Your actual hotfix testing logic
        let db_host = store.get(b"db:host")?;
        let timeout = store.get(b"db:timeout")?;
        
        // Simulate hotfix logic
        match (db_host, timeout) {
            (Some(host), Some(timeout_val)) => {
                let host_str = String::from_utf8_lossy(&host);
                let timeout_str = String::from_utf8_lossy(&timeout_val);
                
                // Your hotfix validation logic here
                if host_str.contains("prod") && timeout_str.parse::<u32>().unwrap_or(0) > 1000 {
                    Ok("Hotfix works correctly".to_string())
                } else {
                    Err("Hotfix validation failed".into())
                }
            }
            _ => Err("Required configuration missing".into()),
        }
    }
    
    fn test_all_branches(&self) -> Result<Vec<TestResult>, Box<dyn std::error::Error>> {
        let branches = vec!["staging", "production", "production-sample"];
        let mut results = Vec::new();
        
        for branch in branches {
            match self.test_against_branch(branch) {
                Ok(result) => results.push(result),
                Err(e) => {
                    results.push(TestResult {
                        branch: branch.to_string(),
                        passed: false,
                        details: format!("Error: {}", e),
                    });
                }
            }
        }
        
        Ok(results)
    }
}

#[test]
fn test_hotfix_cross_branch() {
    let tester = CrossBranchTester::new("./config");
    let results = tester.test_all_branches().unwrap();
    
    for result in results {
        println!("Branch: {} - Passed: {} - Details: {}", 
            result.branch, result.passed, result.details);
        
        // You can assert specific conditions here
        // assert!(result.passed, "Hotfix failed for branch: {}", result.branch);
    }
}
```

## Advanced Debugging Techniques

### 1. Historical Data Testing

```bash
# Test against specific historical commits
cd config
git-prolly checkout abc123def  # Specific commit
cd ..
cargo test

# Test against tagged versions
cd config
git-prolly checkout v1.2.3
cd ..
cargo test
```

### 2. Data Diff Analysis

```bash
# Compare data between branches
git-prolly diff production staging

# Compare specific commits
git-prolly diff abc123def def456abc

# JSON output for automation
git-prolly diff production staging --format=json > data_diff.json
```

### 3. Debugging with Multiple Datasets

```rust
// src/debug_tools.rs
use prollytree::git::VersionedKvStore;

pub fn debug_with_multiple_datasets() -> Result<(), Box<dyn std::error::Error>> {
    let datasets = vec![
        ("staging", "./config"),
        ("production", "./config"),
        ("production-sample", "./config"),
    ];
    
    for (name, path) in datasets {
        println!("=== Debugging with {} dataset ===", name);
        
        let mut store = VersionedKvStore::open(path)?;
        store.checkout(name)?;
        
        // Your debugging logic here
        debug_specific_issue(&store, name)?;
    }
    
    Ok(())
}

fn debug_specific_issue(store: &VersionedKvStore<32>, dataset: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Example: Debug a specific configuration issue
    let problematic_config = store.get(b"feature:problematic_feature")?;
    
    if let Some(config) = problematic_config {
        println!("Dataset {}: problematic_feature = {}", 
            dataset, String::from_utf8_lossy(&config));
        
        // Apply your fix logic and test
        let result = test_fix_logic(&config);
        println!("Fix result for {}: {:?}", dataset, result);
    }
    
    Ok(())
}

fn test_fix_logic(config: &[u8]) -> bool {
    // Your fix logic here
    true
}
```

## Best Practices

### Repository Structure

#### Separate Repositories
```
# Use when:
- Teams manage data and code separately
- Different release cycles for data and code
- Multiple applications share the same data
- Strict separation of concerns required

# Benefits:
- Clear ownership boundaries
- Independent versioning
- Reusable data across projects
- Granular access control
```

#### Monorepo
```
# Use when:
- Small team with unified workflow
- Data and code are tightly coupled
- Atomic updates required
- Simple deployment pipeline

# Benefits:
- Atomic commits
- Simplified dependency management
- Unified testing and CI/CD
- Easier refactoring
```

### Branch Strategy

#### For Data Branches
```bash
# Environment branches
main              # Production-ready
staging          # Pre-production testing
development      # Integration testing

# Feature branches
feature/new-ui-config     # UI configuration changes
feature/api-v2-schema     # API schema updates
hotfix/critical-config    # Critical configuration fixes
```

#### For Code Branches
```bash
# Standard Git flow
main                    # Production code
develop                 # Integration branch
feature/new-feature     # Feature development
hotfix/critical-fix     # Critical fixes
```

### Testing Strategy

#### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_with_mock_data() {
        // Test with controlled data
        let mut store = create_test_store();
        store.insert(b"test:key".to_vec(), b"test:value".to_vec()).unwrap();
        
        // Your test logic
        assert_eq!(get_processed_value(&store), Some("expected".to_string()));
    }
    
    fn create_test_store() -> VersionedKvStore<32> {
        // Create a temporary store for testing
        let temp_dir = tempfile::tempdir().unwrap();
        VersionedKvStore::init(temp_dir.path()).unwrap()
    }
}
```

#### Integration Tests
```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[test]
    fn test_with_real_data() {
        // Test with real data from different branches
        let tester = CrossBranchTester::new("./config");
        let results = tester.test_all_branches().unwrap();
        
        for result in results {
            assert!(result.passed, "Integration test failed for {}: {}", 
                result.branch, result.details);
        }
    }
}
```

### CI/CD Integration

#### GitHub Actions Example
```yaml
# .github/workflows/test.yml
name: Test with Multiple Datasets

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        dataset: [staging, production, production-sample]
    
    steps:
    - uses: actions/checkout@v2
      with:
        submodules: true  # For separate repo approach
    
    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
    
    - name: Switch to test dataset
      run: |
        cd config
        git-prolly checkout ${{ matrix.dataset }}
        cd ..
    
    - name: Run tests
      run: cargo test
```

## Common Scenarios

### Scenario 1: Feature Development with Data Changes

```bash
# Developer workflow
git checkout -b feature/recommendation-engine

# Update KV data
cd config
git-prolly set ml:model_version "2.1"
git-prolly set ml:confidence_threshold "0.85"
git-prolly commit -m "Update ML model configuration"
cd ..

# Update application code
# ... make code changes ...

# Test together
cargo test

# Commit everything
git add .
git commit -m "Implement recommendation engine v2.1"
```

### Scenario 2: Hotfix Testing

```bash
# Critical bug in production
git checkout -b hotfix/memory-leak-fix

# Fix the code
vim src/memory_manager.rs

# Test against production data
cd config
git-prolly checkout production
cd ..
cargo test --test memory_tests

# Test against staging data
cd config
git-prolly checkout staging
cd ..
cargo test --test memory_tests

# Deploy with confidence
git checkout main
git merge hotfix/memory-leak-fix
```

### Scenario 3: Environment Promotion

```bash
# Promote from staging to production
git checkout staging

# Verify staging tests pass
cargo test

# Update production KV data
cd config
git-prolly checkout production
git-prolly merge staging
git-prolly commit -m "Promote staging configuration to production"
cd ..

# Deploy to production
git checkout main
git merge staging
git push origin main
```

### Scenario 4: Data Migration

```bash
# Migrate data schema
cd config
git-prolly branch migration/v2-schema
git-prolly checkout migration/v2-schema

# Update schema
git-prolly set schema:version "2"
git-prolly set schema:user_table "id,name,email,created_at,updated_at"
git-prolly delete schema:legacy_fields
git-prolly commit -m "Migrate to schema v2"

# Test migration
cd ..
cargo test --test migration_tests

# Merge when ready
cd config
git-prolly checkout main
git-prolly merge migration/v2-schema
```

## Conclusion

git-prolly provides powerful workflows for managing versioned key-value data alongside your application code. Whether you choose separate repositories or a monorepo approach, the key is to:

1. **Maintain consistency** between code and data versions
2. **Test thoroughly** across different data states
3. **Use Git workflows** you're already familiar with
4. **Automate testing** for multiple datasets
5. **Document your patterns** for team consistency

Choose the approach that best fits your team size, deployment complexity, and organizational structure. Both patterns provide robust solutions for different scenarios.