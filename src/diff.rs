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

#[derive(Debug, PartialEq)]
pub enum DiffResult {
    Added(Vec<u8>, Vec<u8>),
    Removed(Vec<u8>, Vec<u8>),
    Modified(Vec<u8>, Vec<u8>, Vec<u8>),
}

#[derive(Debug, PartialEq)]
pub enum MergeResult {
    Added(Vec<u8>, Vec<u8>),
    Removed(Vec<u8>),
    Modified(Vec<u8>, Vec<u8>),
    Conflict(MergeConflict),
}

#[derive(Debug, PartialEq)]
pub struct MergeConflict {
    pub key: Vec<u8>,
    pub base_value: Option<Vec<u8>>,
    pub source_value: Option<Vec<u8>>,
    pub destination_value: Option<Vec<u8>>,
}
