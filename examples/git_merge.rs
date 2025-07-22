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

//! Git-like Tree Merging Example
//!
//! This example demonstrates how to merge two Prolly Trees that have evolved
//! separately from a common root, similar to git branch merging. It shows
//! how to handle different types of conflicts and create a unified tree.

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
    println!("\x1b[1;36mGit-like Tree Merging Example\x1b[0m\n");

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
    // Scenario 1: Fast-forward merge (no conflicts)
    // ============================================================================
    println!("\x1b[1;32mScenario 1: Fast-forward merge\x1b[0m");
    println!("\x1b[90m═══════════════════════════════════\x1b[0m");

    let (main_tree_ff, feature_tree_ff) = create_fast_forward_scenario(config.clone());

    println!("   \x1b[33mBefore merge:\x1b[0m");
    println!("      Main branch: {} files", main_tree_ff.size());
    println!("      Feature branch: {} files", feature_tree_ff.size());

    let merged_ff = perform_fast_forward_merge(&main_tree_ff, &feature_tree_ff);
    println!("   \x1b[32mFast-forward merge completed!\x1b[0m");
    println!("      Merged tree: {} files", merged_ff.size());
    println!("      \x1b[90mStrategy: Fast-forward (no conflicts)\x1b[0m\n");

    // ============================================================================
    // Scenario 2: Three-way merge with automatic resolution
    // ============================================================================
    println!("\x1b[1;34mScenario 2: Three-way merge (auto-resolvable)\x1b[0m");
    println!("\x1b[90m═══════════════════════════════════════════════\x1b[0m");

    let (base_tree, branch_a, branch_b) = create_three_way_scenario(config.clone());

    println!("   \x1b[33mBefore merge:\x1b[0m");
    println!("      Base (common ancestor): {} files", base_tree.size());
    println!("      Branch A: {} files", branch_a.size());
    println!("      Branch B: {} files", branch_b.size());

    let merged_3way = perform_three_way_merge(&base_tree, &branch_a, &branch_b);
    println!("   \x1b[32mThree-way merge completed!\x1b[0m");
    println!("      Merged tree: {} files", merged_3way.size());
    println!("      \x1b[90mStrategy: Three-way merge (auto-resolved)\x1b[0m\n");

    // ============================================================================
    // Scenario 3: Merge with conflicts requiring resolution
    // ============================================================================
    println!("\x1b[1;31mScenario 3: Merge with conflicts\x1b[0m");
    println!("\x1b[90m══════════════════════════════════════\x1b[0m");

    let (base_conflict, branch_conflict_a, branch_conflict_b) = create_conflict_scenario(config);

    println!("   \x1b[33mBefore merge:\x1b[0m");
    println!("      Base: {} files", base_conflict.size());
    println!("      Branch A: {} files", branch_conflict_a.size());
    println!("      Branch B: {} files", branch_conflict_b.size());

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
    println!("      \x1b[90mStrategy: Conflict resolution\x1b[0m\n");

    // ============================================================================
    // Summary and Best Practices
    // ============================================================================
    println!("\x1b[1;36mSummary & Best Practices:\x1b[0m");
    println!("\x1b[90m═══════════════════════════════\x1b[0m");
    println!("   \x1b[32m1. Fast-forward: When target branch is ahead of source\x1b[0m");
    println!("   \x1b[34m2. Three-way: When both branches have changes from common base\x1b[0m");
    println!("   \x1b[31m3. Conflicts: When same data is modified differently\x1b[0m");
    println!("   \x1b[33m4. Always verify merge results with diff analysis\x1b[0m");
    println!("   \x1b[35m5. Prolly Trees provide cryptographic verification of merges\x1b[0m");
    println!("   \x1b[36m6. Each merge creates a new tree with verifiable history\x1b[0m");

    // Show the final verification
    println!("\n\x1b[1;33mMerge Verification:\x1b[0m");
    println!(
        "   \x1b[32mFast-forward hash: {:?}\x1b[0m",
        merged_ff.get_root_hash()
    );
    println!(
        "   \x1b[34mThree-way hash: {:?}\x1b[0m",
        merged_3way.get_root_hash()
    );
    println!(
        "   \x1b[31mConflict-resolved hash: {:?}\x1b[0m",
        merged_with_conflicts.get_root_hash()
    );
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
    // since it contains all of main's changes plus additional ones

    let storage = InMemoryNodeStorage::<32>::default();
    let mut merged = ProllyTree::new(storage, TreeConfig::default());

    // Copy all data from feature branch (which includes main + new changes)
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
    // This is a simplified implementation
    // In practice, you'd traverse the tree to collect all key-value pairs
    let mut result = Vec::new();

    // For this example, we'll use the diff against an empty tree to get all entries
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
