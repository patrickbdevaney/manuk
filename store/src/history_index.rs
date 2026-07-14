//! G-b — **local-first searchable history**.
//!
//! Query the pages you have actually read, entirely on-device, with the index
//! **encrypted at rest** by reusing E2's audited AEAD (`chacha20poly1305`). Nothing
//! leaves the machine and no crypto primitive is hand-rolled.
//!
//! ## What this is, precisely
//!
//! Retrieval is **lexical**, not neural: [`HashingEmbedder`] weights word unigrams and
//! bigrams by sublinear term frequency and L2-normalizes them; queries score by cosine
//! similarity. That genuinely answers "which page said this", across word order and
//! partial phrasing — but it does **not** understand synonyms, and calling it *semantic*
//! would overclaim. The [`Embedder`] trait is the seam: a `fastembed`/ONNX
//! sentence-transformer slots in behind it (returning [`Embedding::Dense`]) without
//! touching the index, the storage format, or the query path. That is the tracked
//! follow-up, and it is `[PS]` — ONNX Runtime ships per-OS binaries.
//!
//! The lexical vectors are [`Embedding::Sparse`], keyed by the **full** 64-bit feature
//! hash rather than bucketed into a fixed width. That matters: with a fixed-width dense
//! projection, unrelated pages pick up a small nonzero cosine purely from bucket
//! collisions (measured here at 0.05-0.11, against true matches of 0.33-0.38), which
//! would force an arbitrary score threshold. Keyed sparsely, **lexically disjoint pages
//! score exactly zero**, so "no hits" means no hits. Memory is proportional to a page's
//! distinct terms, not to a chosen dimensionality.
//!
//! ## LEANN-style storage
//!
//! Vectors are **never persisted**. The sealed blob stores only `(url, title, text)`,
//! and [`SemanticIndex::open`] recomputes every embedding on load. This is the LEANN
//! trade — recompute at query time instead of storing the vectors — and it is natural
//! here because we already own the encoder and re-parse pages anyway. The text we keep
//! is needed for snippets regardless. [`SemanticIndex::storage_estimate`] reports what
//! the recompute saved.
//!
//! **Documented gaps (not faked):** brute-force cosine over all entries (no HNSW/ANN —
//! fine to tens of thousands of pages, and the honest choice until a real vector index
//! is warranted); no volatility-scored re-embed or content-addressing yet; no stemming
//! or stop-word list.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use serde::{Deserialize, Serialize};

/// A vector produced by an [`Embedder`].
///
/// Two shapes, because the two kinds of embedder genuinely differ:
///
/// * `Sparse` — a lexical embedder emits a handful of *named* features per page, keyed
///   by the full 64-bit feature hash. There is **no modulo into a fixed width**, so two
///   different words never share a bucket: lexically disjoint pages score *exactly*
///   zero, not a small collision artifact. Memory is proportional to the distinct terms
///   on the page, not to a chosen dimensionality.
/// * `Dense` — what a neural sentence-transformer (`fastembed`/ONNX) returns.
///
/// Both are L2-normalized, so [`Embedding::dot`] *is* cosine similarity.
#[derive(Clone, Debug, PartialEq)]
pub enum Embedding {
    Dense(Vec<f32>),
    /// Sorted by key, so the dot product is a linear merge-join.
    Sparse(Vec<(u64, f32)>),
}

impl Embedding {
    /// Build a normalized sparse embedding from feature weights.
    pub fn sparse(mut features: Vec<(u64, f32)>) -> Self {
        features.sort_unstable_by_key(|(k, _)| *k);
        let norm = features.iter().map(|(_, w)| w * w).sum::<f32>().sqrt();
        if norm > 0.0 {
            for (_, w) in &mut features {
                *w /= norm;
            }
        }
        Embedding::Sparse(features)
    }

    /// Build a normalized dense embedding.
    pub fn dense(mut v: Vec<f32>) -> Self {
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut v {
                *x /= norm;
            }
        }
        Embedding::Dense(v)
    }

    /// Cosine similarity. Mixing shapes is a programming error, not a silent 0 — but we
    /// return 0 rather than panic, since a mismatched index is recoverable by re-embedding.
    pub fn dot(&self, other: &Embedding) -> f32 {
        match (self, other) {
            (Embedding::Dense(a), Embedding::Dense(b)) => a.iter().zip(b).map(|(x, y)| x * y).sum(),
            (Embedding::Sparse(a), Embedding::Sparse(b)) => {
                // Merge-join on sorted keys.
                let (mut i, mut j, mut acc) = (0usize, 0usize, 0.0f32);
                while i < a.len() && j < b.len() {
                    match a[i].0.cmp(&b[j].0) {
                        std::cmp::Ordering::Less => i += 1,
                        std::cmp::Ordering::Greater => j += 1,
                        std::cmp::Ordering::Equal => {
                            acc += a[i].1 * b[j].1;
                            i += 1;
                            j += 1;
                        }
                    }
                }
                acc
            }
            _ => 0.0,
        }
    }

    /// Bytes this vector would occupy if it were persisted.
    pub fn persisted_bytes(&self) -> usize {
        match self {
            Embedding::Dense(v) => v.len() * 4,
            Embedding::Sparse(v) => v.len() * 12, // u64 key + f32 weight
        }
    }
}

