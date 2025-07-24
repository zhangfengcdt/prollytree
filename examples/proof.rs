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

//! Demonstration of Prolly Tree proof visualization
//!
//! This example showcases the new `print_proof` functionality that combines
//! tree visualization with cryptographic proof path highlighting.

use prollytree::config::TreeConfig;
use prollytree::storage::InMemoryNodeStorage;
use prollytree::tree::{ProllyTree, Tree};

fn main() {
    println!("🌳 Prolly Tree Proof Visualization Demo 🌳\n");

    // Create a prolly tree with configuration that will create a multi-level structure
    let storage = InMemoryNodeStorage::<32>::default();
    let config = TreeConfig {
        base: 131,
        modulus: 1_000_000_009,
        min_chunk_size: 3,
        max_chunk_size: 6,
        pattern: 0b111,
        root_hash: None,
        key_schema: None,
        value_schema: None,
        encode_types: vec![],
    };

    let mut tree = ProllyTree::new(storage, config);

    // Insert sequential data to create a structured tree
    println!("📊 Inserting data into the tree...");
    for i in 0..25 {
        tree.insert(vec![i], format!("value_{}", i).into_bytes());
    }

    // Show the regular tree structure first
    println!("\n📋 Regular tree structure:");
    tree.print();

    // Demonstrate proof visualization for different scenarios
    println!("\n🔍 Proof Visualization Examples:\n");

    // Example 1: Proof for an existing key
    let key1 = vec![10];
    println!("🟢 Example 1: Proof for existing key {:?}", key1);
    println!("   (Green nodes show the cryptographic proof path)");
    let is_valid1 = tree.print_proof(&key1);
    println!("   ✅ Proof validation result: {}\n", is_valid1);

    // Example 2: Proof for another existing key
    let key2 = vec![20];
    println!("🟢 Example 2: Proof for existing key {:?}", key2);
    let is_valid2 = tree.print_proof(&key2);
    println!("   ✅ Proof validation result: {}\n", is_valid2);

    // Example 3: Proof for a non-existing key
    let key3 = vec![30];
    println!("🔴 Example 3: Proof for non-existing key {:?}", key3);
    println!("   (Shows proof path to where the key would be located)");
    let is_valid3 = tree.print_proof(&key3);
    println!("   ❌ Proof validation result: {}\n", is_valid3);

    // Summary
    println!("🎯 Summary:");
    println!("   • The proof visualization highlights the cryptographic path");
    println!("   • Green nodes with hash information show the verification trail");
    println!("   • Valid proofs confirm data integrity and membership");
    println!("   • Invalid proofs demonstrate absence in a verifiable way");
    println!("\n✨ This enables transparent verification of data in distributed systems!");
}
