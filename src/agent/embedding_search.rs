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

use async_trait::async_trait;
use std::collections::HashMap;

use super::traits::{EmbeddingGenerator, MemoryError, SearchableMemoryStore};
use super::types::*;

/// Simple embedding generator that uses text length as a proxy
/// In a real implementation, this would use a proper embedding model
pub struct MockEmbeddingGenerator;

#[async_trait]
impl EmbeddingGenerator for MockEmbeddingGenerator {
    async fn generate(&self, text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        // Mock implementation: create a simple vector based on text characteristics
        let words: Vec<&str> = text.split_whitespace().collect();
        let word_count = words.len() as f32;
        let char_count = text.len() as f32;
        let avg_word_length = if word_count > 0.0 {
            char_count / word_count
        } else {
            0.0
        };

        // Create a 384-dimensional vector (common embedding size)
        let mut embedding = vec![0.0; 384];

        // Fill with simple features
        embedding[0] = word_count / 100.0; // Normalized word count
        embedding[1] = char_count / 1000.0; // Normalized character count
        embedding[2] = avg_word_length / 10.0; // Average word length

        // Add some pseudo-random elements based on text content
        for (i, word) in words.iter().take(50).enumerate() {
            let word_hash = self.simple_hash(word) % 100;
            if i + 3 < embedding.len() {
                embedding[i + 3] = (word_hash as f32) / 100.0;
            }
        }

        // Normalize the vector
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for val in &mut embedding {
                *val /= magnitude;
            }
        }

        Ok(embedding)
    }

    async fn generate_batch(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
        let mut embeddings = Vec::new();
        for text in texts {
            embeddings.push(self.generate(text).await?);
        }
        Ok(embeddings)
    }
}

impl MockEmbeddingGenerator {
    fn simple_hash(&self, s: &str) -> usize {
        s.chars()
            .fold(0, |acc, c| acc.wrapping_mul(31).wrapping_add(c as usize))
    }
}

/// Advanced search functionality for memory stores
pub struct MemorySearchEngine<T: SearchableMemoryStore> {
    store: T,
    #[allow(dead_code)]
    embedding_cache: HashMap<String, Vec<f32>>,
}

impl<T: SearchableMemoryStore> MemorySearchEngine<T> {
    pub fn new(store: T) -> Self {
        Self {
            store,
            embedding_cache: HashMap::new(),
        }
    }

    /// Perform hybrid search combining text and semantic search
    pub async fn hybrid_search(
        &mut self,
        text_query: &str,
        semantic_weight: f64,
        text_weight: f64,
        namespace: Option<&MemoryNamespace>,
        limit: usize,
    ) -> Result<Vec<(MemoryDocument, f64)>, MemoryError> {
        // Get text search results
        let text_results = self.store.text_search(text_query, namespace).await?;

        // Create mock semantic query
        let mock_embeddings = vec![0.0; 384]; // Would generate from text_query
        let semantic_query = SemanticQuery {
            embeddings: mock_embeddings,
            threshold: 0.1,
            metric: DistanceMetric::Cosine,
        };

        // Get semantic search results
        let semantic_results = self
            .store
            .semantic_search(semantic_query, namespace)
            .await?;

        // Combine and score results
        let mut combined_scores: HashMap<String, f64> = HashMap::new();
        let mut all_memories: HashMap<String, MemoryDocument> = HashMap::new();

        // Add text search scores
        for memory in text_results {
            let score = text_weight * self.calculate_text_relevance(text_query, &memory);
            combined_scores.insert(memory.id.clone(), score);
            all_memories.insert(memory.id.clone(), memory);
        }

        // Add semantic search scores
        for (memory, sem_score) in semantic_results {
            let existing_score = combined_scores.get(&memory.id).unwrap_or(&0.0);
            let total_score = existing_score + (semantic_weight * sem_score);
            combined_scores.insert(memory.id.clone(), total_score);
            all_memories.insert(memory.id.clone(), memory);
        }

        // Sort by combined score and return top results
        let mut scored_results: Vec<(MemoryDocument, f64)> = combined_scores
            .into_iter()
            .filter_map(|(id, score)| all_memories.remove(&id).map(|memory| (memory, score)))
            .collect();

        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(limit);

        Ok(scored_results)
    }

    /// Find memories similar to a given memory
    pub async fn find_similar_memories(
        &mut self,
        reference_memory_id: &str,
        similarity_threshold: f64,
        limit: usize,
    ) -> Result<Vec<(MemoryDocument, f64)>, MemoryError> {
        // This would use the reference memory's embeddings to find similar ones
        // For now, delegate to the store's find_related method
        let related = self.store.find_related(reference_memory_id, limit).await?;

        // Convert to scored results with mock similarity scores
        let scored_results = related
            .into_iter()
            .enumerate()
            .map(|(i, memory)| {
                let score = 1.0 - (i as f64 * 0.1); // Decreasing scores
                (memory, score.max(similarity_threshold))
            })
            .filter(|(_, score)| *score >= similarity_threshold)
            .collect();

        Ok(scored_results)
    }

