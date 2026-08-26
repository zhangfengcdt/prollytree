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

use prollytree::node::ProllyNode;
use prollytree::storage::{FileNodeStorage, NodeStorage};
use tempfile::TempDir;

#[test]
fn file_node_storage_round_trips_node_and_config() {
    let temp = TempDir::new().unwrap();
    let mut storage = FileNodeStorage::<32>::new(temp.path().to_path_buf()).unwrap();

    let mut node = ProllyNode::<32>::default();
    node.keys = vec![b"key".to_vec()];
    node.values = vec![b"value".to_vec()];
    let hash = node.get_hash();

    storage.insert_node(hash.clone(), node.clone()).unwrap();
    storage.save_config("tree", b"config-bytes");

    assert_eq!(
        storage
            .get_node_by_hash(&hash)
            .map(|stored| stored.keys.clone()),
        Some(node.keys)
    );
    assert_eq!(
        storage.get_config("tree").as_deref(),
        Some(&b"config-bytes"[..])
    );
}

#[cfg(unix)]
mod unix_atomicity {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn insert_node_replaces_bad_final_path_without_writing_through_it() {
        let temp = TempDir::new().unwrap();
        let blocked_target = temp.path().join("blocked-target");
        fs::create_dir(&blocked_target).unwrap();

        let mut storage = FileNodeStorage::<32>::new(temp.path().to_path_buf()).unwrap();
        let mut node = ProllyNode::<32>::default();
        node.keys = vec![b"atomic-key".to_vec()];
        node.values = vec![b"atomic-value".to_vec()];
        let hash = node.get_hash();
        let node_path = temp.path().join(format!("{hash:x}"));
        symlink(&blocked_target, &node_path).unwrap();

        storage.insert_node(hash.clone(), node.clone()).unwrap();

        assert!(
            !fs::symlink_metadata(&node_path)
                .unwrap()
                .file_type()
                .is_symlink(),
            "atomic rename should replace the bad final-path symlink"
        );
        assert_eq!(
            storage
                .get_node_by_hash(&hash)
                .map(|stored| stored.values.clone()),
            Some(node.values)
        );
    }

    #[test]
    fn save_config_replaces_bad_final_path_without_writing_through_it() {
        let temp = TempDir::new().unwrap();
        let blocked_target = temp.path().join("blocked-target");
        fs::create_dir(&blocked_target).unwrap();
        let config_path = temp.path().join("config_tree");
        symlink(&blocked_target, &config_path).unwrap();

        let storage = FileNodeStorage::<32>::new(temp.path().to_path_buf()).unwrap();
        storage.save_config("tree", b"atomic-config");

        assert!(
            !fs::symlink_metadata(&config_path)
                .unwrap()
                .file_type()
                .is_symlink(),
            "atomic rename should replace the bad final-path symlink"
        );
        assert_eq!(
            storage.get_config("tree").as_deref(),
            Some(&b"atomic-config"[..])
        );
    }
}
