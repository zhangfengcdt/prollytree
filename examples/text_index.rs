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

//! Text-index example on `NamespacedKvStore`.
//!
//! Demonstrates:
//!   - Dual-write pattern: primary tree (source of truth) + text index
//!     (pointer structure that only stores `(id, vector)` pairs).
//!   - Top-k search resolved back to original text via the primary tree.
//!   - Cascade mode: primary writes auto-mirror into the registered index.
//!   - Multi-chunk indexing via `LineChunker` (one document, many chunks;
//!     search dedups back to the document id).
//!
//! Uses `HashEmbedder` to avoid network / model downloads. Swap in
//! `MiniLmEmbedder` (feature `proximity_text`) for real semantic search.

use prollytree::git::versioned_store::GitNamespacedKvStore;
use prollytree::proximity::{HashEmbedder, LineChunker, TextIndexConfig};
use std::process::Command;
use tempfile::TempDir;

fn init_git_repo(repo_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()?;
    Command::new("git")
        .args(["config", "user.name", "TextIndex Demo"])
        .current_dir(repo_path)
        .output()?;
    Command::new("git")
        .args(["config", "user.email", "demo@example.com"])
        .current_dir(repo_path)
        .output()?;
    Ok(())
}

fn demo_dual_write_and_search() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nDemo 1: Dual-write (primary + index) + resolve hits back to text");
    println!("============================================================");

    let temp = TempDir::new()?;
    init_git_repo(temp.path())?;
    let dataset = temp.path().join("dataset");
    std::fs::create_dir_all(&dataset)?;

    let mut store = GitNamespacedKvStore::<32>::init(&dataset)?;

    let docs: Vec<(&[u8], &str)> = vec![
        (b"doc:1", "the quick brown fox jumps over the lazy dog"),
        (b"doc:2", "rust is a systems programming language"),
        (b"doc:3", "merkle trees enable verifiable data structures"),
        (b"doc:4", "the fox and the hound are forest friends"),
    ];

    {
        let mut personal = store.namespace("personal");
        let mut idx =
            personal.text_index("docs", TextIndexConfig::new(HashEmbedder::new(64, 0)))?;
        for (id, text) in &docs {
            idx.insert(id, text)?;
        }
        // Drop the index handle so we can re-borrow `personal` for primary writes.
    }
    {
        // Primary tree carries the source bytes — without this, search hits
        // can't be resolved back to text, and a future reindex has no source.
        let mut personal = store.namespace("personal");
        for (id, text) in &docs {
            personal.insert(id.to_vec(), text.as_bytes().to_vec())?;
        }
    }
    store.commit("seed corpus + index")?;

    // Search returns (id_bytes, distance); resolve each id back to body bytes.
    let mut personal = store.namespace("personal");
    let mut idx = personal.text_index("docs", TextIndexConfig::new(HashEmbedder::new(64, 0)))?;
    let hits = idx.search("the quick brown fox", 2)?;
    println!("Query: 'the quick brown fox' -> top {}", hits.len());
    drop(idx);
    for hit in hits {
        let body = personal
            .get(&hit.id)
            .map(|v| String::from_utf8_lossy(&v).into_owned())
            .unwrap_or_default();
        println!(
            "  {:?}  distance={:.4}  body={:?}",
            String::from_utf8_lossy(&hit.id),
            hit.score,
            body
        );
    }
    Ok(())
}

fn demo_cascade_mode() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nDemo 2: Cascade — primary writes auto-mirror into the index");
    println!("============================================================");

    let temp = TempDir::new()?;
    init_git_repo(temp.path())?;
    let dataset = temp.path().join("dataset");
    std::fs::create_dir_all(&dataset)?;

    let mut store = GitNamespacedKvStore::<32>::init(&dataset)?;
    {
        let mut notes = store.namespace("notes");
        let _ = notes.text_index("by_body", TextIndexConfig::new(HashEmbedder::new(64, 0)))?;
    }
    store.set_cascade("notes", vec!["by_body".to_string()]);

    {
        let mut notes = store.namespace("notes");
        notes.insert(
            b"note:1".to_vec(),
            b"meeting with the platform team".to_vec(),
        )?;
        notes.insert(
            b"note:2".to_vec(),
            b"draft proposal for Q3 roadmap".to_vec(),
        )?;
    }
    store.commit("cascade-driven indexing")?;

    let mut notes = store.namespace("notes");
    let mut idx = notes.text_index("by_body", TextIndexConfig::new(HashEmbedder::new(64, 0)))?;
    let hits = idx.search("platform meeting", 2)?;
    println!("Cascade-indexed search results:");
    for hit in &hits {
        println!(
            "  {:?}  distance={:.4}",
            String::from_utf8_lossy(&hit.id),
            hit.score
        );
    }

    println!(
        "\nIndex holds {} document(s) after cascade",
        idx.len()
    );
    Ok(())
}

fn demo_line_chunker() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nDemo 3: Multi-chunk indexing via LineChunker");
    println!("============================================================");

    let temp = TempDir::new()?;
    init_git_repo(temp.path())?;
    let dataset = temp.path().join("dataset");
    std::fs::create_dir_all(&dataset)?;

    let mut store = GitNamespacedKvStore::<32>::init(&dataset)?;
    let log = "2026-05-20T09:00 startup: loading config\n\
               2026-05-20T09:01 startup: bound port 8080\n\
               2026-05-20T09:42 error: database timeout after 30s\n\
               2026-05-20T09:43 retry: reconnecting to database\n\
               2026-05-20T09:43 recovery: database connection restored\n";

    {
        let mut logs = store.namespace("logs");
        let mut idx = logs.text_index(
            "by_line",
            TextIndexConfig::new(HashEmbedder::new(64, 0)).with_chunker(LineChunker),
        )?;
        idx.insert(b"log:2026-05-20", log)?;
        println!(
            "len = {} document, chunk_count = {} chunks",
            idx.len(),
            idx.chunk_count()
        );
    }
    store.commit("ingest log file")?;

    let mut logs = store.namespace("logs");
    let mut idx = logs.text_index(
        "by_line",
        TextIndexConfig::new(HashEmbedder::new(64, 0)).with_chunker(LineChunker),
    )?;
    let hits = idx.search("database timeout", 3)?;
    println!("Query: 'database timeout' (dedupped by document):");
    for hit in &hits {
        println!(
            "  {:?}  distance={:.4}",
            String::from_utf8_lossy(&hit.id),
            hit.score
        );
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Text indexing on NamespacedKvStore");
    println!("============================================================");
    println!("Uses HashEmbedder (deterministic, no deps). For real semantic");
    println!("search swap in `MiniLmEmbedder` (feature `proximity_text`).");

    demo_dual_write_and_search()?;
    demo_cascade_mode()?;
    demo_line_chunker()?;

    println!("\nAll demos completed successfully.");
    println!("\nKey takeaways:");
    println!("- Primary tree is the source of truth; text index stores only");
    println!("  (id, vector) pairs. Dual-write (demo 1) or cascade (demo 2).");
    println!("- text_index(name, config) creates or re-opens; embedder identity");
    println!("  is persisted and validated on reopen.");
    println!("- with_chunker(LineChunker) splits one doc into per-line chunks;");
    println!("  search dedups results back to the document id.");
    Ok(())
}
