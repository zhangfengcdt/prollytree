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

use crate::digest::ValueDigest;
use crate::node::ProllyNode;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use super::NodeStorage;

#[derive(Clone, Debug)]
pub struct FileNodeStorage<const N: usize> {
    storage_dir: PathBuf,
}

impl<const N: usize> FileNodeStorage<N> {
    pub fn new(storage_dir: PathBuf) -> Self {
        fs::create_dir_all(&storage_dir).unwrap();
        FileNodeStorage { storage_dir }
    }

    fn node_path(&self, hash: &ValueDigest<N>) -> PathBuf {
        self.storage_dir.join(format!("{hash:x}"))
    }

    fn config_path(&self, key: &str) -> PathBuf {
        self.storage_dir.join(format!("config_{key}"))
    }
}

impl<const N: usize> NodeStorage<N> for FileNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<Arc<ProllyNode<N>>> {
        let path = self.node_path(hash);
        if path.exists() {
            let mut file = File::open(path).unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            // split/merged are #[serde(skip)] so they deserialize as false.
            let node: ProllyNode<N> = bincode::deserialize(&data).unwrap();
            Some(Arc::new(node))
        } else {
            None
        }
    }

    fn insert_node(&mut self, hash: ValueDigest<N>, node: ProllyNode<N>) -> Option<()> {
        let path = self.node_path(&hash);
        let data = bincode::serialize(&node).unwrap();
        let mut file = File::create(path).unwrap();
        file.write_all(&data).unwrap();
        Some(())
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Option<()> {
        let path = self.node_path(hash);
        if path.exists() {
            fs::remove_file(path).unwrap();
            Some(())
        } else {
            None
        }
    }

    fn save_config(&self, key: &str, config: &[u8]) {
        let path = self.config_path(key);
        let mut file = File::create(path).unwrap();
        file.write_all(config).unwrap();
    }

    fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.config_path(key);
        if path.exists() {
            let mut file = File::open(path).unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            Some(data)
        } else {
            None
        }
    }
}
