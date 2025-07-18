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

use clap::{Parser, Subcommand};
use prollytree::git::{DiffOperation, GitOperations, MergeResult, VersionedKvStore};
use prollytree::tree::Tree;
use std::env;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "git-prolly")]
#[command(about = "KV-aware Git operations for ProllyTree")]
#[command(version = "0.2.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new git-backed KV store
    ///
    /// Basic Usage:
    ///   git-prolly init [path]
    ///
    /// Description:
    ///   Creates a new ProllyTree versioned key-value store in the specified directory.
    ///   This must be run from within a Git repository, and will create a 'dataset'
    ///   subdirectory if none is specified.
    ///
    /// Examples:
    ///   git-prolly init                    # Initialize in current directory
    ///   git-prolly init ./my-dataset       # Initialize in specific directory
    Init {
        #[arg(help = "Directory to initialize (defaults to current directory)")]
        path: Option<PathBuf>,
    },

    /// Set a key-value pair (stages the change)
    ///
    /// Basic Usage:
    ///   git-prolly set <key> <value>
    ///
    /// Description:
    ///   Stages a key-value pair for the next commit. The change is not permanent
    ///   until you run 'git-prolly commit'. If the key already exists, it will be
    ///   updated with the new value.
    ///
    /// Examples:
    ///   git-prolly set name "Alice"        # Set a string value
    ///   git-prolly set age "25"            # Set a numeric value as string
    ///   git-prolly set config.debug "true" # Set a configuration value
    Set {
        #[arg(help = "Key to set")]
        key: String,
        #[arg(help = "Value to set")]
        value: String,
    },

    /// Get a value by key
    ///
    /// Basic Usage:
    ///   git-prolly get <key>
    ///
    /// Description:
    ///   Retrieves the current value for the specified key. Shows the value from
    ///   the current working tree, including any staged changes.
    ///
    /// Examples:
    ///   git-prolly get name               # Get value for 'name' key
    ///   git-prolly get config.debug       # Get nested configuration value
    Get {
        #[arg(help = "Key to get")]
        key: String,
    },

    /// Delete a key (stages the change)
    ///
    /// Basic Usage:
    ///   git-prolly delete <key>
    ///
    /// Description:
    ///   Stages a key for deletion in the next commit. The key will be removed
    ///   from the store when you run 'git-prolly commit'.
    ///
    /// Examples:
    ///   git-prolly delete name            # Delete the 'name' key
    ///   git-prolly delete temp.*          # Delete keys matching pattern
    Delete {
        #[arg(help = "Key to delete")]
        key: String,
    },

    /// List all keys
    ///
    /// Basic Usage:
    ///   git-prolly list [--values] [--graph]
    ///
    /// Description:
    ///   Lists all keys in the current state of the store. Can optionally show
    ///   values and/or the internal ProllyTree structure.
    ///
    /// Options:
    ///   --values  Show key-value pairs instead of just keys
    ///   --graph   Show the internal ProllyTree structure
    ///
    /// Examples:
    ///   git-prolly list                   # List all keys
    ///   git-prolly list --values          # List keys with values
    ///   git-prolly list --graph           # Show tree structure
    ///   git-prolly list --values --graph  # Show both values and structure
    List {
        #[arg(long, help = "Show values as well")]
        values: bool,
        #[arg(long, help = "Show prolly tree structure")]
        graph: bool,
    },

    /// Show staging area status
    ///
    /// Basic Usage:
    ///   git-prolly status
    ///
    /// Description:
    ///   Shows the current status of the staging area, including what changes
    ///   are staged for the next commit. Similar to 'git status' but for KV pairs.
    ///
    /// Examples:
    ///   git-prolly status                 # Show staging area status
    Status,

    /// Commit staged changes
    ///
    /// Basic Usage:
    ///   git-prolly commit -m "message"
    ///
    /// Description:
    ///   Commits all staged changes to the repository with the provided message.
    ///   This creates a new commit in the current branch and clears the staging area.
    ///
    /// Examples:
    ///   git-prolly commit -m "Add user data"      # Commit with message
    ///   git-prolly commit --message "Update config"  # Long form
    Commit {
        #[arg(short, long, help = "Commit message")]
        message: String,
    },

    #[command(about = r#"Show KV-aware diff between commits

