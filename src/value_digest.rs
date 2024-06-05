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

use sha2::{Sha256, Digest};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ValueDigest<const N: usize>([u8; N]);

impl<const N: usize> ValueDigest<N> {
    pub fn new(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();

        let mut hash = [0u8; N];
        hash.copy_from_slice(&result[..N]);
        ValueDigest(hash)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

