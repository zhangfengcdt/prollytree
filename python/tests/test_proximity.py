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

"""Integration tests for the proximity / text-search Python bindings.

These tests exercise the additive `text_index_*` / cascade / GC methods on
`NamespacedKvStore`. They skip cleanly when the wheel under test was built
without the `proximity` feature so that this file is safe to keep in the
default test suite.
"""

import os
import subprocess
import tempfile

import pytest

prollytree = pytest.importorskip("prollytree")

HashEmbedder = getattr(prollytree, "HashEmbedder", None)
CallableEmbedder = getattr(prollytree, "CallableEmbedder", None)
if HashEmbedder is None:
    pytest.skip(
        "wheel built without the `proximity` feature — skipping text-index tests",
        allow_module_level=True,
    )

NamespacedKvStore = prollytree.NamespacedKvStore


def _make_dataset(tmpdir):
    """Initialise a git repo + dataset subdirectory the way CLAUDE.md requires."""
    subprocess.run(["git", "init"], cwd=tmpdir, check=True, capture_output=True)
    subprocess.run(["git", "config", "user.name", "Test User"], cwd=tmpdir, check=True)
    subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=tmpdir, check=True)
    dataset = os.path.join(tmpdir, "dataset")
    os.makedirs(dataset)
    return dataset


def test_hash_embedder_basics():
    """The deterministic HashEmbedder is callable from Python."""
    e = HashEmbedder(16, 0)
    assert e.dim == 16
    v1 = e.embed("hello world")
    v2 = e.embed("hello world")
    assert len(v1) == 16
    assert v1 == v2, "HashEmbedder must be deterministic"
    v3 = e.embed("a different sentence")
    assert v1 != v3


def test_callable_embedder_basics():
    """Wrap a plain Python function as an Embedder."""

    def my_embed(text: str):
        # Deterministic toy embedding: char-code sums + length, padded to dim.
        vec = [0.0] * 8
        for i, ch in enumerate(text):
            vec[i % 8] += float(ord(ch)) / 256.0
        return vec

    e = CallableEmbedder(id="user:char-sum", version="v1", dim=8, embed_fn=my_embed)
    assert e.id == "user:char-sum"
    assert e.version == "v1"
    assert e.dim == 8
    out = e.embed("hello")
    assert len(out) == 8
    assert out == my_embed("hello")


def test_callable_embedder_rejects_wrong_dim():
    """The embedder surfaces dimension drift as a clear error."""
    e = CallableEmbedder(
        id="user:bad-dim", version="v1", dim=4, embed_fn=lambda _t: [0.0, 0.0, 0.0]
    )
    with pytest.raises(ValueError) as excinfo:
        e.embed("anything")
    assert "wrong dim" in str(excinfo.value)


def test_callable_embedder_end_to_end_in_text_index():
    """A user-supplied callable embedder works inside a text index."""
    table = {
        "alpha document one": [1.0, 0.0, 0.0, 0.0],
        "beta document two": [0.0, 1.0, 0.0, 0.0],
        "gamma document three": [0.0, 0.0, 1.0, 0.0],
        "delta document four": [0.0, 0.0, 0.0, 1.0],
    }

    def embed(text):
        return table.get(text, [0.25, 0.25, 0.25, 0.25])

    with tempfile.TemporaryDirectory() as tmpdir:
        dataset = _make_dataset(tmpdir)
        store = NamespacedKvStore(dataset)
        emb = CallableEmbedder(id="user:lookup", version="v1", dim=4, embed_fn=embed)
        store.text_index_open("personal", "docs", emb)
        for doc_id, text in zip(
            [b"alpha", b"beta", b"gamma", b"delta"], list(table.keys())
        ):
            store.text_index_insert("personal", "docs", doc_id, text)

        # Exact match on `gamma document three` ranks gamma first.
        hits = store.text_index_search("personal", "docs", "gamma document three", 1)
        assert len(hits) == 1
        assert hits[0][0] == b"gamma"


def test_text_index_open_insert_search():
    """End-to-end: open a text index, insert, search."""
    with tempfile.TemporaryDirectory() as tmpdir:
        dataset = _make_dataset(tmpdir)
        store = NamespacedKvStore(dataset)
        embedder = HashEmbedder(32, 0)

        store.text_index_open("personal", "docs", embedder)
        store.text_index_insert("personal", "docs", b"doc:1", "the quick brown fox")
        store.text_index_insert("personal", "docs", b"doc:2", "lazy dog asleep on the mat")

        hits = store.text_index_search("personal", "docs", "the quick brown fox", 2)
        assert len(hits) >= 1
        # `doc:1` is an exact match and must rank first.
        assert hits[0][0] == b"doc:1"
        # Each hit is (id_bytes, score).
        assert isinstance(hits[0][1], float)

        assert store.text_index_len("personal", "docs") == 2
        assert store.text_index_chunk_count("personal", "docs") == 2