    /// Search for memories by temporal patterns
    pub async fn temporal_search(
        &self,
        time_pattern: TemporalPattern,
        namespace: Option<&MemoryNamespace>,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        let time_range = match time_pattern {
            TemporalPattern::LastHour => {
                let end = chrono::Utc::now();
                let start = end - chrono::Duration::hours(1);
                TimeRange {
                    start: Some(start),
                    end: Some(end),
                }
            }
            TemporalPattern::LastDay => {
                let end = chrono::Utc::now();
                let start = end - chrono::Duration::days(1);
                TimeRange {
                    start: Some(start),
                    end: Some(end),
                }
            }
            TemporalPattern::LastWeek => {
                let end = chrono::Utc::now();
                let start = end - chrono::Duration::weeks(1);
                TimeRange {
                    start: Some(start),
                    end: Some(end),
                }
            }
            TemporalPattern::Custom(start, end) => TimeRange { start, end },
        };

        let query = MemoryQuery {
            namespace: namespace.cloned(),
            memory_types: None,
            tags: None,
            time_range: Some(time_range),
            text_query: None,
            semantic_query: None,
            limit: None,
            include_expired: false,
        };

        self.store.query(query).await
    }

    /// Search by tags with boolean logic
    pub async fn tag_search(
        &self,
        tag_query: TagQuery,
        namespace: Option<&MemoryNamespace>,
    ) -> Result<Vec<MemoryDocument>, MemoryError> {
        match tag_query {
            TagQuery::And(tags) => {
                let query = MemoryQuery {
                    namespace: namespace.cloned(),
                    memory_types: None,
                    tags: Some(tags),
                    time_range: None,
                    text_query: None,
                    semantic_query: None,
                    limit: None,
                    include_expired: false,
                };
                self.store.query(query).await
            }
            TagQuery::Or(tags) => {
                // For OR queries, we need to search for each tag separately and combine
                let mut all_results = Vec::new();
                let mut seen_ids = std::collections::HashSet::new();

                for tag in tags {
                    let query = MemoryQuery {
                        namespace: namespace.cloned(),
                        memory_types: None,
                        tags: Some(vec![tag]),
                        time_range: None,
                        text_query: None,
                        semantic_query: None,
                        limit: None,
                        include_expired: false,
                    };

                    let results = self.store.query(query).await?;
                    for memory in results {
                        if !seen_ids.contains(&memory.id) {
                            seen_ids.insert(memory.id.clone());
                            all_results.push(memory);
                        }
                    }
                }

                Ok(all_results)
            }
            TagQuery::Not(tag) => {
                // Get all memories and filter out those with the tag
                let query = MemoryQuery {
                    namespace: namespace.cloned(),
                    memory_types: None,
                    tags: None,
                    time_range: None,
                    text_query: None,
                    semantic_query: None,
                    limit: None,
                    include_expired: false,
                };

                let all_results = self.store.query(query).await?;
                let filtered = all_results
                    .into_iter()
                    .filter(|memory| !memory.metadata.tags.contains(&tag))
                    .collect();

                Ok(filtered)
            }
        }
    }

    /// Calculate text relevance score
    fn calculate_text_relevance(&self, query: &str, memory: &MemoryDocument) -> f64 {
        let query_lower = query.to_lowercase();
        let content_str = memory.content.to_string().to_lowercase();

        // Simple scoring based on term frequency
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        let content_words: Vec<&str> = content_str.split_whitespace().collect();

        if query_words.is_empty() || content_words.is_empty() {
            return 0.0;
        }

        let mut score = 0.0;
        for query_word in &query_words {
            let count = content_words
                .iter()
                .filter(|&&word| word == *query_word)
                .count();
            score += count as f64;
        }

        // Normalize by content length
        score / content_words.len() as f64
    }
}

/// Temporal patterns for time-based searches
#[derive(Debug, Clone)]
pub enum TemporalPattern {
    LastHour,
    LastDay,
    LastWeek,
    Custom(
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ),
}

/// Tag query with boolean logic
#[derive(Debug, Clone)]
pub enum TagQuery {
    And(Vec<String>),
    Or(Vec<String>),
    Not(String),
}

/// Distance calculation utilities
pub struct DistanceCalculator;

impl DistanceCalculator {
    /// Calculate cosine similarity between two vectors
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            0.0
        } else {
            (dot_product / (magnitude_a * magnitude_b)) as f64
        }
    }

    /// Calculate Euclidean distance between two vectors
    pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() {
            return f64::INFINITY;
        }

        let distance: f32 = a
            .iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<f32>()
            .sqrt();

        distance as f64
    }

    /// Calculate dot product between two vectors
    pub fn dot_product(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() {
            return 0.0;
        }

        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>() as f64
    }
}