/// Turns text into a vector. The seam a real sentence-transformer slots into.
pub trait Embedder {
    fn embed(&self, text: &str) -> Embedding;
}

/// Lexical embedder: unigrams + bigrams, sublinear TF, L2-normalized, **sparse**.
///
/// Deterministic and dependency-free, so an index built on one machine scores identically
/// on another. **This is lexical retrieval, not a neural embedding** — see the module docs.
#[derive(Clone, Debug, Default)]
pub struct HashingEmbedder;

/// FNV-1a — a *non-cryptographic* hash used only to name features. Never security.
fn feature_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// Lowercase alphanumeric tokens. Short tokens are kept — "42" and "ai" carry meaning.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

impl Embedder for HashingEmbedder {
    fn embed(&self, text: &str) -> Embedding {
        let tokens = tokenize(text);
        let mut counts: HashMap<u64, f32> = HashMap::new();
        for t in &tokens {
            *counts.entry(feature_hash(t)).or_default() += 1.0;
        }
        // Bigrams give word order *some* weight, which pure bag-of-words throws away.
        for w in tokens.windows(2) {
            *counts
                .entry(feature_hash(&format!("{} {}", w[0], w[1])))
                .or_default() += 1.0;
        }
        // Sublinear TF: the tenth occurrence of a word matters far less than the second,
        // so long pages do not drown short ones.
        let features: Vec<(u64, f32)> =
            counts.into_iter().map(|(h, c)| (h, 1.0 + c.ln())).collect();
        Embedding::sparse(features)
    }
}

/// A page the user read. `text` is the clean extracted text `engine/page` already
/// produces (`Page::visible_text`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub url: String,
    pub title: String,
    pub text: String,
}

/// One search hit.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit<'a> {
    pub score: f32,
    pub entry: &'a Entry,
}

/// An on-device, encrypted-at-rest index over pages the user has read.
pub struct SemanticIndex<E: Embedder = HashingEmbedder> {
    embedder: E,
    entries: Vec<Entry>,
    /// Recomputed on load; never persisted (see the module docs on LEANN).
    vectors: Vec<Embedding>,
    key: [u8; 32],
}

impl SemanticIndex<HashingEmbedder> {
    pub fn new(key: [u8; 32]) -> Self {
        SemanticIndex::with_embedder(key, HashingEmbedder)
    }
}

impl<E: Embedder> SemanticIndex<E> {
    pub fn with_embedder(key: [u8; 32], embedder: E) -> Self {
        SemanticIndex {
            embedder,
            entries: Vec::new(),
            vectors: Vec::new(),
            key,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Index a page. Re-indexing the same URL **replaces** it rather than duplicating,
    /// so revisiting a page that changed does not leave a stale copy behind.
    pub fn add(
        &mut self,
        url: impl Into<String>,
        title: impl Into<String>,
        text: impl Into<String>,
    ) {
        let entry = Entry {
            url: url.into(),
            title: title.into(),
            text: text.into(),
        };
        let vec = self.embed_entry(&entry);
        match self.entries.iter().position(|e| e.url == entry.url) {
            Some(i) => {
                self.entries[i] = entry;
                self.vectors[i] = vec;
            }
            None => {
                self.entries.push(entry);
                self.vectors.push(vec);
            }
        }
    }

    pub fn remove(&mut self, url: &str) -> bool {
        match self.entries.iter().position(|e| e.url == url) {
            Some(i) => {
                self.entries.remove(i);
                self.vectors.remove(i);
                true
            }
            None => false,
        }
    }

    /// The title is weighted by repeating it — a page's title is a strong signal about
    /// what it is *about*, and a single occurrence would be lost in a long body.
    fn embed_entry(&self, e: &Entry) -> Embedding {
        let doc = format!("{} {} {}", e.title, e.title, e.text);
        self.embedder.embed(&doc)
    }

    /// The `k` best matches for `query`, best first. Hits scoring `<= 0` are dropped:
    /// a non-positive cosine means the query shares nothing with the page.
    pub fn query(&self, query: &str, k: usize) -> Vec<Hit<'_>> {
        if query.trim().is_empty() || self.entries.is_empty() {
            return Vec::new();
        }
        let q = self.embedder.embed(query);
        let mut hits: Vec<Hit<'_>> = self
            .vectors
            .iter()
            .enumerate()
            .map(|(i, v)| Hit {
                score: q.dot(v),
                entry: &self.entries[i],
            })
            .filter(|h| h.score > 0.0)
            .collect();
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(k);
        hits
    }

    /// Seal the index for disk. **Vectors are not written** — only `(url, title, text)`.
    /// Layout is `nonce (12 B) || ciphertext`, the same shape E2's password store uses.
    pub fn seal(&self) -> Result<Vec<u8>> {
        let plaintext = serde_json::to_vec(&self.entries).context("serialize history index")?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ct = cipher
            .encrypt(&nonce, plaintext.as_ref())
            .map_err(|_| anyhow!("AEAD seal failed"))?;
        let mut out = Vec::with_capacity(12 + ct.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ct);
        Ok(out)
    }

    /// Open a sealed index and **recompute** every embedding (LEANN-style). Fails on a
    /// wrong key or a tampered blob — the AEAD's integrity guarantee.
    pub fn open_with(key: [u8; 32], embedder: E, blob: &[u8]) -> Result<Self> {
        if blob.len() < 12 {
            return Err(anyhow!("sealed index too short"));
        }
        let (nonce_bytes, ct) = blob.split_at(12);
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ct)
            .map_err(|_| anyhow!("AEAD open failed (wrong key or tampered index)"))?;
        let entries: Vec<Entry> =
            serde_json::from_slice(&plaintext).context("deserialize history index")?;

        let mut idx = SemanticIndex {
            embedder,
            entries,
            vectors: Vec::new(),
            key,
        };
        idx.vectors = idx.entries.iter().map(|e| idx.embed_entry(e)).collect();
        Ok(idx)
    }

