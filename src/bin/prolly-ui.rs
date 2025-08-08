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
    /// Path to the git-prolly repository
    #[arg(help = "Repository path (defaults to current directory)")]
    repo_path: Option<PathBuf>,

    /// Output HTML file path
    #[arg(short, long, default_value = "prolly-ui.html")]
    output: PathBuf,

    /// Include additional repositories for dataset switching
    #[arg(short = 'd', long = "dataset", value_name = "NAME:PATH")]
    datasets: Vec<String>,
}

#[derive(Debug, Clone)]
struct BranchInfo {
    name: String,
    commits: Vec<CommitInfo>,
    current: bool,
}

#[derive(Debug, Clone)]
struct RepositoryData {
    name: String,
    path: PathBuf,
    branches: Vec<BranchInfo>,
    commit_details: HashMap<String, CommitDiff>,
}

#[derive(Debug, Clone)]
struct CommitDiff {
    info: CommitInfo,
    changes: Vec<KvDiff>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut repositories = Vec::new();

    // Process main repository
    let main_path = cli.repo_path.unwrap_or_else(|| PathBuf::from("."));
    let main_name = main_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("main")
        .to_string();

    println!("📊 Processing main repository: {}", main_path.display());
    let main_repo = process_repository(main_name, &main_path)?;
    repositories.push(main_repo);

    // Process additional datasets
    for dataset in cli.datasets {
        let parts: Vec<&str> = dataset.splitn(2, ':').collect();
        if parts.len() != 2 {
            eprintln!("⚠️  Invalid dataset format: {dataset} (expected NAME:PATH)");
            continue;
        }

        let name = parts[0].to_string();
        let path = PathBuf::from(parts[1]);

        println!("📊 Processing dataset '{}': {}", name, path.display());
        match process_repository(name, &path) {
            Ok(repo) => repositories.push(repo),
            Err(e) => eprintln!("⚠️  Failed to process dataset: {e}"),
        }
    }

    // Generate HTML
    println!("🎨 Generating HTML visualization...");
    let html = generate_html(&repositories)?;

    // Write to file
    fs::write(&cli.output, html)?;
    println!("✅ HTML visualization saved to: {}", cli.output.display());

    Ok(())
}

fn process_repository(
    name: String,
    path: &Path,
) -> Result<RepositoryData, Box<dyn std::error::Error>> {
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

    Ok(RepositoryData {
        name,
        path: path.to_path_buf(),
        branches: branch_infos,
        commit_details,
    })
}

