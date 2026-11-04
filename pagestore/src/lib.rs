//! R2.3 page store v1. An a11y tree is stored as content-addressed Merkle nodes: a node's hash covers
//! (role, name, state, value, quantised rect) and its children's hashes, so an unchanged subtree
//! (a footer) is one row however many visits contain it. A snapshot is its root hash; a diff descends
//! only where hashes differ. Records are zstd-compressed with a per-site dictionary.
use anyhow::{anyhow, Result};
use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// node hash -> zstd(record json)
const NODES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("nodes");
/// site -> zstd dictionary bytes
const DICTS: TableDefinition<&str, &[u8]> = TableDefinition::new("dicts");
/// (site, seq) -> json Snap
const SNAPS: TableDefinition<(&str, u64), &[u8]> = TableDefinition::new("snaps");
/// (site, handle key) -> handle number
const HANDLES: TableDefinition<(&str, &str), u32> = TableDefinition::new("handles");
/// site -> next handle number
const NEXT: TableDefinition<&str, u32> = TableDefinition::new("next_handle");
/// (from root hex, action) -> to root hex   (transition edges)
const EDGES: TableDefinition<(&str, &str), &str> = TableDefinition::new("edges");

/// Rects are snapped to this grid so sub-pixel jitter does not change a hash.
pub const QUANT: f32 = 8.0;
/// Train a per-site dictionary once a snapshot yields at least this many node records.
const DICT_MIN_SAMPLES: usize = 64;

/// A plain tree the store hashes; built from the engine's a11y tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PNode {
    pub role: String,
    pub name: String,
    pub state: String,
    pub value: String,
    pub rect: Option<[i32; 4]>,
    pub children: Vec<PNode>,
}

impl PNode {
    pub fn from_a11y(n: &manuk_a11y::A11yNode) -> PNode {
        let q = |v: f32| (v / QUANT).round() as i32;
        PNode {
            role: match &n.role {
                manuk_a11y::Role::Heading { level } => format!("heading{level}"),
                r => r.as_str().to_string(),
            },
            name: n.name.clone(),
            state: {
                let mut s = n.state.clone();
                s.value = None;
                format!("{s:?}")
            },
            value: n.state.value.clone().unwrap_or_default(),
            rect: n.bbox.map(|b| [q(b.x), q(b.y), q(b.width), q(b.height)]),
            children: n.children.iter().map(PNode::from_a11y).collect(),
        }
    }

    /// Text outline, one node per line, indented.
    pub fn outline(&self) -> String {
        let mut o = String::new();
        self.out(0, &mut o);
        o
    }
    fn out(&self, d: usize, o: &mut String) {
        o.push_str(&"  ".repeat(d));
        o.push_str(&self.role);
        if !self.name.is_empty() {
            o.push_str(&format!(" {:?}", self.name));
        }
        if !self.value.is_empty() {
            o.push_str(&format!(" ={}", self.value));
        }
        o.push('\n');
        for c in &self.children {
            c.out(d + 1, o);
        }
    }
}

/// What is stored per node: its own fields and its children's hashes.
#[derive(Serialize, Deserialize)]
struct Rec {
    role: String,
    name: String,
    state: String,
    value: String,
    rect: Option<[i32; 4]>,
    kids: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Snap {
    root: String,
    ts: u64,
}

pub type Hash = String;

/// Result of storing a snapshot.
#[derive(Debug, Clone)]
pub struct Stored {
    pub seq: u64,
    pub root: Hash,
    /// node records written (absent before) / already present
    pub new_nodes: usize,
    pub reused_nodes: usize,
}

/// A recalled snapshot.
#[derive(Debug, Clone)]
pub struct Recalled {
    pub seq: u64,
    pub root: Hash,
    pub age_secs: u64,
    pub tree: PNode,
}

/// One difference found by [`PageStore::diff`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Added(String),
    Removed(String),
    /// same position, same role, a field differs
    Changed { before: String, after: String },
}

