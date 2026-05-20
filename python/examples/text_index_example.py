#!/usr/bin/env python3

# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""
Example: Text indexing + vector search on a NamespacedKvStore.

Each namespace can own zero or more text sub-indexes. A text index turns
documents into vectors via a configurable embedder and gives you top-k
similarity search that's versioned alongside the primary KV tree.

This example walks through:
  1. Opening a text index and running an explicit insert + search.
  2. Cascade mode — primary writes auto-mirror into the text index.
  3. Multi-chunk indexing via the LineChunker.
  4. Drift detection via audit_text_index.
  5. CallableEmbedder — bring your own (Python) embedding function.
  6. Optionally: MiniLmEmbedder if the wheel was built with proximity_text.
"""

import os
import shutil
import subprocess
import sys
import tempfile

import prollytree


def setup_example_repo():
    """Create a temp git repo + dataset directory the store expects."""
    tmpdir = tempfile.mkdtemp(prefix="prollytree_text_example_")
    print(f"Created temporary directory: {tmpdir}")
    subprocess.run(["git", "init"], cwd=tmpdir, check=True, capture_output=True)
    subprocess.run(
        ["git", "config", "user.name", "Example User"],
        cwd=tmpdir,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "config", "user.email", "user@example.com"],
        cwd=tmpdir,
        check=True,
        capture_output=True,
    )
    dataset_dir = os.path.join(tmpdir, "dataset")
    os.makedirs(dataset_dir, exist_ok=True)
    return tmpdir, dataset_dir


def demo_basic_text_index():
    """Dual-write pattern: primary tree (source of truth) + text index (pointer)."""
    print("\nDemo 1: Open + dual-write + search (with primary-tree lookup)")
    print("=" * 60)

    tmpdir, dataset_dir = setup_example_repo()
    try:
        store = prollytree.NamespacedKvStore(dataset_dir)
        embedder = prollytree.HashEmbedder(dim=64, seed=0)

        # text_index_open creates the index on first call and validates the
        # embedder identity on subsequent calls. The embedder + chunker are
        # cached on the Python wrapper for the rest of the process.
        store.text_index_open("personal", "docs", embedder)

        # IMPORTANT: the proximity index stores (id, vector) pairs only — the
        # original text is NOT recoverable from it. Always write the body into
        # the primary tree too, so search hits can be resolved back to text
        # and the corpus can be reindexed if the embedder ever changes.
        docs = {
            b"doc:1": "the quick brown fox jumps over the lazy dog",
            b"doc:2": "rust is a systems programming language",
            b"doc:3": "merkle trees enable verifiable data structures",
            b"doc:4": "the fox and the hound are forest friends",
        }
        for doc_id, text in docs.items():
            store.ns_insert("personal", doc_id, text.encode())       # primary tree
            store.text_index_insert("personal", "docs", doc_id, text)  # text index
        store.commit("seed corpus + index")

        print(f"Primary holds {len(store.ns_list_keys('personal'))} documents")
        print(f"Index holds {store.text_index_len('personal', 'docs')} documents")

        # Search returns (doc_id_bytes, distance). Resolve each id back to its
        # body via the primary tree — the canonical two-step read pattern.
        hits = store.text_index_search("personal", "docs", "the quick brown fox", k=2)
        print(f"\nQuery: 'the quick brown fox' -> top {len(hits)}")
        for doc_id, score in hits:
            body = store.ns_get("personal", doc_id).decode()
            print(f"  {doc_id!r}  distance={score:.4f}  body={body!r}")
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


def demo_cascade_mode():
    """Primary writes auto-mirror into the registered text index."""
    print("\nDemo 2: Cascade — text index follows primary writes")
    print("=" * 60)

    tmpdir, dataset_dir = setup_example_repo()
    try:
        store = prollytree.NamespacedKvStore(dataset_dir)
        embedder = prollytree.HashEmbedder(dim=64, seed=0)
        store.text_index_open("notes", "by_body", embedder)
        store.set_cascade("notes", ["by_body"])

        # ns_insert now also embeds + indexes (because the namespace's cascade
        # list contains "by_body"). One commit covers both writes.
        store.ns_insert("notes", b"note:1", b"meeting with the platform team")
        store.ns_insert("notes", b"note:2", b"draft proposal for Q3 roadmap")
        store.commit("cascade-driven indexing")

        hits = store.text_index_search("notes", "by_body", "platform meeting", k=2)
        print("Cascade-indexed search results:")
        for doc_id, score in hits:
            print(f"  {doc_id!r}  distance={score:.4f}")

        # Deletes cascade too.
        store.ns_delete("notes", b"note:1")
        store.commit("cascade-driven delete")
        print(
            f"\nAfter ns_delete('note:1'): index now holds "
            f"{store.text_index_len('notes', 'by_body')} document(s)"
        )

        store.clear_cascade("notes")
        print(f"Cascade cleared: {store.cascade_for_namespace('notes')}")
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


def demo_multi_chunk_with_line_chunker():
    """One document, many chunks — search dedups back to the document id.

    This demo focuses on chunker mechanics, so it skips the primary-tree
    write for brevity; see demo 1 (or enable cascade as in demo 2) for the
    full dual-write pattern.
    """
    print("\nDemo 3: Multi-chunk indexing via LineChunker")
    print("=" * 60)

    tmpdir, dataset_dir = setup_example_repo()
    try:
        store = prollytree.NamespacedKvStore(dataset_dir)
        embedder = prollytree.HashEmbedder(dim=64, seed=0)
        # Chunker is passed by name: 'identity' (default, 1 chunk per doc) or
        # 'line' (one chunk per non-empty line).
        store.text_index_open("logs", "by_line", embedder, chunker="line")

        log = (
            "2026-05-20T09:00 startup: loading config\n"
            "2026-05-20T09:01 startup: bound port 8080\n"
            "2026-05-20T09:42 error: database timeout after 30s\n"
            "2026-05-20T09:43 retry: reconnecting to database\n"
            "2026-05-20T09:43 recovery: database connection restored\n"
        )
        store.text_index_insert("logs", "by_line", b"log:2026-05-20", log)
        store.commit("ingest log file")

        print(
            f"len = {store.text_index_len('logs', 'by_line')} document, "
            f"chunk_count = {store.text_index_chunk_count('logs', 'by_line')} chunks"
        )

        hits = store.text_index_search("logs", "by_line", "database timeout", k=3)
        print("Query: 'database timeout' (dedupped by document):")
        for doc_id, score in hits:
            print(f"  {doc_id!r}  distance={score:.4f}")
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


def demo_audit_drift():
    """Detect (and repair) drift between primary and index."""
    print("\nDemo 4: Drift detection via audit_text_index")
    print("=" * 60)

    tmpdir, dataset_dir = setup_example_repo()
    try:
        store = prollytree.NamespacedKvStore(dataset_dir)
        embedder = prollytree.HashEmbedder(dim=64, seed=0)
        store.text_index_open("personal", "docs", embedder)

        # No cascade configured — primary and index diverge on purpose.
        store.ns_insert("personal", b"doc:in-primary-only", b"only in the primary tree")
        store.commit("primary write without indexing")

        store.text_index_insert(
            "personal", "docs", b"doc:in-index-only", "only in the index"
        )
        store.commit("index write without primary")

        report = store.audit_text_index("personal", "docs")
        print(f"is_in_sync       : {report['is_in_sync']}")
        print(f"orphans_in_index : {report['orphans_in_index']}")
        print(f"missing_from_index: {report['missing_from_index']}")

        # purge_text_index_orphans removes index entries that have no matching
        # primary row. The 'missing_from_index' set is the user's job — they
        # decide whether to insert into the index or delete from the primary.
        removed = store.purge_text_index_orphans("personal", "docs")
        print(f"\npurge_text_index_orphans removed {removed} orphan(s)")
        store.commit("repair: purge orphans")
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


def demo_callable_embedder():
    """Use a plain Python function as the embedder.

    Focuses on the embedder API; for the full primary+index dual write see
    demo 1 (or enable cascade as in demo 2).
    """
    print("\nDemo 5: CallableEmbedder — bring your own embedding function")
    print("=" * 60)

    # Stand-in for an external embedding API. Deterministic for the demo.
    def toy_embed(text: str):
        vec = [0.0] * 8
        for i, ch in enumerate(text):
            vec[i % 8] += float(ord(ch)) / 256.0
        return vec

    tmpdir, dataset_dir = setup_example_repo()
    try:
        store = prollytree.NamespacedKvStore(dataset_dir)
        embedder = prollytree.CallableEmbedder(
            id="user:toy-char-sum",
            version="v1",
            dim=8,
            embed_fn=toy_embed,
        )
        store.text_index_open("personal", "docs", embedder)
        store.text_index_insert("personal", "docs", b"doc:a", "alpha document")
        store.text_index_insert("personal", "docs", b"doc:b", "beta document")
        store.commit("seed with custom embedder")

        print("Embedder identity is persisted with the index:")
        print(f"  id = {embedder.id}")
        print(f"  version = {embedder.version}")
        print(f"  dim = {embedder.dim}")

        hits = store.text_index_search("personal", "docs", "alpha document", k=1)
        print(f"\nSearch 'alpha document' -> {hits!r}")
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


def demo_minilm_embedder():
    """Run a search with the bundled Candle + MiniLM embedder, if available."""
    if not getattr(prollytree, "proximity_text_available", False):
        print("\nDemo 6: MiniLmEmbedder (skipped — wheel built without proximity_text)")
        return

    print("\nDemo 6: MiniLmEmbedder — bundled Candle + all-MiniLM-L6-v2")
    print("=" * 60)
    print(
        "First call downloads ~90 MB of weights into "
        "$PROLLYTREE_EMBEDDER_CACHE (default ~/.cache/prollytree). "
        "Subsequent runs reuse the cache."
    )

    tmpdir, dataset_dir = setup_example_repo()
    try:
        store = prollytree.NamespacedKvStore(dataset_dir)
        embedder = prollytree.MiniLmEmbedder()
        print(f"Embedder: {embedder!r}")
        store.text_index_open("library", "books", embedder)

        store.text_index_insert(
            "library", "books", b"book:1", "a treatise on probabilistic data structures"
        )
        store.text_index_insert(
            "library", "books", b"book:2", "introduction to systems programming in rust"
        )
        store.text_index_insert(
            "library", "books", b"book:3", "the architecture of distributed databases"
        )
        store.commit("seed library")

        hits = store.text_index_search(
            "library", "books", "approximate nearest neighbour search", k=2
        )
        print("Semantic search results:")
        for doc_id, score in hits:
            print(f"  {doc_id!r}  distance={score:.4f}")
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


def main():
    print("Text indexing on NamespacedKvStore")
    print("=" * 60)
    # `getattr` keeps this example friendly to older wheels that pre-date the
    # `proximity_available` flag — they surface a clear hint instead of an
    # opaque AttributeError.
    proximity_available = getattr(prollytree, "proximity_available", False)
    proximity_text_available = getattr(prollytree, "proximity_text_available", False)
    print(f"proximity_available      = {proximity_available}")
    print(f"proximity_text_available = {proximity_text_available}")

    if not proximity_available:
        print("\nThe installed `prollytree` wheel was built without the 'proximity'")
        print("feature. Rebuild and reinstall with one of:")
        print("  ./python/build_python.sh --all-features --install")
        print("  maturin develop --features 'python git proximity proximity_text'")
        sys.exit(1)

    try:
        demo_basic_text_index()
        demo_cascade_mode()
        demo_multi_chunk_with_line_chunker()
        demo_audit_drift()
        demo_callable_embedder()
        demo_minilm_embedder()
        print("\nAll demos completed successfully.")
        print("\nKey takeaways:")
        print("- Primary KV tree is the source of truth; text index stores only")
        print("  (id, vector) pairs. Write document bodies to BOTH (demo 1) or")
        print("  use set_cascade to auto-mirror primary writes into the index (demo 2).")
        print("- text_index_open(ns, idx, embedder, chunker) creates or re-opens.")
        print("- chunker='line' splits one document into many chunks; search dedups.")
        print("- audit_text_index + purge_text_index_orphans repair drift.")
        print("- HashEmbedder for tests; CallableEmbedder for any Python function;")
        print("  MiniLmEmbedder for bundled semantic search.")
    except KeyboardInterrupt:
        print("\nExample interrupted by user.")
        sys.exit(1)
    except Exception as exc:  # pragma: no cover - illustrative only
        print(f"\nExample failed: {exc}")
        import traceback

        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
