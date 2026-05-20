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

//! Large-value externalisation envelopes (PR 0b).
//!
//! # Wire format
//!
//! When a [`NamespacedKvStore`](crate::git::versioned_store::NamespacedKvStore)
//! is configured with a threshold (via `set_externalize_threshold`), values
//! whose length exceeds the threshold are stored as separate content-
//! addressed blobs via [`NodeStorage::insert_blob`](crate::storage::NodeStorage)
//! and the in-leaf value becomes a fixed-shape **envelope**:
//!
//! ```text
//! ┌──────────────────┬──────────────────────────┬───────────────────────┐
//! │ MAGIC (4 bytes)  │ blob content hash (N B)  │ original size (8 LE)  │
//! └──────────────────┴──────────────────────────┴───────────────────────┘
//! ```
//!
//! For the standard `N = 32` (SHA-256), an envelope is exactly **44 bytes**.
//!
//! # Back-compat
//!
//! Existing stores written under earlier versions had no externalisation:
//! every leaf value was inline user bytes. After this PR lands, those leaves
//! deserialise byte-for-byte unchanged — the `ProllyNode` wire format does
//! **not** change. Externalisation is a per-value interpretation layer on
//! top of the unchanged byte slot.
//!
//! Detection is `(length == ENVELOPE_LEN) && (prefix == MAGIC)`. A user
//! value that happens to satisfy both is statistically very rare; the
//! mitigation when it occurs is described in [`unwrap_value`].

use crate::digest::ValueDigest;
use crate::storage::NodeStorage;

/// Sentinel 4-byte prefix identifying an externalised-value envelope. Chosen
/// for high entropy: `0x00` as the first byte (rare at the start of UTF-8 or
/// length-prefixed binary data) plus `0xFF` (rare as the second byte of
/// natural-language UTF-8) plus the ASCII letters `P` and `X` (mnemonic for
/// "Prolly eXternal" — useful when hex-dumping the wire format).
pub const EXTERNAL_MAGIC: [u8; 4] = [0x00, 0xFF, b'P', b'X'];

/// Number of bytes used for the original-size field in an envelope.
pub const SIZE_FIELD_BYTES: usize = 8;

/// Compute the total envelope length for a given digest size `N`. For
/// `N = 32` (the standard SHA-256 digest), this is 44 bytes.
pub const fn envelope_len<const N: usize>() -> usize {
    EXTERNAL_MAGIC.len() + N + SIZE_FIELD_BYTES
}

/// Build a fixed-shape envelope for an externalised value.
///
/// The caller is responsible for ensuring `hash` matches the SHA-256 of the
/// original byte payload (i.e. `ValueDigest::<N>::new(&payload)`).
pub fn make_envelope<const N: usize>(hash: &ValueDigest<N>, original_size: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(envelope_len::<N>());
    buf.extend_from_slice(&EXTERNAL_MAGIC);
    buf.extend_from_slice(hash.as_bytes());
    buf.extend_from_slice(&original_size.to_le_bytes());
    buf
}

/// Necessary check that `bytes` *might* be an envelope: correct length, magic
/// prefix. **Not sufficient** — a real user value can satisfy these even
/// without being a true envelope. Callers must additionally verify that
/// `get_blob(hash)` returns the actual blob before treating the parse as
/// real. See [`unwrap_value`] for the canonical lookup pattern.
pub fn parse_envelope<const N: usize>(bytes: &[u8]) -> Option<(ValueDigest<N>, u64)> {
    if bytes.len() != envelope_len::<N>() {
        return None;
    }
    if bytes[..EXTERNAL_MAGIC.len()] != EXTERNAL_MAGIC {
        return None;
    }
    let hash_bytes = &bytes[EXTERNAL_MAGIC.len()..EXTERNAL_MAGIC.len() + N];
    let hash = ValueDigest::<N>::raw_hash(hash_bytes);
    let size_bytes: [u8; SIZE_FIELD_BYTES] = bytes
        [EXTERNAL_MAGIC.len() + N..EXTERNAL_MAGIC.len() + N + SIZE_FIELD_BYTES]
        .try_into()
        .ok()?;
    let original_size = u64::from_le_bytes(size_bytes);
    Some((hash, original_size))
}

