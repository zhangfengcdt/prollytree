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

    // Set configuration for the trees
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
    // Create the trees
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

        println!("\n");

        // Find value and generate proof for increasing order
        let proof = tree_increasing.generate_proof(&keys[i]);
        let is_valid = tree_increasing.verify(proof.clone(), &keys[i], Some(&value_for_all));
        println!(
            "Proof for key \x1b[32m{:?}\x1b[0m in increasing order is valid: {}",
            keys[i], is_valid
        );
        println!("Proof: {proof:#?}"); // Assuming Debug trait is implemented
                                       // Sleep for 2 seconds
        sleep(Duration::from_millis(200));
    }

    // Clear the screen
    print!("\x1B[2J\x1B[1;1H");

    // delete keys in reverse order
    for i in 0..num_keys {
        // Clear the screen
        print!("\x1B[2J\x1B[1;1H");
        io::stdout().flush().unwrap();

        // Delete the next key in increasing order
        tree_increasing.delete(&keys[i]);

        // Delete the next key in reverse order
        tree_reverse.delete(&keys_reverse[i]);

        // Print the trees
        println!("Deleting key/value pairs in an increasing order:");
        println!("Deleting key: {:?}", keys[i]);
        tree_increasing.print();

        println!("\n");

        println!("Deleting key/value pairs in a reverse order:");
        println!("Deleting key: {:?}", keys_reverse[i]);
        tree_reverse.print();

        println!("\n");

        // Find value and generate proof for reverse order
        let proof = tree_reverse.generate_proof(&keys[i]);
        let is_valid = tree_reverse.verify(proof.clone(), &keys[i], None);
        println!(
            "Proof for key \x1b[32m{:?}\x1b[0m in reverse order is valid: {}",
            keys[i], is_valid
        );
        println!("Proof: {proof:#?}"); // Assuming Debug trait is implemented
                                       // Sleep for 2 seconds
        sleep(Duration::from_millis(200));
    }
}
