# Text Search

Runnable Python examples for the text-index + vector-search surface on `NamespacedKvStore`. For the conceptual model and full surface area see [Text Indexing & Vector Search](../text_search.md).

A complete runnable script — covering every snippet on this page plus a MiniLM end-to-end demo — lives at [`python/examples/text_index_example.py`](https://github.com/zhangfengcdt/prollytree/blob/main/python/examples/text_index_example.py).

!!! tip "Browser demo"
    Want to see the workflow without installing anything? The [interactive demo](../text_search_demo.html) runs a toy search against a static corpus in your browser, and includes the same code snippets shown below.

## Setup

```python
import os, subprocess, tempfile
from prollytree import NamespacedKvStore, HashEmbedder

tmp = tempfile.mkdtemp()
subprocess.run(["git", "init"], cwd=tmp, check=True, capture_output=True)
subprocess.run(["git", "config", "user.name",  "You"],       cwd=tmp, check=True)
subprocess.run(["git", "config", "user.email", "you@x.com"], cwd=tmp, check=True)
dataset = os.path.join(tmp, "dataset"); os.makedirs(dataset)

store = NamespacedKvStore(dataset)
emb = HashEmbedder(dim=64, seed=0)        # deterministic, ML-free; swap for MiniLmEmbedder for real semantic search
```

## Dual-write + resolve hits back to text

The primary KV tree is the source of truth; the index stores only `(id, vector)` pairs. Write both, then resolve search hits back to their text via the primary.

```python
store.text_index_open("personal", "docs", emb)

docs = {
    b"doc:1": "the quick brown fox jumps over the lazy dog",
    b"doc:2": "rust is a systems programming language",
    b"doc:3": "merkle trees enable verifiable data structures",
    b"doc:4": "the fox and the hound are forest friends",
}
for doc_id, text in docs.items():
    store.ns_insert("personal", doc_id, text.encode())            # primary
    store.text_index_insert("personal", "docs", doc_id, text)     # index
store.commit("seed corpus + index")

for doc_id, score in store.text_index_search("personal", "docs",
                                             "the quick brown fox", k=2):
    body = store.ns_get("personal", doc_id).decode()
    print(f"{doc_id!r}  distance={score:.4f}  body={body!r}")
```

## Cascade — one call writes to both

```python
store.text_index_open("notes", "by_body", emb)
store.set_cascade("notes", ["by_body"])

# ns_insert now also embeds + inserts into the registered text indexes.
store.ns_insert("notes", b"note:1", b"meeting with the platform team")
store.ns_insert("notes", b"note:2", b"draft proposal for Q3 roadmap")
store.commit("cascade-driven indexing")

# Deletes cascade too.
store.ns_delete("notes", b"note:1")
store.commit("cascade-driven delete")

print(store.cascade_for_namespace("notes"))    # ['by_body']
store.clear_cascade("notes")                   # disable
```

## Multi-chunk via `LineChunker`

```python
store.text_index_open("logs", "by_line", emb, chunker="line")

log = (
    "2026-05-20T09:00 startup: loading config\n"
    "2026-05-20T09:01 startup: bound port 8080\n"
    "2026-05-20T09:42 error: database timeout after 30s\n"
    "2026-05-20T09:43 retry: reconnecting to database\n"
    "2026-05-20T09:43 recovery: database connection restored\n"
)
store.text_index_insert("logs", "by_line", b"log:2026-05-20", log)
store.commit("ingest log")

print(store.text_index_len("logs", "by_line"))           # 1 document
print(store.text_index_chunk_count("logs", "by_line"))   # 5 chunks
hits = store.text_index_search("logs", "by_line", "database timeout", k=3)
# Returns deduplicated documents at their best-chunk distance.
```

## Drift detection and repair

```python
# No cascade configured — primary and index can diverge.
store.text_index_open("personal", "docs", emb)
store.ns_insert("personal", b"doc:only-in-primary", b"only in primary")
store.commit("primary write without indexing")

store.text_index_insert("personal", "docs", b"doc:only-in-index", "only in index")
store.commit("index write without primary")

report = store.audit_text_index("personal", "docs")
# {"orphans_in_index":     [b"doc:only-in-index"],
#  "missing_from_index":   [b"doc:only-in-primary"],
#  "is_in_sync": False}

# Remove index entries that have no primary row.
removed = store.purge_text_index_orphans("personal", "docs")
store.commit("repair: purge orphans")
```

## Bring your own embedder via `CallableEmbedder`

```python
from prollytree import CallableEmbedder

# Stand-in for any external embedder (OpenAI, Cohere, sentence-transformers, ...).
def my_embed(text: str):
    vec = [0.0] * 8
    for i, ch in enumerate(text):
        vec[i % 8] += float(ord(ch)) / 256.0
    return vec

emb = CallableEmbedder(
    id="user:char-sum",      # persisted with the index; change when distribution changes
    version="v1",
    dim=8,
    embed_fn=my_embed,
)
store.text_index_open("personal", "docs", emb)
store.text_index_insert("personal", "docs", b"doc:a", "alpha document")
store.commit("custom embedder")
```

## Bundled semantic search with `MiniLmEmbedder`

Requires a wheel built with the `proximity_text` feature (default on PyPI).

```python
from prollytree import MiniLmEmbedder

emb = MiniLmEmbedder()                       # all-MiniLM-L6-v2 (384-d)
store.text_index_open("library", "books", emb)

store.ns_insert("library", b"book:1",
                b"a treatise on probabilistic data structures")
store.ns_insert("library", b"book:2",
                b"introduction to systems programming in rust")
store.ns_insert("library", b"book:3",
                b"the architecture of distributed databases")
store.text_index_insert("library", "books", b"book:1",
                        "a treatise on probabilistic data structures")
store.text_index_insert("library", "books", b"book:2",
                        "introduction to systems programming in rust")
store.text_index_insert("library", "books", b"book:3",
                        "the architecture of distributed databases")
store.commit("seed library")

for doc_id, score in store.text_index_search(
    "library", "books", "approximate nearest neighbour search", k=2
):
    body = store.ns_get("library", doc_id).decode()
    print(f"{doc_id!r}  distance={score:.4f}  body={body!r}")
```

First call downloads model weights (~90 MB) into `$PROLLYTREE_EMBEDDER_CACHE` (default `~/.cache/prollytree/embedders/`). Subsequent calls reuse the cache.

## Feature-availability flags

Examples designed to run on slim wheels should check what's compiled in:

```python
import prollytree as p

if p.proximity_text_available:
    emb = p.MiniLmEmbedder()
elif p.proximity_available:
    emb = p.HashEmbedder(384, 0)         # still gives you the API surface
else:
    raise RuntimeError("wheel built without proximity features — rebuild with"
                       " `./python/build_python.sh --all-features --install`")
```

## Where to go next

- [Text Indexing & Vector Search](../text_search.md) — design overview, embedder identity, branching/merging, GC.
- [Python API → NamespacedKvStore](../api/python.md#namespacedkvstore) — full method reference.
