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

//! Integration tests for the `git-prolly` CLI binary.
//!
//! Each test creates an isolated temporary git repository, initializes a
//! dataset, and exercises CLI commands end-to-end.

#![cfg(all(feature = "git", feature = "sql"))]

mod common;

use predicates::prelude::*;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn prolly(dir: &std::path::Path) -> assert_cmd::Command {
    common::git_prolly_cmd(dir)
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

#[test]
fn test_cli_init_creates_store() {
    let (_temp, dataset) = common::setup_cli_env();

    prolly(&dataset).arg("init").assert().success();

    // Verify prolly config was created inside the dataset directory
    let config_path = dataset.join("prolly_config_tree_config");
    assert!(
        config_path.exists(),
        "prolly config should exist after init at {:?}",
        config_path
    );
}

// ---------------------------------------------------------------------------
// Set / Get roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_cli_set_get_roundtrip() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    prolly(&dataset)
        .args(["set", "mykey", "myvalue"])
        .assert()
        .success();

    prolly(&dataset)
        .args(["get", "mykey"])
        .assert()
        .success()
        .stdout(predicate::str::contains("myvalue"));
}

// ---------------------------------------------------------------------------
// Set, Commit, Get lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_cli_set_commit_get_lifecycle() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    prolly(&dataset)
        .args(["set", "k1", "v1"])
        .assert()
        .success();
    prolly(&dataset)
        .args(["set", "k2", "v2"])
        .assert()
        .success();
    prolly(&dataset)
        .args(["commit", "-m", "add k1 and k2"])
        .assert()
        .success();

    prolly(&dataset)
        .args(["get", "k1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("v1"));
    prolly(&dataset)
        .args(["get", "k2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("v2"));
}

// ---------------------------------------------------------------------------
// Delete and Status
// ---------------------------------------------------------------------------

#[test]
fn test_cli_delete_and_status() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    prolly(&dataset)
        .args(["set", "delme", "gone"])
        .assert()
        .success();
    prolly(&dataset)
        .args(["commit", "-m", "add delme"])
        .assert()
        .success();

    prolly(&dataset)
        .args(["delete", "delme"])
        .assert()
        .success();

    prolly(&dataset)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("delme"));
}

// ---------------------------------------------------------------------------
// List keys
// ---------------------------------------------------------------------------

#[test]
fn test_cli_list_keys() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    for i in 0..5 {
        prolly(&dataset)
            .args(["set", &format!("key{i}"), &format!("val{i}")])
            .assert()
            .success();
    }
    prolly(&dataset)
        .args(["commit", "-m", "add keys"])
        .assert()
        .success();

    let output = prolly(&dataset).arg("list").output().expect("list failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    for i in 0..5 {
        assert!(
            stdout.contains(&format!("key{i}")),
            "list should contain key{i}"
        );
    }

    // list --values should include values
    let output = prolly(&dataset)
        .args(["list", "--values"])
        .output()
        .expect("list --values failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("val0"), "list --values should show values");
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

#[test]
fn test_cli_stats() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    for i in 0..10 {
        prolly(&dataset)
            .args(["set", &format!("s{i}"), &format!("v{i}")])
            .assert()
            .success();
    }
    prolly(&dataset)
        .args(["commit", "-m", "data"])
        .assert()
        .success();

    prolly(&dataset).arg("stats").assert().success();
}

// ---------------------------------------------------------------------------
// Get nonexistent key
// ---------------------------------------------------------------------------

#[test]
fn test_cli_get_nonexistent_key_exits_nonzero() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    prolly(&dataset)
        .args(["get", "does_not_exist"])
        .assert()
        .failure();
}

// ---------------------------------------------------------------------------
// Commit with empty staging
// ---------------------------------------------------------------------------

#[test]
fn test_cli_commit_empty_staging() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    // Committing with no staged changes should indicate nothing to commit
    let output = prolly(&dataset)
        .args(["commit", "-m", "empty"])
        .output()
        .expect("commit failed");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // Should mention no changes or empty staging
    assert!(
        combined.to_lowercase().contains("no")
            || combined.to_lowercase().contains("empty")
            || combined.to_lowercase().contains("nothing"),
        "commit of empty staging should indicate nothing to do, got: {combined}"
    );
}

// ---------------------------------------------------------------------------
// SQL create and select
// ---------------------------------------------------------------------------

#[cfg(feature = "sql")]
#[test]
fn test_cli_sql_create_and_select() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    prolly(&dataset)
        .args(["sql", "CREATE TABLE users (id INTEGER, name TEXT)"])
        .assert()
        .success();

    prolly(&dataset)
        .args(["sql", "INSERT INTO users VALUES (1, 'Alice')"])
        .assert()
        .success();

    prolly(&dataset)
        .args(["sql", "SELECT * FROM users"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice"));
}

// ---------------------------------------------------------------------------
// SQL JSON output
// ---------------------------------------------------------------------------

#[cfg(feature = "sql")]
#[test]
fn test_cli_sql_json_output() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    prolly(&dataset)
        .args(["sql", "CREATE TABLE items (id INTEGER, name TEXT)"])
        .assert()
        .success();

    prolly(&dataset)
        .args(["sql", "INSERT INTO items VALUES (1, 'Widget')"])
        .assert()
        .success();

    prolly(&dataset)
        .args(["sql", "--format", "json", "SELECT * FROM items"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Widget"));
}

// ---------------------------------------------------------------------------
// Clear with confirm
// ---------------------------------------------------------------------------

#[test]
fn test_cli_clear_with_confirm() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    prolly(&dataset)
        .args(["set", "clearme", "val"])
        .assert()
        .success();
    prolly(&dataset)
        .args(["commit", "-m", "add data"])
        .assert()
        .success();

    prolly(&dataset)
        .args(["clear", "--confirm"])
        .assert()
        .success();

    // After clear, the key should be gone
    prolly(&dataset).args(["get", "clearme"]).assert().failure();
}

// ---------------------------------------------------------------------------
// History for a key
// ---------------------------------------------------------------------------

#[test]
fn test_cli_history_for_key() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    prolly(&dataset)
        .args(["set", "tracked", "v1"])
        .assert()
        .success();
    prolly(&dataset)
        .args(["commit", "-m", "first"])
        .assert()
        .success();

    prolly(&dataset)
        .args(["set", "tracked", "v2"])
        .assert()
        .success();
    prolly(&dataset)
        .args(["commit", "-m", "second"])
        .assert()
        .success();

    prolly(&dataset)
        .args(["history", "tracked"])
        .assert()
        .success()
        .stdout(predicate::str::contains("first").and(predicate::str::contains("second")));
}

// ---------------------------------------------------------------------------
// Show at commit
// ---------------------------------------------------------------------------

#[test]
fn test_cli_show_at_commit() {
    let (_temp, dataset) = common::setup_cli_env();
    prolly(&dataset).arg("init").assert().success();

    prolly(&dataset)
        .args(["set", "showkey", "showval"])
        .assert()
        .success();
    prolly(&dataset)
        .args(["commit", "-m", "show test"])
        .assert()
        .success();

    prolly(&dataset)
        .args(["show", "HEAD"])
        .assert()
        .success()
        .stdout(predicate::str::contains("showkey"));
}