pub struct PageStore {
    db: Database,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn nm<E: std::fmt::Display>(e: E) -> anyhow::Error {
    anyhow!("{e}")
}

fn rec_hash(r: &Rec) -> Hash {
    let mut h = blake3::Hasher::new();
    for f in [&r.role, &r.name, &r.state, &r.value] {
        h.update(&(f.len() as u64).to_le_bytes());
        h.update(f.as_bytes());
    }
    match r.rect {
        Some(a) => {
            h.update(&[1]);
            for v in a {
                h.update(&v.to_le_bytes());
            }
        }
        None => {
            h.update(&[0]);
        }
    }
    for k in &r.kids {
        h.update(k.as_bytes());
    }
    h.finalize().to_hex().to_string()
}

fn line(r: &Rec) -> String {
    if r.name.is_empty() {
        r.role.clone()
    } else {
        format!("{} {:?}", r.role, r.name)
    }
}

impl PageStore {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Database::create(path).map_err(nm)?;
        let w = db.begin_write().map_err(nm)?;
        {
            w.open_table(NODES).map_err(nm)?;
            w.open_table(DICTS).map_err(nm)?;
            w.open_table(SNAPS).map_err(nm)?;
            w.open_table(HANDLES).map_err(nm)?;
            w.open_table(NEXT).map_err(nm)?;
            w.open_table(EDGES).map_err(nm)?;
        }
        w.commit().map_err(nm)?;
        Ok(PageStore { db })
    }

    fn dict(&self, site: &str) -> Result<Option<Vec<u8>>> {
        let r = self.db.begin_read().map_err(nm)?;
        let t = r.open_table(DICTS).map_err(nm)?;
        Ok(t.get(site).map_err(nm)?.map(|v| v.value().to_vec()))
    }

    /// Collect records bottom-up, returning (hash, record) pairs and the root hash.
    fn flatten(n: &PNode, out: &mut Vec<(Hash, Rec)>) -> Hash {
        let kids: Vec<Hash> = n.children.iter().map(|c| Self::flatten(c, out)).collect();
        let r = Rec {
            role: n.role.clone(),
            name: n.name.clone(),
            state: n.state.clone(),
            value: n.value.clone(),
            rect: n.rect,
            kids,
        };
        let h = rec_hash(&r);
        out.push((h.clone(), r));
        h
    }

    /// Store `tree` as the next snapshot of `site`.
    pub fn put(&self, site: &str, tree: &PNode) -> Result<Stored> {
        let mut recs = Vec::new();
        let root = Self::flatten(tree, &mut recs);
        // First snapshot of a site with enough records trains its dictionary.
        let mut dict = self.dict(site)?;
        if dict.is_none() && recs.len() >= DICT_MIN_SAMPLES {
            let samples: Vec<Vec<u8>> = recs.iter().map(|(_, r)| serde_json::to_vec(r).unwrap()).collect();
            if let Ok(d) = zstd::dict::from_samples(&samples, 4096) {
                dict = Some(d);
            }
        }
        let w = self.db.begin_write().map_err(nm)?;
        let (mut new_nodes, mut reused_nodes) = (0, 0);
        let seq;
        {
            if let Some(d) = &dict {
                let mut dt = w.open_table(DICTS).map_err(nm)?;
                if dt.get(site).map_err(nm)?.is_none() {
                    dt.insert(site, d.as_slice()).map_err(nm)?;
                }
            }
            let mut c = match &dict {
                Some(d) => zstd::bulk::Compressor::with_dictionary(3, d).map_err(nm)?,
                None => zstd::bulk::Compressor::new(3).map_err(nm)?,
            };
            let mut nt = w.open_table(NODES).map_err(nm)?;
            for (h, r) in &recs {
                if nt.get(h.as_bytes()).map_err(nm)?.is_some() {
                    reused_nodes += 1;
                    continue;
                }
                let z = c.compress(&serde_json::to_vec(r).map_err(nm)?).map_err(nm)?;
                nt.insert(h.as_bytes(), z.as_slice()).map_err(nm)?;
                new_nodes += 1;
            }
            let mut st = w.open_table(SNAPS).map_err(nm)?;
            seq = st
                .range((site, 0u64)..=(site, u64::MAX))
                .map_err(nm)?
                .next_back()
                .map(|x| x.map(|(k, _)| k.value().1 + 1))
                .transpose()
                .map_err(nm)?
                .unwrap_or(1);
            let s = serde_json::to_vec(&Snap { root: root.clone(), ts: now() }).map_err(nm)?;
            st.insert((site, seq), s.as_slice()).map_err(nm)?;
        }
        w.commit().map_err(nm)?;
        Ok(Stored { seq, root, new_nodes, reused_nodes })
    }

