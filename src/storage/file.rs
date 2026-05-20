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

use super::{NodeStorage, StorageError};

#[derive(Clone, Debug)]
pub struct FileNodeStorage<const N: usize> {
    storage_dir: PathBuf,
}

impl<const N: usize> FileNodeStorage<N> {
    pub fn new(storage_dir: PathBuf) -> Result<Self, StorageError> {
        fs::create_dir_all(&storage_dir)?;
        Ok(FileNodeStorage { storage_dir })
    }

    fn node_path(&self, hash: &ValueDigest<N>) -> PathBuf {
        self.storage_dir.join(format!("{hash:x}"))
    }

    fn config_path(&self, key: &str) -> PathBuf {
        self.storage_dir.join(format!("config_{key}"))
    }

    /// Subdirectory for externalised blob storage. Kept under the same root
    /// as nodes so a single `storage_dir` is the complete persistence target.
    fn blobs_dir(&self) -> PathBuf {
        self.storage_dir.join("blobs")
    }

    fn blob_path(&self, hash: &ValueDigest<N>) -> PathBuf {
        self.blobs_dir().join(format!("{hash:x}"))
    }
}

impl<const N: usize> NodeStorage<N> for FileNodeStorage<N> {
    fn get_node_by_hash(&self, hash: &ValueDigest<N>) -> Option<Arc<ProllyNode<N>>> {
        let path = self.node_path(hash);
        if path.exists() {
            let mut file = File::open(path).ok()?;
            let mut data = Vec::new();
            file.read_to_end(&mut data).ok()?;
            // split/merged are #[serde(skip)] so they deserialize as false.
            let node: ProllyNode<N> = bincode::deserialize(&data).ok()?;
            Some(Arc::new(node))
        } else {
            None
        }
    }

    fn insert_node(
        &mut self,
        hash: ValueDigest<N>,
        node: ProllyNode<N>,
    ) -> Result<(), StorageError> {
        let path = self.node_path(&hash);
        let data = bincode::serialize(&node)?;
        let mut file = File::create(path)?;
        file.write_all(&data)?;
        Ok(())
    }

    fn delete_node(&mut self, hash: &ValueDigest<N>) -> Result<(), StorageError> {
        let path = self.node_path(hash);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn save_config(&self, key: &str, config: &[u8]) {
        let path = self.config_path(key);
        if let Ok(mut file) = File::create(path) {
            let _ = file.write_all(config);
        }
    }

    fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.config_path(key);
        if path.exists() {
            let mut file = File::open(path).ok()?;
            let mut data = Vec::new();
            file.read_to_end(&mut data).ok()?;
            Some(data)
        } else {
            None
        }
    }

    fn insert_blob(&mut self, hash: ValueDigest<N>, bytes: &[u8]) -> Result<(), StorageError> {
        let blobs_dir = self.blobs_dir();
        fs::create_dir_all(&blobs_dir)?;
        let path = self.blob_path(&hash);
        if path.exists() {
            // Content-addressed: same hash ⇒ same bytes; nothing to do.
            return Ok(());
        }
        // Write to a temp path then rename so a partial write never leaves a
        // corrupted blob behind.
        let tmp_path = blobs_dir.join(format!("{hash:x}.partial"));
        let mut file = File::create(&tmp_path)?;
        file.write_all(bytes)?;
        // Ensure the bytes are flushed before rename so a crash mid-write
        // doesn't leave the rename target pointing at unfinished data.
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp_path, &path)?;
        Ok(())
    }

    fn get_blob(&self, hash: &ValueDigest<N>) -> Option<Vec<u8>> {
        let path = self.blob_path(hash);
        if !path.exists() {
            return None;
        }
        let mut file = File::open(path).ok()?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).ok()?;
        Some(data)
    }

    fn delete_blob(&mut self, hash: &ValueDigest<N>) -> Result<(), StorageError> {
        let path = self.blob_path(hash);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}
