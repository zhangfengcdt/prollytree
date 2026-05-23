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

//! Thin wrappers around bincode 2.x that preserve the bincode 1.x wire format
//! and 1.x-style `serialize`/`deserialize` ergonomics for existing call sites.
//!
//! The on-disk format produced by [`serialize`] is byte-compatible with
//! bincode 1.3's default config (fixed-int, little-endian, no length limit) so
//! existing git blobs, RocksDB rows, and proximity index snapshots written by
//! older versions of this crate continue to round-trip.

use bincode::error::{DecodeError, EncodeError};
use serde::de::DeserializeOwned;
use serde::Serialize;

#[inline]
fn config() -> impl bincode::config::Config {
    bincode::config::legacy()
}

pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, EncodeError> {
    bincode::serde::encode_to_vec(value, config())
}

pub fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    let (value, _read) = bincode::serde::decode_from_slice(bytes, config())?;
    Ok(value)
}