    fn load(&self, site: &str, h: &str) -> Result<Rec> {
        let r = self.db.begin_read().map_err(nm)?;
        let t = r.open_table(NODES).map_err(nm)?;
        let z = t.get(h.as_bytes()).map_err(nm)?.ok_or_else(|| anyhow!("missing node {h}"))?;
        let raw = match self.dict(site)? {
            Some(d) => zstd::bulk::Decompressor::with_dictionary(&d)
                .map_err(nm)?
                .decompress(z.value(), 1 << 22),
            None => zstd::bulk::Decompressor::new().map_err(nm)?.decompress(z.value(), 1 << 22),
        }
        .map_err(nm)?;
        serde_json::from_slice(&raw).map_err(nm)
    }

    /// Rebuild the tree under `root`.
    pub fn tree(&self, site: &str, root: &str) -> Result<PNode> {
        let r = self.load(site, root)?;
        Ok(PNode {
            children: r.kids.iter().map(|k| self.tree(site, k)).collect::<Result<_>>()?,
            role: r.role,
            name: r.name,
            state: r.state,
            value: r.value,
            rect: r.rect,
        })
    }

    /// The last snapshot of `site` with its age.
    pub fn recall(&self, site: &str) -> Result<Option<Recalled>> {
        let (seq, s) = {
            let r = self.db.begin_read().map_err(nm)?;
            let t = r.open_table(SNAPS).map_err(nm)?;
            let last = t.range((site, 0u64)..=(site, u64::MAX)).map_err(nm)?.next_back();
            match last {
                None => return Ok(None),
                Some(x) => {
                    let (k, v) = x.map_err(nm)?;
                    (k.value().1, serde_json::from_slice::<Snap>(v.value()).map_err(nm)?)
                }
            }
        };
        Ok(Some(Recalled {
            seq,
            age_secs: now().saturating_sub(s.ts),
            tree: self.tree(site, &s.root)?,
            root: s.root,
        }))
    }

    /// Differences between two roots. Subtrees with equal hashes are skipped without being read,
    /// so the cost is proportional to the changed paths. Returns the changes and the number of
    /// node records read.
    pub fn diff(&self, site: &str, a: &str, b: &str) -> Result<(Vec<Change>, usize)> {
        let mut out = Vec::new();
        let mut reads = 0;
        self.diff_in(site, Some(a), Some(b), &mut out, &mut reads)?;
        Ok((out, reads))
    }

    fn all(&self, site: &str, h: &str, f: &dyn Fn(String) -> Change, out: &mut Vec<Change>, reads: &mut usize) -> Result<()> {
        let r = self.load(site, h)?;
        *reads += 1;
        out.push(f(line(&r)));
        for k in &r.kids {
            self.all(site, k, f, out, reads)?;
        }
        Ok(())
    }

    fn diff_in(&self, site: &str, a: Option<&str>, b: Option<&str>, out: &mut Vec<Change>, reads: &mut usize) -> Result<()> {
        match (a, b) {
            (None, None) => {}
            (Some(x), Some(y)) if x == y => {}
            (Some(x), None) => self.all(site, x, &Change::Removed, out, reads)?,
            (None, Some(y)) => self.all(site, y, &Change::Added, out, reads)?,
            (Some(x), Some(y)) => {
                let (ra, rb) = (self.load(site, x)?, self.load(site, y)?);
                *reads += 2;
                if (&ra.role, &ra.name, &ra.state, &ra.value, ra.rect) != (&rb.role, &rb.name, &rb.state, &rb.value, rb.rect) {
                    out.push(Change::Changed { before: line(&ra), after: line(&rb) });
                }
                // Children are matched by position; a hash match skips the whole subtree.
                for i in 0..ra.kids.len().max(rb.kids.len()) {
                    self.diff_in(site, ra.kids.get(i).map(|s| s.as_str()), rb.kids.get(i).map(|s| s.as_str()), out, reads)?;
                }
            }
        }
        Ok(())
    }

