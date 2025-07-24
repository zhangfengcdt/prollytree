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
#[cfg(feature = "sql")]
use gluesql_core::{executor::Payload, prelude::Glue};
use prollytree::git::{DiffOperation, GitOperations, GitVersionedKvStore, MergeResult};
#[cfg(feature = "sql")]
use prollytree::sql::ProllyStorage;
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
        #[arg(help = "Commit to show (defaults to HEAD)")]
        commit: Option<String>,
        #[arg(long, help = "Show only keys")]
        keys_only: bool,
    },

    /// Merge another branch
    Merge {
        #[arg(help = "Branch to merge")]
        branch: String,
        #[arg(long, help = "Merge strategy")]
        strategy: Option<String>,
    },

    /// Show KV store statistics
    Stats {
        #[arg(help = "Commit to analyze (defaults to HEAD)")]
        commit: Option<String>,
    },

    /// Execute SQL queries against the ProllyTree dataset
    #[cfg(feature = "sql")]
    Sql {
        #[arg(help = "SQL query to execute")]
        query: Option<String>,
        #[arg(short, long, help = "Execute query from file")]
        file: Option<PathBuf>,
        #[arg(short = 'o', long, help = "Output format (table, json, csv)")]
        format: Option<String>,
        #[arg(short, long, help = "Start interactive SQL shell")]
        interactive: bool,
        #[arg(long, help = "Show detailed error messages")]
        verbose: bool,
    },

    /// Clear all tree nodes, staging changes, and git blobs for the current dataset
    Clear {
        #[arg(long, help = "Confirm the destructive operation")]
        confirm: bool,
        #[arg(long, help = "Keep git history but clear tree data")]
        keep_history: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    #[cfg(feature = "sql")]
    if let Commands::Sql {
        query,
        file,
        format,
        interactive,
        verbose,
    } = &cli.command
    {
        // Create a tokio runtime for SQL commands
        let rt = tokio::runtime::Runtime::new()?;
        return rt.block_on(handle_sql(
            query.clone(),
            file.clone(),
            format.clone(),
            *interactive,
            *verbose,
        ));
    }

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
        Commands::Clear {
            confirm,
            keep_history,
        } => {
            handle_clear(confirm, keep_history)?;
        }
        #[cfg(feature = "sql")]
        Commands::Sql { .. } => {
            // Handled above
            unreachable!()
        }
    }

    Ok(())
}

fn handle_init(path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let target_path =
        path.unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    println!("Initializing ProllyTree KV store in {target_path:?}...");

    let _store = GitVersionedKvStore::<32>::init(&target_path)?;

    println!("✓ Initialized empty ProllyTree KV store");
    println!("✓ Git repository initialized");
    println!("✓ Ready to use!");

    Ok(())
}

fn handle_set(key: String, value: String) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let mut store = GitVersionedKvStore::<32>::open(&current_dir)?;

    store.insert(key.as_bytes().to_vec(), value.as_bytes().to_vec())?;

    println!("✓ Staged: {key} = \"{value}\"");
    println!("  (Use 'git prolly commit' to save changes)");

    Ok(())
}

fn handle_get(key: String) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let store = GitVersionedKvStore::<32>::open(&current_dir)?;

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
    let mut store = GitVersionedKvStore::<32>::open(&current_dir)?;

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
    let mut store = GitVersionedKvStore::<32>::open(&current_dir)?;

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
    let store = GitVersionedKvStore::<32>::open(&current_dir)?;

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
    let mut store = GitVersionedKvStore::<32>::open(&current_dir)?;

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
    let store = GitVersionedKvStore::<32>::open(&current_dir)?;
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