fn generate_html(repositories: &[RepositoryData]) -> Result<String, Box<dyn std::error::Error>> {
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
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            padding: 20px;
        }}

        .container {{
            max-width: 1400px;
            margin: 0 auto;
        }}

        .header {{
            background: rgba(255, 255, 255, 0.95);
            border-radius: 16px;
            padding: 24px;
            margin-bottom: 24px;
            box-shadow: 0 10px 40px rgba(0, 0, 0, 0.1);
        }}

        .header h1 {{
            color: #2d3748;
            font-size: 32px;
            margin-bottom: 16px;
        }}

        .dataset-selector {{
            display: flex;
            align-items: center;
            gap: 12px;
        }}

        .dataset-selector label {{
            color: #4a5568;
            font-weight: 500;
        }}

        .dataset-selector select {{
            padding: 8px 16px;
            border-radius: 8px;
            border: 2px solid #e2e8f0;
            background: white;
            color: #2d3748;
            font-size: 16px;
            cursor: pointer;
            transition: all 0.3s ease;
        }}

        .dataset-selector select:hover {{
            border-color: #667eea;
        }}

        .main-content {{
            display: grid;
            grid-template-columns: 1fr 400px;
            gap: 24px;
        }}

        .graph-panel {{
            background: rgba(255, 255, 255, 0.95);
            border-radius: 16px;
            padding: 24px;
            box-shadow: 0 10px 40px rgba(0, 0, 0, 0.1);
        }}

        .details-panel {{
            background: rgba(255, 255, 255, 0.95);
            border-radius: 16px;
            padding: 24px;
            box-shadow: 0 10px 40px rgba(0, 0, 0, 0.1);
            max-height: 80vh;
            overflow-y: auto;
        }}

        .branch {{
            margin-bottom: 32px;
        }}

        .branch-header {{
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 16px;
        }}

        .branch-name {{
            background: linear-gradient(135deg, #667eea, #764ba2);
            color: white;
            padding: 6px 16px;
            border-radius: 20px;
            font-weight: 600;
            font-size: 14px;
        }}

        .branch-current {{
            background: linear-gradient(135deg, #48bb78, #38a169);
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
            left: -20px;
            top: 12px;
            bottom: 12px;
            width: 2px;
            background: linear-gradient(180deg, #667eea, #764ba2);
        }}

        .commit {{
            display: flex;
            align-items: center;
            gap: 12px;
            padding: 12px;
            background: #f7fafc;
            border-radius: 8px;
            cursor: pointer;
            transition: all 0.3s ease;
            position: relative;
        }}

        .commit::before {{
            content: '';
            position: absolute;
            left: -26px;
            width: 12px;
            height: 12px;
            background: #667eea;
            border: 3px solid white;
            border-radius: 50%;
            box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
        }}

        .commit:hover {{
            background: #edf2f7;
            transform: translateX(4px);
        }}

        .commit.selected {{
            background: linear-gradient(135deg, #e6f3ff, #d4e8ff);
            border: 2px solid #667eea;
        }}

        .commit-hash {{
            font-family: 'Courier New', monospace;
            font-size: 12px;
            color: #718096;
            background: white;
            padding: 2px 8px;
            border-radius: 4px;
        }}

        .commit-message {{
            flex: 1;
            color: #2d3748;
            font-size: 14px;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }}

        .commit-time {{
            font-size: 12px;
            color: #a0aec0;
        }}

        .details-header {{
            color: #2d3748;
            font-size: 20px;
            font-weight: 600;
            margin-bottom: 20px;
            padding-bottom: 12px;
            border-bottom: 2px solid #e2e8f0;
        }}

        .commit-info {{
            background: #f7fafc;
            padding: 16px;
            border-radius: 8px;
            margin-bottom: 20px;
        }}

        .commit-info-row {{
            display: flex;
            margin-bottom: 8px;
        }}

        .commit-info-label {{
            font-weight: 600;
            color: #4a5568;
            width: 100px;
        }}

        .commit-info-value {{
            color: #2d3748;
            flex: 1;
            word-break: break-all;
        }}

        .changes-section {{
            margin-top: 20px;
        }}

        .changes-header {{
            color: #2d3748;
            font-size: 16px;
            font-weight: 600;
            margin-bottom: 12px;
        }}

        .change-item {{
            background: white;
            border-left: 4px solid #48bb78;
            padding: 12px;
            margin-bottom: 8px;
            border-radius: 4px;
        }}

        .change-item.removed {{
            border-left-color: #f56565;
        }}

        .change-item.modified {{
            border-left-color: #ed8936;
        }}

        .change-type {{
            font-size: 12px;
            font-weight: 600;
            color: white;
            padding: 2px 8px;
            border-radius: 4px;
            display: inline-block;
            margin-bottom: 8px;
        }}

        .change-type.added {{
            background: #48bb78;
        }}

        .change-type.removed {{
            background: #f56565;
        }}

        .change-type.modified {{
            background: #ed8936;
        }}

        .change-key {{
            font-family: 'Courier New', monospace;
            font-size: 14px;
            color: #2d3748;
            margin-bottom: 4px;
            font-weight: 600;
        }}

        .change-value {{
            font-family: 'Courier New', monospace;
            font-size: 13px;
            color: #4a5568;
            background: #f7fafc;
            padding: 8px;
            border-radius: 4px;
            margin-top: 4px;
            word-break: break-all;
        }}

        .empty-state {{
            text-align: center;
            color: #718096;
            padding: 40px;
        }}

        .empty-state svg {{
            width: 64px;
            height: 64px;
            margin-bottom: 16px;
            opacity: 0.5;
        }}

        .dataset-hidden {{
            display: none;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🌳 ProllyTree Repository Visualization</h1>
            <div class="dataset-selector">
                <label for="dataset-select">Dataset:</label>
                <select id="dataset-select" onchange="switchDataset(this.value)">
                    {options}
                </select>
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
        const repositories = {{}};
        {repository_data}

        function switchDataset(name) {{
            document.querySelectorAll('.dataset-content').forEach(el => {{
                el.classList.add('dataset-hidden');
            }});
            document.getElementById('dataset-' + name).classList.remove('dataset-hidden');
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

        function showCommitDetails(dataset, commitId, element) {{
            const repo = repositories[dataset];
            const commit = repo.commits[commitId];

            if (!commit) return;

            // Remove previous selection
            document.querySelectorAll('.commit').forEach(el => {{
                el.classList.remove('selected');
            }});

            // Add selection to current commit
            element.classList.add('selected');

            const detailsHtml = `
                <div class="details-header">Commit Details</div>
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
    </script>
</body>
</html>"#,
        options = generate_dataset_options(repositories),
        datasets = generate_datasets_html_no_js(repositories),
        repository_data = generate_repository_data(repositories)
    );

    Ok(html)
}

fn generate_dataset_options(repositories: &[RepositoryData]) -> String {
    repositories
        .iter()
        .enumerate()
        .map(|(i, repo)| {
            format!(
                r#"<option value="{}"{}>{} ({})</option>"#,
                sanitize_name(&repo.name),
                if i == 0 { " selected" } else { "" },
                repo.name,
                repo.path.display()
            )
        })
        .collect::<Vec<_>>()
        .join("\n                    ")
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

fn generate_datasets_html(repositories: &[RepositoryData]) -> String {
    repositories
        .iter()
        .enumerate()
        .map(|(i, repo)| {
            let dataset_name = sanitize_name(&repo.name);
            let is_hidden = if i == 0 { "" } else { " dataset-hidden" };

            // Generate JavaScript object for this repository
            let js_commits = repo
                .commit_details
                .iter()
                .map(|(id, details)| {
                    format!(
                        r#""{id}": {{
                            info: {{
                                id: "{}",
                                author: "{}",
                                message: "{}",
                                timestamp: {}
                            }},
                            changes: {}
                        }}"#,
                        details.info.id,
                        escape_js_string(&details.info.author),
                        escape_js_string(&details.info.message),
                        details.info.timestamp,
                        serialize_changes(&details.changes)
                    )
                })
                .collect::<Vec<_>>()
                .join(",\n        ");

            let js_repo = format!(
                r#"repositories["{dataset_name}"] = {{
    commits: {{
        {js_commits}
    }}
}};"#
            );

            let branches_html = repo
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
                        r#"<div class="branch">
                    <div class="branch-header">
                        <span class="{}">{}{}</span>
                    </div>
                    <div class="commits">
                        {}
                    </div>
                </div>"#,
                        branch_class,
                        branch.name,
                        if branch.current { " (current)" } else { "" },
                        commits_html
                    )
                })
                .collect::<Vec<_>>()
                .join("\n                ");

            format!(
                r#"<div id="dataset-{dataset_name}" class="dataset-content{is_hidden}">
                {branches_html}
            </div>
            <script>
            {js_repo}
            </script>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n            ")
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

fn generate_repository_data(repositories: &[RepositoryData]) -> String {
    repositories
        .iter()
        .map(|repo| {
            let dataset_name = sanitize_name(&repo.name);

            // Generate JavaScript object for this repository
            let js_commits = repo
                .commit_details
                .iter()
                .map(|(id, details)| {
                    format!(
                        r#""{id}": {{
                            info: {{
                                id: "{}",
                                author: "{}",
                                message: "{}",
                                timestamp: {}
                            }},
                            changes: {}
                        }}"#,
                        details.info.id,
                        escape_js_string(&details.info.author),
                        escape_js_string(&details.info.message),
                        details.info.timestamp,
                        serialize_changes(&details.changes)
                    )
                })
                .collect::<Vec<_>>()
                .join(",\n        ");

            format!(
                r#"repositories["{dataset_name}"] = {{
    commits: {{
        {js_commits}
    }}
}};"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n        ")
}

fn generate_datasets_html_no_js(repositories: &[RepositoryData]) -> String {
    repositories
        .iter()
        .enumerate()
        .map(|(i, repo)| {
            let dataset_name = sanitize_name(&repo.name);
            let is_hidden = if i == 0 { "" } else { " dataset-hidden" };

            let branches_html = repo
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
                        r#"<div class="branch">
                    <div class="branch-header">
                        <span class="{}">{}{}</span>
                    </div>
                    <div class="commits">
                        {}
                    </div>
                </div>"#,
                        branch_class,
                        branch.name,
                        if branch.current { " (current)" } else { "" },
                        commits_html
                    )
                })
                .collect::<Vec<_>>()
                .join("\n                ");

            format!(
                r#"<div id="dataset-{dataset_name}" class="dataset-content{is_hidden}">
                {branches_html}
            </div>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n            ")
}