    /// Handle numbers for `tree`, stable across visits and re-mounts: a node keeps the number its
    /// (role, name, container, nth-duplicate) key was given the first time the site was seen.
    /// Returns key -> handle in document order.
    pub fn handles(&self, site: &str, tree: &PNode) -> Result<Vec<(String, u32)>> {
        let mut keys = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        fn walk(n: &PNode, ctx: &str, seen: &mut HashMap<String, usize>, keys: &mut Vec<String>) {
            let base = format!("{}|{}|{}", n.role, n.name, ctx);
            let k = seen.entry(base.clone()).or_insert(0);
            *k += 1;
            keys.push(format!("{base}#{k}"));
            let nctx = if n.name.is_empty() { ctx.to_string() } else { format!("{}:{}", n.role, n.name) };
            for c in &n.children {
                walk(c, &nctx, seen, keys);
            }
        }
        walk(tree, "", &mut seen, &mut keys);
        let w = self.db.begin_write().map_err(nm)?;
        let mut res = Vec::new();
        {
            let mut ht = w.open_table(HANDLES).map_err(nm)?;
            let mut nx = w.open_table(NEXT).map_err(nm)?;
            let mut next = nx.get(site).map_err(nm)?.map(|v| v.value()).unwrap_or(1);
            for k in keys {
                let have = ht.get((site, k.as_str())).map_err(nm)?.map(|v| v.value());
                let h = match have {
                    Some(v) => v,
                    None => {
                        let h = next;
                        next += 1;
                        ht.insert((site, k.as_str()), h).map_err(nm)?;
                        h
                    }
                };
                res.push((k, h));
            }
            nx.insert(site, next).map_err(nm)?;
        }
        w.commit().map_err(nm)?;
        Ok(res)
    }

    /// The number the next new handle of `site` would get: a session that seeds from the store
    /// allocates above it, so its own numbers never collide with stored ones.
    pub fn next_handle(&self, site: &str) -> Result<u32> {
        let r = self.db.begin_read().map_err(nm)?;
        let t = r.open_table(NEXT).map_err(nm)?;
        Ok(t.get(site).map_err(nm)?.map(|v| v.value()).unwrap_or(1))
    }

    /// Record that `action` taken on the page with root `from` led to root `to`.
    pub fn record_edge(&self, from: &str, action: &str, to: &str) -> Result<()> {
        let w = self.db.begin_write().map_err(nm)?;
        w.open_table(EDGES).map_err(nm)?.insert((from, action), to).map_err(nm)?;
        w.commit().map_err(nm)?;
        Ok(())
    }

    /// Known (action, to-root) transitions out of `from`.
    pub fn edges(&self, from: &str) -> Result<Vec<(String, Hash)>> {
        let r = self.db.begin_read().map_err(nm)?;
        let t = r.open_table(EDGES).map_err(nm)?;
        let mut v = Vec::new();
        for x in t.range((from, "")..).map_err(nm)? {
            let (k, to) = x.map_err(nm)?;
            if k.value().0 != from {
                break;
            }
            v.push((k.value().1.to_string(), to.value().to_string()));
        }
        Ok(v)
    }

    /// Total bytes stored in the node table (compressed).
    pub fn node_bytes(&self) -> Result<u64> {
        let r = self.db.begin_read().map_err(nm)?;
        let t = r.open_table(NODES).map_err(nm)?;
        let mut n = 0;
        for x in t.iter().map_err(nm)? {
            n += x.map_err(nm)?.1.value().len() as u64;
        }
        Ok(n)
    }

    pub fn node_count(&self) -> Result<u64> {
        let r = self.db.begin_read().map_err(nm)?;
        r.open_table(NODES).map_err(nm)?.len().map_err(nm)
    }
}
