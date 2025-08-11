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

use clap::Parser;
use prollytree::git::versioned_store::HistoricalAccess;
use prollytree::git::{CommitInfo, DiffOperation, GitVersionedKvStore, KvDiff};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "prolly-ui")]
#[command(about = "Generate static HTML visualization for git-prolly repositories")]
#[command(version = "0.3.0")]
struct Cli {
    /// Path to the main git repository containing datasets as subdirectories
    #[arg(help = "Repository path (defaults to current directory)")]
    repo_path: Option<PathBuf>,

    /// Output HTML file path
    #[arg(short, long, default_value = "prolly-ui.html")]
    output: PathBuf,

    /// Specify which subdirectories are datasets (if not specified, all subdirectories with prolly data will be used)
    #[arg(short = 'd', long = "dataset", value_name = "NAME")]
    datasets: Vec<String>,

    /// Filter to specific branches (if not specified, all branches will be processed)
    #[arg(short = 'b', long = "branch", value_name = "BRANCH")]
    branches: Vec<String>,
}

#[derive(Debug, Clone)]
struct BranchInfo {
    name: String,
    commits: Vec<CommitInfo>,
    current: bool,
}

#[derive(Debug, Clone)]
struct DatasetInfo {
    name: String,
    path: PathBuf,
    branches: Vec<BranchInfo>,
    commit_details: HashMap<String, CommitDiff>,
}

#[derive(Debug, Clone)]
struct RepositoryData {
    path: PathBuf,
    datasets: Vec<DatasetInfo>,
    git_branches: Vec<GitBranchInfo>,
    _git_commits: HashMap<String, GitCommitInfo>,
}

#[derive(Debug, Clone)]
struct GitBranchInfo {
    name: String,
    commits: Vec<GitCommitInfo>,
    current: bool,
}

#[derive(Debug, Clone)]
struct GitCommitInfo {
    id: String,
    author: String,
    message: String,
    timestamp: i64,
    dataset_changes: HashMap<String, Vec<KvDiff>>,
}

#[derive(Debug, Clone)]
struct CommitDiff {
    info: CommitInfo,
    changes: Vec<KvDiff>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Process main repository
    let repo_path = cli.repo_path.unwrap_or_else(|| PathBuf::from("."));

    println!("📊 Processing repository: {}", repo_path.display());

    // Discover or use specified datasets
    let dataset_names = if cli.datasets.is_empty() {
        // Auto-discover datasets and format them properly
        let discovered = discover_datasets(&repo_path)?;
        // Convert discovered dataset names to proper format with full paths
        discovered
            .into_iter()
            .map(|name| {
                // Capitalize first letter for the tag
                let tag = name
                    .chars()
                    .enumerate()
                    .map(|(i, c)| {
                        if i == 0 {
                            c.to_uppercase().to_string()
                        } else {
                            c.to_string()
                        }
                    })
                    .collect::<String>();
                // Return in "Tag:Path" format
                format!("{}:{}", tag, repo_path.join(&name).display())
            })
            .collect()
    } else {
        cli.datasets
    };

    if dataset_names.is_empty() {
        return Err("No datasets found in the repository".into());
    }

    println!(
        "📁 Found {} dataset(s): {:?}",
        dataset_names.len(),
        dataset_names
    );

    // Process each dataset subdirectory
    let mut datasets = Vec::new();
    for dataset_name in dataset_names {
        // Parse dataset name which can be either:
        // 1. Simple name (subdirectory of repo_path)
        // 2. "Tag:Path" format where Path is absolute
        let (tag, dataset_path) = if dataset_name.contains(':') {
            let parts: Vec<&str> = dataset_name.splitn(2, ':').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), PathBuf::from(parts[1]))
            } else {
                (dataset_name.clone(), repo_path.join(&dataset_name))
            }
        } else {
            (dataset_name.clone(), repo_path.join(&dataset_name))
        };

        if !dataset_path.exists() {
            eprintln!(
                "⚠️  Dataset directory does not exist: {}",
                dataset_path.display()
            );
            continue;
        }

        println!(
            "  📊 Processing dataset '{}': {}",
            tag,
            dataset_path.display()
        );
        match process_dataset(tag.clone(), &dataset_path, &cli.branches) {
            Ok(dataset) => datasets.push(dataset),
            Err(e) => eprintln!("  ⚠️  Failed to process dataset '{tag}': {e}"),
        }
    }

    if datasets.is_empty() {
        return Err("No valid datasets could be processed".into());
    }

    // Process actual git repository
    println!("🔍 Processing git repository structure...");

    // Create a mapping from dataset tags to directory names for git processing
    let dataset_mappings: Vec<(String, String)> = datasets
        .iter()
        .filter_map(|dataset| {
            // Extract the directory name from the dataset's actual path
            if let Some(dir_name) = dataset.path.file_name() {
                if let Some(dir_str) = dir_name.to_str() {
                    return Some((dataset.name.clone(), dir_str.to_string()));
                }
            }
            None
        })
        .collect();

    let (git_branches, git_commits) =
        process_git_repository(&repo_path, &dataset_mappings, &datasets, &cli.branches)?;

    let repository_data = RepositoryData {
        path: repo_path,
        datasets,
        git_branches,
        _git_commits: git_commits,
    };

    // Generate HTML
    println!("🎨 Generating HTML visualization...");
    let html = generate_html(&repository_data)?;

    // Write to file
    fs::write(&cli.output, html)?;
    println!("✅ HTML visualization saved to: {}", cli.output.display());

    Ok(())
}

fn discover_datasets(repo_path: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut datasets = Vec::new();

    for entry in fs::read_dir(repo_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Check if this directory contains prolly data files
            let has_prolly_config = path.join("prolly_config_tree_config").exists();
            let has_hash_mappings = path.join("prolly_hash_mappings").exists();

            if has_prolly_config || has_hash_mappings {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    // Skip hidden directories and git directory
                    if !name.starts_with('.') {
                        datasets.push(name.to_string());
                    }
                }
            }
        }
    }

    datasets.sort();
    Ok(datasets)
}

type GitRepositoryResult =
    Result<(Vec<GitBranchInfo>, HashMap<String, GitCommitInfo>), Box<dyn std::error::Error>>;

