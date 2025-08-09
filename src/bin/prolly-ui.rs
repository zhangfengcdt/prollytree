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
use prollytree::git::{CommitInfo, DiffOperation, GitVersionedKvStore, KvDiff};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "prolly-ui")]
#[command(about = "Generate static HTML visualization for git-prolly repositories")]
#[command(version = "0.1.0")]
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
    branches: Vec<BranchInfo>,
    commit_details: HashMap<String, CommitDiff>,
}

#[derive(Debug, Clone)]
struct RepositoryData {
    path: PathBuf,
    datasets: Vec<DatasetInfo>,
    git_branches: Vec<GitBranchInfo>,
    git_commits: HashMap<String, GitCommitInfo>,
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
        discover_datasets(&repo_path)?
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
        let dataset_path = repo_path.join(&dataset_name);

        if !dataset_path.exists() {
            eprintln!(
                "⚠️  Dataset directory does not exist: {}",
                dataset_path.display()
            );
            continue;
        }

        println!(
            "  📊 Processing dataset '{}': {}",
            dataset_name,
            dataset_path.display()
        );
        match process_dataset(dataset_name.clone(), &dataset_path) {
            Ok(dataset) => datasets.push(dataset),
            Err(e) => eprintln!("  ⚠️  Failed to process dataset '{dataset_name}': {e}"),
        }
    }

    if datasets.is_empty() {
        return Err("No valid datasets could be processed".into());
    }

    // Process actual git repository
    println!("🔍 Processing git repository structure...");
    let (git_branches, git_commits) = process_git_repository(&repo_path)?;

    let repository_data = RepositoryData {
        path: repo_path,
        datasets,
        git_branches,
        git_commits,
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

fn process_git_repository(
    repo_path: &Path,
) -> Result<(Vec<GitBranchInfo>, HashMap<String, GitCommitInfo>), Box<dyn std::error::Error>> {
    use std::process::Command;

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
    let branch_names: Vec<String> = branch_list
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

    // Get commits for each branch
    for branch_name in branch_names {
        let is_current = branch_name == current_branch;

        // Get commits for this branch
        let log_output = Command::new("git")
            .args([
                "-C",
                &repo_path.to_string_lossy(),
                "log",
                &branch_name,
                "--format=%H|%an|%s|%ct",
                "--max-count=50", // Limit to avoid too many commits
            ])
            .output()?;

        if !log_output.status.success() {
            continue; // Skip branches we can't read
        }

        let log_text = String::from_utf8(log_output.stdout)?;
        let mut branch_commits = Vec::new();

        for line in log_text.lines() {
            if let Some((id, rest)) = line.split_once('|') {
                if let Some((author, rest)) = rest.split_once('|') {
                    if let Some((message, timestamp_str)) = rest.split_once('|') {
                        if let Ok(timestamp) = timestamp_str.parse::<i64>() {
                            let commit = GitCommitInfo {
                                id: id.to_string(),
                                author: author.to_string(),
                                message: message.to_string(),
                                timestamp,
                            };

                            branch_commits.push(commit.clone());
                            all_commits.insert(id.to_string(), commit);
                        }
                    }
                }
            }
        }

        git_branches.push(GitBranchInfo {
            name: branch_name,
            commits: branch_commits,
            current: is_current,
        });
    }

    Ok((git_branches, all_commits))
}

fn process_dataset(name: String, path: &Path) -> Result<DatasetInfo, Box<dyn std::error::Error>> {
    let mut store = GitVersionedKvStore::<32>::open(path)?;

    // Get all branches
    let branches = store.list_branches()?;
    let current_branch = store.current_branch().to_string();

    let mut branch_infos = Vec::new();
    let mut commit_details = HashMap::new();
    let mut processed_commits = HashSet::new();

    for branch_name in branches {
        // Checkout branch to get its commits
        store.checkout(&branch_name)?;
        let commits = store.log()?;

        // Process each commit
        for (i, commit) in commits.iter().enumerate() {
            let commit_id = commit.id.to_string();

            if !processed_commits.contains(&commit_id) {
                processed_commits.insert(commit_id.clone());

                // Get changes for this commit
                let changes = if i < commits.len() - 1 {
                    let parent = &commits[i + 1].id.to_string();
                    store.diff(parent, &commit_id).unwrap_or_default()
                } else {
                    // For initial commit, show all keys as added
                    // Temporarily checkout this commit to get its state
                    let original_branch = store.current_branch().to_string();
                    if store.checkout(&commit_id).is_ok() {
                        let keys = store.list_keys();
                        let changes: Vec<KvDiff> = keys
                            .into_iter()
                            .filter_map(|key| {
                                store.get(&key).map(|value| KvDiff {
                                    key: key.clone(),
                                    operation: DiffOperation::Added(value),
                                })
                            })
                            .collect();
                        // Restore original branch
                        let _ = store.checkout(&original_branch);
                        changes
                    } else {
                        Vec::new()
                    }
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

    // Restore original branch
    store.checkout(&current_branch)?;

    Ok(DatasetInfo {
        name,
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
    <title>ProllyTree Repository Visualization</title>
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

        .repository-path {{
            color: #6b7280;
            font-size: 14px;
            margin-bottom: 16px;
            font-family: ui-monospace, 'SF Mono', 'Monaco', 'Cascadia Code', 'Courier New', monospace;
        }}

        .dataset-tags {{
            display: flex;
            align-items: center;
            gap: 8px;
            margin-bottom: 16px;
            flex-wrap: wrap;
        }}

        .dataset-tag {{
            background: #3b82f6;
            color: white;
            padding: 6px 12px;
            border-radius: 20px;
            font-size: 13px;
            font-weight: 500;
            border: 2px solid #2563eb;
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
            background: #3b82f6;
            border: 2px solid white;
            border-radius: 50%;
            box-shadow: 0 0 0 1px #e5e7eb, 0 1px 3px rgba(0, 0, 0, 0.1);
            z-index: 2;
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
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🌳 ProllyTree Repository Visualization</h1>
            <div class="repository-path">Repository: {repo_path}</div>
            <div class="dataset-tags">
                {dataset_tags}
            </div>
            <div class="controls">
                <div class="branch-selector">
                    <label for="branch-select">Branch:</label>
                    <select id="branch-select" onchange="filterByBranch(this.value)">
                        <option value="all">All Branches</option>
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

            const graphPanel = document.querySelector('.graph-panel');

            if (selectedBranch === 'all') {{
                // Show all datasets with their branch sections
                showAllDatasets();
            }} else {{
                // Create unified timeline for the selected branch
                createUnifiedBranchTimeline(selectedBranch);
            }}
        }}

        function showAllDatasets() {{
            const graphPanel = document.querySelector('.graph-panel');

            // Restore original content
            graphPanel.innerHTML = `{original_datasets_html}`;
        }}

        function createUnifiedBranchTimeline(branchName) {{
            const graphPanel = document.querySelector('.graph-panel');

            // Find the git branch
            const gitBranch = gitRepository.branches.find(branch => branch.name === branchName);

            if (!gitBranch) {{
                graphPanel.innerHTML = `
                    <div class="branch-timeline">
                        <div class="branch-timeline-header">
                            <h2>📊 Branch: ${{branchName}}</h2>
                            <p class="timeline-description">Branch not found</p>
                        </div>
                        <div class="empty-state">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <circle cx="12" cy="12" r="10"></circle>
                                <path d="M12 6v6l4 2"></path>
                            </svg>
                            <p>Branch "${{branchName}}" not found</p>
                        </div>
                    </div>
                `;
                return;
            }}

            // Get commits from this git branch (already sorted by git log)
            const commits = gitBranch.commits;

            // Generate unified timeline HTML
            let timelineHtml = `
                <div class="branch-timeline">
                    <div class="branch-timeline-header">
                        <h2>📊 Branch: ${{branchName}}</h2>
                        <p class="timeline-description">Git commit timeline (${{commits.length}} commits)</p>
                    </div>
                    <div class="commits">
            `;

            if (commits.length === 0) {{
                timelineHtml += `
                    <div class="empty-state">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <circle cx="12" cy="12" r="10"></circle>
                            <path d="M8 12l2 2 4-4"></path>
                        </svg>
                        <p>No commits found for branch "${{branchName}}"</p>
                    </div>
                `;
            }} else {{
                commits.forEach(commit => {{
                    const shortHash = commit.id.substring(0, 8);
                    timelineHtml += `
                        <div class="commit unified-commit" onclick="showGitCommitDetails('${{commit.id}}', this)">
                            <span class="commit-hash">${{shortHash}}</span>
                            <span class="commit-message">${{escapeHtml(commit.message)}}</span>
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

            // Find the commit
            let commit = null;
            for (const branch of gitRepository.branches) {{
                commit = branch.commits.find(c => c.id === commitId);
                if (commit) break;
            }}

            if (!commit) return;

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
                    <div class="changes-header">Git Commit</div>
                    <p style="color: #6b7280; font-style: italic;">This shows the actual git repository commit. To see prolly-tree changes, select "All Branches" and click on dataset-specific commits.</p>
                </div>
            `;

            document.getElementById('commit-details').innerHTML = detailsHtml;
        }}

    </script>
</body>
</html>"#,
        repo_path = repository.path.display(),
        dataset_tags = generate_dataset_tags(&repository.datasets),
        branch_options = generate_branch_options(repository),
        datasets = generate_datasets_html(&repository.datasets),
        original_datasets_html = generate_datasets_html(&repository.datasets)
            .replace('"', r#"\""#)
            .replace('\n', r#"\n"#),
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
            format!(
                r#"<option value="{}">{}</option>"#,
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
                    format!(
                        r#"{{
                            id: "{}",
                            author: "{}",
                            message: "{}",
                            timestamp: {}
                        }}"#,
                        escape_js_string(&commit.id),
                        escape_js_string(&commit.author),
                        escape_js_string(&commit.message),
                        commit.timestamp
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

fn format_relative_time(timestamp: i64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let commit_time = UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64);
    let now = SystemTime::now();

    if let Ok(duration) = now.duration_since(commit_time) {
        let seconds = duration.as_secs();
        if seconds < 60 {
            return format!("{seconds} sec ago");
        } else if seconds < 3600 {
            return format!("{} min ago", seconds / 60);
        } else if seconds < 86400 {
            return format!("{} hours ago", seconds / 3600);
        } else if seconds < 2592000 {
            return format!("{} days ago", seconds / 86400);
        } else if seconds < 31536000 {
            return format!("{} months ago", seconds / 2592000);
        } else {
            return format!("{} years ago", seconds / 31536000);
        }
    }

    format!("{timestamp}")
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

fn generate_datasets_html(datasets: &[DatasetInfo]) -> String {
    datasets
        .iter()
        .map(|dataset| {
            let dataset_name = sanitize_name(&dataset.name);

            // Show dataset header
            let dataset_header = format!(
                r#"<div class="dataset-header">
                    <h3>📁 {}</h3>
                </div>"#,
                dataset.name
            );

            let branches_html = dataset
                .branches
                .iter()
                .map(|branch| {
                    let branch_class = if branch.current {
                        "branch-name branch-current"
                    } else {
                        "branch-name"
                    };

                    let commits_html = branch
                        .commits
                        .iter()
                        .map(|commit| {
                            let short_hash = &commit.id.to_string()[..8];
                            format!(
                                r#"<div class="commit" onclick="showCommitDetails('{}', '{}', this)">
                                    <span class="commit-hash">{}</span>
                                    <span class="commit-message">{}</span>
                                    <span class="commit-time">{}</span>
                                </div>"#,
                                dataset_name,
                                commit.id,
                                short_hash,
                                escape_html(&commit.message),
                                format_relative_time(commit.timestamp)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n                    ");

                    format!(
                        r#"<div class="branch" data-branch="{}">
                    <div class="branch-header">
                        <span class="{}">{}{}</span>
                    </div>
                    <div class="commits">
                        {}
                    </div>
                </div>"#,
                        branch.name,
                        branch_class,
                        branch.name,
                        if branch.current { " (current)" } else { "" },
                        commits_html
                    )
                })
                .collect::<Vec<_>>()
                .join("\n                ");

            format!(
                r#"<div id="dataset-{dataset_name}" class="dataset-content">
                {dataset_header}
                {branches_html}
            </div>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n            ")
}