fn handle_show(commit: Option<String>, keys_only: bool) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    let store = GitVersionedKvStore::<32>::open(&current_dir)?;
    let ops = GitOperations::new(store);

    let commit_ref = commit.unwrap_or_else(|| "HEAD".to_string());
    let details = ops.show(&commit_ref)?;

    if keys_only {
        println!("Keys at commit {commit_ref}:");
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
    let store = GitVersionedKvStore::<32>::open(&current_dir)?;
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
            // Check if this is our "guide merge needed" indicator
            if conflicts.len() == 1 && conflicts[0].key == b"<merge>" {
                println!("⚠ Cannot automatically merge branches");
                println!("  The branches have diverged and require guide merging");
                println!("  Use 'git merge {branch}' to perform a guide merge");
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
    let store = GitVersionedKvStore::<32>::open(&current_dir)?;

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

fn handle_clear(confirm: bool, keep_history: bool) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;

    // Safety check - require confirmation for destructive operation
    if !confirm {
        eprintln!("⚠ This will permanently delete all tree data and staging changes!");
        eprintln!("  Use --confirm to proceed with this destructive operation");
        eprintln!("  Use --keep-history to preserve git history");
        std::process::exit(1);
    }

    println!("🧹 Clearing ProllyTree dataset...");

    // Open the store to get access to internal structures
    let mut store = GitVersionedKvStore::<32>::open(&current_dir)?;

    // Clear staging area first
    println!("  ↳ Clearing staging changes...");
    let status = store.status();
    if !status.is_empty() {
        // Reset staging area by recreating the store
        store = GitVersionedKvStore::<32>::open(&current_dir)?;
        println!("    ✓ Cleared {} staged changes", status.len());
    } else {
        println!("    ✓ No staged changes to clear");
    }

    // Clear the tree data
    println!("  ↳ Clearing tree nodes and data...");

    // Get current key count before clearing
    let keys = store.list_keys();
    let key_count = keys.len();

    // Clear all keys from the tree
    for key in keys {
        store.delete(&key)?;
    }

    // Clear the staging area to make sure deletions are staged
    // The staging should already contain the deletions from above

    println!("    ✓ Cleared {key_count} keys from tree");

    // Clear mapping files and node storage
    println!("  ↳ Clearing node mappings...");

    // Get the dataset directory structure
    let git_prolly_dir = current_dir.join(".git-prolly");
    let staging_file = git_prolly_dir.join("staging.json");
    let mapping_file = git_prolly_dir.join("mapping.json");

    // Remove staging file
    if staging_file.exists() {
        std::fs::remove_file(&staging_file)?;
        println!("    ✓ Removed staging file");
    }

    // Clear or remove mapping file
    if mapping_file.exists() {
        if keep_history {
            // Just clear the contents but keep the file structure
            std::fs::write(&mapping_file, "{}")?;
            println!("    ✓ Cleared mapping file contents");
        } else {
            std::fs::remove_file(&mapping_file)?;
            println!("    ✓ Removed mapping file");
        }
    }

    // Clear git blobs if not keeping history
    if !keep_history {
        println!("  ↳ Clearing git blob objects...");

        // Run git gc to clean up unreferenced objects
        let git_dir = current_dir.join(".git");
        if git_dir.exists() {
            // Remove all prolly-related refs and objects
            let objects_dir = git_dir.join("objects");
            if objects_dir.exists() {
                // Use git prune to remove unreachable objects
                use std::process::Command;

                let output = Command::new("git")
                    .args(["prune", "--expire=now"])
                    .current_dir(&current_dir)
                    .output();

                match output {
                    Ok(result) if result.status.success() => {
                        println!("    ✓ Pruned unreachable git objects");
                    }
                    _ => {
                        println!("    ⚠ Could not prune git objects (git prune failed)");
                    }
                }

                // Also run git gc aggressively
                let gc_output = Command::new("git")
                    .args(["gc", "--aggressive", "--prune=now"])
                    .current_dir(&current_dir)
                    .output();

                match gc_output {
                    Ok(result) if result.status.success() => {
                        println!("    ✓ Cleaned up git repository");
                    }
                    _ => {
                        println!("    ⚠ Could not clean up git repository (git gc failed)");
                    }
                }
            }
        }
    } else {
        println!("  ↳ Keeping git history (--keep-history specified)");
    }

    // Reinitialize empty tree structure
    println!("  ↳ Reinitializing empty tree structure...");

    // Commit the empty state if keeping history
    if keep_history {
        // Make sure the deletions are committed to create an empty state
        let status = store.status();
        if !status.is_empty() {
            let commit_id = store.commit("Clear all data")?;
            println!("    ✓ Committed empty state: {commit_id}");
        } else {
            println!("    ✓ Tree already empty, no commit needed");
        }
    }

    println!("✅ Successfully cleared ProllyTree dataset!");

    if keep_history {
        println!(
            "   Git history preserved - use 'git prolly show <commit>' to view previous states"
        );
    } else {
        println!("   All data permanently removed - repository is now clean");
    }

    println!("   Ready for new data - use 'git prolly set <key> <value>' to add data");

    Ok(())
}

#[cfg(feature = "sql")]
async fn handle_sql(
    query: Option<String>,
    file: Option<PathBuf>,
    format: Option<String>,
    interactive: bool,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;

    // Open the ProllyTree storage
    let storage = ProllyStorage::<32>::open(&current_dir).map_err(|e| {
        if verbose {
            format!("Failed to open ProllyTree storage: {e}")
        } else {
            "Failed to open dataset. Make sure you're in a git-prolly directory.".to_string()
        }
    })?;

    let mut glue = Glue::new(storage);
    let output_format = format.unwrap_or_else(|| "table".to_string());

    if interactive {
        // Start interactive SQL shell
        println!("🌟 ProllyTree SQL Interactive Shell");
        println!("====================================");
        println!("Type 'exit' or 'quit' to exit");
        println!("Type 'help' for available commands\n");

        loop {
            print!("prolly-sql> ");
            std::io::Write::flush(&mut std::io::stdout())?;

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            match input.to_lowercase().as_str() {
                "exit" | "quit" => {
                    println!("Goodbye!");
                    break;
                }
                "help" => {
                    print_help();
                    continue;
                }
                _ => {}
            }

            match execute_query(&mut glue, input, &output_format, verbose).await {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Error: {e}");
                    if verbose {
                        eprintln!("Query: {input}");
                    }
                }
            }
            println!();
        }
    } else if let Some(query_str) = query {
        // Execute single query
        execute_query(&mut glue, &query_str, &output_format, verbose).await?;
    } else if let Some(file_path) = file {
        // Execute query from file
        let query_str = std::fs::read_to_string(file_path)?;
        execute_query(&mut glue, &query_str, &output_format, verbose).await?;
    } else {
        eprintln!("Error: Must provide either a query, file, or use interactive mode");
        eprintln!("Usage:");
        eprintln!("  git prolly sql \"SELECT * FROM table\"");
        eprintln!("  git prolly sql --file query.sql");
        eprintln!("  git prolly sql --interactive");
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(feature = "sql")]
async fn execute_query(
    glue: &mut Glue<ProllyStorage<32>>,
    query: &str,
    format: &str,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = std::time::Instant::now();

    let result = glue.execute(query).await.map_err(|e| {
        if verbose {
            format!("SQL execution error: {e}")
        } else {
            format!("Query failed: {e}")
        }
    })?;

    let execution_time = start_time.elapsed();

    if result.is_empty() {
        println!("Query executed successfully (no results)");
        if verbose {
            println!("Execution time: {execution_time:?}");
        }
        return Ok(());
    }

    for payload in result {
        format_payload(&payload, format)?;
    }

    if verbose {
        println!("\nExecution time: {execution_time:?}");
    }

    Ok(())
}

#[cfg(feature = "sql")]
fn format_payload(payload: &Payload, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    match payload {
        Payload::Select { labels, rows } => {
            if rows.is_empty() {
                println!("(No results)");
                return Ok(());
            }

            match format {
                "table" => {
                    format_table(labels, rows);
                }
                "json" => {
                    format_json(labels, rows)?;
                }
                "csv" => {
                    format_csv(labels, rows);
                }
                _ => {
                    eprintln!("Unknown format: {format}. Supported: table, json, csv");
                    std::process::exit(1);
                }
            }
        }
        Payload::Insert(count) => {
            println!("✓ Inserted {count} rows");
        }
        Payload::Update(count) => {
            println!("✓ Updated {count} rows");
        }
        Payload::Delete(count) => {
            println!("✓ Deleted {count} rows");
        }
        Payload::Create => {
            println!("✓ Table created successfully");
        }
        Payload::DropTable => {
            println!("✓ Table dropped successfully");
        }
        _ => {
            println!("✓ Operation completed successfully");
        }
    }

    Ok(())
}

#[cfg(feature = "sql")]
fn format_table(labels: &[String], rows: &[Vec<gluesql_core::data::Value>]) {
    // Calculate column widths
    let mut widths: Vec<usize> = labels.iter().map(|l| l.len()).collect();

    for row in rows {
        for (i, value) in row.iter().enumerate() {
            if i < widths.len() {
                let value_str = format!("{value:?}");
                widths[i] = widths[i].max(value_str.len());
            }
        }
    }

    // Print header
    print!("│");
    for (i, label) in labels.iter().enumerate() {
        print!(" {:width$} │", label, width = widths[i]);
    }
    println!();

    // Print separator
    print!("├");
    for width in &widths {
        print!("{:─>width$}┼", "", width = width + 2);
    }
    println!("┤");

    // Print rows
    for row in rows {
        print!("│");
        for (i, value) in row.iter().enumerate() {
            if i < widths.len() {
                let value_str = format!("{value:?}");
                print!(" {:width$} │", value_str, width = widths[i]);
            }
        }
        println!();
    }
}

#[cfg(feature = "sql")]
fn format_json(
    labels: &[String],
    rows: &[Vec<gluesql_core::data::Value>],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut json_rows = Vec::new();

    for row in rows {
        let mut json_row = serde_json::Map::new();
        for (i, value) in row.iter().enumerate() {
            if i < labels.len() {
                let json_value = match value {
                    gluesql_core::data::Value::Bool(b) => serde_json::Value::Bool(*b),
                    gluesql_core::data::Value::I8(n) => serde_json::Value::Number((*n).into()),
                    gluesql_core::data::Value::I16(n) => serde_json::Value::Number((*n).into()),
                    gluesql_core::data::Value::I32(n) => serde_json::Value::Number((*n).into()),
                    gluesql_core::data::Value::I64(n) => serde_json::Value::Number((*n).into()),
                    gluesql_core::data::Value::I128(n) => serde_json::Value::String(n.to_string()),
                    gluesql_core::data::Value::U8(n) => serde_json::Value::Number((*n).into()),
                    gluesql_core::data::Value::U16(n) => serde_json::Value::Number((*n).into()),
                    gluesql_core::data::Value::U32(n) => serde_json::Value::Number((*n).into()),
                    gluesql_core::data::Value::U64(n) => serde_json::Value::Number((*n).into()),
                    gluesql_core::data::Value::U128(n) => serde_json::Value::String(n.to_string()),
                    gluesql_core::data::Value::F32(f) => serde_json::Value::Number(
                        serde_json::Number::from_f64(*f as f64).unwrap_or_else(|| 0.into()),
                    ),
                    gluesql_core::data::Value::F64(f) => serde_json::Value::Number(
                        serde_json::Number::from_f64(*f).unwrap_or_else(|| 0.into()),
                    ),
                    gluesql_core::data::Value::Str(s) => serde_json::Value::String(s.clone()),
                    gluesql_core::data::Value::Null => serde_json::Value::Null,
                    _ => serde_json::Value::String(format!("{value:?}")),
                };
                json_row.insert(labels[i].clone(), json_value);
            }
        }
        json_rows.push(serde_json::Value::Object(json_row));
    }

    let output = serde_json::to_string_pretty(&json_rows)?;
    println!("{output}");

    Ok(())
}

#[cfg(feature = "sql")]
fn format_csv(labels: &[String], rows: &[Vec<gluesql_core::data::Value>]) {
    // Print header
    println!("{}", labels.join(","));

    // Print rows
    for row in rows {
        let row_strs: Vec<String> = row
            .iter()
            .map(|v| {
                let s = format!("{v:?}");
                // Simple CSV escaping - wrap in quotes if contains comma
                if s.contains(',') {
                    format!("\"{}\"", s.replace('"', "\"\""))
                } else {
                    s
                }
            })
            .collect();
        println!("{}", row_strs.join(","));
    }
}

#[cfg(feature = "sql")]
fn print_help() {
    println!("ProllyTree SQL Commands:");
    println!("  SQL statements: CREATE TABLE, INSERT, SELECT, UPDATE, DELETE");
    println!("  Special commands:");
    println!("    help     - Show this help message");
    println!("    exit     - Exit the SQL shell");
    println!("    quit     - Exit the SQL shell");
    println!();
    println!("Examples:");
    println!("  CREATE TABLE users (id INTEGER, name TEXT, email TEXT);");
    println!("  INSERT INTO users VALUES (1, 'Alice', 'alice@example.com');");
    println!("  SELECT * FROM users;");
    println!("  SELECT name FROM users WHERE id = 1;");
    println!();
    println!("Note: Data is stored in the ProllyTree and versioned with Git.");
}