/// Resolve a raw in-leaf byte slice to the user-visible value.
///
/// If `raw` parses as an envelope **and** `storage` actually has a blob at
/// the embedded hash, the blob bytes are returned. Otherwise the original
/// `raw` bytes are returned unchanged.
///
/// This **fall-through-on-miss** behaviour is what makes false-positive
/// envelope parses (a real user value that coincidentally has the magic
/// prefix and exact envelope length) recoverable: the hash won't point at
/// any blob, so we degrade gracefully to inline.
pub fn unwrap_value<const N: usize, S: NodeStorage<N>>(raw: &[u8], storage: &S) -> Vec<u8> {
    if let Some((hash, _original_size)) = parse_envelope::<N>(raw) {
        if let Some(blob) = storage.get_blob(&hash) {
            return blob;
        }
        // Envelope-shaped bytes but no matching blob → treat as inline.
    }
    raw.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryNodeStorage;

    #[test]
    fn envelope_len_for_n32_is_44() {
        assert_eq!(envelope_len::<32>(), 44);
        assert_eq!(EXTERNAL_MAGIC.len(), 4);
        assert_eq!(SIZE_FIELD_BYTES, 8);
    }

    #[test]
    fn make_envelope_has_expected_shape() {
        let hash = ValueDigest::<32>::new(b"payload");
        let env = make_envelope::<32>(&hash, 1234);
        assert_eq!(env.len(), 44);
        assert_eq!(&env[..4], &EXTERNAL_MAGIC);
        assert_eq!(&env[4..36], hash.as_bytes());
        assert_eq!(u64::from_le_bytes(env[36..44].try_into().unwrap()), 1234);
    }

    #[test]
    fn parse_envelope_round_trips() {
        let hash = ValueDigest::<32>::new(b"payload");
        let env = make_envelope::<32>(&hash, 42);
        let (got_hash, got_size) = parse_envelope::<32>(&env).expect("should parse");
        assert_eq!(got_hash, hash);
        assert_eq!(got_size, 42);
    }

    #[test]
    fn parse_envelope_rejects_wrong_length() {
        assert!(parse_envelope::<32>(b"too short").is_none());
        let too_long = [0u8; 100];
        assert!(parse_envelope::<32>(&too_long).is_none());
    }

    #[test]
    fn parse_envelope_rejects_wrong_magic() {
        let mut bytes = [0u8; 44];
        bytes[..4].copy_from_slice(b"NOPE");
        assert!(parse_envelope::<32>(&bytes).is_none());
    }

    #[test]
    fn parse_envelope_44_byte_random_user_value_is_likely_rejected() {
        // 44 bytes of clearly-user content (zero magic match) parses as None.
        let payload = b"this is exactly forty-four byte value!......".to_vec();
        assert_eq!(payload.len(), 44);
        assert!(parse_envelope::<32>(&payload).is_none());
    }

    #[test]
    fn unwrap_value_returns_blob_when_hash_present() {
        let mut storage = InMemoryNodeStorage::<32>::new();
        let payload = b"the original large payload".to_vec();
        let hash = ValueDigest::<32>::new(&payload);
        storage.insert_blob(hash.clone(), &payload).unwrap();

        let env = make_envelope::<32>(&hash, payload.len() as u64);
        assert_eq!(unwrap_value::<32, _>(&env, &storage), payload);
    }

    #[test]
    fn unwrap_value_returns_raw_when_not_envelope() {
        let storage = InMemoryNodeStorage::<32>::new();
        let raw = b"a small inline value".to_vec();
        assert_eq!(unwrap_value::<32, _>(&raw, &storage), raw);
    }

    #[test]
    fn unwrap_value_falls_back_when_envelope_hash_missing() {
        // Envelope-shaped bytes but no blob behind the hash → graceful fallback
        // returns the envelope bytes as-is. This is the false-positive recovery
        // path that makes the magic-prefix scheme safe.
        let storage = InMemoryNodeStorage::<32>::new();
        let phantom_hash = ValueDigest::<32>::new(b"never_inserted");
        let env = make_envelope::<32>(&phantom_hash, 100);
        let got = unwrap_value::<32, _>(&env, &storage);
        // Got back the envelope bytes (not crashed, not data loss).
        assert_eq!(got, env);
    }
}
