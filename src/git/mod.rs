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

pub mod operations;
pub mod storage;
pub mod types;
pub mod versioned_store;
pub mod thread_safe_store;

// Re-export commonly used types
pub use operations::GitOperations;
pub use storage::GitNodeStorage;
pub use types::{
    CommitDetails, CommitInfo, DiffOperation, GitKvError, KvConflict, KvDiff, KvStorageMetadata,
    MergeResult,
};
pub use versioned_store::{GitVersionedKvStore, VersionedKvStore};
pub use thread_safe_store::{ThreadSafeVersionedKvStore, ThreadSafeGitVersionedKvStore};
