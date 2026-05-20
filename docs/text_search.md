# Text Indexing & Vector Search

ProllyTree includes a **version-controlled approximate-nearest-neighbour (ANN) index** that sits inside any namespace of a `NamespacedKvStore`. You can do semantic similarity search on the same data that the rest of the store versions, branches, and merges — without standing up a separate vector database.

For the conceptual model see [Architecture → Proximity / text-search layer](architecture.md#7-proximity-text-search-layer). For runnable code see [Examples → Text Search](examples/text_search.md).

## When to use it

- You're already using ProllyTree (or want to) for versioned storage.
- You need top-k nearest-neighbour over short-ish text (notes, docs, logs, chunks).
- You want index + primary data to commit, branch, and merge **atomically** — no separate sync job between a vector DB and your source of truth.

It is not a replacement for a high-end ANN library on billion-scale corpora. The current implementation targets per-namespace corpora in the thousands-to-millions range with deterministic, history-independent index shape.

## How it works in 60 seconds

```
NamespacedKvStore (one git repo)
├── namespace "docs"
│   ├── primary tree           ← source of truth: doc_id → body bytes
│   ├── text sub-index "by_body"      ← (id, vector) pairs, ANN-searchable
│   └── text sub-index "by_summary"   ← multiple indexes per namespace
└── ...
```

A text index turns each document into one or more vectors via a configurable **embedder** and stores them inside a Dolt-style proximity tree (Merkle ANN structure — see [the design discussion](https://www.dolthub.com/blog/2025-06-23-vector-index-deep-dive/)). The proximity tree's *shape* is a pure function of the current `(id, vector)` set, so two replicas with the same data converge to the same root hash regardless of insertion order — the same content-defined-shape property the prolly tree itself has.

The proximity index stores only `(id, vector)` pairs — never the source text. **The primary KV tree is the source of truth.** Search results give you ids; you resolve back to the original text via the primary tree.

## Feature flags

ProllyTree's text-search surface is gated behind two Cargo features:

| Feature | Pulls in | Purpose |
|---|---|---|
| `proximity` | nothing extra | Raw vector index + text-index infrastructure. ML-free; ships [`HashEmbedder`](#hashembedder) for tests and the [`CallableEmbedder`](#callableembedder) shim for "bring your own embedder". |
| `proximity_text` | Candle (pure-Rust ML), tokenizers, ureq | Adds the bundled [`MiniLmEmbedder`](#minilmembedder). First call downloads ~90 MB of weights into `$PROLLYTREE_EMBEDDER_CACHE` (default `~/.cache/prollytree/embedders`). |

PyPI wheels and `cargo install`'d builds ship both features by default.

## Supported backends

v1 of the proximity index is fully supported on the **File** and **RocksDB** storage backends. `InMemoryNodeStorage` works for testing. `GitNodeStorage` is mechanically functional but its hash-mapping is only flushed by the higher-level commit path, so production use of git-backed proximity is exercised through `NamespacedKvStore` rather than direct `ProximityIndex<_, GitNodeStorage<_>>` constructions.

## API tour (Python)

A complete runnable walkthrough lives in [Examples → Text Search](examples/text_search.md). The minimum surface you need:

### Open or re-open an index

```python
from prollytree import NamespacedKvStore, MiniLmEmbedder

store = NamespacedKvStore("./data")
emb = MiniLmEmbedder()                       # or HashEmbedder / CallableEmbedder

# Creates the index on first call; on subsequent calls validates that the
# supplied embedder's id + version match what's persisted on disk.
store.text_index_open("docs", "by_body", emb)
```

The embedder's `id` and `version` are persisted on first open and re-checked on every reopen. Mismatch raises a clear error so you don't silently mix vectors produced by different models.

### Dual-write (the canonical pattern)

```python
docs = {
    b"doc:1": "the quick brown fox",
    b"doc:2": "lazy dog asleep on the mat",
}
for doc_id, text in docs.items():
    store.ns_insert("docs", doc_id, text.encode())            # primary tree (truth)
    store.text_index_insert("docs", "by_body", doc_id, text)  # index (pointer)
store.commit("seed corpus")
```

Both writes land in the **same git commit** atomically. The primary tree carries the source bytes; the index carries the vectors. If you ever change embedders, you can re-embed every doc from the primary tree.

### Cascade — replace the dual write with one call

```python
store.text_index_open("docs", "by_body", emb)
store.set_cascade("docs", ["by_body"])                        # opt-in once

store.ns_insert("docs", b"doc:3", b"branching is first-class")
store.commit("cascade-driven indexing")                       # also updated the index
```

Cascade is per-namespace and runtime-only (not persisted in the store registry). `ns_delete` cascades too. A namespace can cascade into multiple indexes — e.g. `["by_body", "by_title"]` — and each index can run its own value-transformer for non-UTF-8 primary values.

### Search and resolve back to text

```python
for doc_id, score in store.text_index_search("docs", "by_body", "vulpine animal", k=5):
    body = store.ns_get("docs", doc_id).decode()
    print(f"{doc_id} (distance={score:.3f}): {body}")
```

Returns `(id_bytes, distance)` tuples ordered by ascending distance (closer first). Multi-chunk indexes (see below) automatically dedup so each document appears once at its best chunk's score.

### Multi-chunk indexing

Pass a chunker by name when opening the index. The chunk-id encoding is `[doc_id_len:4][doc_id_bytes][chunk_idx:4]`, so every chunk for a doc shares a prefix and `text_index_delete` can prefix-scan-remove them all.

```python
store.text_index_open("logs", "by_line", emb, chunker="line")
store.text_index_insert("logs", "by_line", b"log:2026-05-20",
                        "alpha\nbeta\ngamma")     # 3 chunks under one doc id
store.text_index_len("logs", "by_line")           # 1 (distinct documents)
store.text_index_chunk_count("logs", "by_line")   # 3 (raw chunks)
```

Built-in chunkers: `"identity"` (default — one chunk per doc) and `"line"` (one chunk per non-empty line).

## Embedders

### `HashEmbedder`

Deterministic SHA-256-based embedder. Pure Rust, no deps, useful for tests and exact-match lookup. **Not semantic** — `"a cat sat"` and `"a feline rested"` will land in unrelated parts of the vector space.

```python
from prollytree import HashEmbedder
emb = HashEmbedder(dim=384, seed=0)
```

### `MiniLmEmbedder`

Bundled Candle + `sentence-transformers/all-MiniLM-L6-v2` (384-d). Real semantic search. First call downloads weights (~90 MB) to `~/.cache/prollytree/embedders/`.

```python
from prollytree import MiniLmEmbedder
emb = MiniLmEmbedder()                                       # default model + revision
emb = MiniLmEmbedder(model_id="...", revision="main")        # override either field
```

Requires the `proximity_text` feature. Set `PROLLYTREE_EMBEDDER_CACHE` to relocate the cache directory.

### `CallableEmbedder`

Wrap any Python embedding function as an Embedder — use this to plug in OpenAI, Cohere, sentence-transformers, your own pipeline, etc.

```python
from prollytree import CallableEmbedder
from openai import OpenAI

client = OpenAI()
def openai_embed(text):
    return client.embeddings.create(
        input=text, model="text-embedding-3-small"
    ).data[0].embedding

emb = CallableEmbedder(
    id="openai:text-embedding-3-small",
    version="2024-01",
    dim=1536,
    embed_fn=openai_embed,
)
```

The `id` and `version` are what's persisted. **Change `version` whenever the embedding distribution changes** (model upgrade, new tokenizer) so reopens correctly surface the mismatch.

## Drift management

If you write to the primary tree without cascade and forget to mirror into the index — or you change the embedder mid-history — the index can drift from the primary. Detection and repair are first-class:

```python
report = store.audit_text_index("docs", "by_body")
# {"orphans_in_index": [...], "missing_from_index": [...], "is_in_sync": False}

store.purge_text_index_orphans("docs", "by_body")    # remove index entries
                                                     # that have no primary row
```

Filling `missing_from_index` is your call — typically a loop over the listed ids that re-inserts each from the primary tree.

## Branching and merging

Every text index is owned by its namespace and versioned alongside the namespace's primary tree. The store-wide `branch`, `checkout`, `merge` operations move every namespace's primary tree **and** every sub-index together. Switching branches gives you that branch's view of both data and search results.

Three-way merge for text indexes runs the same nine-case logic as the primary KV merge, routed through a `ProximityConflictResolver`. Built-in resolvers (in the Rust crate; not yet surfaced in Python):

- `TakeSourceProximityResolver` / `TakeDestinationProximityResolver`
- `LatestVectorResolver<F>` — timestamp-extractor function picks the newer vector
- `MeanVectorResolver` — averages conflicting vectors (L2 / Cosine only)

## Externalisation + blob GC

Large documents (set via `store.set_externalize_threshold(bytes)`) are stored as content-addressed blobs alongside the prolly tree, with only a 44-byte envelope inline in the leaf. `store.gc_blobs()` walks the current store and reclaims unreferenced blobs. Useful when you're indexing real document bodies rather than short snippets.

```python
store.set_externalize_threshold(64 * 1024)        # 64 KiB threshold
report = store.gc_blobs()
# {"total": 12, "referenced": 12, "removed": 0, "errors": []}
```

Externalisation is currently supported on the **File** and **RocksDB** backends.

## Where to go next

- [Examples → Text Search](examples/text_search.md) — runnable end-to-end demos.
- [Python API](api/python.md#namespacedkvstore) — full method-level reference.
- [Architecture → Proximity / text-search layer](architecture.md#7-proximity-text-search-layer) — how the proximity tree integrates with the rest of the stack.
