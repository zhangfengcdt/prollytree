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

use crate::git::types::*;
use gix::prelude::*;
use std::path::{Path, PathBuf};

/// Trait abstracting version-control metadata operations.
///
/// This decouples `VersionedKvStore` from a concrete git implementation,
/// enabling alternative metadata backends in the future.
pub trait MetadataBackend: Send {
    // ── Path access ──────────────────────────────────────────────────

    /// Path to the metadata directory (e.g. `.git`).
    fn metadata_dir(&self) -> &Path;

    /// Working directory root, if any.
    fn work_dir(&self) -> Option<PathBuf>;

    // ── User identity ────────────────────────────────────────────────

    /// Returns `(name, email)` for commit signatures.
    fn user_config(&self) -> (String, String);

    // ── Commit creation ──────────────────────────────────────────────

    /// Stage all files under `git_root` and create a tree object.
    fn stage_and_write_tree(&self, git_root: &Path) -> Result<gix::ObjectId, GitKvError>;

    /// Create a commit on top of the current HEAD (or as a root commit).
    fn write_commit(
        &self,
        tree_id: gix::ObjectId,
        message: &str,
    ) -> Result<gix::ObjectId, GitKvError>;

    // ── Reference management ─────────────────────────────────────────

    /// Current HEAD commit ID, or `Err` if HEAD is unborn.
    fn head_commit_id(&self) -> Result<gix::ObjectId, GitKvError>;

    /// Commit ID that a branch points to.
    fn branch_commit_id(&self, branch: &str) -> Result<gix::ObjectId, GitKvError>;

    /// Point an existing or new branch ref at `commit_id`.
    fn update_branch(&self, branch: &str, commit_id: gix::ObjectId) -> Result<(), GitKvError>;

    /// Make HEAD a symbolic ref to `refs/heads/<branch>`.
    fn update_head(&self, branch: &str) -> Result<(), GitKvError>;

    /// Create a new branch ref pointing at the current HEAD commit.
    fn create_branch(&self, name: &str) -> Result<(), GitKvError>;

    /// All branch names, sorted.
    fn list_branches(&self) -> Result<Vec<String>, GitKvError>;

    /// Resolve a human-readable reference (branch name, SHA prefix, tag, "HEAD")
    /// to a full commit ID.
    fn resolve_reference(&self, reference: &str) -> Result<gix::ObjectId, GitKvError>;

    // ── History ──────────────────────────────────────────────────────

    /// Walk commit history from HEAD, newest first, up to `limit` entries.
    fn walk_history(&self, limit: usize) -> Result<Vec<CommitInfo>, GitKvError>;

    /// Parent commit IDs for a given commit.
    fn commit_parents(&self, commit_id: &gix::ObjectId) -> Result<Vec<gix::ObjectId>, GitKvError>;

    // ── File access from commits ─────────────────────────────────────

    /// Read a file (by path relative to repo root) from a specific commit's tree.
    fn read_file_at_commit(
        &self,
        commit_id: &gix::ObjectId,
        file_path: &str,
    ) -> Result<Vec<u8>, GitKvError>;
}

// ══════════════════════════════════════════════════════════════════════
// Git implementation
// ══════════════════════════════════════════════════════════════════════

/// `MetadataBackend` backed by a real git repository via `gix`.
pub struct GitMetadataBackend {
    repo: gix::Repository,
}

impl GitMetadataBackend {
    /// Wrap an already-opened `gix::Repository`.
    pub fn new(repo: gix::Repository) -> Self {
        Self { repo }
    }

    /// Direct access to the underlying repository — needed by
    /// `GitNodeStorage` and `GitOperations` which perform their own
    /// object-level git work.
    pub fn repo(&self) -> &gix::Repository {
        &self.repo
    }

    /// Clone the underlying repository handle.
    pub fn clone_repo(&self) -> gix::Repository {
        self.repo.clone()
    }

    // ── private helpers ──────────────────────────────────────────────

