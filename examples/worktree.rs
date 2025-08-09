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
//! This example demonstrates how to use the Git worktree-like functionality
//! for multi-agent systems with versioned key-value storage and intelligent merging.

use prollytree::diff::{AgentPriorityResolver, SemanticMergeResolver};
use prollytree::git::versioned_store::GitVersionedKvStore;
use prollytree::git::worktree::{WorktreeManager, WorktreeVersionedKvStore};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Multi-Agent Worktree Integration Demo");
    println!("=========================================");

    // Setup temporary directory for demo
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path();

    // Initialize Git repository
    init_git_repo(repo_path)?;

    // Create the main versioned store
    let mut main_store = GitVersionedKvStore::<16>::init(repo_path.join("main_data"))?;

    // Add some initial shared data
    main_store.insert(
        b"shared_config".to_vec(),
        br#"{"version": 1, "mode": "production"}"#.to_vec(),
    )?;
    main_store.insert(b"global_counter".to_vec(), b"0".to_vec())?;
    main_store.commit("Initial shared data")?;

    println!("✅ Initialized main repository with shared data");

    // Create worktree manager
    let worktree_manager = WorktreeManager::new(repo_path)?;
    let manager_arc = Arc::new(Mutex::new(worktree_manager));

    // Simulate multiple AI agents working concurrently
    let agent_scenarios = vec![
        ("agent1", "customer-service", "Handle customer inquiries"),
        ("agent2", "data-analysis", "Analyze user behavior"),
        ("agent3", "content-generation", "Generate marketing content"),
    ];

    let mut agent_worktrees = Vec::new();

    // Create isolated worktrees for each agent
    for (agent_id, session_id, description) in &agent_scenarios {
        let agent_path = temp_dir.path().join(format!("{}_workspace", agent_id));
        let branch_name = format!("{}-{}", agent_id, session_id);

        // Add worktree
        let worktree_info = {
            let mut manager = manager_arc.lock().unwrap();
            manager.add_worktree(&agent_path, &branch_name, true)?
        };

        // Create WorktreeVersionedKvStore for the agent
        let agent_store =
            WorktreeVersionedKvStore::<16>::from_worktree(worktree_info, manager_arc.clone())?;

        agent_worktrees.push((agent_id.to_string(), agent_store, description.to_string()));

        println!(
            "🏗️  Created isolated workspace for {} on branch {}",
            agent_id, branch_name
        );
    }

    // Simulate each agent working on their specific tasks
    println!("\n📊 Agents performing their tasks...");

    for (agent_id, worktree_store, description) in &mut agent_worktrees {
        println!("   {} working on: {}", agent_id, description);

        // Each agent modifies different aspects of the data
        match agent_id.as_str() {
            "agent1" => {
                // Customer service agent updates customer data
                worktree_store.store_mut().insert(
                    b"customer:latest".to_vec(),
                    br#"{"id": "cust_001", "issue": "billing", "priority": "high"}"#.to_vec(),
                )?;
                worktree_store.store_mut().insert(
                    b"response_templates".to_vec(),
                    br#"{"billing": "Thank you for contacting us about billing..."}"#.to_vec(),
                )?;
                worktree_store
                    .store_mut()
                    .commit("Customer service data updates")?;
            }
            "agent2" => {
                // Data analysis agent updates analytics
                worktree_store.store_mut().insert(
                    b"analytics:daily".to_vec(),
                    br#"{"users": 1250, "sessions": 3400, "conversion": 0.045}"#.to_vec(),
                )?;
                worktree_store
                    .store_mut()
                    .insert(b"global_counter".to_vec(), b"25".to_vec())?; // This will create a conflict!
                worktree_store
                    .store_mut()
                    .commit("Analytics data updates")?;
            }
            "agent3" => {
                // Content generation agent creates marketing content
                worktree_store.store_mut().insert(
                    b"content:campaign".to_vec(),
                    br#"{"title": "Summer Sale", "body": "Get 20% off...", "target": "email"}"#
                        .to_vec(),
                )?;
                worktree_store.store_mut().insert(
                    b"shared_config".to_vec(),
                    br#"{"version": 1, "mode": "production", "feature_flags": {"new_ui": true}}"#
                        .to_vec(),
                )?; // Potential conflict
                worktree_store
                    .store_mut()
                    .commit("Marketing content updates")?;
            }
            _ => {}
        }
    }

    println!("✅ All agents completed their tasks");

    // Now demonstrate different merge strategies
    println!("\n🔄 Merging agent work back to main repository...");

    // Strategy 1: Simple merge (ignore conflicts)
    println!("\n1️⃣  Simple merge (ignoring conflicts):");
    let merge_result1 = agent_worktrees[0]
        .1
        .merge_to_main(&mut main_store, "Merge customer service updates")?;
    println!("   {}", merge_result1);

    // Strategy 2: Semantic merge for structured data
    println!("\n2️⃣  Semantic merge (JSON-aware):");
    let semantic_resolver = SemanticMergeResolver::default();
    let merge_result2 = agent_worktrees[2].1.merge_to_branch_with_resolver(
        &mut main_store,
        "main",
        &semantic_resolver,
        "Merge content generation with semantic resolution",
    )?;
    println!("   {}", merge_result2);

    // Strategy 3: Priority-based merge
    println!("\n3️⃣  Priority-based merge:");
    let mut priority_resolver = AgentPriorityResolver::new();
    priority_resolver.set_agent_priority("agent2".to_string(), 10); // High priority for data analysis
    priority_resolver.set_agent_priority("agent1".to_string(), 5); // Medium priority

    let merge_result3 = agent_worktrees[1].1.merge_to_branch_with_resolver(
        &mut main_store,
        "main",
        &priority_resolver,
        "Merge analytics with priority resolution",
    )?;
    println!("   {}", merge_result3);

    // Verify final state
    println!("\n📋 Final merged state:");
    if let Some(config_value) = main_store.get(b"shared_config") {
        let config_json: serde_json::Value = serde_json::from_slice(&config_value)?;
        println!("   • shared_config: {}", config_json);
    }

    if let Some(counter_value) = main_store.get(b"global_counter") {
        let counter_str = String::from_utf8_lossy(&counter_value);
        println!("   • global_counter: {}", counter_str);
    }

    // Show conflict detection capabilities
    println!("\n🔍 Conflict detection example:");
    let mut temp_worktree = agent_worktrees.pop().unwrap().1;
    temp_worktree
        .store_mut()
        .insert(b"global_counter".to_vec(), b"999".to_vec())?; // Create a conflicting change
    temp_worktree.store_mut().commit("Conflicting update")?;

    match temp_worktree.try_merge_to_main(&mut main_store) {
        Ok(conflicts) => {
            if conflicts.is_empty() {
                println!("   ✅ No conflicts detected");
            } else {
                println!("   ⚠️  {} conflicts detected:", conflicts.len());
                for conflict in &conflicts {
                    println!("      - Key: {}", String::from_utf8_lossy(&conflict.key));
                }
            }
        }
        Err(e) => println!("   Error checking conflicts: {}", e),
    }

    println!("\n🎉 Multi-agent worktree integration demo completed!");
    println!("\n💡 Key capabilities demonstrated:");
    println!("   • Isolated workspaces for concurrent agents");
    println!("   • Git-like branching and merging for AI agent data");
    println!("   • Intelligent conflict resolution strategies");
    println!("   • Semantic merging for structured JSON data");
    println!("   • Priority-based agent coordination");
    println!("   • Conflict detection and prevention");

    Ok(())
}

fn init_git_repo(repo_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    // Initialize Git repository
    Command::new("git")
        .args(&["init"])
        .current_dir(repo_path)
        .output()?;

    // Configure Git user
    Command::new("git")
        .args(&["config", "user.name", "Multi-Agent Demo"])
        .current_dir(repo_path)
        .output()?;

    Command::new("git")
        .args(&["config", "user.email", "demo@multiagent.ai"])
        .current_dir(repo_path)
        .output()?;

    // Create initial commit
    std::fs::write(repo_path.join("README.md"), "# Multi-Agent Repository")?;
    Command::new("git")
        .args(&["add", "."])
        .current_dir(repo_path)
        .output()?;

    Command::new("git")
        .args(&["commit", "-m", "Initial repository setup"])
        .current_dir(repo_path)
        .output()?;

    Ok(())
}
