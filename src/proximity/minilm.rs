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

//! MiniLM-L6-v2 sentence embedder backed by Candle (PR 4b).
//!
//! Wires the `Embedder` trait to `sentence-transformers/all-MiniLM-L6-v2`:
//! a 384-dimensional BERT-based sentence encoder. Pure Rust via Candle, no
//! native deps (other than what `tokenizers` already brings in via `onig`).
//!
//! # Lifecycle
//!
//! The model and tokenizer are downloaded lazily on the first `embed()` call
//! into a local cache (`~/.cache/prollytree/embedders/<model_id>/<revision>/`
//! by default; override via `$PROLLYTREE_EMBEDDER_CACHE`). Subsequent calls
//! reuse the cached files. The download is ~90 MB total (tokenizer +
//! safetensors weights).
//!
//! # Determinism
//!
//! The version string is `"sentence-transformers/all-MiniLM-L6-v2@<revision>"`,
//! so a [`crate::proximity::TextIndex`] built under one revision refuses to
//! reopen under another (unless you call `reindex_from_texts` to re-embed).

use crate::proximity::embedder::{EmbedError, Embedder};
use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use parking_lot::Mutex;
use std::path::PathBuf;
use tokenizers::Tokenizer;

/// Default HuggingFace model id.
pub const DEFAULT_MODEL_ID: &str = "sentence-transformers/all-MiniLM-L6-v2";
/// Default revision — pinned to an immutable HuggingFace commit SHA so the
/// persisted version string (`"{model_id}@{revision}"`) cannot silently
/// accept reopens after upstream republishes `main`. Update this constant
/// (and bump the persisted version) only when intentionally moving to a new
/// model build. The SHA below corresponds to the snapshot used to bring up
/// this integration; to refresh, fetch the latest commit on the model's
/// `main` branch and pin its hash.
pub const DEFAULT_REVISION: &str = "8b3219a92973c328a8e22fadcfa821b5dc75636a";
/// MiniLM-L6-v2 produces 384-d embeddings.
pub const MINILM_DIM: u16 = 384;

/// Sentence embedder using all-MiniLM-L6-v2 via Candle.
pub struct MiniLmEmbedder {
    model_id: String,
    revision: String,
    /// Cached version string: `"{model_id}@{revision}"`.
    version: String,
    /// Lazily-initialised model + tokenizer + device. Locked behind a Mutex
    /// so `embed()` can take `&self` and remain `Send + Sync`.
    state: Mutex<Option<LoadedModel>>,
}

struct LoadedModel {
    tokenizer: Tokenizer,
    bert: BertModel,
    device: Device,
}

impl std::fmt::Debug for MiniLmEmbedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiniLmEmbedder")
            .field("model_id", &self.model_id)
            .field("revision", &self.revision)
            .field("loaded", &self.state.lock().is_some())
            .finish()
    }
}

impl Default for MiniLmEmbedder {
    fn default() -> Self {
        Self::new(DEFAULT_MODEL_ID, DEFAULT_REVISION)
    }
}

impl MiniLmEmbedder {
    /// Build an embedder pointing at the given HuggingFace model id and
    /// revision. Use [`MiniLmEmbedder::default`] for the recommended
    /// `sentence-transformers/all-MiniLM-L6-v2@main`.
    pub fn new(model_id: &str, revision: &str) -> Self {
        let version = format!("{model_id}@{revision}");
        Self {
            model_id: model_id.to_string(),
            revision: revision.to_string(),
            version,
            state: Mutex::new(None),
        }
    }

    /// Eagerly download + load the model. Useful if the caller wants to pay
    /// the cold-start cost up-front (e.g., during server startup) rather
    /// than during the first `embed()` call.
    pub fn warm_up(&self) -> Result<(), EmbedError> {
        self.ensure_loaded()
    }