fn process_git_repository(
    repo_path: &Path,
    dataset_mappings: &[(String, String)],
    datasets: &[DatasetInfo],
    branch_filter: &[String],
) -> GitRepositoryResult {
    use std::process::Command;

    // Use the provided dataset mappings instead of discovering them
    let _datasets: Vec<String> = dataset_mappings
        .iter()
        .map(|(_, dir)| dir.clone())
        .collect();

    // Get all branches
    let branch_output = Command::new("git")
        .args(["-C", &repo_path.to_string_lossy(), "branch", "-a"])
        .output()?;

    if !branch_output.status.success() {
        return Err("Failed to list git branches".into());
    }

    let branch_list = String::from_utf8(branch_output.stdout)?;
    let mut git_branches = Vec::new();
    let mut all_commits = HashMap::new();
    let mut current_branch = String::new();

    // Parse branches
    let all_branch_names: Vec<String> = branch_list
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('*') {
                current_branch = trimmed[2..].to_string();
                Some(current_branch.clone())
            } else if !trimmed.is_empty() && !trimmed.contains("remotes/") {
                Some(trimmed.to_string())
            } else {
                None
            }
        })
        .collect();

    // Filter branches if specified
    let branch_names: Vec<String> = if branch_filter.is_empty() {
        all_branch_names
    } else {
        all_branch_names
            .into_iter()
            .filter(|branch| branch_filter.contains(branch))
            .collect()
    };

    // Store original branch to restore later
    let original_branch = current_branch.clone();

    // First, get all commits from all branches to build a complete picture
    let all_log_output = Command::new("git")
        .args([
            "-C",
            &repo_path.to_string_lossy(),
            "log",
            "--all",
            "--format=%H|%an|%s|%ct",
            // Get ALL commits, no limit
        ])
        .output()?;

    if !all_log_output.status.success() {
        return Err("Failed to get git log --all".into());
    }

    let all_log_text = String::from_utf8(all_log_output.stdout)?;
    let all_commit_lines: Vec<&str> = all_log_text.lines().collect();

    // Parse all commits into a map
    for (i, line) in all_commit_lines.iter().enumerate() {
        if let Some((id, rest)) = line.split_once('|') {
            if let Some((author, rest)) = rest.split_once('|') {
                if let Some((message, timestamp_str)) = rest.split_once('|') {
                    if let Ok(timestamp) = timestamp_str.parse::<i64>() {
                        // Extract data for ALL commits now that git-prolly show is faster
                        let should_extract_data = true; // Extract data for ALL commits
                        let dataset_changes = calculate_commit_changes(
                            repo_path,
                            id,
                            if i + 1 < all_commit_lines.len() {
                                Some(all_commit_lines[i + 1].split('|').next().unwrap_or(""))
                            } else {
                                None
                            },
                            dataset_mappings,
                            datasets,
                            should_extract_data,
                        )
                        .unwrap_or_default();

                        let commit = GitCommitInfo {
                            id: id.to_string(),
                            author: author.to_string(),
                            message: message.to_string(),
                            timestamp,
                            dataset_changes,
                        };

                        all_commits.insert(id.to_string(), commit);
                    }
                }
            }
        }
    }

    // Now process each branch to get its specific commit ordering
    for branch_name in branch_names {
        println!("🔍 Processing branch: {branch_name}");
        let is_current = branch_name == current_branch;

        // Checkout the branch
        Command::new("git")
            .args(["-C", &repo_path.to_string_lossy(), "checkout", &branch_name])
            .output()?;

        // Get commits for this specific branch, showing most recent first
        // This approach shows the commits in reverse chronological order for this branch
        let branch_log_output = Command::new("git")
            .args([
                "-C",
                &repo_path.to_string_lossy(),
                "log",
                &branch_name, // Specify the branch explicitly
                "--format=%H",
                // Show ALL commits for this branch, no limit
            ])
            .output()?;

        if !branch_log_output.status.success() {
            continue; // Skip branches we can't read
        }

        let branch_log_text = String::from_utf8(branch_log_output.stdout)?;
        let mut branch_commits = Vec::new();

        // Build commits for this branch using the pre-parsed commit data
        for line in branch_log_text.lines() {
            let commit_id = line.trim();
            if let Some(commit) = all_commits.get(commit_id) {
                branch_commits.push(commit.clone());
                println!(
                    "    ✓ Branch {}: Added commit {}",
                    branch_name,
                    &commit_id[..8]
                );
            }
        }

        println!(
            "    📊 Branch {} has {} commits",
            branch_name,
            branch_commits.len()
        );

        git_branches.push(GitBranchInfo {
            name: branch_name,
            commits: branch_commits,
            current: is_current,
        });
    }

    // Restore original branch
    Command::new("git")
        .args([
            "-C",
            &repo_path.to_string_lossy(),
            "checkout",
            &original_branch,
        ])
        .output()?;

    Ok((git_branches, all_commits))
}

fn calculate_commit_changes(
    repo_path: &Path,
    commit_id: &str,
    parent_commit_id: Option<&str>,
    dataset_mappings: &[(String, String)],
    datasets: &[DatasetInfo],
    should_extract_data: bool,
) -> Result<HashMap<String, Vec<KvDiff>>, Box<dyn std::error::Error>> {
    use std::process::Command;

    let mut dataset_changes = HashMap::new();

    // Use git show to see what files changed in this commit
    let show_output = if let Some(parent_id) = parent_commit_id {
        Command::new("git")
            .args([
                "-C",
                &repo_path.to_string_lossy(),
                "diff",
                "--name-only",
                parent_id,
                commit_id,
            ])
            .output()?
    } else {
        Command::new("git")
            .args([
                "-C",
                &repo_path.to_string_lossy(),
                "show",
                "--name-only",
                "--format=",
                commit_id,
            ])
            .output()?
    };

    if !show_output.status.success() {
        return Ok(HashMap::new());
    }

    let changed_files = String::from_utf8(show_output.stdout)?;
    println!(
        "  📋 Changed files in commit {}: {:?}",
        &commit_id[..8],
        changed_files.lines().collect::<Vec<_>>()
    );

    let affected_datasets: HashSet<String> = changed_files
        .lines()
        .filter_map(|file_path| {
            // Check if this file belongs to a dataset using the mapping
            for (tag, dir) in dataset_mappings {
                if file_path.starts_with(&format!("{dir}/")) {
                    return Some(tag.clone());
                }
            }
            None
        })
        .collect();

    println!("  📊 Affected datasets from git files: {affected_datasets:?}");

    // Check ALL datasets for prolly data changes in this commit, not just the ones with file changes
    for (dataset_tag, _dataset_dir) in dataset_mappings {
        // Check if this dataset had git file changes (for informational purposes)
        let _has_git_changes = affected_datasets.contains(dataset_tag);

        // Always try to get prolly data changes for each dataset

        if should_extract_data {
            // Always try to extract prolly-tree data changes for every dataset
            // Find the actual dataset path
            if let Some(dataset_info) = datasets.iter().find(|d| &d.name == dataset_tag) {
                // Always try to extract prolly-tree data changes for every dataset
                if let Ok(changes) = get_actual_prolly_changes_with_path(
                    commit_id,
                    parent_commit_id,
                    dataset_tag,
                    &dataset_info.path,
                ) {
                    if !changes.is_empty() {
                        dataset_changes.insert(dataset_tag.clone(), changes);
                    }
                }
            }
        }
    }

    Ok(dataset_changes)
}

