use prollytree::node::{Node, ProllyNode};
use prollytree::storage::{InMemoryNodeStorage, NodeStorage};

use std::thread::sleep;
use std::time::Duration;
use std::io::{self, Write};

fn main() {
    let num_keys = 100; // Variable to control the number of total pairs inserted

    // Initialize storage and prolly trees
    let mut storage_increasing = InMemoryNodeStorage::<32>::default();
    let mut storage_reverse = InMemoryNodeStorage::<32>::default();
    let value_for_all = vec![100];

    let mut node_increasing: ProllyNode<32> = ProllyNode::builder()
        .pattern(0b1111)
        .min_chunk_size(4)
        .build();

    let mut node_reverse: ProllyNode<32> = ProllyNode::builder()
        .pattern(0b1111)
        .min_chunk_size(4)
        .build();

    // Generate keys
    let keys: Vec<Vec<u8>> = (0..num_keys).map(|i| vec![i as u8]).collect();
    let keys_reverse: Vec<Vec<u8>> = (0..num_keys).rev().map(|i| vec![i as u8]).collect();

    for i in 0..num_keys {
        // Clear the screen
        print!("\x1B[2J\x1B[1;1H");
        io::stdout().flush().unwrap();

        // Insert the next key in increasing order
        node_increasing.insert(keys[i].clone(), value_for_all.clone(), &mut storage_increasing, Vec::new());
        storage_increasing.insert_node(node_increasing.get_hash(), node_increasing.clone());

        // Insert the next key in reverse order
        node_reverse.insert(keys_reverse[i].clone(), value_for_all.clone(), &mut storage_reverse, Vec::new());
        storage_reverse.insert_node(node_reverse.get_hash(), node_reverse.clone());

        // Print the trees
        println!("Inserting key/value pairs in an increasing order:");
        println!("Inserting key: {:?}", keys[i]);
        node_increasing.print_tree(&storage_increasing);

        println!("\n");

        println!("Inserting key/value pairs in a reverse order:");
        println!("Inserting key: {:?}", keys_reverse[i]);
        node_reverse.print_tree(&storage_reverse);

        // Sleep for 2 seconds
        sleep(Duration::from_millis(200));
    }
}