Basic Usage:
  git-prolly diff <from> <to>

Description:
  Shows the differences in key-value pairs between two commits, branches,
  or references. Supports multiple output formats for different use cases.

Arguments:
  from: Source commit/branch reference
  to: Target commit/branch reference
  --format: Output format (compact, detailed, json)
  --keys: Filter by key pattern (not fully implemented)

Output Formats:
  1. Compact Format (default):
     - Shows changes with color coding
     - + key = "value" (green) for additions
     - - key = "value" (red) for deletions
     - ~ key = "old" -> "new" (yellow) for modifications
  2. Detailed Format:
     - Expanded view with clear sections
     - Shows operation type and values separately
     - Better for analysis of individual changes
  3. JSON Format:
     - Machine-readable output
     - Structured data for programmatic use

Supported References:
  - Commit hashes: abc123, def456
  - Branch names: main, feature-branch
  - Git references: HEAD, HEAD~1, HEAD~2

Examples:
  git-prolly diff abc123 def456               # Diff between commits
  git-prolly diff main feature-branch        # Diff between branches
  git-prolly diff HEAD~1 HEAD --format detailed  # Detailed format
  git-prolly diff main feature-branch --format json  # JSON output"#)]
    Diff {
        #[arg(help = "From commit/branch")]
        from: String,
        #[arg(help = "To commit/branch")]
        to: String,
        #[arg(long, help = "Output format (compact, detailed, json)")]
        format: Option<String>,
        #[arg(long, help = "Filter by key pattern")]
        keys: Option<String>,
    },

    #[command(about = r#"Show KV state at specific commit

Basic Usage:
  git-prolly show <commit> [--keys-only]

Description:
  Shows the complete key-value state at a specific commit. This reconstructs
  the ProllyTree state from the given commit and displays all key-value pairs
  that existed at that point in time.

Arguments:
  commit: The commit hash, branch name, or reference to show
  --keys-only: Show only keys without values

Examples:
  git-prolly show HEAD                     # Show current state
  git-prolly show abc123                   # Show state at commit
  git-prolly show main                     # Show state at branch
  git-prolly show HEAD~2 --keys-only      # Show only keys from 2 commits ago"#)]
    Show {
        #[arg(help = "Commit to show")]
        commit: String,
        #[arg(long, help = "Show only keys")]
        keys_only: bool,
    },

    #[command(about = r#"Merge another branch

Basic Usage:
  git-prolly merge <branch>

Description:
  Merges another branch into the current branch. Currently supports
  fast-forward merges only. For complex merges, use standard git merge.

Arguments:
  branch: The branch to merge
  --strategy: Merge strategy (not implemented)

Merge Types:
  1. Fast-forward: When the target branch is a direct descendant
  2. Manual merge needed: When branches have diverged

Examples:
  git-prolly merge feature-branch         # Merge feature branch
  git-prolly merge hotfix                 # Merge hotfix branch

Note: For complex merges, use 'git merge' instead."#)]
    Merge {
        #[arg(help = "Branch to merge")]
        branch: String,
        #[arg(long, help = "Merge strategy")]
        strategy: Option<String>,
    },

    /// Show repository statistics
    ///
    /// Basic Usage:
    ///   git-prolly stats [commit]
    ///
    /// Description:
    ///   Shows detailed statistics about the ProllyTree repository, including
    ///   tree depth, node count, key count, and storage efficiency metrics.
    ///
    /// Arguments:
    ///   commit: Commit to analyze (defaults to HEAD)
    ///
    /// Statistics Include:
    ///   - Dataset information
    ///   - Tree structure metrics
    ///   - Key-value counts
    ///   - Storage efficiency
    ///   - Branching factor
    ///
    /// Examples:
    ///   git-prolly stats                        # Stats for current HEAD
    ///   git-prolly stats abc123                 # Stats for specific commit
    ///   git-prolly stats main                   # Stats for branch
    Stats {
        #[arg(help = "Commit to analyze (defaults to HEAD)")]
        commit: Option<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path } => {
            handle_init(path)?;
        }
        Commands::Set { key, value } => {
            handle_set(key, value)?;
        }
        Commands::Get { key } => {
            handle_get(key)?;
        }
        Commands::Delete { key } => {
            handle_delete(key)?;
        }
        Commands::List { values, graph } => {
            handle_list(values, graph)?;
        }
        Commands::Status => {
            handle_status()?;
        }
        Commands::Commit { message } => {
            handle_commit(message)?;
        }
        Commands::Diff {
            from,
            to,
            format,
            keys,
        } => {
            handle_diff(from, to, format, keys)?;
        }
        Commands::Show { commit, keys_only } => {
            handle_show(commit, keys_only)?;
        }
        Commands::Merge { branch, strategy } => {
            handle_merge(branch, strategy)?;
        }
        Commands::Stats { commit } => {
            handle_stats(commit)?;
        }
    }

    Ok(())
}

