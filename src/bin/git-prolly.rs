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
    Init {
        #[arg(help = "Directory to initialize (defaults to current directory)")]
        path: Option<PathBuf>,
    },

    /// Set a key-value pair (stages the change)
    Set {
        #[arg(help = "Key to set")]
        key: String,
        #[arg(help = "Value to set")]
        value: String,
    },

    /// Get a value by key
    Get {
        #[arg(help = "Key to get")]
        key: String,
    },

    /// Delete a key (stages the change)
    Delete {
        #[arg(help = "Key to delete")]
        key: String,
    },

    /// List all keys
    List {
        #[arg(long, help = "Show values as well")]
        values: bool,
        #[arg(long, help = "Show prolly tree structure")]
        graph: bool,
    },

    /// Show staging area status
    Status,

    /// Commit staged changes
    Commit {
        #[arg(short, long, help = "Commit message")]
        message: String,
    },

    /// Show KV-aware diff between commits
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

    /// Show KV state at specific commit
    Show {
        #[arg(help = "Commit to show")]
        commit: String,
        #[arg(long, help = "Show only keys")]
        keys_only: bool,
    },

    /// KV-aware log with summaries
    Log {
        #[arg(long, help = "Show KV change summary")]
        kv_summary: bool,
        #[arg(long, help = "Filter by key pattern")]
        keys: Option<String>,
        #[arg(long, help = "Limit number of commits")]
        limit: Option<usize>,
    },

    /// List all branches
    Branch,

    /// Switch to a branch or commit
    Checkout {
        #[arg(help = "Branch or commit to checkout")]
        target: String,
        #[arg(short = 'b', long = "branch", help = "Create a new branch from current branch")]
        create_branch: bool,
    },

    /// Merge another branch
    Merge {
        #[arg(help = "Branch to merge")]
        branch: String,
        #[arg(long, help = "Merge strategy")]
        strategy: Option<String>,
    },

    /// Revert a commit
    Revert {
        #[arg(help = "Commit to revert")]
        commit: String,
    },

    /// Show repository statistics
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
        Commands::Log {
            kv_summary,
            keys,
            limit,
        } => {
            handle_log(kv_summary, keys, limit)?;
        }
        Commands::Branch => {
            handle_branch()?;
        }
        Commands::Checkout { target, create_branch } => {
            handle_checkout(target, create_branch)?;
        }
        Commands::Merge { branch, strategy } => {
            handle_merge(branch, strategy)?;
        }
        Commands::Revert { commit } => {
            handle_revert(commit)?;
        }
        Commands::Stats { commit } => {
            handle_stats(commit)?;
        }
    }

    Ok(())
}

fn handle_init(path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let target_path = path.unwrap_or_else(|| env::current_dir().unwrap());

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

    // Clone the strings before moving them into insert
    let key_display = key.clone();
    let value_display = value.clone();

    store.insert(key.into_bytes(), value.into_bytes())?;

    println!("✓ Staged: {key_display} = \"{value_display}\"");
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

fn handle_log(
    kv_summary: bool,
    _keys: Option<String>,
    limit: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let store = VersionedKvStore::<32>::open(&current_dir)?;

    let mut history = store.log()?;

    if let Some(limit) = limit {
        history.truncate(limit);
    }

    // Check current branch for the first commit (HEAD)
    let current_branch = store.current_branch();
    let head_commit_id = store.git_repo().head_id().ok();

    for (index, commit) in history.iter().enumerate() {
        let date = chrono::DateTime::from_timestamp(commit.timestamp, 0).unwrap_or_default();

        // Format like git log: "Wed Jul 16 22:27:36 2025 -0700"
        let formatted_date = date.format("%a %b %d %H:%M:%S %Y %z");

        // Add branch reference for HEAD commit
        let branch_ref = if index == 0
            && head_commit_id.as_ref().map(|id| id.as_ref()) == Some(commit.id.as_ref())
        {
            format!(" (HEAD -> {current_branch})")
        } else {
            String::new()
        };

        if kv_summary {
            // Get changes for this commit - create a new store instance
            let ops_store = VersionedKvStore::<32>::open(&current_dir)?;
            let ops = GitOperations::new(ops_store);
            let changes = match ops.show(&commit.id.to_string()) {
                Ok(details) => details.changes,
                Err(_) => vec![],
            };

            let added = changes
                .iter()
                .filter(|c| matches!(c.operation, DiffOperation::Added(_)))
                .count();
            let removed = changes
                .iter()
                .filter(|c| matches!(c.operation, DiffOperation::Removed(_)))
                .count();
            let modified = changes
                .iter()
                .filter(|c| matches!(c.operation, DiffOperation::Modified { .. }))
                .count();

            println!("commit {}{}", commit.id, branch_ref);
            println!("Author: {}", commit.author);
            println!("Date:   {formatted_date}");
            println!();
            println!(
                "    {} (+{} ~{} -{})",
                commit.message, added, modified, removed
            );
            println!();
        } else {
            println!("commit {}{}", commit.id, branch_ref);
            println!("Author: {}", commit.author);
            println!("Date:   {formatted_date}");
            println!();
            println!("    {}", commit.message);
            println!();
        }
    }

    Ok(())
}

fn handle_branch() -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let store = VersionedKvStore::<32>::open(&current_dir)?;

    let branches = store.list_branches()?;
    let current_branch = store.current_branch();

    if branches.is_empty() {
        println!("No branches found");
        return Ok(());
    }

    for branch in branches {
        if branch == current_branch {
            println!("* {branch}");
        } else {
            println!("  {branch}");
        }
    }

    Ok(())
}

fn handle_checkout(target: String, create_branch: bool) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let mut store = VersionedKvStore::<32>::open(&current_dir)?;

    if create_branch {
        store.create_branch(&target)?;
        println!("✓ Created and switched to new branch: {target}");
    } else {
        store.checkout(&target)?;
        println!("✓ Switched to: {target}");
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
                println!("  Use 'git merge {}' to perform a manual merge", branch);
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

fn handle_revert(commit: String) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let store = VersionedKvStore::<32>::open(&current_dir)?;
    let mut ops = GitOperations::new(store);

    ops.revert(&commit)?;

    println!("✓ Reverted commit: {commit}");

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
