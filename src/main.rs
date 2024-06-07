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

mod digest;
mod node;
mod page;
mod tree;
mod visitor;

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