    fn ensure_loaded(&self) -> Result<(), EmbedError> {
        let mut state = self.state.lock();
        if state.is_some() {
            return Ok(());
        }
        let loaded = load_model(&self.model_id, &self.revision)
            .map_err(|e| EmbedError::Failure(format!("MiniLM load: {e}")))?;
        *state = Some(loaded);
        Ok(())
    }
}

impl Embedder for MiniLmEmbedder {
    fn id(&self) -> &str {
        "candle:minilm-l6-v2"
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn dim(&self) -> u16 {
        MINILM_DIM
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        self.ensure_loaded()?;
        let state = self.state.lock();
        let loaded = state.as_ref().expect("loaded by ensure_loaded");

        embed_one(&loaded.tokenizer, &loaded.bert, &loaded.device, text)
            .map_err(|e| EmbedError::Failure(format!("MiniLM embed: {e}")))
    }
}

// ---------------------------------------------------------------------------
// Implementation helpers
// ---------------------------------------------------------------------------

fn load_model(
    model_id: &str,
    revision: &str,
) -> Result<LoadedModel, Box<dyn std::error::Error + Send + Sync>> {
    let cache_dir = embedder_cache_dir(model_id, revision)?;
    std::fs::create_dir_all(&cache_dir)?;

    let config_path = fetch_with_cache(&cache_dir, model_id, revision, "config.json")?;
    let tokenizer_path = fetch_with_cache(&cache_dir, model_id, revision, "tokenizer.json")?;
    let weights_path = fetch_with_cache(&cache_dir, model_id, revision, "model.safetensors")?;

    let config_json = std::fs::read_to_string(&config_path)?;
    let config: Config = serde_json::from_str(&config_json)?;

    let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(
        |e| -> Box<dyn std::error::Error + Send + Sync> { format!("tokenizer: {e}").into() },
    )?;

    let device = Device::Cpu;
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)? };
    let bert = BertModel::load(vb, &config)?;

    Ok(LoadedModel {
        tokenizer,
        bert,
        device,
    })
}

/// Resolve the on-disk cache directory for a given (model_id, revision).
///
/// Honoured precedence:
///
/// 1. `$PROLLYTREE_EMBEDDER_CACHE/<model_id>/<revision>/`
/// 2. `<dirs::cache_dir()>/prollytree/embedders/<model_id>/<revision>/`
fn embedder_cache_dir(
    model_id: &str,
    revision: &str,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let base = if let Ok(custom) = std::env::var("PROLLYTREE_EMBEDDER_CACHE") {
        PathBuf::from(custom)
    } else {
        dirs::cache_dir()
            .ok_or("could not resolve a user cache directory")?
            .join("prollytree")
            .join("embedders")
    };
    // `model_id` and `revision` come from the public `MiniLmEmbedder::new`
    // constructor — so they're user-controlled. Reject absolute paths or
    // `..` segments outright, and sanitise other separators, to make sure the
    // resulting cache path stays under `base` rather than escaping it (which
    // `PathBuf::join` would silently allow for an absolute argument).
    Ok(base
        .join(sanitise_cache_component(model_id)?)
        .join(sanitise_cache_component(revision)?))
}

/// Validate a path component that came from user-controlled input. The
/// component is rejected if it would let the join escape its base, and
/// otherwise has its forward/backslashes replaced with `_` so the result is
/// always a single safe directory name.
fn sanitise_cache_component(s: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if s.is_empty() {
        return Err("embedder cache component must not be empty".into());
    }
    if std::path::Path::new(s).is_absolute() {
        return Err(format!("embedder cache component must not be absolute: {s:?}").into());
    }
    if s.split(['/', '\\']).any(|seg| seg == ".." || seg == ".") {
        return Err(format!("embedder cache component must not contain `..` or `.`: {s:?}").into());
    }
    // Replace path separators with `_` so a name like
    // `sentence-transformers/all-MiniLM-L6-v2` becomes a single directory
    // rather than two nested levels (keeps the layout predictable across
    // platforms where one separator might be more privileged than another).
    Ok(s.replace(['/', '\\'], "_"))
}