    /// Bytes the sealed blob would occupy, and what storing vectors instead would cost.
    /// Used to show the §3 storage budget honestly.
    pub fn storage_estimate(&self) -> StorageEstimate {
        let text_bytes: usize = self
            .entries
            .iter()
            .map(|e| e.url.len() + e.title.len() + e.text.len())
            .sum();
        StorageEstimate {
            entries: self.entries.len(),
            text_bytes,
            vector_bytes_avoided: self.vectors.iter().map(|v| v.persisted_bytes()).sum(),
        }
    }
}

impl SemanticIndex<HashingEmbedder> {
    pub fn open(key: [u8; 32], blob: &[u8]) -> Result<Self> {
        SemanticIndex::open_with(key, HashingEmbedder, blob)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageEstimate {
    pub entries: usize,
    pub text_bytes: usize,
    /// What persisting `f32` vectors would have added — the LEANN saving.
    pub vector_bytes_avoided: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7u8; 32];

    fn index() -> SemanticIndex<HashingEmbedder> {
        let mut i = SemanticIndex::new(KEY);
        i.add(
            "https://rust-lang.org/",
            "Rust Programming Language",
            "Rust is a systems programming language focused on memory safety without a garbage collector.",
        );
        i.add(
            "https://cooking.example/risotto",
            "Mushroom Risotto Recipe",
            "Stir arborio rice slowly with warm stock and add porcini mushrooms and parmesan cheese.",
        );
        i.add(
            "https://astro.example/jupiter",
            "Jupiter's Moons",
            "Io, Europa, Ganymede and Callisto are the four Galilean moons of the planet Jupiter.",
        );
        i
    }

    /// G-b acceptance: a query over previously-read pages returns the right page.
    #[test]
    fn a_query_returns_the_right_page() {
        let i = index();

        let hits = i.query("memory safety without garbage collection", 3);
        assert_eq!(hits[0].entry.url, "https://rust-lang.org/");

        let hits = i.query("arborio rice and parmesan", 3);
        assert_eq!(hits[0].entry.url, "https://cooking.example/risotto");

        let hits = i.query("galilean moons", 3);
        assert_eq!(hits[0].entry.url, "https://astro.example/jupiter");
    }

    /// Matching the *title* is enough — a page's title says what it is about, and the
    /// index weights it accordingly.
    #[test]
    fn a_title_match_finds_the_page() {
        let i = index();
        let hits = i.query("risotto recipe", 3);
        assert_eq!(hits[0].entry.url, "https://cooking.example/risotto");
    }

    /// Word order is not required, and a query sharing nothing returns nothing rather
    /// than the least-bad page.
    #[test]
    fn unrelated_queries_return_no_hits_instead_of_noise() {
        let i = index();
        assert!(i.query("quantum chromodynamics lagrangian", 3).is_empty());
        assert!(i.query("", 3).is_empty());
        assert!(SemanticIndex::new(KEY).query("anything", 3).is_empty());
    }

    #[test]
    fn re_indexing_a_url_replaces_it_rather_than_duplicating() {
        let mut i = SemanticIndex::new(KEY);
        i.add("https://a.test/", "Old", "old body about kittens");
        i.add("https://a.test/", "New", "new body about spacecraft");
        assert_eq!(i.len(), 1);
        assert_eq!(i.entries()[0].title, "New");

        // The stale content is really gone, not merely shadowed.
        assert!(i.query("kittens", 3).is_empty());
        assert_eq!(i.query("spacecraft", 3)[0].entry.url, "https://a.test/");

        assert!(i.remove("https://a.test/"));
        assert!(!i.remove("https://a.test/"));
        assert!(i.is_empty());
    }

    /// The index is encrypted at rest with E2's AEAD, and a wrong key fails closed.
    #[test]
    fn the_index_seals_and_opens_and_a_wrong_key_fails() {
        let i = index();
        let blob = i.seal().unwrap();

        // The plaintext must not be readable in the blob.
        assert!(
            !String::from_utf8_lossy(&blob).contains("risotto"),
            "sealed index leaked plaintext"
        );

        let back = SemanticIndex::open(KEY, &blob).unwrap();
        assert_eq!(back.len(), 3);
        // Vectors were recomputed on open, so querying still works.
        assert_eq!(
            back.query("porcini mushrooms", 3)[0].entry.url,
            "https://cooking.example/risotto"
        );

        // Wrong key, and a tampered blob, both fail closed.
        assert!(SemanticIndex::open([9u8; 32], &blob).is_err());
        let mut bad = blob.clone();
        *bad.last_mut().unwrap() ^= 0xff;
        assert!(SemanticIndex::open(KEY, &bad).is_err());
    }

    /// LEANN: vectors are never persisted, and the estimate reports what that saved.
    #[test]
    fn vectors_are_not_persisted_and_the_saving_is_reported() {
        let i = index();
        let est = i.storage_estimate();
        assert_eq!(est.entries, 3);
        // Sparse vectors cost 12 bytes per distinct feature; the saving is real and
        // proportional to the vocabulary, not to a chosen dimensionality.
        assert!(est.vector_bytes_avoided > 0);

        // The sealed blob is on the order of the text, not the text plus 6 KB of floats.
        let blob = i.seal().unwrap();
        assert!(
            blob.len() < est.text_bytes + est.vector_bytes_avoided,
            "the blob must not be carrying vectors"
        );
    }

    #[test]
    fn embeddings_are_deterministic_and_normalized() {
        let e = HashingEmbedder;
        let a = e.embed("the quick brown fox");
        let b = e.embed("the quick brown fox");
        assert_eq!(a, b, "an index built elsewhere must score identically");
        // Normalized => a vector's cosine with itself is 1.
        assert!(
            (a.dot(&a) - 1.0).abs() < 1e-5,
            "embeddings must be L2-normalized"
        );

        // The empty string yields an empty vector, not a NaN.
        assert_eq!(e.embed(""), Embedding::Sparse(vec![]));
        assert_eq!(e.embed("").dot(&a), 0.0);
    }

    /// The sparse representation keys features by their **full** hash, so two pages that
    /// share no vocabulary score *exactly* zero — no fixed-width bucket collisions.
    #[test]
    fn lexically_disjoint_texts_score_exactly_zero() {
        let e = HashingEmbedder;
        let a = e.embed("mushroom risotto arborio parmesan");
        let b = e.embed("ganymede callisto europa io");
        assert_eq!(a.dot(&b), 0.0);
    }

    #[test]
    fn mixing_dense_and_sparse_scores_zero_rather_than_panicking() {
        let s = Embedding::sparse(vec![(1, 1.0)]);
        let d = Embedding::dense(vec![1.0, 0.0]);
        assert_eq!(s.dot(&d), 0.0);
    }

    /// A custom embedder slots in behind the trait without touching the index — the
    /// seam a real `fastembed` model uses.
    #[test]
    fn a_custom_embedder_can_replace_the_default() {
        struct Tiny;
        impl Embedder for Tiny {
            fn embed(&self, text: &str) -> Embedding {
                // A dense two-axis "model": axis 0 for cats, axis 1 otherwise. This is
                // the shape a real `fastembed` sentence-transformer returns.
                if text.to_lowercase().contains("cat") {
                    Embedding::dense(vec![1.0, 0.0])
                } else {
                    Embedding::dense(vec![0.0, 1.0])
                }
            }
        }

        let mut i = SemanticIndex::with_embedder(KEY, Tiny);
        i.add("https://a/", "cat page", "a cat");
        i.add("https://b/", "dog page", "a dog");
        assert_eq!(i.query("cat", 1)[0].entry.url, "https://a/");

        // And it survives a seal/open round trip with the same embedder.
        let blob = i.seal().unwrap();
        let back = SemanticIndex::open_with(KEY, Tiny, &blob).unwrap();
        assert_eq!(back.query("cat", 1)[0].entry.url, "https://a/");
    }
}
