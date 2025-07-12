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
use std::fmt;

#[derive(Serialize, Deserialize, Clone)]
pub struct Proof<const N: usize> {
    pub path: Vec<ValueDigest<N>>, // Hashes of the nodes along the path
    pub target_hash: Option<ValueDigest<N>>, // Hash of the target node (if exists)
}

// Assuming ValueDigest has a ToString implementation or similar
impl<const N: usize> fmt::Debug for Proof<N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Proof")
            .field(
                "path",
                &self
                    .path
                    .iter()
                    .map(|digest| {
                        let bytes = digest.as_bytes();
                        if bytes.len() > 8 {
                            format!("{bytes:02x?}...")
                        } else {
                            format!("{bytes:02x?}")
                        }
                    })
                    .collect::<Vec<_>>(),
            )
            .field(
                "target_hash",
                &self.target_hash.as_ref().map(|digest| {
                    let bytes = digest.as_bytes();
                    if bytes.len() > 8 {
                        format!("{bytes:02x?}...")
                    } else {
                        format!("{bytes:02x?}")
                    }
                }),
            )
            .finish()
    }
}
