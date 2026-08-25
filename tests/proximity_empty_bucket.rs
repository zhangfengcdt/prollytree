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

#![cfg(feature = "proximity")]

use prollytree::proximity::{vector_level, Metric, ProximityConfig, ProximityIndex};
use std::collections::BTreeMap;

fn config() -> ProximityConfig {
    ProximityConfig {
        dim: 2,
        metric: Metric::InnerProduct,
        level_bits: 1,
        max_bucket_size: 64,
    }
}

fn promoted_collinear_pair() -> ((Vec<u8>, Vec<f32>), (Vec<u8>, Vec<f32>)) {
    let high_vector = vec![10.0, 0.0];
    let low_vector = vec![1.0, 0.0];
    let mut high_by_level: BTreeMap<u8, Vec<Vec<u8>>> = BTreeMap::new();
    let mut low_by_level: BTreeMap<u8, Vec<Vec<u8>>> = BTreeMap::new();

    for i in 0..20_000u32 {
        let high_id = format!("high-{i:05}").into_bytes();
        let high_level = vector_level(&high_id, &high_vector, 1);
        if high_level > 0 {
            high_by_level.entry(high_level).or_default().push(high_id);
        }

        let low_id = format!("low-{i:05}").into_bytes();
        let low_level = vector_level(&low_id, &low_vector, 1);
        if low_level > 0 {
            low_by_level.entry(low_level).or_default().push(low_id);
        }
    }

    for (level, high_ids) in high_by_level.iter().rev() {
        if let Some(low_ids) = low_by_level.get(level) {
            return (
                (high_ids[0].clone(), high_vector),
                (low_ids[0].clone(), low_vector),
            );
        }
    }

    panic!("test fixture could not find a promoted collinear pair");
}

#[test]
fn inner_product_empty_bucket_does_not_drop_knn_results() {
    let ((high_id, high_vector), (low_id, low_vector)) = promoted_collinear_pair();
    let mut index = ProximityIndex::<32, _>::new_in_memory(config());
    index.insert(high_id, high_vector).unwrap();
    index.insert(low_id.clone(), low_vector).unwrap();

    let hits = index.knn(&[-1.0, 0.0], 1, 1).unwrap();

    assert_eq!(
        hits.first().map(|(id, _)| id),
        Some(&low_id),
        "empty boundary buckets must not consume the beam or drop populated siblings"
    );
}
