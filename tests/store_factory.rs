// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Integration tests for the `StoreFactory` API.

#![cfg(feature = "git")]

mod common;

use prollytree::git::versioned_store::StoreFactory;

// ---------------------------------------------------------------------------
// Memory store
// ---------------------------------------------------------------------------

#[test]
fn test_factory_memory_creates_store() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    let mut store = StoreFactory::memory::<32, _>(&dataset).expect("memory init failed");
    store
        .insert(b"hello".to_vec(), b"world".to_vec())
        .expect("insert failed");
    assert_eq!(store.get(b"hello"), Some(b"world".to_vec()));
}

// ---------------------------------------------------------------------------
// File store — persistence
// ---------------------------------------------------------------------------

#[test]
fn test_factory_file_creates_store() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    let mut store = StoreFactory::file::<32, _>(&dataset).expect("file init failed");
    store
        .insert(b"persistent".to_vec(), b"data".to_vec())
        .expect("insert failed");
    store.commit("save").expect("commit failed");
    drop(store);

    let store = StoreFactory::file_open::<32, _>(&dataset).expect("file open failed");
    assert_eq!(store.get(b"persistent"), Some(b"data".to_vec()));
}

// ---------------------------------------------------------------------------
// Git store
// ---------------------------------------------------------------------------

#[test]
fn test_factory_git_creates_store() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    let mut store = StoreFactory::git::<32, _>(&dataset).expect("git init failed");
    store
        .insert(b"gitkey".to_vec(), b"gitval".to_vec())
        .expect("insert failed");
    store.commit("initial").expect("commit failed");

    assert_eq!(store.get(b"gitkey"), Some(b"gitval".to_vec()));
}

#[test]
fn test_factory_git_open_restores_state() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    {
        let mut store = StoreFactory::git::<32, _>(&dataset).expect("git init failed");
        store
            .insert(b"survive".to_vec(), b"reopen".to_vec())
            .expect("insert failed");
        store.commit("persist").expect("commit failed");
    }

    let store = StoreFactory::git_open::<32, _>(&dataset).expect("git open failed");
    assert_eq!(store.get(b"survive"), Some(b"reopen".to_vec()));
}

// ---------------------------------------------------------------------------
// Thread-safe variants
// ---------------------------------------------------------------------------

#[test]
fn test_factory_git_threadsafe() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    let store =
        StoreFactory::git_threadsafe::<32, _>(&dataset).expect("git_threadsafe init failed");

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let s = store.clone();
            std::thread::spawn(move || {
                s.insert(
                    format!("key{i}").into_bytes(),
                    format!("val{i}").into_bytes(),
                )
                .expect("insert failed");
            })
        })
        .collect();
    for h in handles {
        h.join().expect("thread panicked");
    }

    store.commit("threaded").expect("commit failed");
    for i in 0..4 {
        assert!(store.get(format!("key{i}").as_bytes()).is_some());
    }
}

#[test]
fn test_factory_memory_threadsafe() {
    let (_temp, dataset) = common::setup_repo_and_dataset();

    let store =
        StoreFactory::memory_threadsafe::<32, _>(&dataset).expect("memory_threadsafe init failed");

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let s = store.clone();
            std::thread::spawn(move || {
                s.insert(format!("mk{i}").into_bytes(), format!("mv{i}").into_bytes())
                    .expect("insert failed");
            })
        })
        .collect();
    for h in handles {
        h.join().expect("thread panicked");
    }

    for i in 0..4 {
        assert!(store.get(format!("mk{i}").as_bytes()).is_some());
    }
}

// ---------------------------------------------------------------------------
// Error paths
// ---------------------------------------------------------------------------

#[test]
fn test_factory_open_nonexistent_fails() {
    let result = StoreFactory::git_open::<32, _>("/tmp/prollytree_does_not_exist_12345");
    assert!(result.is_err(), "opening nonexistent path should fail");
}
