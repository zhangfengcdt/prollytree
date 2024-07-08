use prollytree::node::{Node, ProllyNode};
use prollytree::storage::{InMemoryNodeStorage, NodeStorage};

use std::thread::sleep;
use std::time::Duration;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::io::{self, Write};

fn main() {
    let num_keys = 20; // Variable to control the number of total pairs inserted

    // Initialize random number generator
    let mut rng = thread_rng();

    // Initialize storage and prolly trees
    let mut storage_increasing = InMemoryNodeStorage::<32>::default();
    let mut storage_random = InMemoryNodeStorage::<32>::default();
    let value_for_all = vec![100];

    let mut node_increasing: ProllyNode<32> = ProllyNode::builder()
        .pattern(0b11)
        .min_chunk_size(2)
        .build();

    let mut node_random: ProllyNode<32> = ProllyNode::builder()
        .pattern(0b11)
        .min_chunk_size(2)
        .build();

    // Generate keys
    let mut keys: Vec<Vec<u8>> = (0..num_keys).map(|i| vec![i as u8]).collect();
    let mut keys_random = keys.clone();
    keys_random.shuffle(&mut rng);

    for i in 0..num_keys {
        // Clear the screen
        print!("\x1B[2J\x1B[1;1H");
        io::stdout().flush().unwrap();

        // Insert the next key in increasing order
        node_increasing.insert(keys[i].clone(), value_for_all.clone(), &mut storage_increasing, Vec::new());
        storage_increasing.insert_node(node_increasing.get_hash(), node_increasing.clone());

        // Insert the next key in random order
        node_random.insert(keys_random[i].clone(), value_for_all.clone(), &mut storage_random, Vec::new());
        storage_random.insert_node(node_random.get_hash(), node_random.clone());

        // Print the trees
        println!("Prolly Tree with Increasing Order:");
        node_increasing.print_tree(&storage_increasing);
        println!("\nProlly Tree with Random Order:");
        node_random.print_tree(&storage_random);

        // Sleep for 2 seconds
        sleep(Duration::from_secs(2));
    }
}