def test_text_index_delete_and_drop():
    with tempfile.TemporaryDirectory() as tmpdir:
        dataset = _make_dataset(tmpdir)
        store = NamespacedKvStore(dataset)
        embedder = HashEmbedder(16, 0)
        store.text_index_open("personal", "docs", embedder)
        store.text_index_insert("personal", "docs", b"id-a", "one")
        store.text_index_insert("personal", "docs", b"id-b", "two")

        assert store.text_index_delete("personal", "docs", b"id-a") is True
        assert store.text_index_len("personal", "docs") == 1

        # Drop the in-memory cache; subsequent operations should fail with the
        # "not opened" error.
        assert store.text_index_drop("personal", "docs") is True
        with pytest.raises(ValueError):
            store.text_index_insert("personal", "docs", b"id-c", "three")


def test_text_index_line_chunker_multichunk():
    """LineChunker splits the document into one chunk per non-empty line."""
    with tempfile.TemporaryDirectory() as tmpdir:
        dataset = _make_dataset(tmpdir)
        store = NamespacedKvStore(dataset)
        embedder = HashEmbedder(16, 0)
        store.text_index_open("personal", "lines", embedder, "line")
        store.text_index_insert(
            "personal", "lines", b"doc:1", "alpha\nbeta\ngamma"
        )
        assert store.text_index_len("personal", "lines") == 1
        assert store.text_index_chunk_count("personal", "lines") == 3


def test_cascade_mirrors_primary_inserts():
    with tempfile.TemporaryDirectory() as tmpdir:
        dataset = _make_dataset(tmpdir)
        store = NamespacedKvStore(dataset)
        embedder = HashEmbedder(16, 0)
        store.text_index_open("personal", "docs", embedder)
        store.set_cascade("personal", ["docs"])
        assert store.cascade_for_namespace("personal") == ["docs"]

        # Primary insert mirrors into the text index.
        store.ns_insert("personal", b"doc:1", b"the cascading text")
        store.commit("cascade insert")

        # The cascaded chunk is searchable.
        hits = store.text_index_search("personal", "docs", "the cascading text", 1)
        assert len(hits) == 1
        assert hits[0][0] == b"doc:1"

        # Clearing cascade is observable.
        store.clear_cascade("personal")
        assert store.cascade_for_namespace("personal") is None


def test_audit_text_index_in_sync():
    with tempfile.TemporaryDirectory() as tmpdir:
        dataset = _make_dataset(tmpdir)
        store = NamespacedKvStore(dataset)
        embedder = HashEmbedder(16, 0)
        store.text_index_open("personal", "docs", embedder)
        store.set_cascade("personal", ["docs"])

        store.ns_insert("personal", b"doc:1", b"first")
        store.ns_insert("personal", b"doc:2", b"second")
        store.commit("two docs")

        report = store.audit_text_index("personal", "docs")
        assert report["is_in_sync"] is True
        assert report["orphans_in_index"] == []
        assert report["missing_from_index"] == []


def test_externalize_threshold_accessor_round_trip():
    """The threshold accessor stores and returns the value.

    Note: the Python `NamespacedKvStore` wraps the Git-backed namespaced store,
    whose `NodeStorage` impl does not (yet) support blob externalisation —
    actually committing a > threshold value fails with a clear backend error.
    File and RocksDB backends will get their own Python wrappers in a follow-up.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        dataset = _make_dataset(tmpdir)
        store = NamespacedKvStore(dataset)
        assert store.externalize_threshold() is None
        store.set_externalize_threshold(64)
        assert store.externalize_threshold() == 64
        store.set_externalize_threshold(None)
        assert store.externalize_threshold() is None


def test_gc_blobs_reports_empty_on_git_backend():
    """`gc_blobs()` is callable but a no-op on the Git-backed wrapper.

    The Git `NodeStorage` impl returns an empty `list_blobs()`, so GC walks
    the namespace tree, finds nothing referenced, and reports a clean zero.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        dataset = _make_dataset(tmpdir)
        store = NamespacedKvStore(dataset)
        report = store.gc_blobs()
        assert report["total"] == 0
        assert report["referenced"] == 0
        assert report["removed"] == 0
        assert report["errors"] == []


def test_repr_unchanged_back_compat():
    """The existing __repr__ surface area is untouched by the proximity additions."""
    with tempfile.TemporaryDirectory() as tmpdir:
        dataset = _make_dataset(tmpdir)
        store = NamespacedKvStore(dataset)
        text = repr(store)
        assert text.startswith("NamespacedKvStore(")