fn get_prolly_changes_from_commit(
    dataset_path: &Path,
    commit_id: &str,
) -> Result<Vec<KvDiff>, Box<dyn std::error::Error>> {
    use std::process::Command;

    let git_prolly_path = std::env::current_exe()
        .map(|p| p.parent().unwrap().join("git-prolly"))
        .unwrap_or_else(|_| PathBuf::from("git-prolly"));

    // First try the commit ID directly (in case it exists in this dataset)
    let output = Command::new(&git_prolly_path)
        .args(["show", commit_id])
        .current_dir(dataset_path)
        .output()?;

    if output.status.success() {
        let stdout_str = String::from_utf8(output.stdout)?;
        // Parse the output from git-prolly show
        return parse_prolly_show_output(&stdout_str);
    }

    // If the exact commit doesn't exist, try to find recent commits with changes
    // This handles the case where the main repo commit doesn't exist in the dataset repo
    let log_output = Command::new(&git_prolly_path)
        .args(["log", "--limit", "20"])
        .current_dir(dataset_path)
        .output()?;

    if log_output.status.success() {
        let log_text = String::from_utf8(log_output.stdout)?;

        // Parse log output to find commits with actual changes
        for line in log_text.lines() {
            if line.starts_with("Commit: ") {
                if let Some(prolly_commit) = line
                    .strip_prefix("Commit: ")
                    .and_then(|s| s.split(' ').next())
                {
                    // Try to get changes for this prolly commit
                    let show_output = Command::new(&git_prolly_path)
                        .args(["show", prolly_commit])
                        .current_dir(dataset_path)
                        .output()?;

                    if show_output.status.success() {
                        if let Ok(changes) =
                            parse_prolly_show_output(&String::from_utf8(show_output.stdout)?)
                        {
                            if !changes.is_empty() {
                                return Ok(changes);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(Vec::new())
}

fn parse_prolly_show_output(show_output: &str) -> Result<Vec<KvDiff>, Box<dyn std::error::Error>> {
    let mut diffs = Vec::new();
    let mut in_changes_section = false;

    for line in show_output.lines() {
        let line = line.trim();

        if line == "Key-Value Changes:" {
            in_changes_section = true;
            continue;
        }

        if !in_changes_section {
            continue;
        }

        // Parse lines with ANSI color codes: "+ key = value", "- key = value", "M key: old -> new"
        if line.contains("[32m+ ") && line.contains(" = ") {
            // Added key (green) - extract between [32m+ and [0m
            if let Some(start) = line.find("[32m+ ") {
                if let Some(end) = line.find("[0m") {
                    let content = &line[start + 6..end]; // Skip "[32m+ "
                    if let Some(eq_pos) = content.find(" = ") {
                        let key = content[..eq_pos].to_string();
                        let mut value = content[eq_pos + 3..].to_string();
                        // Remove any trailing ANSI escape sequences
                        if let Some(esc_pos) = value.find('\u{1b}') {
                            value = value[..esc_pos].to_string();
                        }
                        // Clean up quotes properly
                        value = value.trim_matches('"').to_string();
                        diffs.push(KvDiff {
                            key: key.into_bytes(),
                            operation: DiffOperation::Added(value.into_bytes()),
                        });
                    }
                }
            }
        } else if line.contains("[31m- ") && line.contains(" = ") {
            // Removed key (red)
            if let Some(start) = line.find("[31m- ") {
                if let Some(end) = line.find("[0m") {
                    let content = &line[start + 6..end]; // Skip "[31m- "
                    if let Some(eq_pos) = content.find(" = ") {
                        let key = content[..eq_pos].to_string();
                        let mut value = content[eq_pos + 3..].to_string();
                        // Remove any trailing ANSI escape sequences
                        if let Some(esc_pos) = value.find('\u{1b}') {
                            value = value[..esc_pos].to_string();
                        }
                        // Clean up quotes properly
                        value = value.trim_matches('"').to_string();
                        diffs.push(KvDiff {
                            key: key.into_bytes(),
                            operation: DiffOperation::Removed(value.into_bytes()),
                        });
                    }
                }
            }
        } else if line.contains("[33m~ ") && line.contains(" = ") && line.contains(" -> ") {
            // Modified key (yellow) - format: [33m~ key = "old" -> "new"[0m
            if let Some(start) = line.find("[33m~ ") {
                if let Some(end) = line.find("[0m") {
                    let content = &line[start + 6..end]; // Skip "[33m~ "
                    if let Some(eq_pos) = content.find(" = ") {
                        let key = content[..eq_pos].to_string();
                        let change_part = &content[eq_pos + 3..]; // Skip " = "
                        if let Some(arrow_pos) = change_part.find(" -> ") {
                            let mut old_value = change_part[..arrow_pos].to_string();
                            let mut new_value = change_part[arrow_pos + 4..].to_string();
                            // Remove any trailing ANSI escape sequences
                            if let Some(esc_pos) = old_value.find('\u{1b}') {
                                old_value = old_value[..esc_pos].to_string();
                            }
                            if let Some(esc_pos) = new_value.find('\u{1b}') {
                                new_value = new_value[..esc_pos].to_string();
                            }
                            // Clean up quotes properly
                            old_value = old_value.trim_matches('"').to_string();
                            new_value = new_value.trim_matches('"').to_string();

                            diffs.push(KvDiff {
                                key: key.into_bytes(),
                                operation: DiffOperation::Modified {
                                    old: old_value.into_bytes(),
                                    new: new_value.into_bytes(),
                                },
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(diffs)
}

fn get_actual_prolly_changes_with_path(
    commit_id: &str,
    _parent_commit_id: Option<&str>,
    _dataset_tag: &str,
    dataset_path: &Path,
) -> Result<Vec<KvDiff>, Box<dyn std::error::Error>> {
    // Use git-prolly show to get the actual changes for this commit
    let diffs = get_prolly_changes_from_commit(dataset_path, commit_id)?;
    Ok(diffs)
}

/// Get commit history for a specific branch without checking out
fn get_branch_commits(
    store: &GitVersionedKvStore<32>,
    branch_name: &str,
) -> Result<Vec<CommitInfo>, Box<dyn std::error::Error>> {
    // Use git rev-list to get commits for this specific branch
    let git_repo = store.git_repo();
    let repo_path = git_repo
        .path()
        .parent()
        .ok_or("Failed to get parent directory")?;

    let output = std::process::Command::new("git")
        .args([
            "rev-list",
            "--format=format:%H|%an|%cn|%s|%at",
            &format!("refs/heads/{branch_name}"),
        ])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Failed to get commits for branch {}: {}",
            branch_name,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let mut commits = Vec::new();

    for line in stdout.lines() {
        if line.starts_with("commit ") {
            continue; // Skip the "commit <hash>" lines
        }
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 5 {
            let commit_id = gix::ObjectId::from_hex(parts[0].as_bytes())?;
            let author = parts[1].to_string();
            let committer = parts[2].to_string();
            let message = parts[3].to_string();
            let timestamp = parts[4].parse::<i64>().unwrap_or(0);

            commits.push(CommitInfo {
                id: commit_id,
                author,
                committer,
                message,
                timestamp,
            });
        }
    }

    Ok(commits)
}

/// Get diff between two commits using read-only historical access
fn get_diff_between_commits(
    store: &GitVersionedKvStore<32>,
    from_commit: &str,
    to_commit: &str,
) -> Result<Vec<KvDiff>, Box<dyn std::error::Error>> {
    // Get key-value state at both commits using read-only access
    let from_state = store.get_keys_at_ref(from_commit)?;
    let to_state = store.get_keys_at_ref(to_commit)?;

    let mut diffs = Vec::new();

    // Find added and modified keys
    for (key, to_value) in &to_state {
        match from_state.get(key) {
            Some(from_value) if from_value != to_value => {
                // Modified
                diffs.push(KvDiff {
                    key: key.clone(),
                    operation: DiffOperation::Modified {
                        old: from_value.clone(),
                        new: to_value.clone(),
                    },
                });
            }
            None => {
                // Added
                diffs.push(KvDiff {
                    key: key.clone(),
                    operation: DiffOperation::Added(to_value.clone()),
                });
            }
            _ => {} // Unchanged
        }
    }

    // Find removed keys
    for (key, from_value) in &from_state {
        if !to_state.contains_key(key) {
            diffs.push(KvDiff {
                key: key.clone(),
                operation: DiffOperation::Removed(from_value.clone()),
            });
        }
    }

    // Sort diffs by key for consistent output
    diffs.sort_by(|a, b| a.key.cmp(&b.key));

    Ok(diffs)
}

fn process_dataset(
    name: String,
    path: &Path,
    branch_filter: &[String],
) -> Result<DatasetInfo, Box<dyn std::error::Error>> {
    let store = GitVersionedKvStore::<32>::open(path)?;

    // Get all branches
    let all_branches = store.list_branches()?;
    let current_branch = store.current_branch().to_string();

    // Filter branches if specified
    let branches = if branch_filter.is_empty() {
        all_branches
    } else {
        all_branches
            .into_iter()
            .filter(|branch| branch_filter.contains(branch))
            .collect()
    };

    let mut branch_infos = Vec::new();
    let mut commit_details = HashMap::new();
    let mut processed_commits = HashSet::new();

    for branch_name in branches {
        // Get commits for this branch without checking out
        // We'll use git commands directly to get the commit history for each branch
        let commits = get_branch_commits(&store, &branch_name)?;

        // Process each commit
        for (i, commit) in commits.iter().enumerate() {
            let commit_id = commit.id.to_string();

            if !processed_commits.contains(&commit_id) {
                processed_commits.insert(commit_id.clone());

                // Get changes for this commit using read-only historical access
                let changes = if i < commits.len() - 1 {
                    let parent = &commits[i + 1].id.to_string();
                    get_diff_between_commits(&store, parent, &commit_id).unwrap_or_default()
                } else {
                    // For initial commit, show all keys as added using historical access
                    let keys_at_commit = store.get_keys_at_ref(&commit_id).unwrap_or_default();
                    keys_at_commit
                        .into_iter()
                        .map(|(key, value)| KvDiff {
                            key,
                            operation: DiffOperation::Added(value),
                        })
                        .collect()
                };

                commit_details.insert(
                    commit_id.clone(),
                    CommitDiff {
                        info: commit.clone(),
                        changes,
                    },
                );
            }
        }

        branch_infos.push(BranchInfo {
            name: branch_name.clone(),
            commits,
            current: branch_name == current_branch,
        });
    }

    Ok(DatasetInfo {
        name,
        path: path.to_path_buf(),
        branches: branch_infos,
        commit_details,
    })
}

fn generate_html(repository: &RepositoryData) -> Result<String, Box<dyn std::error::Error>> {
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Git-Prolly Visualization (beta)</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            background: #fafafa;
            min-height: 100vh;
            padding: 20px;
            color: #1a1a1a;
        }}

        .container {{
            max-width: 1400px;
            margin: 0 auto;
        }}

        .header {{
            background: #ffffff;
            border-radius: 12px;
            padding: 24px;
            margin-bottom: 24px;
            box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
            border: 1px solid #e5e5e5;
        }}

        .header h1 {{
            color: #1a1a1a;
            font-size: 28px;
            font-weight: 600;
            margin-bottom: 16px;
        }}

        .repo-and-datasets {{
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 16px;
            flex-wrap: wrap;
        }}

        .repository-path {{
            display: inline-block;
            background: #f3f4f6;
            color: #6b7280;
            font-size: 12px;
            padding: 4px 8px;
            border-radius: 6px;
            font-family: ui-monospace, 'SF Mono', 'Monaco', 'Cascadia Code', 'Courier New', monospace;
            border: 1px solid #e5e7eb;
        }}

        .dataset-tags {{
            display: flex;
            align-items: center;
            gap: 8px;
            flex-wrap: wrap;
        }}

        .dataset-tag {{
            background: #f9fafb;
            color: #374151;
            padding: 4px 10px;
            border-radius: 6px;
            font-size: 12px;
            font-weight: 500;
            border: 1px solid #d1d5db;
            cursor: default;
        }}

        .controls {{
            display: flex;
            align-items: center;
            gap: 16px;
        }}

        .branch-selector {{
            display: flex;
            align-items: center;
            gap: 8px;
        }}

        .branch-selector label {{
            color: #6b7280;
            font-weight: 500;
            font-size: 14px;
        }}

        .branch-selector select {{
            padding: 8px 12px;
            border-radius: 6px;
            border: 1px solid #d1d5db;
            background: white;
            color: #1a1a1a;
            font-size: 14px;
            cursor: pointer;
            transition: all 0.2s ease;
            min-width: 120px;
        }}

        .branch-selector select:hover {{
            border-color: #3b82f6;
        }}

        .branch-selector select:focus {{
            outline: none;
            border-color: #3b82f6;
            box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
        }}

        .main-content {{
            display: grid;
            grid-template-columns: 1fr 400px;
            gap: 24px;
        }}

        .graph-panel {{
            background: #ffffff;
            border-radius: 12px;
            padding: 24px;
            box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
            border: 1px solid #e5e5e5;
            max-height: 80vh;
            overflow-y: auto;
            position: relative;
        }}

        .graph-panel::-webkit-scrollbar {{
            width: 8px;
        }}

        .graph-panel::-webkit-scrollbar-track {{
            background: #f1f1f1;
            border-radius: 4px;
        }}

        .graph-panel::-webkit-scrollbar-thumb {{
            background: #c1c1c1;
            border-radius: 4px;
        }}

        .graph-panel::-webkit-scrollbar-thumb:hover {{
            background: #a8a8a8;
        }}

        .details-panel {{
            background: #ffffff;
            border-radius: 12px;
            padding: 24px;
            box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
            border: 1px solid #e5e5e5;
            max-height: 80vh;
            overflow-y: auto;
            position: sticky;
            top: 0;
        }}

        .branch {{
            margin-bottom: 32px;
        }}

        .branch.branch-hidden {{
            display: none;
        }}

        .branch-header {{
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 16px;
        }}

        .branch-name {{
            background: #3b82f6;
            color: white;
            padding: 6px 12px;
            border-radius: 6px;
            font-weight: 500;
            font-size: 13px;
        }}

        .branch-current {{
            background: #10b981;
        }}

        .commits {{
            display: flex;
            flex-direction: column;
            gap: 4px;
            margin-left: 40px;
            position: relative;
        }}

        .commits::before {{
            content: '';
            position: absolute;
            left: -21px;
            top: 12px;
            bottom: 12px;
            width: 2px;
            background: #d1d5db;
            z-index: 0;
        }}

        .commit {{
            display: flex;
            align-items: center;
            gap: 12px;
            padding: 12px;
            background: #f9fafb;
            border-radius: 6px;
            cursor: pointer;
            transition: all 0.2s ease;
            position: relative;
            border: 1px solid transparent;
        }}

        .commit::before {{
            content: '';
            position: absolute;
            left: -27px;
            width: 12px;
            height: 12px;
            background: #3b82f6; /* Default blue for main */
            border: 2px solid white;
            border-radius: 50%;
            box-shadow: 0 0 0 1px #e5e7eb, 0 1px 3px rgba(0, 0, 0, 0.1);
            z-index: 2;
        }}

        /* Branch-specific dot colors */
        .commit.branch-main::before {{
            background: #3b82f6; /* Blue for main */
        }}

        .commit.branch-feature-bulk-orders::before {{
            background: #f59e0b; /* Orange for bulk-orders */
        }}

        .commit.branch-feature-new-products::before {{
            background: #f59e0b; /* Orange for new-products */
        }}

        .commit.branch-feature-user-management::before {{
            background: #f59e0b; /* Orange for user-management */
        }}

        .commit.branch-hotfix-user-validation::before {{
            background: #f59e0b; /* Orange for user-validation (hotfix) */
        }}

        .commit.branch-other::before {{
            background: #f59e0b; /* Orange for other branches */
        }}

        .commit::after {{
            content: '';
            position: absolute;
            left: -25px;
            top: 50%;
            width: 8px;
            height: 2px;
            background: #3b82f6;
            transform: translateY(-50%);
            z-index: 1;
            border-radius: 1px;
        }}

        .commit:hover {{
            background: #f3f4f6;
            transform: translateX(2px);
            border-color: #e5e7eb;
        }}

        .commit.selected {{
            background: #eff6ff;
            border-color: #3b82f6;
        }}

        .commit-hash {{
            font-family: ui-monospace, 'SF Mono', 'Monaco', 'Cascadia Code', 'Courier New', monospace;
            font-size: 11px;
            color: #6b7280;
            background: #f3f4f6;
            padding: 2px 6px;
            border-radius: 3px;
            border: 1px solid #e5e7eb;
        }}

        .commit-message {{
            flex: 1;
            color: #1f2937;
            font-size: 14px;
            font-weight: 500;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }}

        .commit-time {{
            font-size: 12px;
            color: #9ca3af;
        }}

        .details-header {{
            color: #1f2937;
            font-size: 18px;
            font-weight: 600;
            margin-bottom: 20px;
            padding-bottom: 12px;
            border-bottom: 1px solid #e5e7eb;
            display: flex;
            align-items: center;
            justify-content: space-between;
        }}

        .details-dataset-tag {{
            background: #3b82f6;
            color: white;
            padding: 4px 10px;
            border-radius: 16px;
            font-size: 12px;
            font-weight: 500;
        }}

        .commit-info {{
            background: #f9fafb;
            padding: 16px;
            border-radius: 6px;
            margin-bottom: 20px;
            border: 1px solid #f3f4f6;
        }}

        .commit-info-row {{
            display: flex;
            margin-bottom: 8px;
        }}

        .commit-info-label {{
            font-weight: 500;
            color: #6b7280;
            width: 100px;
        }}

        .commit-info-value {{
            color: #1f2937;
            flex: 1;
            word-break: break-all;
        }}

        .changes-section {{
            margin-top: 20px;
        }}

        .changes-header {{
            color: #1f2937;
            font-size: 16px;
            font-weight: 600;
            margin-bottom: 12px;
        }}

        .change-item {{
            background: white;
            border-left: 3px solid #10b981;
            padding: 12px;
            margin-bottom: 8px;
            border-radius: 6px;
            border: 1px solid #f3f4f6;
        }}

        .change-item.removed {{
            border-left-color: #ef4444;
        }}

        .change-item.modified {{
            border-left-color: #f59e0b;
        }}

        .change-type {{
            font-size: 11px;
            font-weight: 600;
            color: white;
            padding: 2px 6px;
            border-radius: 3px;
            display: inline-block;
            margin-bottom: 8px;
        }}

        .change-type.added {{
            background: #10b981;
        }}

        .change-type.removed {{
            background: #ef4444;
        }}

        .change-type.modified {{
            background: #f59e0b;
        }}

        .change-key {{
            font-family: ui-monospace, 'SF Mono', 'Monaco', 'Cascadia Code', 'Courier New', monospace;
            font-size: 13px;
            color: #1f2937;
            margin-bottom: 4px;
            font-weight: 600;
        }}

        .change-value {{
            font-family: ui-monospace, 'SF Mono', 'Monaco', 'Cascadia Code', 'Courier New', monospace;
            font-size: 12px;
            color: #6b7280;
            background: #f9fafb;
            padding: 8px;
            border-radius: 4px;
            margin-top: 4px;
            word-break: break-all;
            border: 1px solid #f3f4f6;
        }}

        .empty-state {{
            text-align: center;
            color: #9ca3af;
            padding: 40px;
        }}

        .empty-state svg {{
            width: 64px;
            height: 64px;
            margin-bottom: 16px;
            opacity: 0.5;
        }}

        .dataset-header {{
            background: #f9fafb;
            border: 1px solid #e5e7eb;
            border-radius: 8px;
            padding: 16px;
            margin-bottom: 20px;
            margin-top: 20px;
        }}

        .dataset-header h3 {{
            color: #1f2937;
            font-size: 18px;
            font-weight: 600;
            margin: 0;
        }}

        .dataset-content {{
            margin-bottom: 40px;
        }}

        .dataset-content:last-child {{
            margin-bottom: 0;
        }}

        .branch-timeline {{
            padding: 20px 0;
        }}

        .branch-timeline-header {{
            margin-bottom: 24px;
            text-align: center;
            background: linear-gradient(135deg, #3b82f6, #1d4ed8);
            color: white;
            padding: 24px;
            border-radius: 12px;
            box-shadow: 0 4px 12px rgba(59, 130, 246, 0.3);
        }}

        .branch-timeline-header h2 {{
            font-size: 24px;
            font-weight: 600;
            margin-bottom: 8px;
        }}

        .timeline-description {{
            font-size: 14px;
            opacity: 0.9;
            margin: 0;
        }}

        .unified-commit {{
            background: linear-gradient(135deg, #f8fafc, #f1f5f9);
            border-left: 4px solid #3b82f6;
            margin-bottom: 12px;
            position: relative;
            transition: all 0.3s ease;
        }}

        .unified-commit:hover {{
            background: linear-gradient(135deg, #f1f5f9, #e2e8f0);
            transform: translateX(4px);
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
        }}

        .commit-dataset-tag {{
            background: linear-gradient(135deg, #10b981, #059669);
            color: white;
            padding: 4px 10px;
            border-radius: 12px;
            font-size: 11px;
            font-weight: 600;
            margin-right: 12px;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            box-shadow: 0 2px 4px rgba(16, 185, 129, 0.3);
        }}

        .dataset-changes {{
            margin-bottom: 20px;
        }}

        .dataset-changes-header {{
            background: linear-gradient(135deg, #8b5cf6, #7c3aed);
            color: white;
            padding: 8px 16px;
            border-radius: 8px;
            font-size: 14px;
            font-weight: 600;
            margin-bottom: 12px;
            box-shadow: 0 2px 8px rgba(139, 92, 246, 0.3);
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🌳 Git-Prolly Visualization (beta)</h1>
            <div class="repo-and-datasets">
                <div class="repository-path">Repository: {repo_path}</div>
                <div class="dataset-tags">
                    {dataset_tags}
                </div>
            </div>
            <div class="controls">
                <div class="branch-selector">
                    <label for="branch-select">Branch:</label>
                    <select id="branch-select" onchange="filterByBranch(this.value)">
                        {branch_options}
                    </select>
                </div>
            </div>
        </div>

        <div class="main-content">
            <div class="graph-panel">
                {datasets}
            </div>

            <div class="details-panel">
                <div id="commit-details">
                    <div class="empty-state">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <circle cx="12" cy="12" r="10"></circle>
                            <path d="M12 6v6l4 2"></path>
                        </svg>
                        <p>Select a commit to view details</p>
                    </div>
                </div>
            </div>
        </div>
    </div>

    <script>
        const datasets = {{}};
        {dataset_data}

        const gitRepository = {{}};
        {git_data}

        function clearCommitDetails() {{
            document.getElementById('commit-details').innerHTML = `
                <div class="empty-state">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <circle cx="12" cy="12" r="10"></circle>
                        <path d="M12 6v6l4 2"></path>
                    </svg>
                    <p>Select a commit to view details</p>
                </div>
            `;
        }}

        function showCommitDetails(datasetName, commitId, element) {{
            const dataset = datasets[datasetName];
            const commit = dataset.commits[commitId];

            if (!commit) return;

            // Remove previous selection
            document.querySelectorAll('.commit').forEach(el => {{
                el.classList.remove('selected');
            }});

            // Add selection to current commit
            element.classList.add('selected');

            const detailsHtml = `
                <div class="details-header">
                    Commit Details
                    <span class="details-dataset-tag">` + datasetName + `</span>
                </div>
                <div class="commit-info">
                    <div class="commit-info-row">
                        <span class="commit-info-label">Hash:</span>
                        <span class="commit-info-value">` + commit.info.id + `</span>
                    </div>
                    <div class="commit-info-row">
                        <span class="commit-info-label">Author:</span>
                        <span class="commit-info-value">` + commit.info.author + `</span>
                    </div>
                    <div class="commit-info-row">
                        <span class="commit-info-label">Message:</span>
                        <span class="commit-info-value">` + commit.info.message + `</span>
                    </div>
                    <div class="commit-info-row">
                        <span class="commit-info-label">Timestamp:</span>
                        <span class="commit-info-value">` + new Date(commit.info.timestamp * 1000).toLocaleString() + `</span>
                    </div>
                </div>

                <div class="changes-section">
                    <div class="changes-header">Changes (` + commit.changes.length + `)</div>
                    ` + commit.changes.map(change => {{
                        let changeType = '';
                        let changeClass = '';
                        let valueHtml = '';

                        if (change.operation.Added) {{
                            changeType = 'ADDED';
                            changeClass = 'added';
                            valueHtml = '<div class="change-value">' + escapeHtml(arrayToString(change.operation.Added)) + '</div>';
                        }} else if (change.operation.Removed) {{
                            changeType = 'REMOVED';
                            changeClass = 'removed';
                            valueHtml = '<div class="change-value">' + escapeHtml(arrayToString(change.operation.Removed)) + '</div>';
                        }} else if (change.operation.Modified) {{
                            changeType = 'MODIFIED';
                            changeClass = 'modified';
                            valueHtml = '<div class="change-value">Old: ' + escapeHtml(arrayToString(change.operation.Modified.old)) + '</div>' +
                                       '<div class="change-value">New: ' + escapeHtml(arrayToString(change.operation.Modified.new)) + '</div>';
                        }}

                        return '<div class="change-item ' + changeClass + '">' +
                               '<span class="change-type ' + changeClass + '">' + changeType + '</span>' +
                               '<div class="change-key">' + escapeHtml(arrayToString(change.key)) + '</div>' +
                               valueHtml +
                               '</div>';
                    }}).join('') + `
                </div>
            `;

            document.getElementById('commit-details').innerHTML = detailsHtml;
        }}

        function escapeHtml(text) {{
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        }}

        function arrayToString(arr) {{
            if (Array.isArray(arr)) {{
                try {{
                    return new TextDecoder().decode(new Uint8Array(arr));
                }} catch {{
                    return arr.join(', ');
                }}
            }}
            return String(arr);
        }}

        function formatTime(timestamp) {{
            const date = new Date(timestamp * 1000);
            const now = new Date();
            const diff = now - date;

            if (diff < 3600000) {{
                return Math.floor(diff / 60000) + ' min ago';
            }} else if (diff < 86400000) {{
                return Math.floor(diff / 3600000) + ' hours ago';
            }} else {{
                return Math.floor(diff / 86400000) + ' days ago';
            }}
        }}

        function filterByBranch(selectedBranch) {{
            clearCommitDetails();
            // Always create unified timeline for the selected branch
            createUnifiedBranchTimeline(selectedBranch);
        }}

        function filterCommitsForBranch(commits, branchName) {{
            // Return ALL commits for this branch, sorted by timestamp descending (most recent first)
            return [...commits].sort((a, b) => b.timestamp - a.timestamp);
        }}

        function getBranchCssClass(branchName) {{
            // Convert branch name to CSS class
            const cleanBranchName = branchName.replace(/[^a-zA-Z0-9-]/g, '-').toLowerCase();
            return `branch-${{cleanBranchName}}`;
        }}

        function isCommitInMainBranch(commitId) {{
            // Check if this commit exists in main branch
            const mainBranch = gitRepository.branches.find(branch => branch.name === 'main');
            if (mainBranch) {{
                return mainBranch.commits.some(commit => commit.id === commitId);
            }}
            return false;
        }}

        function createUnifiedBranchTimeline(branchName) {{
            const graphPanel = document.querySelector('.graph-panel');

            // Find the git branch
            const gitBranch = gitRepository.branches.find(branch => branch.name === branchName);

            if (!gitBranch) {{
                graphPanel.innerHTML = `
                    <div class="branch-timeline">
                        <div class="empty-state">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <circle cx="12" cy="12" r="10"></circle>
                                <path d="M12 6v6l4 2"></path>
                            </svg>
                            <p>Branch not found</p>
                        </div>
                    </div>
                `;
                return;
            }}

            // Get commits from this git branch and sort by timestamp descending (most recent first)
            const commits = [...gitBranch.commits].sort((a, b) => b.timestamp - a.timestamp);

            // Filter commits to show branch-relevant ones
            const relevantCommits = filterCommitsForBranch(commits, branchName);

            // Generate unified timeline HTML without branch header
            let timelineHtml = `
                <div class="branch-timeline">
                    <div class="commits">
            `;

            if (relevantCommits.length === 0) {{
                timelineHtml += `
                    <div class="empty-state">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <circle cx="12" cy="12" r="10"></circle>
                            <path d="M8 12l2 2 4-4"></path>
                        </svg>
                        <p>No commits found</p>
                    </div>
                `;
            }} else {{
                relevantCommits.forEach(commit => {{
                    const shortHash = commit.id.substring(0, 8);

                    // No activity indicator needed - keep commit messages clean
                    const branchActivityIndicator = "";

                    // Determine dot color: blue for commits in main branch, orange for branch-only commits
                    const isInMain = isCommitInMainBranch(commit.id);
                    const branchCssClass = isInMain ? 'branch-main' : 'branch-other';

                    timelineHtml += `
                        <div class="commit unified-commit ${{branchCssClass}}" onclick="showGitCommitDetails('${{commit.id}}', this)">
                            <span class="commit-hash">${{shortHash}}</span>
                            <span class="commit-message">${{branchActivityIndicator}}${{escapeHtml(commit.message)}}</span>
                            <span class="commit-time">${{formatTime(commit.timestamp)}}</span>
                        </div>
                    `;
                }});
            }}

            timelineHtml += `
                    </div>
                </div>
            `;

            graphPanel.innerHTML = timelineHtml;
        }}

        function showGitCommitDetails(commitId, element) {{
            // Remove previous selection
            document.querySelectorAll('.commit').forEach(el => {{
                el.classList.remove('selected');
            }});

            // Add selection to current commit
            element.classList.add('selected');

            // Find the commit, prioritizing main branch for shared commits
            let commit = null;
            // First try to find the commit in main branch (which has real data extraction)
            const mainBranch = gitRepository.branches.find(b => b.name === 'main');
            if (mainBranch) {{
                commit = mainBranch.commits.find(c => c.id === commitId);
            }}
            // If not found in main branch, search other branches
            if (!commit) {{
                for (const branch of gitRepository.branches) {{
                    commit = branch.commits.find(c => c.id === commitId);
                    if (commit) break;
                }}
            }}

            if (!commit) return;

            // Generate dataset changes HTML
            let datasetChangesHtml = '';
            let totalChanges = 0;

            if (commit.datasetChanges) {{
                Object.keys(commit.datasetChanges).forEach(datasetName => {{
                    const changes = commit.datasetChanges[datasetName];
                    if (changes && changes.length > 0) {{
                        totalChanges += changes.length;
                        datasetChangesHtml += `
                            <div class="dataset-changes">
                                <h4 class="dataset-changes-header">📁 ${{datasetName}} (${{changes.length}} changes)</h4>
                                ${{changes.map(change => {{
                                    let changeType = '';
                                    let changeClass = '';
                                    let valueHtml = '';

                                    if (change.operation.Added) {{
                                        changeType = 'ADDED';
                                        changeClass = 'added';
                                        valueHtml = '<div class="change-value">' + escapeHtml(arrayToString(change.operation.Added)) + '</div>';
                                    }} else if (change.operation.Removed) {{
                                        changeType = 'REMOVED';
                                        changeClass = 'removed';
                                        valueHtml = '<div class="change-value">' + escapeHtml(arrayToString(change.operation.Removed)) + '</div>';
                                    }} else if (change.operation.Modified) {{
                                        changeType = 'MODIFIED';
                                        changeClass = 'modified';
                                        valueHtml = '<div class="change-value">Old: ' + escapeHtml(arrayToString(change.operation.Modified.old)) + '</div>' +
                                                   '<div class="change-value">New: ' + escapeHtml(arrayToString(change.operation.Modified.new)) + '</div>';
                                    }}

                                    return '<div class="change-item ' + changeClass + '">' +
                                           '<span class="change-type ' + changeClass + '">' + changeType + '</span>' +
                                           '<div class="change-key">' + escapeHtml(arrayToString(change.key)) + '</div>' +
                                           valueHtml +
                                           '</div>';
                                }}).join('')}}
                            </div>
                        `;
                    }}
                }});
            }}

            if (datasetChangesHtml === '') {{
                datasetChangesHtml = `
                    <div class="empty-state">
                        <p style="color: #6b7280; font-style: italic;">No prolly-tree changes detected in this commit.</p>
                    </div>
                `;
            }}

            const detailsHtml = `
                <div class="details-header">
                    Git Commit Details
                </div>
                <div class="commit-info">
                    <div class="commit-info-row">
                        <span class="commit-info-label">Hash:</span>
                        <span class="commit-info-value">${{commit.id}}</span>
                    </div>
                    <div class="commit-info-row">
                        <span class="commit-info-label">Author:</span>
                        <span class="commit-info-value">${{commit.author}}</span>
                    </div>
                    <div class="commit-info-row">
                        <span class="commit-info-label">Message:</span>
                        <span class="commit-info-value">${{commit.message}}</span>
                    </div>
                    <div class="commit-info-row">
                        <span class="commit-info-label">Timestamp:</span>
                        <span class="commit-info-value">${{new Date(commit.timestamp * 1000).toLocaleString()}}</span>
                    </div>
                </div>
                <div class="changes-section">
                    <div class="changes-header">Prolly-Tree Data Changes (${{totalChanges}} total)</div>
                    ${{datasetChangesHtml}}
                </div>
            `;

            document.getElementById('commit-details').innerHTML = detailsHtml;
        }}

        // Initialize the page with main branch selected
        document.addEventListener('DOMContentLoaded', function() {{
            const branchSelect = document.getElementById('branch-select');
            if (branchSelect && branchSelect.value) {{
                filterByBranch(branchSelect.value);
            }}
        }});

    </script>
</body>
</html>"#,
        repo_path = repository.path.display(),
        dataset_tags = generate_dataset_tags(&repository.datasets),
        branch_options = generate_branch_options(repository),
        datasets = "", // No longer used since we always show branch timeline
        dataset_data = generate_dataset_data(&repository.datasets),
        git_data = generate_git_data(repository)
    );

    Ok(html)
}

fn generate_dataset_tags(datasets: &[DatasetInfo]) -> String {
    datasets
        .iter()
        .map(|dataset| format!(r#"<span class="dataset-tag">{}</span>"#, dataset.name))
        .collect::<Vec<_>>()
        .join("\n                ")
}

fn generate_branch_options(repository: &RepositoryData) -> String {
    repository
        .git_branches
        .iter()
        .map(|branch| {
            let selected = if branch.name == "main" {
                r#" selected"#
            } else {
                ""
            };
            format!(
                r#"<option value="{}"{selected}>{}</option>"#,
                escape_html(&branch.name),
                escape_html(&branch.name)
            )
        })
        .collect::<Vec<_>>()
        .join("\n                        ")
}

fn generate_git_data(repository: &RepositoryData) -> String {
    let branches_js = repository
        .git_branches
        .iter()
        .map(|branch| {
            let commits_js = branch
                .commits
                .iter()
                .map(|commit| {
                    // Generate dataset changes JSON
                    let dataset_changes_js = commit
                        .dataset_changes
                        .iter()
                        .map(|(dataset_name, changes)| {
                            let changes_json = serialize_changes(changes);
                            format!(r#""{dataset_name}": {changes_json}"#)
                        })
                        .collect::<Vec<_>>()
                        .join(",\n                ");

                    format!(
                        r#"{{
                            id: "{}",
                            author: "{}",
                            message: "{}",
                            timestamp: {},
                            datasetChanges: {{
                                {}
                            }}
                        }}"#,
                        escape_js_string(&commit.id),
                        escape_js_string(&commit.author),
                        escape_js_string(&commit.message),
                        commit.timestamp,
                        dataset_changes_js
                    )
                })
                .collect::<Vec<_>>()
                .join(",\n            ");

            format!(
                r#"{{
                    name: "{}",
                    current: {},
                    commits: [
                        {}
                    ]
                }}"#,
                escape_js_string(&branch.name),
                branch.current,
                commits_js
            )
        })
        .collect::<Vec<_>>()
        .join(",\n        ");

    format!(
        r#"gitRepository.branches = [
        {branches_js}
    ];"#
    )
}

fn serialize_changes(changes: &[KvDiff]) -> String {
    let items: Vec<String> = changes
        .iter()
        .map(|change| {
            let operation_obj = match &change.operation {
                DiffOperation::Added(value) => {
                    format!(
                        r#"{{"Added": {}}}"#,
                        serde_json::to_string(&value.to_vec())
                            .unwrap_or_else(|_| "null".to_string())
                    )
                }
                DiffOperation::Removed(value) => {
                    format!(
                        r#"{{"Removed": {}}}"#,
                        serde_json::to_string(&value.to_vec())
                            .unwrap_or_else(|_| "null".to_string())
                    )
                }
                DiffOperation::Modified { old, new } => {
                    format!(
                        r#"{{"Modified": {{"old": {}, "new": {}}}}}"#,
                        serde_json::to_string(&old.to_vec()).unwrap_or_else(|_| "null".to_string()),
                        serde_json::to_string(&new.to_vec()).unwrap_or_else(|_| "null".to_string())
                    )
                }
            };

            format!(
                r#"{{"key": {}, "operation": {}}}"#,
                serde_json::to_string(&change.key).unwrap_or_else(|_| "[]".to_string()),
                operation_obj
            )
        })
        .collect();

    format!("[{}]", items.join(", "))
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn escape_html(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '&' => "&amp;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&#39;".to_string(),
            _ => c.to_string(),
        })
        .collect()
}

fn escape_js_string(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '"' => r#"\""#.to_string(),
            '\\' => r"\\".to_string(),
            '\n' => r"\n".to_string(),
            '\r' => r"\r".to_string(),
            '\t' => r"\t".to_string(),
            _ => c.to_string(),
        })
        .collect()
}

fn generate_dataset_data(datasets: &[DatasetInfo]) -> String {
    datasets
        .iter()
        .map(|dataset| {
            let dataset_name = sanitize_name(&dataset.name);

            // Generate branches array
            let js_branches = dataset
                .branches
                .iter()
                .map(|branch| {
                    format!(
                        r#"{{
                            name: "{}",
                            current: {}
                        }}"#,
                        escape_js_string(&branch.name),
                        branch.current
                    )
                })
                .collect::<Vec<_>>()
                .join(",\n            ");

            // Generate JavaScript object for this dataset with branch associations
            let js_commits = dataset
                .commit_details
                .iter()
                .map(|(id, details)| {
                    // Find which branches contain this commit
                    let containing_branches: Vec<String> = dataset
                        .branches
                        .iter()
                        .filter_map(|branch| {
                            if branch
                                .commits
                                .iter()
                                .any(|commit| commit.id.to_string() == *id)
                            {
                                Some(branch.name.clone())
                            } else {
                                None
                            }
                        })
                        .collect();

                    let branches_json = containing_branches
                        .iter()
                        .map(|b| format!(r#""{}""#, escape_js_string(b)))
                        .collect::<Vec<_>>()
                        .join(", ");

                    format!(
                        r#""{id}": {{
                            info: {{
                                id: "{}",
                                author: "{}",
                                message: "{}",
                                timestamp: {}
                            }},
                            changes: {},
                            branches: [{}]
                        }}"#,
                        details.info.id,
                        escape_js_string(&details.info.author),
                        escape_js_string(&details.info.message),
                        details.info.timestamp,
                        serialize_changes(&details.changes),
                        branches_json
                    )
                })
                .collect::<Vec<_>>()
                .join(",\n        ");

            format!(
                r#"datasets["{dataset_name}"] = {{
    branches: [
        {js_branches}
    ],
    commits: {{
        {js_commits}
    }}
}};"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n        ")
}