    fn read_file_from_tree_recursive(
        &self,
        tree: &gix::objs::TreeRef,
        path_parts: &[&str],
        part_index: usize,
    ) -> Result<Vec<u8>, GitKvError> {
        if part_index >= path_parts.len() {
            return Err(GitKvError::GitObjectError(
                "Path traversal error".to_string(),
            ));
        }

        let current_part = path_parts[part_index];

        for entry in &tree.entries {
            if entry.filename == current_part.as_bytes() {
                if part_index == path_parts.len() - 1 {
                    let mut file_buffer = Vec::new();
                    let file_obj = self
                        .repo
                        .objects
                        .find(entry.oid, &mut file_buffer)
                        .map_err(|e| {
                            GitKvError::GitObjectError(format!("Failed to find file object: {e}"))
                        })?;

                    match file_obj.kind {
                        gix::object::Kind::Blob => return Ok(file_obj.data.to_vec()),
                        _ => {
                            return Err(GitKvError::GitObjectError(
                                "File is not a blob".to_string(),
                            ))
                        }
                    }
                } else {
                    let mut subtree_buffer = Vec::new();
                    let subtree_obj = self
                        .repo
                        .objects
                        .find(entry.oid, &mut subtree_buffer)
                        .map_err(|e| {
                            GitKvError::GitObjectError(format!(
                                "Failed to find subtree object: {e}"
                            ))
                        })?;

                    match subtree_obj.kind {
                        gix::object::Kind::Tree => {
                            let subtree = gix::objs::TreeRef::from_bytes(subtree_obj.data)
                                .map_err(|e| {
                                    GitKvError::GitObjectError(format!(
                                        "Failed to parse subtree: {e}"
                                    ))
                                })?;
                            return self.read_file_from_tree_recursive(
                                &subtree,
                                path_parts,
                                part_index + 1,
                            );
                        }
                        _ => {
                            return Err(GitKvError::GitObjectError(
                                "Expected directory but found file".to_string(),
                            ))
                        }
                    }
                }
            }
        }

        Err(GitKvError::GitObjectError(format!(
            "Path component '{current_part}' not found in tree"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};
    use tempfile::TempDir;

    fn run_git(repo_path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .output()
            .expect("failed to run git");
        assert!(
            output.status.success(),
            "git {:?} failed: stdout={} stderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git output should be UTF-8")
    }

    fn write_commit_object(repo_path: &Path, commit_content: &str) -> String {
        let mut child = Command::new("git")
            .args(["hash-object", "-t", "commit", "-w", "--stdin"])
            .current_dir(repo_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to run git hash-object");
        child
            .stdin
            .as_mut()
            .expect("hash-object stdin")
            .write_all(commit_content.as_bytes())
            .expect("failed to write commit content");
        let output = child.wait_with_output().expect("failed to hash commit");
        assert!(
            output.status.success(),
            "git hash-object failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("object id should be UTF-8")
            .trim()
            .to_string()
    }

    #[test]
    fn walk_history_reports_missing_parent_commit() {
        let temp_dir = TempDir::new().unwrap();
        gix::init(temp_dir.path()).unwrap();
        run_git(temp_dir.path(), &["config", "user.name", "Test User"]);
        run_git(
            temp_dir.path(),
            &["config", "user.email", "test@example.com"],
        );

        std::fs::write(temp_dir.path().join("data.txt"), "base\n").unwrap();
        run_git(temp_dir.path(), &["add", "data.txt"]);
        run_git(temp_dir.path(), &["commit", "-m", "base"]);

        let tree_id = run_git(temp_dir.path(), &["rev-parse", "HEAD^{tree}"])
            .trim()
            .to_string();
        let missing_parent = "1111111111111111111111111111111111111111";
        let bad_commit = write_commit_object(
            temp_dir.path(),
            &format!(
                "tree {tree_id}\nparent {missing_parent}\nauthor Test User <test@example.com> 0 +0000\ncommitter Test User <test@example.com> 0 +0000\n\nbad parent\n"
            ),
        );
        run_git(temp_dir.path(), &["update-ref", "HEAD", &bad_commit]);

        let repo = gix::open(temp_dir.path()).unwrap();
        let backend = GitMetadataBackend::new(repo);
        let err = match backend.walk_history(100) {
            Ok(history) => panic!("history with a missing parent must fail, got {history:?}"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("parent") || err.to_string().contains("commit"),
            "unexpected error: {err}"
        );
    }
}

impl MetadataBackend for GitMetadataBackend {
    // ── Path access ──────────────────────────────────────────────────

    fn metadata_dir(&self) -> &Path {
        self.repo.path()
    }

    fn work_dir(&self) -> Option<PathBuf> {
        self.repo.workdir().map(|p| p.to_path_buf())
    }

    // ── User identity ────────────────────────────────────────────────

    fn user_config(&self) -> (String, String) {
        let config = self.repo.config_snapshot();
        let name = config
            .string("user.name")
            .map(|n| n.to_string())
            .unwrap_or_else(|| "git-prolly".to_string());
        let email = config
            .string("user.email")
            .map(|e| e.to_string())
            .unwrap_or_else(|| "git-prolly@example.com".to_string());
        (name, email)
    }

    // ── Commit creation ──────────────────────────────────────────────

    fn stage_and_write_tree(&self, git_root: &Path) -> Result<gix::ObjectId, GitKvError> {
        let add_cmd = std::process::Command::new("git")
            .args(["add", "-A", "."])
            .current_dir(git_root)
            .output()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to run git add: {e}")))?;

        if !add_cmd.status.success() {
            let stderr = String::from_utf8_lossy(&add_cmd.stderr);
            return Err(GitKvError::GitObjectError(format!(
                "git add failed: {stderr}"
            )));
        }

        let write_tree_cmd = std::process::Command::new("git")
            .args(["write-tree"])
            .current_dir(git_root)
            .output()
            .map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to run git write-tree: {e}"))
            })?;

        if !write_tree_cmd.status.success() {
            let stderr = String::from_utf8_lossy(&write_tree_cmd.stderr);
            return Err(GitKvError::GitObjectError(format!(
                "git write-tree failed: {stderr}"
            )));
        }

        let tree_hash = String::from_utf8_lossy(&write_tree_cmd.stdout)
            .trim()
            .to_string();
        gix::ObjectId::from_hex(tree_hash.as_bytes())
            .map_err(|e| GitKvError::GitObjectError(format!("Invalid tree hash: {e}")))
    }

    fn write_commit(
        &self,
        tree_id: gix::ObjectId,
        message: &str,
    ) -> Result<gix::ObjectId, GitKvError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| GitKvError::GitObjectError(format!("System time error: {e}")))?
            .as_secs() as i64;

        let (name, email) = self.user_config();

        let signature = gix::actor::Signature {
            name: name.into(),
            email: email.into(),
            time: gix::date::Time {
                seconds: now,
                offset: 0,
            },
        };

        let parent_ids = match self.repo.head_commit() {
            Ok(parent) => vec![parent.id().into()],
            Err(_) => vec![],
        };

        let commit = gix::objs::Commit {
            tree: tree_id,
            parents: parent_ids.into(),
            author: signature.clone(),
            committer: signature,
            encoding: None,
            message: message.as_bytes().into(),
            extra_headers: vec![],
        };

        self.repo
            .objects
            .write(&commit)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to write commit: {e}")))
    }

    // ── Reference management ─────────────────────────────────────────

    fn head_commit_id(&self) -> Result<gix::ObjectId, GitKvError> {
        let head = self
            .repo
            .head()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get HEAD: {e}")))?;
        let id = head.id().ok_or_else(|| {
            GitKvError::GitObjectError("HEAD does not point to a commit".to_string())
        })?;
        Ok(id.detach())
    }

    fn branch_commit_id(&self, branch: &str) -> Result<gix::ObjectId, GitKvError> {
        let branch_ref = format!("refs/heads/{branch}");
        match self.repo.refs.find(&branch_ref) {
            Ok(reference) => match reference.target.try_id() {
                Some(commit_id) => Ok(commit_id.to_owned()),
                None => Err(GitKvError::GitObjectError(format!(
                    "Branch {branch} does not point to a commit"
                ))),
            },
            Err(_) => Err(GitKvError::BranchNotFound(branch.to_string())),
        }
    }

    fn update_branch(&self, branch: &str, commit_id: gix::ObjectId) -> Result<(), GitKvError> {
        let refs_dir = self.repo.path().join("refs").join("heads");
        std::fs::create_dir_all(&refs_dir).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to create refs directory: {e}"))
        })?;

        let branch_file = refs_dir.join(branch);
        if let Some(parent) = branch_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to create branch directory: {e}"))
            })?;
        }

        std::fs::write(&branch_file, commit_id.to_hex().to_string()).map_err(|e| {
            GitKvError::GitObjectError(format!("Failed to write branch reference: {e}"))
        })
    }

    fn update_head(&self, branch: &str) -> Result<(), GitKvError> {
        let head_file = self.repo.path().join("HEAD");
        let head_content = format!("ref: refs/heads/{branch}\n");
        std::fs::write(&head_file, head_content)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to write HEAD: {e}")))
    }

    fn create_branch(&self, name: &str) -> Result<(), GitKvError> {
        let head_id = self.head_commit_id()?;
        self.update_branch(name, head_id)
    }

    fn list_branches(&self) -> Result<Vec<String>, GitKvError> {
        let mut branches = Vec::new();
        let refs = self
            .repo
            .refs
            .iter()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to iterate refs: {e}")))?;

        for reference in (refs
            .all()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to get refs: {e}")))?)
        .flatten()
        {
            if let Some(name) = reference.name.as_bstr().strip_prefix(b"refs/heads/") {
                branches.push(String::from_utf8_lossy(name).to_string());
            }
        }

        branches.sort();
        Ok(branches)
    }

    fn resolve_reference(&self, reference: &str) -> Result<gix::ObjectId, GitKvError> {
        // Try branch
        if let Ok(mut branch_ref) = self.repo.find_reference(&format!("refs/heads/{reference}")) {
            if let Ok(peeled) = branch_ref.peel_to_id() {
                return Ok(peeled.detach());
            }
        }

        // Try commit SHA
        if let Ok(commit_id) = gix::ObjectId::from_hex(reference.as_bytes()) {
            let mut buffer = Vec::new();
            if self.repo.objects.find(&commit_id, &mut buffer).is_ok() {
                return Ok(commit_id);
            }
        }

        // Try other reference formats
        if let Ok(mut reference) = self.repo.find_reference(reference) {
            if let Ok(peeled) = reference.peel_to_id() {
                return Ok(peeled.detach());
            }
        }

        Err(GitKvError::InvalidCommit(format!(
            "Reference '{reference}' not found"
        )))
    }

    // ── History ──────────────────────────────────────────────────────

    fn walk_history(&self, limit: usize) -> Result<Vec<CommitInfo>, GitKvError> {
        let mut history = Vec::new();

        let head_commit = match self.repo.head_commit() {
            Ok(commit) => commit,
            Err(_) => return Ok(history),
        };

        let rev_walk = self.repo.rev_walk([head_commit.id()]);

        let walk = rev_walk
            .all()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to walk history: {e}")))?;

        for info in walk.take(limit) {
            let info = info.map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to read commit history: {e}"))
            })?;
            let commit_obj = info.object().map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to load history commit: {e}"))
            })?;
            let commit_ref = commit_obj.decode().map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to decode history commit: {e}"))
            })?;
            let author = commit_ref.author().map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to decode history author: {e}"))
            })?;
            let committer = commit_ref.committer().map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to decode history committer: {e}"))
            })?;
            let timestamp = author.time().map(|t| t.seconds).unwrap_or(0);
            history.push(CommitInfo {
                id: commit_obj.id().into(),
                author: format!(
                    "{} <{}>",
                    String::from_utf8_lossy(author.name),
                    String::from_utf8_lossy(author.email)
                ),
                committer: format!(
                    "{} <{}>",
                    String::from_utf8_lossy(committer.name),
                    String::from_utf8_lossy(committer.email)
                ),
                message: String::from_utf8_lossy(commit_ref.message).to_string(),
                timestamp,
            });
        }

        Ok(history)
    }

    fn commit_parents(&self, commit_id: &gix::ObjectId) -> Result<Vec<gix::ObjectId>, GitKvError> {
        let mut buffer = Vec::new();
        let commit_obj = self
            .repo
            .objects
            .find(commit_id, &mut buffer)
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to find commit: {e}")))?;

        let commit_ref = commit_obj
            .decode()
            .map_err(|e| GitKvError::GitObjectError(format!("Failed to decode commit: {e}")))?
            .into_commit()
            .ok_or_else(|| GitKvError::GitObjectError("Object is not a commit".to_string()))?;

        Ok(commit_ref.parents().collect())
    }

    // ── File access from commits ─────────────────────────────────────

    fn read_file_at_commit(
        &self,
        commit_id: &gix::ObjectId,
        file_path: &str,
    ) -> Result<Vec<u8>, GitKvError> {
        let mut commit_buffer = Vec::new();
        let commit_obj = self
            .repo
            .objects
            .find(commit_id, &mut commit_buffer)
            .map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to find commit {commit_id}: {e}"))
            })?;

        let commit = match commit_obj.kind {
            gix::object::Kind::Commit => gix::objs::CommitRef::from_bytes(commit_obj.data)
                .map_err(|e| GitKvError::GitObjectError(format!("Failed to parse commit: {e}")))?,
            _ => {
                return Err(GitKvError::InvalidCommit(format!(
                    "{commit_id} is not a commit"
                )))
            }
        };

        let tree_id = commit.tree();

        let mut tree_buffer = Vec::new();
        let tree_obj = self
            .repo
            .objects
            .find(&tree_id, &mut tree_buffer)
            .map_err(|e| {
                GitKvError::GitObjectError(format!("Failed to find tree {tree_id}: {e}"))
            })?;

        let tree = match tree_obj.kind {
            gix::object::Kind::Tree => gix::objs::TreeRef::from_bytes(tree_obj.data)
                .map_err(|e| GitKvError::GitObjectError(format!("Failed to parse tree: {e}")))?,
            _ => {
                return Err(GitKvError::GitObjectError(format!(
                    "{tree_id} is not a tree"
                )))
            }
        };

        let path_parts: Vec<&str> = file_path.split('/').collect();
        self.read_file_from_tree_recursive(&tree, &path_parts, 0)
    }
}
