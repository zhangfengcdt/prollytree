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

//! Git-like Tree Diffing Example
//!
//! This example demonstrates how to use Prolly Trees to diff two trees that
//! have evolved separately from a common root, similar to git branch diffing.
//! It shows how changes can be detected and analyzed between different versions
//! of data structures.

use prollytree::config::TreeConfig;
use prollytree::diff::DiffResult;
use prollytree::storage::InMemoryNodeStorage;
use prollytree::tree::{ProllyTree, Tree};

fn main() {
    println!("🔀 Git-like Tree Diffing Example 🔀\n");

    // Create a shared configuration for consistency
    let config = TreeConfig {
        base: 131,
        modulus: 1_000_000_009,
        min_chunk_size: 4,
        max_chunk_size: 8 * 1024,
        pattern: 0b101,
        root_hash: None,
        key_schema: None,
        value_schema: None,
        encode_types: vec![],
    };

    // ============================================================================
    // Step 1: Create a common "main" branch with initial data
    // ============================================================================
    println!("📦 Creating main branch with initial data...");
    let storage_main = InMemoryNodeStorage::<32>::default();
    let mut main_tree = ProllyTree::new(storage_main, config.clone());

    // Add initial commit data
    let initial_data = vec![
        ("file1.txt", "Hello World"),
        ("file2.txt", "Initial content"),
        ("config.json", r#"{"version": "1.0", "debug": false}"#),
        ("readme.md", "# Project\nThis is the main project"),
        ("src/main.rs", "fn main() { println!(\"Hello\"); }"),
    ];

    for (key, value) in &initial_data {
        main_tree.insert(key.as_bytes().to_vec(), value.as_bytes().to_vec());
    }

    println!("   ✅ Main branch created with {} files", initial_data.len());
    println!("   🌳 Main tree hash: {:?}", main_tree.get_root_hash());

    // ============================================================================
    // Step 2: Create "feature" branch - simulating git checkout -b feature
    // ============================================================================
    println!("\n🌿 Creating feature branch from main...");
    let storage_feature = InMemoryNodeStorage::<32>::default();
    let mut feature_tree = ProllyTree::new(storage_feature, config.clone());

    // Copy all data from main (simulating branch creation)
    for (key, value) in &initial_data {
        feature_tree.insert(key.as_bytes().to_vec(), value.as_bytes().to_vec());
    }

    // Make changes in feature branch
    let feature_changes = vec![
        // Modified files
        ("file1.txt", "Hello World - Feature Edition!"),
        ("config.json", r#"{"version": "1.1", "debug": true, "feature_flag": true}"#),
        
        // New files
        ("feature.rs", "pub fn new_feature() { /* implementation */ }"),
        ("tests/test_feature.rs", "#[test] fn test_new_feature() { assert!(true); }"),
    ];

    for (key, value) in &feature_changes {
        feature_tree.insert(key.as_bytes().to_vec(), value.as_bytes().to_vec());
    }

    // Delete a file in feature branch
    feature_tree.delete(b"readme.md");

    println!("   ✅ Feature branch created with modifications");
    println!("   🌳 Feature tree hash: {:?}", feature_tree.get_root_hash());

    // ============================================================================
    // Step 3: Create "hotfix" branch - another parallel development
    // ============================================================================
    println!("\n🚀 Creating hotfix branch from main...");
    let storage_hotfix = InMemoryNodeStorage::<32>::default();
    let mut hotfix_tree = ProllyTree::new(storage_hotfix, config);

    // Copy all data from main
    for (key, value) in &initial_data {
        hotfix_tree.insert(key.as_bytes().to_vec(), value.as_bytes().to_vec());
    }

    // Make urgent fixes
    let hotfix_changes = vec![
        ("file2.txt", "Fixed critical bug in initial content"),
        ("src/main.rs", "fn main() { println!(\"Hello - Fixed!\"); }"),
        ("hotfix.patch", "Critical security patch applied"),
    ];

    for (key, value) in &hotfix_changes {
        hotfix_tree.insert(key.as_bytes().to_vec(), value.as_bytes().to_vec());
    }

    println!("   ✅ Hotfix branch created with urgent fixes");
    println!("   🌳 Hotfix tree hash: {:?}", hotfix_tree.get_root_hash());

    // ============================================================================
    // Step 4: Perform Git-like diffs between branches
    // ============================================================================
    println!("\n🔍 Performing Git-like diffs...\n");

    // Diff 1: main vs feature (like git diff main..feature)
    println!("📊 Diff: main -> feature (shows what feature adds/changes)");
    println!("   Similar to: git diff main..feature");
    let main_to_feature_diff = main_tree.diff(&feature_tree);
    print_diff_summary(&main_to_feature_diff, "main", "feature");

    // Diff 2: main vs hotfix (like git diff main..hotfix)
    println!("\n📊 Diff: main -> hotfix (shows what hotfix adds/changes)");
    println!("   Similar to: git diff main..hotfix");
    let main_to_hotfix_diff = main_tree.diff(&hotfix_tree);
    print_diff_summary(&main_to_hotfix_diff, "main", "hotfix");

    // Diff 3: feature vs hotfix (like git diff feature..hotfix)
    println!("\n📊 Diff: feature -> hotfix (shows differences between parallel branches)");
    println!("   Similar to: git diff feature..hotfix");
    let feature_to_hotfix_diff = feature_tree.diff(&hotfix_tree);
    print_diff_summary(&feature_to_hotfix_diff, "feature", "hotfix");

    // ============================================================================
    // Step 5: Show detailed analysis
    // ============================================================================
    println!("\n📈 Detailed Diff Analysis:");
    println!("═══════════════════════════════════════");

    analyze_diff(&main_to_feature_diff, "Main → Feature");
    analyze_diff(&main_to_hotfix_diff, "Main → Hotfix");
    analyze_diff(&feature_to_hotfix_diff, "Feature ↔ Hotfix");

    // ============================================================================
    // Summary
    // ============================================================================
    println!("\n🎯 Summary:");
    println!("   • Prolly Trees enable efficient git-like diffing");
    println!("   • Each branch maintains its own merkle tree structure");
    println!("   • Diffs show exact changes: additions, deletions, modifications");
    println!("   • Perfect for version control, distributed systems, and data synchronization");
    println!("   • Hash-based verification ensures data integrity across branches");
}

fn print_diff_summary(diffs: &[DiffResult], _from_branch: &str, _to_branch: &str) {
    let (added, removed, modified) = count_diff_types(diffs);
    
    println!("   📈 Changes: +{} files, -{} files, ~{} files modified", added, removed, modified);
    
    if diffs.is_empty() {
        println!("   ✨ No differences found - branches are identical");
    }
}

fn count_diff_types(diffs: &[DiffResult]) -> (usize, usize, usize) {
    let mut added = 0;
    let mut removed = 0;
    let mut modified = 0;

    for diff in diffs {
        match diff {
            DiffResult::Added(_, _) => added += 1,
            DiffResult::Removed(_, _) => removed += 1,
            DiffResult::Modified(_, _, _) => modified += 1,
        }
    }

    (added, removed, modified)
}

fn analyze_diff(diffs: &[DiffResult], comparison: &str) {
    println!("\n🔍 {}", comparison);
    println!("───────────────────────────────");

    if diffs.is_empty() {
        println!("   No changes detected");
        return;
    }

    for diff in diffs {
        match diff {
            DiffResult::Added(key, _value) => {
                let filename = String::from_utf8_lossy(key);
                println!("   ➕ Added: {}", filename);
            }
            DiffResult::Removed(key, _value) => {
                let filename = String::from_utf8_lossy(key);
                println!("   ➖ Removed: {}", filename);
            }
            DiffResult::Modified(key, _old_value, _new_value) => {
                let filename = String::from_utf8_lossy(key);
                println!("   🔄 Modified: {}", filename);
            }
        }
    }
}
