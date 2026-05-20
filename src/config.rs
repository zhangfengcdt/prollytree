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
use crate::encoding::EncodingType;
use schemars::schema::RootSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TreeConfig<const N: usize> {
    pub base: u64,
    pub modulus: u64,
    pub min_chunk_size: usize,
    pub max_chunk_size: usize,
    pub pattern: u64,
    pub root_hash: Option<ValueDigest<N>>,
    pub key_schema: Option<RootSchema>,
    pub value_schema: Option<RootSchema>,
    pub encode_types: Vec<EncodingType>,
}

impl<const N: usize> Default for TreeConfig<N> {
    /// Default tuning for the probabilistic chunker.
    ///
    /// The chunker fires a split whenever the rolling hash matches `pattern`. With
    /// `pattern = 0b11111111` (eight `1` bits) the probability of a split at any
    /// given position is `1 / 2^8 = 1/256`, so the **expected** number of entries
    /// per leaf is ~256. Smaller patterns (fewer `1` bits) give smaller, more
    /// numerous nodes; larger patterns give fewer, larger nodes.
    ///
    /// `min_chunk_size` is the rolling-hash window: a node will not split until
    /// it holds at least this many entries.
    ///
    /// `max_chunk_size` is a hard safety cap measured in **entries**. It is set
    /// to ~16× the expected chunk size so it never fires on well-distributed
    /// data, but still prevents pathological runaway nodes on low-entropy or
    /// adversarial inputs. Note this is not a byte cap — see the documentation
    /// on storing large values inline.
    fn default() -> Self {
        TreeConfig {
            base: 257,
            modulus: 1_000_000_007,
            min_chunk_size: 8,
            max_chunk_size: 4096,
            pattern: 0b11111111,
            root_hash: None,
            key_schema: None,
            value_schema: None,
            encode_types: vec![],
        }
    }
}
