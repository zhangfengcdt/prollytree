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
#![allow(dead_code)]

use sha2::{Digest, Sha256};
use std::marker::PhantomData;

/// Represents a cryptographic hash of a value in a prolly tree.
///
/// `ValueDigest` is used to store a fixed-size cryptographic hash of a value associated with a key
/// in the prolly tree. This ensures data integrity and allows for quick comparisons without
/// storing the full value. The hash function used (e.g., SHA-256) ensures that even small changes
/// in the input value produce significantly different hashes.
///
/// Each `ValueDigest` contains the following component:
///
/// - An array of bytes: The fixed-size array that stores the cryptographic hash of the value. The
///   size of this array is specified by the constant parameter `N`, which typically corresponds to
///   the output size of the hash function used (e.g., 32 bytes for SHA-256).
///
/// The `ValueDigest` struct provides methods for creating a new digest from a value, as well as
/// accessing the raw bytes of the hash:
///
/// - `new(data: &[u8]) -> Self`: Creates a new `ValueDigest` from the given data by computing its
///   cryptographic hash.
/// - `as_bytes(&self) -> &[u8]`: Returns a reference to the underlying byte array of the hash.
///
/// `ValueDigest` is an essential component of the prolly tree, enabling secure and efficient
/// handling of key-value pairs.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ValueDigest<const N: usize>([u8; N]);

impl<const N: usize> ValueDigest<N> {
    /// Creates a new `ValueDigest` from the given data.
    ///
    /// This method computes the cryptographic hash of the input data and stores it in a fixed-size
    /// array. The size of the array is determined by the constant parameter `N`.
    ///
    /// # Arguments
    ///
    /// * `data` - A slice of bytes representing the input data to be hashed.
    ///
    /// # Returns
    ///
    /// A `ValueDigest` instance containing the computed hash.
    pub fn new(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();

        let mut hash = [0u8; N];
        hash.copy_from_slice(&result[..N]);
        ValueDigest(hash)
    }

    /// Creates a new `ValueDigest` from the raw hash bytes.
    /// This method is useful for creating a `ValueDigest` from a known hash value.
    ///
    /// # Arguments
    ///
    /// * `data` - A slice of bytes representing the raw hash value.
    ///
    /// # Returns
    ///
    /// A `ValueDigest` instance containing the provided hash value.
    pub fn raw_hash(data: &[u8]) -> Self {
        ValueDigest(<[u8; N]>::try_from(data).unwrap())
    }

    /// Returns a reference to the underlying byte array of the hash.
    ///
    /// This method allows access to the raw bytes of the cryptographic hash, which can be useful
    /// for comparison or serialization purposes.
    ///
    /// # Returns
    ///
    /// A reference to the byte array containing the hash.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ValueDigest2<H: Digest, const N: usize> {
    hash: [u8; N],
    _marker: PhantomData<H>,
}

impl<H: Digest, const N: usize> ValueDigest2<H, N> {
    pub fn new(data: &[u8]) -> Self {
        let mut hasher = H::new();
        hasher.update(data);
        let result = hasher.finalize();

        let mut hash = [0u8; N];
        hash.copy_from_slice(&result[..N]);
        ValueDigest2 {
            hash,
            _marker: PhantomData,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn test_value_digest_new() {
        let data = b"test data";
        let expected_hash = {
            let mut hasher = Sha256::new();
            hasher.update(data);
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result[..32]);
            hash
        };

        let value_digest = ValueDigest::<32>::new(data);
        assert_eq!(value_digest.as_bytes(), &expected_hash);
    }

    #[test]
    fn test_value_digest_as_bytes() {
        let data = b"test data";
        let value_digest = ValueDigest::<32>::new(data);

        let hash_bytes = value_digest.as_bytes();
        assert_eq!(hash_bytes.len(), 32);
    }

    #[test]
    fn test_value_digest_equality() {
        let data1 = b"test data 1";
        let data2 = b"test data 2";
        let digest1 = ValueDigest::<32>::new(data1);
        let digest2 = ValueDigest::<32>::new(data1);
        let digest3 = ValueDigest::<32>::new(data2);

        assert_eq!(digest1, digest2);
        assert_ne!(digest1, digest3);
    }

    #[test]
    fn test_value_digest_clone() {
        let data = b"test data";
        let value_digest = ValueDigest::<32>::new(data);
        let value_digest_clone = value_digest.clone();

        assert_eq!(value_digest, value_digest_clone);
    }

    #[test]
    fn test_value_digest_raw_hash() {
        let data = b"test data";
        let value_digest1 = ValueDigest::<32>::new(data);
        let value_digest2 = ValueDigest::<32>::raw_hash(value_digest1.as_bytes());

        assert_eq!(value_digest1, value_digest2);
    }
}
