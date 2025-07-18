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

//! # Prolly
//!
//!
//! A Prolly Tree is a hybrid data structure that combines the features of B-trees and Merkle trees to provide
//! both efficient data access and verifiable integrity. It is specifically designed to handle the requirements
//! of distributed systems and large-scale databases, making indexes syncable and distributable over
//! peer-to-peer (P2P) networks.
//! ## Features
//!
//! - **Verifiability**: The cryptographic hashing in Prolly Trees ensures data integrity and allows for
//!   verifiable proofs of inclusion/exclusion.
//! - **Performance**: The balanced tree structure provides efficient data access patterns similar to
//!   B-trees, ensuring high performance for both random and sequential access.
//! - **Scalability**: Prolly Trees are suitable for large-scale applications, providing efficient index maintenance
//!   and data distribution capabilities.
//! - **Flexibility**: The probabilistic balancing allows for handling various mutation patterns without degrading
//!   performance or structure.
//!
//! ## Usage
//!
//! To use `prolly`, add the following to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! prolly = "0.1.0"
//! ```
//!
//! Follow examples in the github repository to get started.
//!

#[macro_use]
pub mod digest;
pub mod config;
pub mod diff;
mod encoding;
pub mod errors;
#[cfg(feature = "git")]
pub mod git;
pub mod node;
pub mod proof;
pub mod storage;
mod tracing;
pub mod tree;
