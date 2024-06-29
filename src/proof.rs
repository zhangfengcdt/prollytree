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
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Proof<const N: usize> {
    pub path: Vec<ValueDigest<N>>, // Hashes of the nodes along the path
    pub target_hash: Option<ValueDigest<N>>, // Hash of the target node (if exists)
}

impl<const N: usize> Proof<N> {
    pub fn verify(
        &self,
        root_hash: &ValueDigest<N>,
        _key: &[u8],
        expected_value: Option<&[u8]>,
    ) -> bool {
        // Start with the root hash
        let mut computed_hash = root_hash.clone();

        for hash in &self.path {
            // TODO: Retrieve the node content from storage using the hash
            // Simulate the hash computation along the path
            // This is a simplified example. Actual computation would involve re-hashing the node content
            computed_hash = ValueDigest::combine(&computed_hash, hash);
        }

        // Check the final computed hash against the target hash
        if let Some(expected_target_hash) = &self.target_hash {
            if &computed_hash == expected_target_hash {
                if let Some(_expected_value) = expected_value {
                    // TODO: Retrieve the actual value content from storage using the key
                    // If we are expecting a specific value, ensure the target hash matches it
                    // This part would involve verifying the actual value content
                    true
                } else {
                    true
                }
            } else {
                false
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::ValueDigest;

    #[test]
    fn test_verify_proof() {
        // Create a sample root hash
        let root_hash: ValueDigest<32> = ValueDigest::new(b"root");

        // Create a sample path
        let path = vec![ValueDigest::new(b"node1"), ValueDigest::new(b"node2")];

        // Create a sample target hash
        let target_hash = Some(ValueDigest::new(b"target"));

        // Create a proof
        let proof = Proof {
            path: path.clone(),
            target_hash: target_hash.clone(),
        };

        // Verify the proof (this is a simplified example, in reality, you would need a more complex setup)
        // TODO: Implement the actual verification logic
        // assert!(proof.verify(&root_hash, b"key", None));

        // Test with incorrect root hash
        let incorrect_root_hash = ValueDigest::new(b"incorrect_root");
        assert!(!proof.verify(&incorrect_root_hash, b"key", None));

        // Test with incorrect target hash
        let incorrect_proof = Proof {
            path,
            target_hash: Some(ValueDigest::new(b"incorrect_target")),
        };
        assert!(!incorrect_proof.verify(&root_hash, b"key", None));
    }
}
