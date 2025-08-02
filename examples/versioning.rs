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

//! Git-like Operations Example
//!
//! This example demonstrates how to use Prolly Trees to implement git-like
//! operations including diffing and merging. It shows how changes can be
//! detected and merged between different versions of data structures.

use prollytree::config::TreeConfig;
use prollytree::diff::DiffResult;
use prollytree::storage::InMemoryNodeStorage;
use prollytree::tree::{ProllyTree, Tree};
use std::collections::HashMap;

#[derive(Debug, Clone)]
enum MergeConflict {
    BothModified {
        key: Vec<u8>,
        branch_a_value: Vec<u8>,
        branch_b_value: Vec<u8>,
    },
}

fn main() {
    println!("\x1b[1;36mGit-like Operations with Prolly Trees\x1b[0m\n");

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
    // Part 1: Git-like Diffing
    // ============================================================================
    println!("\x1b[1;34m═══ Part 1: Git-like Diffing ═══\x1b[0m\n");
    demonstrate_diffing(config.clone());

    // ============================================================================
    // Part 2: Git-like Merging
    // ============================================================================
    println!("\n\x1b[1;34m═══ Part 2: Git-like Merging ═══\x1b[0m\n");
    demonstrate_merging(config);

    // ============================================================================
    // Summary
    // ============================================================================
    println!("\n\x1b[1;36mSummary:\x1b[0m");
    println!("   \x1b[32m• Prolly Trees enable efficient git-like operations\x1b[0m");
    println!("   \x1b[32m• Diffing shows exact changes between branches\x1b[0m");
    println!("   \x1b[32m• Merging supports fast-forward and three-way strategies\x1b[0m");
    println!("   \x1b[32m• Conflicts can be detected and resolved automatically\x1b[0m");
    println!("   \x1b[32m• Hash-based verification ensures data integrity\x1b[0m");
}

fn demonstrate_diffing(config: TreeConfig<32>) {
    // Create main branch
    println!("\x1b[1;32mCreating main branch with initial data...\x1b[0m");
    let storage_main = InMemoryNodeStorage::<32>::default();
    let mut main_tree = ProllyTree::new(storage_main, config.clone());

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

    println!(
        "   \x1b[32mMain branch created with {} files\x1b[0m",
        initial_data.len()
    );

    // Create feature branch
    println!("\n\x1b[1;32mCreating feature branch from main...\x1b[0m");
    let storage_feature = InMemoryNodeStorage::<32>::default();
    let mut feature_tree = ProllyTree::new(storage_feature, config.clone());

    // Copy all data from main
    for (key, value) in &initial_data {
        feature_tree.insert(key.as_bytes().to_vec(), value.as_bytes().to_vec());
    }

    // Make changes in feature branch
    let feature_changes = vec![
        ("file1.txt", "Hello World - Feature Edition!"),
        (
            "config.json",
            r#"{"version": "1.1", "debug": true, "feature_flag": true}"#,
        ),
        (
            "feature.rs",
            "pub fn new_feature() { /* implementation */ }",
        ),
    ];

    for (key, value) in &feature_changes {
        feature_tree.insert(key.as_bytes().to_vec(), value.as_bytes().to_vec());
    }

    feature_tree.delete(b"readme.md");

    println!("   \x1b[32mFeature branch created with modifications\x1b[0m");

    // Perform diff
    println!("\n\x1b[1;35mDiff: main -> feature\x1b[0m");
    let diff = main_tree.diff(&feature_tree);
    print_diff_summary(&diff);
    analyze_diff(&diff, "Main → Feature");
}

