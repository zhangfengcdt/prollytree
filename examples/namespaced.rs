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

//! NamespacedKvStore example
//!
//! Demonstrates the multi-tree counterpart of `GitVersionedKvStore`:
//!
//!   - Multiple isolated namespaces in one git repo, each with its own
//!     prolly tree.
//!   - Single atomic commit covering every dirty namespace.
//!   - Branching is store-wide — every namespace flips together on checkout.
//!   - Same key in two namespaces resolves to independent values.

use prollytree::git::versioned_store::GitNamespacedKvStore;
use std::process::Command;
use tempfile::TempDir;

fn init_git_repo(repo_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()?;
    Command::new("git")
        .args(["config", "user.name", "Namespaced Demo"])
        .current_dir(repo_path)
        .output()?;
    Command::new("git")
        .args(["config", "user.email", "demo@example.com"])
        .current_dir(repo_path)
        .output()?;
    Ok(())
}

fn demo_multiple_namespaces() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nDemo 1: Two namespaces, one store, one commit");
    println!("============================================================");

    let temp = TempDir::new()?;
    init_git_repo(temp.path())?;
    let dataset = temp.path().join("dataset");
    std::fs::create_dir_all(&dataset)?;

    let mut store = GitNamespacedKvStore::<32>::init(&dataset)?;

    // "users" namespace holds account records.
    {
        let mut users = store.namespace("users");
        users.insert(b"u:alice".to_vec(), b"Alice <alice@example.com>".to_vec())?;
        users.insert(b"u:bob".to_vec(), b"Bob <bob@example.com>".to_vec())?;
    }

    // "settings" namespace holds configuration. Same keys would be fine here
    // because each namespace owns its own tree.
    {
        let mut settings = store.namespace("settings");
        settings.insert(b"theme".to_vec(), b"dark".to_vec())?;
        settings.insert(b"locale".to_vec(), b"en_US".to_vec())?;
    }

    let commit_id = store.commit("seed users + settings")?;
    println!(
        "Single commit covering both namespaces: {}",
        &commit_id.to_string()[..10]
    );

    println!("Namespaces in store: {:?}", store.list_namespaces());

    // Per-namespace reads.
    println!(
        "users/u:alice    = {:?}",
        store
            .namespace("users")
            .get(b"u:alice")
            .map(|v| String::from_utf8_lossy(&v).into_owned())
    );
    println!(
        "settings/theme   = {:?}",
        store
            .namespace("settings")
            .get(b"theme")
            .map(|v| String::from_utf8_lossy(&v).into_owned())
    );

    Ok(())
}

fn demo_branching_per_namespace() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nDemo 2: Branching is store-wide; both namespaces follow");
    println!("============================================================");

    let temp = TempDir::new()?;
    init_git_repo(temp.path())?;
    let dataset = temp.path().join("dataset");
    std::fs::create_dir_all(&dataset)?;

    let mut store = GitNamespacedKvStore::<32>::init(&dataset)?;

    // Seed on main.
    {
        let mut users = store.namespace("users");
        users.insert(b"u:alice".to_vec(), b"Alice".to_vec())?;
    }
    {
        let mut settings = store.namespace("settings");
        settings.insert(b"theme".to_vec(), b"dark".to_vec())?;
    }
    store.commit("seed on main")?;
    println!(
        "main branch seeded; current branch = {}",
        store.current_branch()
    );

    // Create + switch to experiment branch and diverge both namespaces.
    store.create_branch("experiment")?;
    {
        let mut users = store.namespace("users");
        users.insert(b"u:carol".to_vec(), b"Carol".to_vec())?;
    }
    {
        let mut settings = store.namespace("settings");
        settings.insert(b"theme".to_vec(), b"light".to_vec())?;
    }
    store.commit("experiment: add carol, switch to light")?;

    println!(
        "experiment branch: users   = {:?}",
        store.namespace("users").list_keys()
    );
    println!(
        "experiment branch: theme   = {:?}",
        store
            .namespace("settings")
            .get(b"theme")
            .map(|v| String::from_utf8_lossy(&v).into_owned())
    );

    // Switch back to main — both namespaces snap back.
    store.checkout("main")?;
    println!(
        "\nback on main; users        = {:?}",
        store.namespace("users").list_keys()
    );
    println!(
        "main branch theme          = {:?}",
        store
            .namespace("settings")
            .get(b"theme")
            .map(|v| String::from_utf8_lossy(&v).into_owned())
    );
    println!(
        "main does NOT see u:carol  = {:?}",
        store.namespace("users").get(b"u:carol")
    );

    Ok(())
}

fn demo_isolation_between_namespaces() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nDemo 3: Namespaces are fully isolated");
    println!("============================================================");

    let temp = TempDir::new()?;
    init_git_repo(temp.path())?;
    let dataset = temp.path().join("dataset");
    std::fs::create_dir_all(&dataset)?;

    let mut store = GitNamespacedKvStore::<32>::init(&dataset)?;
    {
        store
            .namespace("users")
            .insert(b"name".to_vec(), b"Alice".to_vec())?;
        store
            .namespace("products")
            .insert(b"name".to_vec(), b"Widget".to_vec())?;
    }
    store.commit("collision-free across namespaces")?;

    println!(
        "users/name    = {:?}",
        store
            .namespace("users")
            .get(b"name")
            .map(|v| String::from_utf8_lossy(&v).into_owned())
    );
    println!(
        "products/name = {:?}",
        store
            .namespace("products")
            .get(b"name")
            .map(|v| String::from_utf8_lossy(&v).into_owned())
    );
    println!("Each namespace owns its own key space — no collision.");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("NamespacedKvStore example");
    println!("============================================================");
    println!("Multiple isolated prolly trees in one git-versioned store.");

    demo_multiple_namespaces()?;
    demo_branching_per_namespace()?;
    demo_isolation_between_namespaces()?;

    println!("\nAll demos completed successfully.");
    println!("\nKey takeaways:");
    println!("- One NamespacedKvStore holds many independent prolly trees.");
    println!("- store.namespace(name) returns a per-namespace handle.");
    println!("- A single commit() lands every dirty namespace atomically.");
    println!("- create_branch + checkout flip every namespace at once.");
    println!("- For text search on a namespace, see the `text_index` example.");
    Ok(())
}
