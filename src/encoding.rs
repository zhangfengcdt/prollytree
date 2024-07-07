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

use serde_json::Value;

pub trait EncodingScheme: Send + Sync {
    fn encode(&self, data: &Value) -> Vec<u8>;
    fn decode(&self, bytes: &[u8]) -> Option<Value>;
}

pub struct JsonEncoding;

impl EncodingScheme for JsonEncoding {
    fn encode(&self, data: &Value) -> Vec<u8> {
        serde_json::to_vec(data).unwrap()
    }

    fn decode(&self, bytes: &[u8]) -> Option<Value> {
        serde_json::from_slice(bytes).ok()
    }
}