fn demonstrate_merging(config: TreeConfig<32>) {
    // Scenario 1: Fast-forward merge
    println!("\x1b[1;32mScenario 1: Fast-forward merge\x1b[0m");
    println!("\x1b[90m═══════════════════════════════════\x1b[0m");

    let (main_tree_ff, feature_tree_ff) = create_fast_forward_scenario(config.clone());
    let merged_ff = perform_fast_forward_merge(&main_tree_ff, &feature_tree_ff);

    println!("   \x1b[32mFast-forward merge completed!\x1b[0m");
    println!("      Merged tree: {} files", merged_ff.size());

    // Scenario 2: Three-way merge
    println!("\n\x1b[1;34mScenario 2: Three-way merge\x1b[0m");
    println!("\x1b[90m═══════════════════════════════════\x1b[0m");

    let (base_tree, branch_a, branch_b) = create_three_way_scenario(config.clone());
    let merged_3way = perform_three_way_merge(&base_tree, &branch_a, &branch_b);

    println!("   \x1b[32mThree-way merge completed!\x1b[0m");
    println!("      Merged tree: {} files", merged_3way.size());

    // Scenario 3: Merge with conflicts
    println!("\n\x1b[1;31mScenario 3: Merge with conflicts\x1b[0m");
    println!("\x1b[90m══════════════════════════════════════\x1b[0m");

    let (base_conflict, branch_conflict_a, branch_conflict_b) = create_conflict_scenario(config);
    let (conflicts, merged_with_conflicts) =
        detect_and_resolve_conflicts(&base_conflict, &branch_conflict_a, &branch_conflict_b);

    if !conflicts.is_empty() {
        println!(
            "   \x1b[31mConflicts detected: {} conflicts\x1b[0m",
            conflicts.len()
        );
        for conflict in &conflicts {
            print_conflict(conflict);
        }
        println!("   \x1b[32mConflicts resolved automatically!\x1b[0m");
    }

    println!("      Merged tree: {} files", merged_with_conflicts.size());
}

// ============================================================================
// Helper Functions
// ============================================================================

