use crate::digest::ValueDigest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TreeConfig<const N: usize> {
    pub base: u64,
    pub modulus: u64,
    pub min_chunk_size: usize,
    pub max_chunk_size: usize,
    pub pattern: u64,
    pub root_hash: Option<ValueDigest<N>>,
}

impl<const N: usize> Default for TreeConfig<N> {
    fn default() -> Self {
        TreeConfig {
            base: 257,
            modulus: 1_000_000_007,
            min_chunk_size: 2,
            max_chunk_size: 16 * 1024,
            pattern: 0b11,
            root_hash: None,
        }
    }
}
