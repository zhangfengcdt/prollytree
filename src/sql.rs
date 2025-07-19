//! GlueSQL storage implementation backed by ProllyTree
//!
//! This module provides a custom storage backend for GlueSQL that uses
//! ProllyTree as the underlying data structure, enabling SQL queries
//! over the versioned key-value store.

#[cfg(feature = "sql")]
pub mod glue_storage;

#[cfg(feature = "sql")]
pub use glue_storage::ProllyStorage;