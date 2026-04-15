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

//! Cross-backend consistency tests.
//!
//! Runs identical operations against Git, InMemory, and File backends and
//! asserts that observable behaviour (keys, values, status, diffs) is the same.

#![cfg(feature = "git")]

mod common;

use prollytree::git::versioned_store::{
    FileVersionedKvStore, GitVersionedKvStore, InMemoryVersionedKvStore,
};

// ---------------------------------------------------------------------------
// Helper: create all three backends in separate dataset dirs
// ---------------------------------------------------------------------------

struct ThreeBackends {
    _temp: tempfile::TempDir,
    pub git: GitVersionedKvStore<32>,
    pub mem: InMemoryVersionedKvStore<32>,
    pub file: FileVersionedKvStore<32>,
}

fn create_three_backends() -> ThreeBackends {
    let temp = common::setup_git_repo();

    let git_ds = temp.path().join("ds_git");
    std::fs::create_dir_all(&git_ds).unwrap();
    let git = GitVersionedKvStore::<32>::init(&git_ds).expect("git init");

    let mem_ds = temp.path().join("ds_mem");
    std::fs::create_dir_all(&mem_ds).unwrap();
    let mem = InMemoryVersionedKvStore::<32>::init(&mem_ds).expect("mem init");

    let file_ds = temp.path().join("ds_file");
    std::fs::create_dir_all(&file_ds).unwrap();
    let file = FileVersionedKvStore::<32>::init(&file_ds).expect("file init");

    ThreeBackends {
        _temp: temp,
        git,
        mem,
        file,
    }
}

// ---------------------------------------------------------------------------
// Insert / Get consistency
// ---------------------------------------------------------------------------

#[test]
fn test_insert_get_consistency_across_backends() {
    let mut backends = create_three_backends();

    for i in 0..50 {
        let key = format!("k{i:03}").into_bytes();
        let val = format!("v{i:03}").into_bytes();
        backends.git.insert(key.clone(), val.clone()).unwrap();
        backends.mem.insert(key.clone(), val.clone()).unwrap();
        backends.file.insert(key.clone(), val.clone()).unwrap();
    }

    for i in 0..50 {
        let key = format!("k{i:03}").into_bytes();
        let expected = format!("v{i:03}").into_bytes();

        assert_eq!(
            backends.git.get(&key),
            Some(expected.clone()),
            "git get k{i:03}"
        );
        assert_eq!(
            backends.mem.get(&key),
            Some(expected.clone()),
            "mem get k{i:03}"
        );
        assert_eq!(backends.file.get(&key), Some(expected), "file get k{i:03}");
    }
}

// ---------------------------------------------------------------------------
// Commit and list keys
// ---------------------------------------------------------------------------

#[test]
fn test_commit_and_list_keys_across_backends() {
    let mut backends = create_three_backends();

    for i in 0..10 {
        let key = format!("lk{i}").into_bytes();
        let val = format!("lv{i}").into_bytes();
        backends.git.insert(key.clone(), val.clone()).unwrap();
        backends.mem.insert(key.clone(), val.clone()).unwrap();
        backends.file.insert(key.clone(), val.clone()).unwrap();
    }

    backends.git.commit("c1").unwrap();
    backends.mem.commit("c1").unwrap();
    backends.file.commit("c1").unwrap();

    let mut git_keys = backends.git.list_keys();
    let mut mem_keys = backends.mem.list_keys();
    let mut file_keys = backends.file.list_keys();

    git_keys.sort();
    mem_keys.sort();
    file_keys.sort();

    assert_eq!(git_keys, mem_keys, "git vs mem key lists");
    assert_eq!(git_keys, file_keys, "git vs file key lists");
}

// ---------------------------------------------------------------------------
// Status shows same staging
// ---------------------------------------------------------------------------

#[test]
fn test_status_shows_same_staging_across_backends() {
    let mut backends = create_three_backends();

    // Stage an insert on all backends
    backends
        .git
        .insert(b"staged".to_vec(), b"val".to_vec())
        .unwrap();
    backends
        .mem
        .insert(b"staged".to_vec(), b"val".to_vec())
        .unwrap();
    backends
        .file
        .insert(b"staged".to_vec(), b"val".to_vec())
        .unwrap();

    let git_status = backends.git.status();
    let mem_status = backends.mem.status();
    let file_status = backends.file.status();

    // All should have exactly one staged insert
    assert_eq!(
        git_status.len(),
        mem_status.len(),
        "status count git vs mem"
    );
    assert_eq!(
        git_status.len(),
        file_status.len(),
        "status count git vs file"
    );
    assert_eq!(git_status.len(), 1);
}

// ---------------------------------------------------------------------------
// Commit history across backends
// ---------------------------------------------------------------------------

#[test]
fn test_commit_history_across_backends() {
    let mut backends = create_three_backends();

    for round in 0..3 {
        let key = format!("round{round}").into_bytes();
        backends.git.insert(key.clone(), b"v".to_vec()).unwrap();
        backends.mem.insert(key.clone(), b"v".to_vec()).unwrap();
        backends.file.insert(key.clone(), b"v".to_vec()).unwrap();

        let msg = format!("commit {round}");
        backends.git.commit(&msg).unwrap();
        backends.mem.commit(&msg).unwrap();
        backends.file.commit(&msg).unwrap();
    }

    let git_log = backends.git.log().unwrap();
    let mem_log = backends.mem.log().unwrap();
    let file_log = backends.file.log().unwrap();

    // All should have the initial commit + 3 commits (init creates one)
    assert_eq!(
        git_log.len(),
        mem_log.len(),
        "log length git({}) vs mem({})",
        git_log.len(),
        mem_log.len()
    );
    assert_eq!(
        git_log.len(),
        file_log.len(),
        "log length git({}) vs file({})",
        git_log.len(),
        file_log.len()
    );

    // Commit messages should match (sorted by message since ordering may differ)
    let mut git_msgs: Vec<_> = git_log.iter().map(|c| c.message.clone()).collect();
    let mut mem_msgs: Vec<_> = mem_log.iter().map(|c| c.message.clone()).collect();
    let mut file_msgs: Vec<_> = file_log.iter().map(|c| c.message.clone()).collect();
    git_msgs.sort();
    mem_msgs.sort();
    file_msgs.sort();
    assert_eq!(git_msgs, mem_msgs, "commit messages git vs mem");
    assert_eq!(git_msgs, file_msgs, "commit messages git vs file");
}
