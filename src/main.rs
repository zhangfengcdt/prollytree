use prollytree::storage::InMemoryNodeStorage;

use prollytree::config::TreeConfig;
use prollytree::tree::{ProllyTree, Tree};
use std::io::{self, Write};
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let num_keys = 100; // Variable to control the number of total pairs inserted

    // Initialize storage and prolly trees
    let storage_increasing = InMemoryNodeStorage::<32>::default();
    let storage_reverse = InMemoryNodeStorage::<32>::default();
    let value_for_all = vec![100];

    let config = TreeConfig {
        base: 257,
        modulus: 1_000_000_007,
        min_chunk_size: 4,
        max_chunk_size: 16 * 1024,
        pattern: 0b1111,
        root_hash: None,
        key_schema: None,
        value_schema: None,
        encode_types: vec![],
    };
    let mut tree_increasing = ProllyTree::new(storage_increasing, config.clone());
    let mut tree_reverse = ProllyTree::new(storage_reverse, config.clone());

    // Generate keys
    let keys: Vec<Vec<u8>> = (0..num_keys).map(|i| vec![i as u8]).collect();
    let keys_reverse: Vec<Vec<u8>> = (0..num_keys).rev().map(|i| vec![i as u8]).collect();

    for i in 0..num_keys {
        // Clear the screen
        print!("\x1B[2J\x1B[1;1H");
        io::stdout().flush().unwrap();

        // Insert the next key in increasing order
        tree_increasing.insert(keys[i].clone(), value_for_all.clone());
        // storage_increasing.insert_node(node_increasing.get_hash(), node_increasing.clone());

        // Insert the next key in reverse order
        tree_reverse.insert(keys_reverse[i].clone(), value_for_all.clone());

        // Print the trees
        println!("Inserting key/value pairs in an increasing order:");
        println!("Inserting key: {:?}", keys[i]);
        tree_increasing.print();

        println!("\n");

        println!("Inserting key/value pairs in a reverse order:");
        println!("Inserting key: {:?}", keys_reverse[i]);
        tree_reverse.print();

        // Sleep for 2 seconds
        sleep(Duration::from_millis(200));
    }
}
