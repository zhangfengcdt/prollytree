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

#![allow(unused_imports)]

use crate::node::ProllyNode;
use arrow::array::{Array, Float64Array};
use arrow::array::{ArrayRef, BooleanArray, Int32Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_writer::ArrowWriter;
use schemars::schema::RootSchema;
use schemars::schema::SchemaObject;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EncodingType {
    Json,
    Arrow,
    Parquet,
}

impl<const N: usize> ProllyNode<N> {
    pub fn encode_pairs(&mut self, encoding_index: usize) {
        let encoded_value = match self.encode_types[encoding_index] {
            EncodingType::Json => self.encode_json(),
            EncodingType::Arrow => self.encode_arrow(),
            EncodingType::Parquet => self.encode_parquet(),
        };
        self.encode_values[encoding_index] = encoded_value;
    }

    fn encode_json(&self) -> Vec<u8> {
        let pairs: Vec<(&Vec<u8>, &Vec<u8>)> = self.keys.iter().zip(self.values.iter()).collect();
        serde_json::to_vec(&pairs).unwrap_or_else(|_| Vec::new())
    }

    fn encode_arrow(&self) -> Vec<u8> {
        // Convert keys and values to arrays based on their schemas
        let key_batch = self.convert_to_arrow_array(&self.keys, &self.key_schema);
        let value_batch = self.convert_to_arrow_array(&self.values, &self.value_schema);

        // Combine the two RecordBatches into one
        let combined_batch = self.combine_record_batches(key_batch, value_batch);

        // Define the schema
        let schema = combined_batch.schema();

        // Encode to Arrow IPC format
        let mut encoded_data = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut encoded_data, &schema).unwrap();
            writer.write(&combined_batch).unwrap();
            writer.finish().unwrap();
        }

        encoded_data
    }

    fn encode_parquet(&self) -> Vec<u8> {
        // Convert keys and values to arrays based on their schemas
        let key_batch = self.convert_to_arrow_array(&self.keys, &self.key_schema);
        let value_batch = self.convert_to_arrow_array(&self.values, &self.value_schema);

        // Combine the two RecordBatches into one
        let combined_batch = self.combine_record_batches(key_batch, value_batch);
        let schema = combined_batch.schema();

        // Encode to Parquet format
        let mut encoded_data = Vec::new();
        let mut writer = ArrowWriter::try_new(&mut encoded_data, schema, None).unwrap();
        writer.write(&combined_batch).unwrap();
        writer.close().unwrap();

        encoded_data
    }

    fn combine_record_batches(
        &self,
        key_batch: RecordBatch,
        value_batch: RecordBatch,
    ) -> RecordBatch {
        // Extract columns from both batches
        let mut columns = Vec::new();
        let mut fields = Vec::new();

        // Add key_batch columns and fields
        for column in key_batch.columns() {
            columns.push(column.clone());
        }
        for field in key_batch.schema().fields() {
            fields.push(field.clone());
        }

        // Add value_batch columns and fields
        for column in value_batch.columns() {
            columns.push(column.clone());
        }
        for field in value_batch.schema().fields() {
            fields.push(field.clone());
        }

        // Create a new schema with combined fields
        let schema = Arc::new(Schema::new(fields));

        // Create a new RecordBatch with combined columns and schema
        RecordBatch::try_new(schema, columns).unwrap()
    }

    fn convert_to_arrow_array(&self, data: &[Vec<u8>], schema: &Option<RootSchema>) -> RecordBatch {
        let schema = schema.as_ref().unwrap();

        if let Some(object) = &schema.schema.object {
            let fields: Vec<Field> = object
                .properties
                .iter()
                .map(|(name, schema)| {
                    let data_type = match &schema {
                        schemars::schema::Schema::Object(SchemaObject {
                            instance_type: Some(instance_type),
                            ..
                        }) => match instance_type {
                            schemars::schema::SingleOrVec::Single(single_type) => {
                                match **single_type {
                                    schemars::schema::InstanceType::String => DataType::Utf8,
                                    schemars::schema::InstanceType::Integer => DataType::Int32,
                                    schemars::schema::InstanceType::Boolean => DataType::Boolean,
                                    schemars::schema::InstanceType::Number => DataType::Float64,
                                    _ => panic!("Unsupported data type in schema"),
                                }
                            }
                            schemars::schema::SingleOrVec::Vec(vec_type) => {
                                match vec_type.as_slice() {
                                    [schemars::schema::InstanceType::String] => DataType::Utf8,
                                    [schemars::schema::InstanceType::Integer] => DataType::Int32,
                                    [schemars::schema::InstanceType::Boolean] => DataType::Boolean,
                                    [schemars::schema::InstanceType::Number] => DataType::Float64,
                                    _ => panic!("Unsupported data type in schema"),
                                }
                            }
                        },
                        _ => panic!("Unsupported schema format"),
                    };
                    Field::new(name, data_type, false)
                })
                .collect();

            let values: Vec<serde_json::Value> = data
                .iter()
                .map(|v| serde_json::from_slice(v).unwrap())
                .collect();

            let arrays: Vec<ArrayRef> = fields
                .iter()
                .map(|field| match field.data_type() {
                    DataType::Utf8 => {
                        let string_values: Vec<&str> = values
                            .iter()
                            .map(|value| value.get(field.name()).unwrap().as_str().unwrap())
                            .collect();
                        Arc::new(StringArray::from(string_values)) as ArrayRef
                    }
                    DataType::Int32 => {
                        let int_values: Vec<i32> = values
                            .iter()
                            .map(|value| value.get(field.name()).unwrap().as_i64().unwrap() as i32)
                            .collect();
                        Arc::new(Int32Array::from(int_values)) as ArrayRef
                    }
                    DataType::Boolean => {
                        let bool_values: Vec<bool> = values
                            .iter()
                            .map(|value| value.get(field.name()).unwrap().as_bool().unwrap())
                            .collect();
                        Arc::new(BooleanArray::from(bool_values)) as ArrayRef
                    }
                    DataType::Float64 => {
                        let float_values: Vec<f64> = values
                            .iter()
                            .map(|value| value.get(field.name()).unwrap().as_f64().unwrap())
                            .collect();
                        Arc::new(Float64Array::from(float_values)) as ArrayRef
                    }
                    _ => panic!("Unsupported data type"),
                })
                .collect();

            // Create a RecordBatch to return
            return RecordBatch::try_new(Arc::new(Schema::new(fields)), arrays).unwrap();
        }
        panic!("Unsupported schema");
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
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use schemars::{schema_for, JsonSchema};

    #[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
    struct ComplexKey {
        id: i64,
        uuid: String,
    }

    #[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
    struct ComplexValue {
        name: String,
        age: i32,
        description: String,
        active: bool,
        balance: f64,
    }

    #[test]
    fn test_encode_json() {
        let mut node: ProllyNode<1024> = ProllyNode::default();
        node.keys = vec![b"key1".to_vec(), b"key2".to_vec()];
        node.values = vec![b"value1".to_vec(), b"value2".to_vec()];
        node.encode_types = vec![EncodingType::Json];

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
    fn test_encode_json_complex() {
        let mut node: ProllyNode<1024> = ProllyNode::default();

        let keys = [
            ComplexKey {
                id: 1,
                uuid: "guid-key1".to_string(),
            },
            ComplexKey {
                id: 2,
                uuid: "guid-key2".to_string(),
            },
        ];
        let values = [
            ComplexValue {
                name: "name1".to_string(),
                age: 30,
                description: "value1".to_string(),
                active: true,
                balance: 100.0,
            },
            ComplexValue {
                name: "name2".to_string(),
                age: 55,
                description: "value2".to_string(),
                active: false,
                balance: -50.0,
            },
        ];

        node.keys = keys
            .iter()
            .map(|k| serde_json::to_vec(k).unwrap())
            .collect();
        node.values = values
            .iter()
            .map(|v| serde_json::to_vec(v).unwrap())
            .collect();
        node.encode_types = vec![EncodingType::Json];

        node.encode_all_pairs();

        for encoded_value in &node.encode_values {
            let decoded: Vec<(Vec<u8>, Vec<u8>)> = serde_json::from_slice(encoded_value).unwrap();
            for (i, (key, value)) in decoded.iter().enumerate() {
                let original_key: ComplexKey = serde_json::from_slice(key).unwrap();
                let original_value: ComplexValue = serde_json::from_slice(value).unwrap();
                assert_eq!(original_key, keys[i]);
                assert_eq!(original_value, values[i]);
            }
        }
    }

    #[test]
    fn test_encode_arrow() {
        let mut node: ProllyNode<1024> = ProllyNode::default();

        let keys = [
            ComplexKey {
                id: 1,
                uuid: "guid-key1".to_string(),
            },
            ComplexKey {
                id: 2,
                uuid: "guid-key2".to_string(),
            },
        ];
        let values = [
            ComplexValue {
                name: "name1".to_string(),
                age: 30,
                description: "value1".to_string(),
                active: true,
                balance: 100.0,
            },
            ComplexValue {
                name: "name2".to_string(),
                age: 55,
                description: "value2".to_string(),
                active: false,
                balance: -50.0,
            },
        ];

        node.keys = keys
            .iter()
            .map(|k| serde_json::to_vec(k).unwrap())
            .collect();
        node.values = values
            .iter()
            .map(|v| serde_json::to_vec(v).unwrap())
            .collect();
        node.encode_types = vec![EncodingType::Arrow];

        let key_schema = schema_for!(ComplexKey);
        let value_schema = schema_for!(ComplexValue);
        node.key_schema = Some(key_schema);
        node.value_schema = Some(value_schema);

        node.encode_all_pairs();

        for encoded_value in &node.encode_values {
            // Decode the Arrow IPC format
            let mut reader = StreamReader::try_new(encoded_value.as_slice(), None).unwrap();
            let batch = reader.next().unwrap().unwrap();

            // Convert the RecordBatch to a string for comparison
            let batch_string = record_batch_to_string(&batch);
            assert_eq!(batch.num_rows(), 2);
            println!("{}", batch_string);
            // Define the expected output
            let expected_output = r#"id: 1, 2
uuid: guid-key1, guid-key2
active: true, false
age: 30, 55
balance: 100, -50
description: value1, value2
name: name1, name2
"#;
            // Sort the lines of both strings to compare them
            let mut actual_lines: Vec<&str> = batch_string.trim().lines().collect();
            actual_lines.sort_unstable();
            let mut expected_lines: Vec<&str> = expected_output.trim().lines().collect();
            expected_lines.sort_unstable();

            assert_eq!(actual_lines, expected_lines);
        }
    }

    #[test]
    fn test_encode_parquet() {
        let mut node: ProllyNode<1024> = ProllyNode::default();

        let keys = [
            ComplexKey {
                id: 1,
                uuid: "guid-key1".to_string(),
            },
            ComplexKey {
                id: 2,
                uuid: "guid-key2".to_string(),
            },
        ];
        let values = [
            ComplexValue {
                name: "name1".to_string(),
                age: 30,
                description: "value1".to_string(),
                active: true,
                balance: 100.0,
            },
            ComplexValue {
                name: "name2".to_string(),
                age: 55,
                description: "value2".to_string(),
                active: false,
                balance: -50.0,
            },
        ];

        node.keys = keys
            .iter()
            .map(|k| serde_json::to_vec(k).unwrap())
            .collect();
        node.values = values
            .iter()
            .map(|v| serde_json::to_vec(v).unwrap())
            .collect();
        node.encode_types = vec![EncodingType::Parquet];

        let key_schema = schema_for!(ComplexKey);
        let value_schema = schema_for!(ComplexValue);
        node.key_schema = Some(key_schema);
        node.value_schema = Some(value_schema);

        node.encode_all_pairs();

        for encoded_value in &node.encode_values {
            // Decode the Parquet format
            let builder = ParquetRecordBatchReaderBuilder::try_new(bytes::Bytes::from(encoded_value.clone())).unwrap();
            let mut reader = builder.build().unwrap();
            let batch = reader.next().unwrap().unwrap();

            // Convert the RecordBatch to a string for comparison
            let batch_string = record_batch_to_string(&batch);
            assert_eq!(batch.num_rows(), 2);
            println!("{}", batch_string);
            // Define the expected output
            let expected_output = r#"id: 1, 2
uuid: guid-key1, guid-key2
name: name1, name2
age: 30, 55
description: value1, value2
active: true, false
balance: 100, -50
"#;
            // Sort the lines of both strings to compare them
            let mut actual_lines: Vec<&str> = batch_string.trim().lines().collect();
            actual_lines.sort_unstable();
            let mut expected_lines: Vec<&str> = expected_output.trim().lines().collect();
            expected_lines.sort_unstable();

            assert_eq!(actual_lines, expected_lines);
        }
    }

    fn record_batch_to_string(batch: &RecordBatch) -> String {
        let mut result = String::new();
        let schema = batch.schema(); // Store schema reference to avoid temporary value issues

        for column_index in 0..batch.num_columns() {
            let column = batch.column(column_index);
            let field = schema.field(column_index); // Use the stored schema reference

            result.push_str(&format!("{}: ", field.name()));

            match column.data_type() {
                DataType::Utf8 => {
                    let array = column.as_any().downcast_ref::<StringArray>().unwrap();
                    for i in 0..array.len() {
                        if i > 0 {
                            result.push_str(", ");
                        }
                        result.push_str(array.value(i));
                    }
                }
                DataType::Int32 => {
                    let array = column.as_any().downcast_ref::<Int32Array>().unwrap();
                    for i in 0..array.len() {
                        if i > 0 {
                            result.push_str(", ");
                        }
                        result.push_str(&array.value(i).to_string());
                    }
                }
                DataType::Boolean => {
                    let array = column.as_any().downcast_ref::<BooleanArray>().unwrap();
                    for i in 0..array.len() {
                        if i > 0 {
                            result.push_str(", ");
                        }
                        result.push_str(&array.value(i).to_string());
                    }
                }
                DataType::Float64 => {
                    let array = column.as_any().downcast_ref::<Float64Array>().unwrap();
                    for i in 0..array.len() {
                        if i > 0 {
                            result.push_str(", ");
                        }
                        result.push_str(&array.value(i).to_string());
                    }
                }
                _ => {
                    panic!("Unsupported data type");
                }
            }

            result.push('\n');
        }

        result
    }
}