/// Fetch a single file from HuggingFace into the cache directory, returning
/// the local path. If the file is already present, the download is skipped.
fn fetch_with_cache(
    cache_dir: &std::path::Path,
    model_id: &str,
    revision: &str,
    filename: &str,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let local_path = cache_dir.join(filename);
    if local_path.exists() {
        return Ok(local_path);
    }

    let url = format!("https://huggingface.co/{model_id}/resolve/{revision}/{filename}");
    let agent = ureq::AgentBuilder::new()
        .redirects(10)
        .timeout_connect(std::time::Duration::from_secs(30))
        .build();
    let response =
        agent
            .get(&url)
            .call()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("download {url}: {e}").into()
            })?;
    if response.status() != 200 {
        return Err(format!("download {url}: unexpected status {}", response.status()).into());
    }

    // Write to a temp file first, then rename atomically so a partial
    // download never leaves a corrupted cache file behind.
    let tmp_path = cache_dir.join(format!("{filename}.partial"));
    {
        let mut reader = response.into_reader();
        let mut writer = std::fs::File::create(&tmp_path)?;
        std::io::copy(&mut reader, &mut writer)?;
    }
    std::fs::rename(&tmp_path, &local_path)?;
    Ok(local_path)
}

fn embed_one(
    tokenizer: &Tokenizer,
    bert: &BertModel,
    device: &Device,
    text: &str,
) -> Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>> {
    let encoding =
        tokenizer
            .encode(text, true)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("encode: {e}").into()
            })?;
    let input_ids: Vec<u32> = encoding.get_ids().to_vec();
    let attention_mask: Vec<u32> = encoding.get_attention_mask().to_vec();
    let seq_len = input_ids.len();

    if seq_len == 0 {
        // Edge case: empty input. Return a zero vector that's still
        // L2-normalised (i.e., zero remains zero — downstream distance
        // calls handle zero gracefully).
        return Ok(vec![0.0_f32; MINILM_DIM as usize]);
    }

    let input_ids = Tensor::new(input_ids.as_slice(), device)?.unsqueeze(0)?;
    let attention_mask_t = Tensor::new(attention_mask.as_slice(), device)?.unsqueeze(0)?;
    let token_type_ids = input_ids.zeros_like()?;

    // Forward pass — token-level embeddings of shape [1, seq_len, hidden].
    let token_embeddings = bert.forward(&input_ids, &token_type_ids, Some(&attention_mask_t))?;

    // Mean-pool over the sequence axis, weighted by the attention mask.
    let mask_f = attention_mask_t.to_dtype(DType::F32)?.unsqueeze(2)?;
    let mask_expanded = mask_f.broadcast_as(token_embeddings.shape())?;
    let weighted = token_embeddings.broadcast_mul(&mask_expanded)?;
    let summed = weighted.sum(1)?; // [1, hidden]
    let counts = attention_mask_t
        .to_dtype(DType::F32)?
        .sum_keepdim(1)?
        .clamp(1e-9_f32, f32::MAX)?; // [1, 1]
    let mean = summed.broadcast_div(&counts)?; // [1, hidden]

    // L2-normalise.
    let norm = mean
        .sqr()?
        .sum_keepdim(1)?
        .sqrt()?
        .clamp(1e-12_f32, f32::MAX)?; // [1, 1]
    let normalised = mean.broadcast_div(&norm)?;

    let vector: Vec<f32> = normalised.i(0)?.to_vec1::<f32>()?;
    if vector.len() != MINILM_DIM as usize {
        return Err(format!("expected {} dims, got {}", MINILM_DIM, vector.len()).into());
    }
    Ok(vector)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: skip embed-running tests when the network or HF cache isn't
    /// available. We attempt one warm-up; if it fails, mark the test as
    /// not-run rather than failing CI on offline machines.
    fn require_loaded_or_skip(e: &MiniLmEmbedder, test_name: &str) -> bool {
        match e.warm_up() {
            Ok(()) => true,
            Err(err) => {
                eprintln!(
                    "[{test_name}] skipping: MiniLM model not loadable ({err}); \
                     run with network access + HF cache to validate."
                );
                false
            }
        }
    }

    // ---- Pure metadata tests (no model load required) --------------------

    #[test]
    fn metadata_is_stable() {
        let e = MiniLmEmbedder::default();
        assert_eq!(e.id(), "candle:minilm-l6-v2");
        assert_eq!(e.dim(), 384);
        assert!(e.version().contains(DEFAULT_MODEL_ID));
        assert!(e.version().contains(DEFAULT_REVISION));
    }

    #[test]
    fn version_distinguishes_revisions() {
        let a = MiniLmEmbedder::new(DEFAULT_MODEL_ID, "main");
        let b = MiniLmEmbedder::new(DEFAULT_MODEL_ID, "refs/pr/1");
        assert_ne!(a.version(), b.version());
    }

    #[test]
    fn version_distinguishes_models() {
        let a = MiniLmEmbedder::new("foo/bar", "main");
        let b = MiniLmEmbedder::new("baz/qux", "main");
        assert_ne!(a.version(), b.version());
    }

    // ---- Network-dependent tests (mark ignored so default test runs skip
    //      them; pass `--include-ignored` to run with model download). -----

    #[test]
    #[ignore = "downloads model from HuggingFace; run with --include-ignored"]
    fn embed_returns_correct_dim() {
        let e = MiniLmEmbedder::default();
        if !require_loaded_or_skip(&e, "embed_returns_correct_dim") {
            return;
        }
        let v = e.embed("hello world").unwrap();
        assert_eq!(v.len(), 384);
    }

    #[test]
    #[ignore = "downloads model from HuggingFace; run with --include-ignored"]
    fn embed_is_deterministic() {
        let e = MiniLmEmbedder::default();
        if !require_loaded_or_skip(&e, "embed_is_deterministic") {
            return;
        }
        let a = e.embed("the quick brown fox").unwrap();
        let b = e.embed("the quick brown fox").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    #[ignore = "downloads model from HuggingFace; run with --include-ignored"]
    fn embed_distinguishes_different_text() {
        let e = MiniLmEmbedder::default();
        if !require_loaded_or_skip(&e, "embed_distinguishes_different_text") {
            return;
        }
        let a = e.embed("the quick brown fox").unwrap();
        let b = e
            .embed("an entirely unrelated sentence about ducks")
            .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    #[ignore = "downloads model from HuggingFace; run with --include-ignored"]
    fn embed_semantically_similar_texts_are_close() {
        let e = MiniLmEmbedder::default();
        if !require_loaded_or_skip(&e, "embed_semantically_similar_texts_are_close") {
            return;
        }
        let a = e.embed("the cat sat on the mat").unwrap();
        let b = e.embed("a cat is sitting on a mat").unwrap();
        let c = e.embed("quantum field theory and renormalization").unwrap();

        // Cosine distance: 1 - cosine_similarity (smaller is closer).
        let cos = |x: &[f32], y: &[f32]| -> f32 {
            let dot: f32 = x.iter().zip(y).map(|(a, b)| a * b).sum();
            let nx: f32 = x.iter().map(|v| v * v).sum::<f32>().sqrt();
            let ny: f32 = y.iter().map(|v| v * v).sum::<f32>().sqrt();
            1.0 - dot / (nx * ny + 1e-9)
        };
        let close = cos(&a, &b);
        let far = cos(&a, &c);
        assert!(
            close < far,
            "expected '{}' / '{}' (cos {:.3}) closer than '{}' / '{}' (cos {:.3})",
            "the cat sat on the mat",
            "a cat is sitting on a mat",
            close,
            "the cat sat on the mat",
            "quantum field theory and renormalization",
            far
        );
    }
}