fn handle_init(path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let target_path =
        path.unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    println!("Initializing ProllyTree KV store in {target_path:?}...");

    let _store = VersionedKvStore::<32>::init(&target_path)?;

    println!("✓ Initialized empty ProllyTree KV store");
    println!("✓ Git repository initialized");
    println!("✓ Ready to use!");

    Ok(())
}

fn handle_set(key: String, value: String) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let mut store = VersionedKvStore::<32>::open(&current_dir)?;

    store.insert(key.as_bytes().to_vec(), value.as_bytes().to_vec())?;

    println!("✓ Staged: {key} = \"{value}\"");
    println!("  (Use 'git prolly commit' to save changes)");

    Ok(())
}

fn handle_get(key: String) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let store = VersionedKvStore::<32>::open(&current_dir)?;

    match store.get(key.as_bytes()) {
        Some(value) => {
            println!("{}", String::from_utf8_lossy(&value));
        }
        None => {
            eprintln!("Key '{key}' not found");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn handle_delete(key: String) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let mut store = VersionedKvStore::<32>::open(&current_dir)?;

    if store.delete(key.as_bytes())? {
        println!("✓ Staged deletion: {key}");
        println!("  (Use 'git prolly commit' to save changes)");
    } else {
        eprintln!("Key '{key}' not found");
        std::process::exit(1);
    }

    Ok(())
}

fn handle_list(show_values: bool, show_graph: bool) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let mut store = VersionedKvStore::<32>::open(&current_dir)?;

    if show_graph {
        // Show the prolly tree structure
        store.tree_mut().print();
        return Ok(());
    }

    let keys = store.list_keys();

    if keys.is_empty() {
        println!("No keys found");
        return Ok(());
    }

    let mut sorted_keys = keys;
    sorted_keys.sort();

    for key in sorted_keys {
        let key_str = String::from_utf8_lossy(&key);

        if show_values {
            if let Some(value) = store.get(&key) {
                let value_str = String::from_utf8_lossy(&value);
                println!("{key_str} = \"{value_str}\"");
            } else {
                println!("{key_str} = <deleted>");
            }
        } else {
            println!("{key_str}");
        }
    }

    Ok(())
}

fn handle_status() -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let store = VersionedKvStore::<32>::open(&current_dir)?;

    let status = store.status();
    let current_branch = store.current_branch();

    println!("On branch {current_branch}");

    if status.is_empty() {
        println!("nothing to commit, working tree clean");
        return Ok(());
    }

    println!("Staged changes:");
    for (key, status_type) in status {
        let key_str = String::from_utf8_lossy(&key);
        let color = match status_type.as_str() {
            "added" => "\x1b[32m",    // Green
            "modified" => "\x1b[33m", // Yellow
            "deleted" => "\x1b[31m",  // Red
            _ => "",
        };
        println!("  {color}{status_type}: {key_str}\x1b[0m");
    }

    Ok(())
}

