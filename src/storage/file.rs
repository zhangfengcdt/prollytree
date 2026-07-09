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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::{NodeStorage, StorageError};

/// Process-local monotonic counter used as part of the partial-write filename
/// so concurrent `insert_blob` callers in the same process can't collide on
/// the temp path. Combined with pid + nanoseconds this gives a globally
/// unique suffix without pulling in `rand`.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", std::process::id(), nanos, n)
}

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

    fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
        let Some(parent) = path.parent() else {
            return Err(StorageError::Other(format!(
                "cannot write path without parent: {}",
                path.display()
            )));
        };
        fs::create_dir_all(parent)?;

        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| StorageError::Other(format!("invalid path: {}", path.display())))?;
        let tmp_path = parent.join(format!("{name}.partial.{}", unique_suffix()));
        let mut file = File::create(&tmp_path)?;
        let result = (|| -> Result<(), StorageError> {
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&tmp_path, path)?;
            // Best-effort directory sync: required for crash durability on
            // filesystems that support it, harmlessly skipped elsewhere.
            if let Ok(dir) = File::open(parent) {
                let _ = dir.sync_all();
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }
        result
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
            let node: ProllyNode<N> = crate::serde_bincode::deserialize(&data).ok()?;
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
        let data = crate::serde_bincode::serialize(&node)?;
        Self::atomic_write(&path, &data)
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
        let _ = Self::atomic_write(&path, config);
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
        // Write to a UNIQUE temp path (pid + nanoseconds + atomic counter) then
        // rename. Two concurrent writers of the same blob each get their own
        // temp file; whoever loses the rename race sees the final path already
        // exist, which is fine — content-addressed storage guarantees the
        // winner wrote bytes identical to ours. Treat that case as success so
        // `insert_blob` stays idempotent across processes.
        let tmp_path = blobs_dir.join(format!("{:x}.partial.{}", hash, unique_suffix()));
        let mut file = File::create(&tmp_path)?;
        file.write_all(bytes)?;
        // Ensure the bytes are flushed before rename so a crash mid-write
        // doesn't leave the rename target pointing at unfinished data.
        file.sync_all()?;
        drop(file);
        match fs::rename(&tmp_path, &path) {
            Ok(()) => Ok(()),
            Err(e) => {
                // Another writer landed first. Verify the target exists
                // (proving the race lost, not some unrelated failure) and
                // clean up our own temp file before returning success.
                if path.exists() {
                    let _ = fs::remove_file(&tmp_path);
                    Ok(())
                } else {
                    let _ = fs::remove_file(&tmp_path);
                    Err(StorageError::Io(e))
                }
            }
        }
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

    fn list_blobs(&self) -> Result<Vec<ValueDigest<N>>, StorageError> {
        let blobs_dir = self.blobs_dir();
        if !blobs_dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(blobs_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name_str) = name.to_str() else {
                continue;
            };
            // Skip atomic-rename temp files left by interrupted writes.
            if name_str.ends_with(".partial") {
                continue;
            }
            if name_str.len() != N * 2 {
                continue;
            }
            // Parse the hex-encoded filename back into the digest bytes.
            let mut arr = [0u8; N];
            let mut ok = true;
            for i in 0..N {
                let byte_str = &name_str[i * 2..i * 2 + 2];
                match u8::from_str_radix(byte_str, 16) {
                    Ok(b) => arr[i] = b,
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                out.push(ValueDigest(arr));
            }
        }
        Ok(out)
    }
}
