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
use crate::node::ProllyNode;
use arrow::array::{Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EncodingType {
    Json,
    Arrow,
}

impl<const N: usize> ProllyNode<N> {
    pub fn encode_pairs(&mut self, encoding_index: usize) {
        let encoded_value = match self.encode_types[encoding_index] {
            EncodingType::Json => self.encode_json(),
            EncodingType::Arrow => self.encode_arrow(),
        };
        self.encode_values[encoding_index] = encoded_value;
    }

    fn encode_json(&self) -> Vec<u8> {
        let pairs: Vec<(&Vec<u8>, &Vec<u8>)> = self.keys.iter().zip(self.values.iter()).collect();
        serde_json::to_vec(&pairs).unwrap_or_else(|_| Vec::new())
    }

    fn encode_arrow(&self) -> Vec<u8> {
        // Prepare the keys and values as strings
        let key_strings: Vec<&str> = self
            .keys
            .iter()
            .map(|key| std::str::from_utf8(key).unwrap_or(""))
            .collect();
        let value_strings: Vec<&str> = self
            .values
            .iter()
            .map(|value| std::str::from_utf8(value).unwrap_or(""))
            .collect();

        // Create Arrow arrays
        let key_array = StringArray::from(key_strings);
        let value_array = StringArray::from(value_strings);

        // Define the schema
        let schema = Schema::new(vec![
            Field::new("keys", DataType::Utf8, false),
            Field::new("values", DataType::Utf8, false),
        ]);

        // Create a RecordBatch
        let record_batch = RecordBatch::try_new(
            schema.clone().into(),
            vec![
                Arc::new(key_array) as Arc<dyn Array>,
                Arc::new(value_array) as Arc<dyn Array>,
            ],
        )
        .unwrap();

        // Encode to Arrow IPC format
        let mut encoded_data = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut encoded_data, &schema).unwrap();
            writer.write(&record_batch).unwrap();
            writer.finish().unwrap();
        }

        encoded_data
    }

    pub fn encode_all_pairs(&mut self) {
        self.encode_values = vec![Vec::new(); self.encode_types.len()];
        for i in 0..self.encode_types.len() {
            self.encode_pairs(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::ipc::reader::StreamReader;

    #[test]
    fn test_encode_json() {
        let mut node: ProllyNode<1024> = ProllyNode::default();
        node.keys = vec![b"key1".to_vec(), b"key2".to_vec()];
        node.values = vec![b"value1".to_vec(), b"value2".to_vec()];
        node.encode_types = vec![EncodingType::Json, EncodingType::Json];

        node.encode_all_pairs();

        for encoded_value in &node.encode_values {
            let decoded: Vec<(Vec<u8>, Vec<u8>)> = serde_json::from_slice(encoded_value).unwrap();
            for (i, (key, value)) in decoded.iter().enumerate() {
                assert_eq!(key, &node.keys[i]);
                assert_eq!(value, &node.values[i]);
            }
        }
    }

    #[test]
    fn test_encode_arrow() {
        let mut node: ProllyNode<1024> = ProllyNode::default();
        node.keys = vec![b"key1".to_vec(), b"key2".to_vec()];
        node.values = vec![b"value1".to_vec(), b"value2".to_vec()];
        node.encode_types = vec![EncodingType::Arrow, EncodingType::Arrow];

        node.encode_all_pairs();

        for encoded_value in &node.encode_values {
            // Create a schema for decoding
            let _schema = Arc::new(Schema::new(vec![
                Field::new("keys", DataType::Utf8, false),
                Field::new("values", DataType::Utf8, false),
            ]));

            // Decode the Arrow IPC format
            let mut reader = StreamReader::try_new(encoded_value.as_slice(), None).unwrap();
            let batch = reader.next().unwrap().unwrap();

            // Extract keys and values from the RecordBatch
            let key_array = batch
                .column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let value_array = batch
                .column(1)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..key_array.len() {
                assert_eq!(key_array.value(i).as_bytes(), node.keys[i].as_slice());
                assert_eq!(value_array.value(i).as_bytes(), node.values[i].as_slice());
            }
        }
    }
}