fn print_diff_summary(diffs: &[DiffResult]) {
    let (added, removed, modified) = count_diff_types(diffs);

    println!("   \x1b[33mChanges: \x1b[32m+{}\x1b[0m files, \x1b[31m-{}\x1b[0m files, \x1b[34m~{}\x1b[0m files modified", added, removed, modified);

    if diffs.is_empty() {
        println!("   \x1b[32mNo differences found - branches are identical\x1b[0m");
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
    println!("\n\x1b[1;34m{}\x1b[0m", comparison);
    println!("\x1b[90m───────────────────────────────\x1b[0m");

    if diffs.is_empty() {
        println!("   No changes detected");
        return;
    }

    for diff in diffs {
        match diff {
            DiffResult::Added(key, _value) => {
                let filename = String::from_utf8_lossy(key);
                println!("   \x1b[32m+ Added: {}\x1b[0m", filename);
            }
            DiffResult::Removed(key, _value) => {
                let filename = String::from_utf8_lossy(key);
                println!("   \x1b[31m- Removed: {}\x1b[0m", filename);
            }
            DiffResult::Modified(key, _old_value, _new_value) => {
                let filename = String::from_utf8_lossy(key);
                println!("   \x1b[34m~ Modified: {}\x1b[0m", filename);
            }
        }
    }
}

fn create_fast_forward_scenario(
    config: TreeConfig<32>,
) -> (
    ProllyTree<32, InMemoryNodeStorage<32>>,
    ProllyTree<32, InMemoryNodeStorage<32>>,
) {
    // Main branch (behind)
    let storage_main = InMemoryNodeStorage::<32>::default();
    let mut main_tree = ProllyTree::new(storage_main, config.clone());

    main_tree.insert(b"readme.md".to_vec(), b"# Project".to_vec());
    main_tree.insert(b"src/lib.rs".to_vec(), b"// Library code".to_vec());

    // Feature branch (ahead with additional commits)
    let storage_feature = InMemoryNodeStorage::<32>::default();
    let mut feature_tree = ProllyTree::new(storage_feature, config);

    // Start with main's content
    feature_tree.insert(b"readme.md".to_vec(), b"# Project".to_vec());
    feature_tree.insert(b"src/lib.rs".to_vec(), b"// Library code".to_vec());

    // Add new features
    feature_tree.insert(
        b"src/feature.rs".to_vec(),
        b"pub fn new_feature() {}".to_vec(),
    );
    feature_tree.insert(
        b"tests/test.rs".to_vec(),
        b"#[test] fn test_feature() {}".to_vec(),
    );

    (main_tree, feature_tree)
}

fn create_three_way_scenario(
    config: TreeConfig<32>,
) -> (
    ProllyTree<32, InMemoryNodeStorage<32>>,
    ProllyTree<32, InMemoryNodeStorage<32>>,
    ProllyTree<32, InMemoryNodeStorage<32>>,
) {
    // Base (common ancestor)
    let storage_base = InMemoryNodeStorage::<32>::default();
    let mut base_tree = ProllyTree::new(storage_base, config.clone());

    base_tree.insert(b"config.json".to_vec(), b"{\"version\": \"1.0\"}".to_vec());
    base_tree.insert(b"main.rs".to_vec(), b"fn main() {}".to_vec());

    // Branch A: adds documentation
    let storage_a = InMemoryNodeStorage::<32>::default();
    let mut branch_a = ProllyTree::new(storage_a, config.clone());

    branch_a.insert(b"config.json".to_vec(), b"{\"version\": \"1.0\"}".to_vec());
    branch_a.insert(b"main.rs".to_vec(), b"fn main() {}".to_vec());
    branch_a.insert(b"docs.md".to_vec(), b"# Documentation".to_vec());

    // Branch B: adds tests
    let storage_b = InMemoryNodeStorage::<32>::default();
    let mut branch_b = ProllyTree::new(storage_b, config);

    branch_b.insert(b"config.json".to_vec(), b"{\"version\": \"1.0\"}".to_vec());
    branch_b.insert(b"main.rs".to_vec(), b"fn main() {}".to_vec());
    branch_b.insert(b"test.rs".to_vec(), b"#[test] fn test() {}".to_vec());

    (base_tree, branch_a, branch_b)
}

fn create_conflict_scenario(
    config: TreeConfig<32>,
) -> (
    ProllyTree<32, InMemoryNodeStorage<32>>,
    ProllyTree<32, InMemoryNodeStorage<32>>,
    ProllyTree<32, InMemoryNodeStorage<32>>,
) {
    // Base
    let storage_base = InMemoryNodeStorage::<32>::default();
    let mut base_tree = ProllyTree::new(storage_base, config.clone());

    base_tree.insert(b"version.txt".to_vec(), b"1.0.0".to_vec());
    base_tree.insert(b"shared.rs".to_vec(), b"// Shared code".to_vec());

    // Branch A: updates version and modifies shared code
    let storage_a = InMemoryNodeStorage::<32>::default();
    let mut branch_a = ProllyTree::new(storage_a, config.clone());

    branch_a.insert(b"version.txt".to_vec(), b"1.1.0".to_vec()); // Conflict!
    branch_a.insert(
        b"shared.rs".to_vec(),
        b"// Shared code - Feature A".to_vec(),
    ); // Conflict!
    branch_a.insert(b"feature_a.rs".to_vec(), b"// Feature A".to_vec());

    // Branch B: updates version differently and modifies shared code differently
    let storage_b = InMemoryNodeStorage::<32>::default();
    let mut branch_b = ProllyTree::new(storage_b, config);

    branch_b.insert(b"version.txt".to_vec(), b"1.0.1".to_vec()); // Conflict!
    branch_b.insert(
        b"shared.rs".to_vec(),
        b"// Shared code - Feature B".to_vec(),
    ); // Conflict!
    branch_b.insert(b"feature_b.rs".to_vec(), b"// Feature B".to_vec());

    (base_tree, branch_a, branch_b)
}

fn perform_fast_forward_merge(
    _main: &ProllyTree<32, InMemoryNodeStorage<32>>,
    feature: &ProllyTree<32, InMemoryNodeStorage<32>>,
) -> ProllyTree<32, InMemoryNodeStorage<32>> {
    // In a fast-forward merge, we simply adopt the feature branch
    let storage = InMemoryNodeStorage::<32>::default();
    let mut merged = ProllyTree::new(storage, TreeConfig::default());

    // Copy all data from feature branch
    collect_all_key_values(feature)
        .into_iter()
        .for_each(|(k, v)| {
            merged.insert(k, v);
        });

    merged
}

fn perform_three_way_merge(
    base: &ProllyTree<32, InMemoryNodeStorage<32>>,
    branch_a: &ProllyTree<32, InMemoryNodeStorage<32>>,
    branch_b: &ProllyTree<32, InMemoryNodeStorage<32>>,
) -> ProllyTree<32, InMemoryNodeStorage<32>> {
    let storage = InMemoryNodeStorage::<32>::default();
    let mut merged = ProllyTree::new(storage, TreeConfig::default());

    // Start with base
    for (key, value) in collect_all_key_values(base) {
        merged.insert(key, value);
    }

    // Apply changes from branch A
    let diff_a = base.diff(branch_a);
    apply_diff_to_tree(&mut merged, &diff_a);

    // Apply non-conflicting changes from branch B
    let diff_b = base.diff(branch_b);
    apply_diff_to_tree(&mut merged, &diff_b);

    merged
}

fn detect_and_resolve_conflicts(
    base: &ProllyTree<32, InMemoryNodeStorage<32>>,
    branch_a: &ProllyTree<32, InMemoryNodeStorage<32>>,
    branch_b: &ProllyTree<32, InMemoryNodeStorage<32>>,
) -> (Vec<MergeConflict>, ProllyTree<32, InMemoryNodeStorage<32>>) {
    let mut conflicts = Vec::new();

    let diff_a = base.diff(branch_a);
    let diff_b = base.diff(branch_b);

    // Group diffs by key to detect conflicts
    let mut changes_a: HashMap<Vec<u8>, &DiffResult> = HashMap::new();
    let mut changes_b: HashMap<Vec<u8>, &DiffResult> = HashMap::new();

    for diff in &diff_a {
        let key = get_diff_key(diff);
        changes_a.insert(key, diff);
    }

    for diff in &diff_b {
        let key = get_diff_key(diff);
        changes_b.insert(key, diff);
    }

    // Detect conflicts
    for (key, diff_a) in &changes_a {
        if let Some(diff_b) = changes_b.get(key) {
            // Both branches modified the same file
            if let (DiffResult::Modified(_, _, new_a), DiffResult::Modified(_, _, new_b)) =
                (diff_a, diff_b)
            {
                conflicts.push(MergeConflict::BothModified {
                    key: key.clone(),
                    branch_a_value: new_a.clone(),
                    branch_b_value: new_b.clone(),
                });
            }
        }
    }

    // Create merged tree with conflict resolution
    let storage = InMemoryNodeStorage::<32>::default();
    let mut merged = ProllyTree::new(storage, TreeConfig::default());

    // Start with base
    for (key, value) in collect_all_key_values(base) {
        merged.insert(key, value);
    }

    // Apply non-conflicting changes
    for diff in &diff_a {
        let key = get_diff_key(diff);
        if !conflicts.iter().any(|c| get_conflict_key(c) == &key) {
            apply_single_diff(&mut merged, diff);
        }
    }

    for diff in &diff_b {
        let key = get_diff_key(diff);
        if !conflicts.iter().any(|c| get_conflict_key(c) == &key) {
            apply_single_diff(&mut merged, diff);
        }
    }

    // Resolve conflicts (using a simple strategy: prefer branch A)
    for conflict in &conflicts {
        match conflict {
            MergeConflict::BothModified {
                key,
                branch_a_value,
                ..
            } => {
                merged.insert(key.clone(), branch_a_value.clone());
            }
        }
    }

    (conflicts, merged)
}

fn collect_all_key_values(
    tree: &ProllyTree<32, InMemoryNodeStorage<32>>,
) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut result = Vec::new();

    // Use diff against an empty tree to get all entries
    let empty_storage = InMemoryNodeStorage::<32>::default();
    let empty_tree = ProllyTree::new(empty_storage, TreeConfig::default());

    let diff = empty_tree.diff(tree);
    for diff_result in diff {
        if let DiffResult::Added(key, value) = diff_result {
            result.push((key, value));
        }
    }

    result
}