fn handle_commit(message: String) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let mut store = VersionedKvStore::<32>::open(&current_dir)?;

    let status = store.status();
    if status.is_empty() {
        println!("No staged changes to commit");
        return Ok(());
    }

    let commit_id = store.commit(&message)?;

    println!("✓ Committed: {commit_id}");
    println!("  Message: {message}");
    println!("  Changes: {} operations", status.len());

    // Show summary of changes
    for (key, status_type) in status {
        let key_str = String::from_utf8_lossy(&key);
        let symbol = match status_type.as_str() {
            "added" => "+",
            "modified" => "~",
            "deleted" => "-",
            _ => "?",
        };
        println!("    {symbol} {key_str}");
    }

    Ok(())
}

fn handle_diff(
    from: String,
    to: String,
    format: Option<String>,
    _keys: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let store = VersionedKvStore::<32>::open(&current_dir)?;
    let ops = GitOperations::new(store);

    let diffs = ops.diff(&from, &to)?;

    if diffs.is_empty() {
        println!("No differences found between {from} and {to}");
        return Ok(());
    }

    let format = format.unwrap_or_else(|| "compact".to_string());

    match format.as_str() {
        "compact" => {
            println!("Key-Value Changes ({from} -> {to}):");
            for diff in diffs {
                let key_str = String::from_utf8_lossy(&diff.key);
                match diff.operation {
                    DiffOperation::Added(value) => {
                        let value_str = String::from_utf8_lossy(&value);
                        println!("  \x1b[32m+ {key_str} = \"{value_str}\"\x1b[0m");
                    }
                    DiffOperation::Removed(value) => {
                        let value_str = String::from_utf8_lossy(&value);
                        println!("  \x1b[31m- {key_str} = \"{value_str}\"\x1b[0m");
                    }
                    DiffOperation::Modified { old, new } => {
                        let old_str = String::from_utf8_lossy(&old);
                        let new_str = String::from_utf8_lossy(&new);
                        println!("  \x1b[33m~ {key_str} = \"{old_str}\" -> \"{new_str}\"\x1b[0m");
                    }
                }
            }
        }
        "detailed" => {
            println!("Detailed Key-Value Changes ({from} -> {to}):");
            println!("═══════════════════════════════════════");
            for diff in diffs {
                let key_str = String::from_utf8_lossy(&diff.key);
                println!("\nKey: {key_str}");
                match diff.operation {
                    DiffOperation::Added(value) => {
                        let value_str = String::from_utf8_lossy(&value);
                        println!("  Status: \x1b[32mAdded\x1b[0m");
                        println!("  Value: \"{value_str}\"");
                    }
                    DiffOperation::Removed(value) => {
                        let value_str = String::from_utf8_lossy(&value);
                        println!("  Status: \x1b[31mRemoved\x1b[0m");
                        println!("  Previous Value: \"{value_str}\"");
                    }
                    DiffOperation::Modified { old, new } => {
                        let old_str = String::from_utf8_lossy(&old);
                        let new_str = String::from_utf8_lossy(&new);
                        println!("  Status: \x1b[33mModified\x1b[0m");
                        println!("  Old Value: \"{old_str}\"");
                        println!("  New Value: \"{new_str}\"");
                    }
                }
            }
        }
        "json" => {
            println!("{{");
            println!("  \"from\": \"{from}\",");
            println!("  \"to\": \"{to}\",");
            println!("  \"changes\": [");
            for (i, diff) in diffs.iter().enumerate() {
                let key_str = String::from_utf8_lossy(&diff.key);
                print!("    {{");
                print!("\"key\": \"{key_str}\", ");
                match &diff.operation {
                    DiffOperation::Added(value) => {
                        let value_str = String::from_utf8_lossy(value);
                        print!("\"operation\": \"added\", \"value\": \"{value_str}\"");
                    }
                    DiffOperation::Removed(value) => {
                        let value_str = String::from_utf8_lossy(value);
                        print!("\"operation\": \"removed\", \"value\": \"{value_str}\"");
                    }
                    DiffOperation::Modified { old, new } => {
                        let old_str = String::from_utf8_lossy(old);
                        let new_str = String::from_utf8_lossy(new);
                        print!(
                            "\"operation\": \"modified\", \"old\": \"{old_str}\", \"new\": \"{new_str}\""
                        );
                    }
                }
                print!("}}");
                if i < diffs.len() - 1 {
                    print!(",");
                }
                println!();
            }
            println!("  ]");
            println!("}}");
        }
        _ => {
            eprintln!("Unknown format: {format}. Use 'compact', 'detailed', or 'json'");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn handle_show(commit: String, keys_only: bool) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let store = VersionedKvStore::<32>::open(&current_dir)?;
    let ops = GitOperations::new(store);

    let details = ops.show(&commit)?;

    if keys_only {
        println!("Keys at commit {commit}:");
        for change in details.changes {
            let key_str = String::from_utf8_lossy(&change.key);
            println!("  {key_str}");
        }
    } else {
        println!("Commit: {} - {}", details.info.id, details.info.message);
        println!("Author: {}", details.info.author);
        println!(
            "Date: {}",
            chrono::DateTime::from_timestamp(details.info.timestamp, 0).unwrap_or_default()
        );
        println!();

        if details.changes.is_empty() {
            println!("No changes in this commit");
        } else {
            println!("Key-Value Changes:");
            for change in details.changes {
                let key_str = String::from_utf8_lossy(&change.key);
                match change.operation {
                    DiffOperation::Added(value) => {
                        let value_str = String::from_utf8_lossy(&value);
                        println!("  \x1b[32m+ {key_str} = \"{value_str}\"\x1b[0m");
                    }
                    DiffOperation::Removed(value) => {
                        let value_str = String::from_utf8_lossy(&value);
                        println!("  \x1b[31m- {key_str} = \"{value_str}\"\x1b[0m");
                    }
                    DiffOperation::Modified { old, new } => {
                        let old_str = String::from_utf8_lossy(&old);
                        let new_str = String::from_utf8_lossy(&new);
                        println!("  \x1b[33m~ {key_str} = \"{old_str}\" -> \"{new_str}\"\x1b[0m");
                    }
                }
            }
        }
    }

    Ok(())
}

fn handle_merge(
    branch: String,
    _strategy: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let store = VersionedKvStore::<32>::open(&current_dir)?;
    let mut ops = GitOperations::new(store);

    println!("Merging branch '{branch}'...");

    match ops.merge(&branch)? {
        MergeResult::FastForward(commit_id) => {
            println!("✓ Fast-forward merge completed");
            println!("  Updated to: {commit_id}");
        }
        MergeResult::ThreeWay(commit_id) => {
            println!("✓ Three-way merge completed");
            println!("  Merge commit: {commit_id}");
        }
        MergeResult::Conflict(conflicts) => {
            // Check if this is our "manual merge needed" indicator
            if conflicts.len() == 1 && conflicts[0].key == b"<merge>" {
                println!("⚠ Cannot automatically merge branches");
                println!("  The branches have diverged and require manual merging");
                println!("  Use 'git merge {branch}' to perform a manual merge");
            } else {
                println!("⚠ Merge conflicts detected:");
                for conflict in conflicts {
                    println!("  {conflict}");
                }
                println!("\nResolve conflicts and run 'git prolly commit' to complete the merge");
            }
        }
    }

    Ok(())
}

fn handle_stats(commit: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let store = VersionedKvStore::<32>::open(&current_dir)?;

    let target = commit.unwrap_or_else(|| "HEAD".to_string());

    println!("ProllyTree Statistics for {target}:");
    println!("═══════════════════════════════════");

    // Get dataset path (name)
    let dataset_path = current_dir.display().to_string();
    let dataset_name = current_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");
    println!("Dataset: {dataset_name} ({dataset_path})");

    // Get prolly tree depth
    let tree_depth = store.tree().depth();
    println!("Tree Depth: {tree_depth}");

    // Get basic stats
    let keys = store.list_keys();
    println!("Total Keys: {}", keys.len());

    // Get branch info
    println!("Current Branch: {}", store.current_branch());

    // Get commit history stats
    let history = store.log()?;
    println!("Total Commits: {}", history.len());

    if let Some(latest) = history.first() {
        let date = chrono::DateTime::from_timestamp(latest.timestamp, 0)
            .unwrap_or_default()
            .format("%Y-%m-%d %H:%M:%S");
        println!("Latest Commit: {date}");
    }

    Ok(())
}
