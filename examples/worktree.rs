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

//! Multi-Agent Worktree Integration Example
//!
//! Demonstrates the parts of the `WorktreeManager` API that are exposed for
//! multi-agent coordination:
//!   - Creating multiple isolated agent workspaces inside one parent git repo.
//!   - Listing, locking, and removing worktrees via `WorktreeManager`.
//!   - Each agent owns an independent `GitVersionedKvStore` for its work.
//!   - Bringing changes back together by writing into a shared main store
//!     with `SemanticMergeResolver` for JSON-aware reconciliation.
//!
//! Note: `WorktreeVersionedKvStore::from_worktree` requires a fully populated
//! worktree checkout (a feature still in flight on the worktree adapter — see
//! `src/git/worktree.rs`), so this example uses the supported pattern of
//! independent per-agent stores under the worktree directory.

use parking_lot::Mutex;
use prollytree::diff::{ConflictResolver, MergeConflict, MergeResult, SemanticMergeResolver};
use prollytree::git::versioned_store::GitVersionedKvStore;
use prollytree::git::worktree::WorktreeManager;
use std::sync::Arc;
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Multi-Agent Worktree Integration Demo");
    println!("=====================================");

    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path();

    init_git_repo(repo_path)?;

    // Main shared store lives in <repo>/data
    let mut main_store = GitVersionedKvStore::<16>::init(repo_path.join("data"))?;
    main_store.insert(
        b"shared_config".to_vec(),
        br#"{"version": 1, "mode": "production"}"#.to_vec(),
    )?;
    main_store.insert(b"global_counter".to_vec(), b"0".to_vec())?;
    main_store.commit("Initial shared data")?;
    println!("[main] Initialized shared repository with shared_config + global_counter");

    // The WorktreeManager tracks per-agent workspaces alongside the main repo.
    let manager_arc = Arc::new(Mutex::new(WorktreeManager::new(repo_path)?));

    let agent_scenarios = vec![
        (
            "agent1",
            "customer-service",
            br#"{"id": "cust_001", "issue": "billing", "priority": "high"}"#.as_slice(),
        ),
        (
            "agent2",
            "data-analysis",
            br#"{"users": 1250, "sessions": 3400, "conversion": 0.045}"#.as_slice(),
        ),
        (
            "agent3",
            "content-generation",
            br#"{"title": "Summer Sale", "body": "Get 20% off...", "target": "email"}"#.as_slice(),
        ),
    ];

    // Each agent gets its own workspace tracked by the manager + an
    // independent prolly store living in a sibling directory. The worktree
    // path holds the linked-worktree metadata; the prolly store sits in its
    // own self-contained git repo so we don't tangle the two layouts.
    let agent_data_root = temp_dir.path().join("agent_data");
    std::fs::create_dir_all(&agent_data_root)?;

    let mut agent_stores: Vec<(String, GitVersionedKvStore<16>, String)> = Vec::new();
    for (agent_id, session_id, _payload) in &agent_scenarios {
        let workspace = repo_path.join(format!("{}_workspace", agent_id));
        let branch_name = format!("{}-{}", agent_id, session_id);

        let worktree_info = {
            let mut manager = manager_arc.lock();
            manager.add_worktree(&workspace, &branch_name, true)?
        };
        println!(
            "[{}] Worktree id={} branch={}",
            agent_id, worktree_info.id, worktree_info.branch
        );

        // Independent prolly dataset in its own git repo (outside the
        // linked-worktree's path, which holds incomplete worktree metadata).
        let agent_repo = agent_data_root.join(agent_id);
        std::fs::create_dir_all(&agent_repo)?;
        init_git_repo(&agent_repo)?;
        let mut agent_store = GitVersionedKvStore::<16>::init(agent_repo.join("data"))?;
        agent_store.commit("agent workspace bootstrap")?;
        agent_stores.push((agent_id.to_string(), agent_store, branch_name));
    }

    // Each agent does its work in isolation.
    println!("\n[agents] running independent workloads...");
    for ((agent_id, agent_store, _branch), (_, _session, payload)) in
        agent_stores.iter_mut().zip(agent_scenarios.iter())
    {
        match agent_id.as_str() {
            "agent1" => {
                agent_store.insert(b"customer:latest".to_vec(), payload.to_vec())?;
                agent_store.insert(
                    b"response_templates".to_vec(),
                    br#"{"billing": "Thank you for contacting us about billing..."}"#.to_vec(),
                )?;
                agent_store.commit("agent1: customer-service updates")?;
            }
            "agent2" => {
                agent_store.insert(b"analytics:daily".to_vec(), payload.to_vec())?;
                agent_store.insert(b"global_counter".to_vec(), b"25".to_vec())?;
                agent_store.commit("agent2: analytics updates")?;
            }
            "agent3" => {
                agent_store.insert(b"content:campaign".to_vec(), payload.to_vec())?;
                agent_store.insert(
                    b"shared_config".to_vec(),
                    br#"{"version": 1, "mode": "production", "feature_flags": {"new_ui": true}}"#
                        .to_vec(),
                )?;
                agent_store.commit("agent3: content updates")?;
            }
            _ => {}
        }
    }
    println!("[agents] all completed.");

    // Bring agent work back into main store. Each (key, value) is inserted
    // into main with the SemanticMergeResolver applied when conflicts arise.
    println!("\n[merge] bringing agent work into main store...");
    let semantic = SemanticMergeResolver::default();
    for (agent_id, agent_store, _branch) in agent_stores.iter() {
        for key in agent_store.list_keys() {
            let value = agent_store.get(&key).expect("agent value");
            let merged = match main_store.get(&key) {
                Some(existing) if existing != value => {
                    let conflict = MergeConflict {
                        key: key.clone(),
                        base_value: None,
                        source_value: Some(value.clone()),
                        destination_value: Some(existing.clone()),
                    };
                    match semantic.resolve_conflict(&conflict) {
                        Some(MergeResult::Modified(_, merged_value))
                        | Some(MergeResult::Added(_, merged_value)) => merged_value,
                        _ => value.clone(),
                    }
                }
                Some(_) => value.clone(),
                None => value.clone(),
            };
            main_store.insert(key.clone(), merged)?;
        }
        main_store.commit(&format!("merge {}'s work into main", agent_id))?;
    }
    println!("[merge] done.");

    // Inspect the converged state.
    println!("\n[main] converged state:");
    for key in main_store.list_keys() {
        let value = main_store.get(&key).expect("key present");
        let key_str = String::from_utf8_lossy(&key);
        let val_str = String::from_utf8_lossy(&value);
        println!("  {} -> {}", key_str, val_str);
    }

    // Demonstrate WorktreeManager housekeeping: list, lock, remove.
    println!("\n[manager] worktree housekeeping...");
    {
        let manager = manager_arc.lock();
        let listed = manager.list_worktrees();
        println!("  {} worktrees registered", listed.len());
        for info in &listed {
            println!("    - id={} branch={}", info.id, info.branch);
        }
    }
    {
        let mut manager = manager_arc.lock();
        let first_id = manager
            .list_worktrees()
            .first()
            .map(|w| w.id.clone())
            .expect("at least one worktree");
        manager.lock_worktree(&first_id, "demo: hold for a long-running job")?;
        println!(
            "  locked worktree {}: {}",
            first_id,
            manager.is_locked(&first_id)
        );
        manager.unlock_worktree(&first_id)?;
        println!(
            "  unlocked worktree {}: {}",
            first_id,
            manager.is_locked(&first_id)
        );
    }

    println!("\nDemo complete.");
    Ok(())
}

fn init_git_repo(repo_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()?;
    Command::new("git")
        .args(["config", "user.name", "Multi-Agent Demo"])
        .current_dir(repo_path)
        .output()?;
    Command::new("git")
        .args(["config", "user.email", "demo@multiagent.ai"])
        .current_dir(repo_path)
        .output()?;
    std::fs::write(repo_path.join("README.md"), "# Multi-Agent Repository")?;
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "Initial repository setup"])
        .current_dir(repo_path)
        .output()?;
    Ok(())
}
