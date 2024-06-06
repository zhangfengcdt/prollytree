mod node;
mod page;
mod tree;
mod value_digest;

use tree::ProllyTree;

fn main() {
    let mut tree = ProllyTree::<32, String, String>::new();

    let key1 = "key1".to_string();
    let value1 = "value1".to_string();
    tree.insert(key1.clone(), value1);

    let key2 = "key2".to_string();
    let value2 = "value2".to_string();
    tree.insert(key2.clone(), value2);

    let root_hash = tree.root_hash();
    println!("Root Hash: {:?}", root_hash);
}
