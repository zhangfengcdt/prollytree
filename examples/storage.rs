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

use prollytree::config::TreeConfig;
use prollytree::storage::RocksDBNodeStorage;
use prollytree::tree::{ProllyTree, Tree};
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for RocksDB
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("prolly_rocksdb");

    println!("Creating RocksDB storage at: {:?}", db_path);

    // Create RocksDB storage
    let storage = RocksDBNodeStorage::<32>::new(db_path)?;

    // Create a ProllyTree with RocksDB storage
    let config = TreeConfig::<32>::default();
    let mut tree = ProllyTree::new(storage, config);

    // Insert some data
    println!("Inserting data into ProllyTree with RocksDB storage...");
    for i in 0..1000 {
        let key = format!("key_{:04}", i);
        let value = format!("value_{:04}", i);
        tree.insert(key.as_bytes().to_vec(), value.as_bytes().to_vec());
    }

    println!("Inserted 1000 key-value pairs");

    // Retrieve and verify data
    println!("Retrieving data...");
    for i in 0..10 {
        let key = format!("key_{:04}", i);
        let node = tree.find(key.as_bytes());
        if let Some(node) = node {
            // Find the key in the node and get its corresponding value
            if let Some(idx) = node.keys.iter().position(|k| k == key.as_bytes()) {
                let value = &node.values[idx];
                println!("  {} -> {}", key, String::from_utf8_lossy(value));
            }
        }
    }

    // Get tree statistics
    let root_hash = tree.get_root_hash();
    println!("\nTree root hash: {:?}", root_hash);

    // Demonstrate persistence by creating a new tree with the same storage
    drop(tree);
    println!("\nCreating new tree with same storage...");

    // Re-open the storage
    let storage2 = RocksDBNodeStorage::<32>::new(temp_dir.path().join("prolly_rocksdb"))?;
    let config2 = TreeConfig::<32>::default();
    let tree2 =
        ProllyTree::load_from_storage(storage2, config2).expect("Failed to load tree from storage");

    // Verify data is still there
    println!("Verifying persistence...");
    for i in 0..5 {
        let key = format!("key_{:04}", i);
        let node = tree2.find(key.as_bytes());
        if let Some(node) = node {
            // Find the key in the node and get its corresponding value
            if let Some(idx) = node.keys.iter().position(|k| k == key.as_bytes()) {
                let value = &node.values[idx];
                println!(
                    "  {} -> {} (persisted)",
                    key,
                    String::from_utf8_lossy(value)
                );
            }
        }
    }

    println!("\nRocksDB storage example completed successfully!");

    Ok(())
}
