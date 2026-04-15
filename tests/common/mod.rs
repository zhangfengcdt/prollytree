// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Shared test helpers for integration tests.
//!
//! Each integration test binary includes this module via `mod common;`.
//! Not every binary uses every helper, so we suppress dead-code warnings.

use std::path::PathBuf;
use tempfile::TempDir;

/// Create a temporary directory with an initialized git repository.
///
/// Sets `user.name` and `user.email` so that commits work without global config.
#[allow(dead_code)]
pub fn setup_git_repo() -> TempDir {
    let temp = TempDir::new().expect("failed to create temp dir");
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run git init");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp.path())
        .output()
        .expect("failed to set git user.name");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(temp.path())
        .output()
        .expect("failed to set git user.email");
    temp
}

/// Create a `dataset` subdirectory inside the given temp dir.
///
/// Stores cannot be initialized at the git repository root, so all tests
/// must use a subdirectory.
#[allow(dead_code)]
pub fn setup_dataset(temp: &TempDir) -> PathBuf {
    let dataset = temp.path().join("dataset");
    std::fs::create_dir_all(&dataset).expect("failed to create dataset dir");
    dataset
}

/// RAII guard that restores the working directory on drop.
#[allow(dead_code)]
pub struct CwdGuard {
    original: PathBuf,
}

#[allow(dead_code)]
impl CwdGuard {
    pub fn new() -> Self {
        CwdGuard {
            original: std::env::current_dir().expect("failed to get cwd"),
        }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

/// Set up a git repo + dataset + git config inside the dataset, returning both.
#[allow(dead_code)]
pub fn setup_repo_and_dataset() -> (TempDir, PathBuf) {
    let temp = setup_git_repo();
    let dataset = setup_dataset(&temp);

    // Also configure git inside the dataset dir (some operations cd there)
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&dataset)
        .output()
        .ok();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&dataset)
        .output()
        .ok();

    (temp, dataset)
}

/// Helper to set up a CLI test environment.
#[allow(dead_code)]
pub fn setup_cli_env() -> (TempDir, PathBuf) {
    let temp = setup_git_repo();
    let dataset = setup_dataset(&temp);

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&dataset)
        .output()
        .ok();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&dataset)
        .output()
        .ok();

    (temp, dataset)
}

/// Run the git-prolly binary with given args in the given directory.
#[allow(dead_code)]
#[cfg(feature = "git")]
pub fn git_prolly_cmd(dir: &std::path::Path) -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::cargo_bin("git-prolly").expect("binary not found");
    cmd.current_dir(dir);
    cmd
}