fn apply_diff_to_tree(tree: &mut ProllyTree<32, InMemoryNodeStorage<32>>, diffs: &[DiffResult]) {
    for diff in diffs {
        apply_single_diff(tree, diff);
    }
}

fn apply_single_diff(tree: &mut ProllyTree<32, InMemoryNodeStorage<32>>, diff: &DiffResult) {
    match diff {
        DiffResult::Added(key, value) => {
            tree.insert(key.clone(), value.clone());
        }
        DiffResult::Modified(key, _, new_value) => {
            tree.insert(key.clone(), new_value.clone());
        }
        DiffResult::Removed(key, _) => {
            tree.delete(key);
        }
    }
}

fn get_diff_key(diff: &DiffResult) -> Vec<u8> {
    match diff {
        DiffResult::Added(key, _) => key.clone(),
        DiffResult::Removed(key, _) => key.clone(),
        DiffResult::Modified(key, _, _) => key.clone(),
    }
}

fn get_conflict_key(conflict: &MergeConflict) -> &Vec<u8> {
    match conflict {
        MergeConflict::BothModified { key, .. } => key,
    }
}

fn print_conflict(conflict: &MergeConflict) {
    match conflict {
        MergeConflict::BothModified {
            key,
            branch_a_value,
            branch_b_value,
            ..
        } => {
            let filename = String::from_utf8_lossy(key);
            println!("      \x1b[31mBoth modified: {}\x1b[0m", filename);
            println!(
                "         \x1b[33mBranch A: {}\x1b[0m",
                String::from_utf8_lossy(branch_a_value)
            );
            println!(
                "         \x1b[33mBranch B: {}\x1b[0m",
                String::from_utf8_lossy(branch_b_value)
            );
            println!("         \x1b[32mResolution: Using Branch A version\x1b[0m");
        }
    }
}
