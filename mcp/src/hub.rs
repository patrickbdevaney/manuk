//! W3.5: the action log and stateless page handles. Every tool call is appended to a per-page log
//! `(seq, handle, effect-diff hash, request ids, storage hash)`; a page handle `<id>:<seq>` *is* a
//! log position. A client that dies loses nothing: any client holding the handle can resume,
//! because the server rebuilds the page by replaying the persisted log (and refuses to continue if
//! a replayed step's hash no longer matches what was recorded). No `Mcp-Session-Id` is involved.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use crate::session::{Output, Session};
use crate::Backend;

/// Calls that do not touch page state and are never logged.
const UNLOGGED: &[&str] = &["screenshot", "storage_state"];

/// FNV-1a 64: stable across builds and platforms (unlike `DefaultHasher`).
pub fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub seq: u64,
    pub tool: String,
    pub args: Map<String, Value>,
    pub ok: bool,
    pub diff_hash: u64,
    /// Wire request ids (`manuk_net::take_request_ids`) issued while the call ran; empty for a call
    /// that touched no network.
    pub req: Vec<u64>,
    pub storage_hash: u64,
}

impl Entry {
    fn to_json(&self, page: &str) -> Value {
        json!({"seq": self.seq, "handle": format!("{page}:{}", self.seq), "tool": self.tool,
               "args": self.args, "ok": self.ok, "diff_hash": format!("{:016x}", self.diff_hash),
               "req": self.req, "storage_hash": format!("{:016x}", self.storage_hash)})
    }
    fn from_json(v: &Value) -> Option<Entry> {
        let hex = |k| u64::from_str_radix(v[k].as_str()?, 16).ok();
        Some(Entry {
            seq: v["seq"].as_u64()?,
            tool: v["tool"].as_str()?.to_string(),
            args: v["args"].as_object()?.clone(),
            ok: v["ok"].as_bool()?,
            diff_hash: hex("diff_hash")?,
            req: v["req"]
                .as_array()?
                .iter()
                .filter_map(Value::as_u64)
                .collect(),
            storage_hash: hex("storage_hash")?,
        })
    }
}

/// R3.2: a checkpoint is a log position of one page (v1, replay-based). Restoring replays that log
/// prefix into a fresh page, so it is exact but costs one page load; it is not a memory snapshot.
#[derive(Clone)]
struct Ckpt {
    log: Vec<Entry>,
    url: String,
    /// The client side of the world at checkpoint time (cookies, web storage, CacheStorage).
    snap: Value,
    /// `CLIENT`: no non-safe request had left this process, so rewinding the client rewinds
    /// everything the page could have changed. `WORLD`: the server had already been written to.
    tag: &'static str,
    /// Position in this process's non-safe request ledger, and the process it is valid in.
    pos: usize,
    pid: u32,
}

struct Page<B: Backend> {
    s: Session<B>,
    log: Vec<Entry>,
    /// A forked branch: non-safe sends are held, not sent, until `commit`.
    fenced: bool,
    held: Vec<manuk_net::ledger::Held>,
}

impl<B: Backend> Page<B> {
    fn new(s: Session<B>, log: Vec<Entry>) -> Self {
        Page {
            s,
            log,
            fenced: false,
            held: Vec::new(),
        }
    }
}

fn held_line(held: &[manuk_net::ledger::Held]) -> String {
    let v: Vec<String> = held.iter().map(|h| h.what.short()).collect();
    format!("held=[{}]", v.join(", "))
}

pub struct Hub<B: Backend> {
    make: Box<dyn Fn() -> B>,
    dir: Option<PathBuf>,
    pages: HashMap<String, Page<B>>,
    ckpts: HashMap<String, Ckpt>,
    default: Option<String>,
    next: u64,
    salt: u64,
}

fn strip_page(a: &Map<String, Value>) -> Map<String, Value> {
    let mut a = a.clone();
    a.remove("page");
    a
}

impl<B: Backend> Hub<B> {
    /// `dir`: where logs persist (`None` = in-memory only; nothing touches the disk).
    pub fn new(make: impl Fn() -> B + 'static, dir: Option<PathBuf>) -> Self {
        let salt = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
            ^ u64::from(std::process::id()) << 40;
        if let Some(d) = &dir {
            let _ = std::fs::create_dir_all(d);
        }
        Hub {
            make: Box::new(make),
            dir,
            pages: HashMap::new(),
            ckpts: HashMap::new(),
            default: None,
            next: 0,
            salt,
        }
    }

    fn fresh_id(&mut self) -> String {
        self.next += 1;
        format!(
            "p{:06x}",
            fnv(format!("{}:{}", self.salt, self.next).as_bytes()) & 0xff_ffff
        )
    }

    fn path(&self, id: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(format!("{id}.jsonl")))
    }

    fn storage_hash(s: &Session<B>) -> u64 {
        s.backend()
            .export_storage_state()
            .ok()
            .map(|v| fnv(v.to_string().as_bytes()))
            .unwrap_or(0)
    }

    fn load(&self, id: &str) -> Result<Vec<Entry>, String> {
        let p = self.path(id).ok_or_else(|| {
            format!("unknown page {id:?} (no persisted log: set MANUK_ACTLOG_DIR)")
        })?;
        let text = std::fs::read_to_string(&p)
            .map_err(|_| format!("unknown page {id:?}: no log at {}", p.display()))?;
        text.lines()
            .map(|l| {
                serde_json::from_str::<Value>(l)
                    .ok()
                    .and_then(|v| Entry::from_json(&v))
                    .ok_or_else(|| format!("corrupt log line for {id}"))
            })
            .collect()
    }

    /// Run one entry's call and return `(ok, diff hash)`.
    async fn run(
        s: &mut Session<B>,
        tool: &str,
        args: &Map<String, Value>,
    ) -> (Result<Output, String>, u64) {
        let r = s.call(tool, args).await.map_err(|e| format!("{e:#}"));
        let h = match &r {
            Ok(Output::Text(t)) => fnv(t.as_bytes()),
            Ok(Output::Image { png, .. }) => fnv(png),
            Err(e) => fnv(e.as_bytes()),
        };
        (r, h)
    }

    /// Rebuild page `id` at position `seq` by replaying its persisted log, verifying every hash.
    async fn resume(&mut self, id: &str, seq: u64) -> Result<(), String> {
        let mut log = self.load(id)?;
        if (log.len() as u64) < seq {
            return Err(format!(
                "page handle {id}:{seq} is ahead of its log (length {})",
                log.len()
            ));
        }
        log.truncate(seq as usize);
        let mut s = Session::new((self.make)());
        for e in &log {
            let (_, h) = Self::run(&mut s, &e.tool, &e.args).await;
            if h != e.diff_hash || Self::storage_hash(&s) != e.storage_hash {
                return Err(format!("REPLAY DIVERGED at {id}:{} ({}): the page no longer reproduces what was recorded; start a new page", e.seq, e.tool));
            }
        }
        self.pages.insert(id.to_string(), Page::new(s, log));
        Ok(())
    }

    fn ckpt_path(&self, id: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(format!("{id}.ckpt")))
    }

    fn ckpt_get(&mut self, id: &str) -> Result<Ckpt, String> {
        if let Some(c) = self.ckpts.get(id) {
            return Ok(c.clone());
        }
        let p = self.ckpt_path(id).ok_or_else(|| {
            format!("unknown checkpoint {id:?} (no persisted store: set MANUK_ACTLOG_DIR)")
        })?;
        let text = std::fs::read_to_string(&p)
            .map_err(|_| format!("unknown checkpoint {id:?}: no file at {}", p.display()))?;
        let mut lines = text.lines();
        let head: Value = serde_json::from_str(lines.next().unwrap_or(""))
            .map_err(|_| format!("corrupt checkpoint {id}"))?;
        let url = head["url"].as_str().unwrap_or("").to_string();
        let log = lines
            .map(|l| {
                serde_json::from_str::<Value>(l)
                    .ok()
                    .and_then(|v| Entry::from_json(&v))
                    .ok_or_else(|| format!("corrupt checkpoint {id}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let c = Ckpt {
            log,
            url,
            snap: head["snap"].clone(),
            tag: if head["tag"] == "WORLD" {
                "WORLD"
            } else {
                "CLIENT"
            },
            pos: head["pos"].as_u64().unwrap_or(0) as usize,
            pid: head["pid"].as_u64().unwrap_or(0) as u32,
        };
        self.ckpts.insert(id.to_string(), c.clone());
        Ok(c)
    }

    /// `checkpoint`: remember the current position of page `id`.
    fn checkpoint(&mut self, id: &str) -> String {
        let page = &self.pages[id];
        let pos = manuk_net::ledger::position();
        let c = Ckpt {
            log: page.log.clone(),
            url: page.s.backend().url(),
            snap: manuk_net::partition::snapshot().unwrap_or(Value::Null),
            tag: if pos == 0 { "CLIENT" } else { "WORLD" },
            pos,
            pid: std::process::id(),
        };
        let cid = format!(
            "c{:06x}",
            fnv(format!("{}:{}:{}", self.salt, self.next, self.ckpts.len()).as_bytes()) & 0xff_ffff
        );
        self.next += 1;
        if let Some(p) = self.ckpt_path(&cid) {
            let head =
                json!({"url": c.url, "tag": c.tag, "pos": c.pos, "pid": c.pid, "snap": c.snap});
            let mut out = format!("{head}\n");
            for e in &c.log {
                out.push_str(&format!("{}\n", e.to_json(id)));
            }
            let _ = std::fs::write(p, out);
        }
        let n = c.log.len();
        let url = c.url.clone();
        let tag = c.tag;
        let note = if tag == "CLIENT" {
            "CLIENT: nothing has been sent to a server yet, so restore is a full rollback."
        } else {
            "WORLD: non-safe requests had already reached a server before this point; restore rewinds the client only."
        };
        self.ckpts.insert(cid.clone(), c);
        format!("checkpoint {cid} [{tag}] at {id}:{n} ({url}); restore/fork replays the log into a fresh page, so it costs one page load. {note}\npage: {id}:{n}")
    }

    /// `commit`: lift the branch's write-fence and send what it held, once each.
    async fn commit(&mut self, id: &str) -> String {
        let page = self.pages.get_mut(id).unwrap();
        let held = std::mem::take(&mut page.held);
        page.fenced = false;
        let (mut sent, mut dropped) = (Vec::new(), Vec::new());
        for h in held {
            if !h.replayable {
                dropped.push(h.what.short());
                continue;
            }
            let hs: Vec<(&str, &str)> = h
                .headers
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let url = h.what.url.clone();
            match manuk_net::request_from(&h.what.method, &url, &hs, h.body.clone(), Some(&url))
                .await
            {
                Ok(_) => sent.push(h.what.short()),
                Err(e) => dropped.push(format!("{} ({e})", h.what.short())),
            }
        }
        let n = page.log.len();
        format!(
            "committed: sent {} [{}]; could not send {} [{}]\npage: {id}:{n}",
            sent.len(),
            sent.join(", "),
            dropped.len(),
            dropped.join(", ")
        )
    }

    /// `restore` / `fork`: replay checkpoint `cid` into a brand-new page (verifying every hash) and
    /// return its handle. `restore` also makes that page the default one.
    async fn from_ckpt(&mut self, cid: &str, make_default: bool) -> Result<Output, String> {
        let c = self.ckpt_get(cid)?;
        // What the server has seen since the checkpoint, computed before the replay adds its own.
        let diverged = if c.pid == std::process::id() {
            Some(manuk_net::ledger::sent_since(c.pos))
        } else {
            None
        };
        let id = self.fresh_id();
        let mut s = Session::new((self.make)());
        s.backend_mut().reset_storage();
        for e in &c.log {
            let (_, h) = Self::run(&mut s, &e.tool, &e.args).await;
            if h != e.diff_hash || Self::storage_hash(&s) != e.storage_hash {
                return Err(format!("REPLAY DIVERGED restoring {cid} at step {} ({}): the page no longer reproduces what was recorded", e.seq, e.tool));
            }
        }
        if let Some(p) = self.path(&id) {
            let text: String = c
                .log
                .iter()
                .map(|e| format!("{}\n", e.to_json(&id)))
                .collect();
            let _ = std::fs::write(p, text);
        }
        // The replay rebuilt the page; the snapshot is the authority for the rest of the partition.
        manuk_net::partition::restore(&c.snap)
            .map_err(|e| format!("partition restore failed: {e}"))?;
        let n = c.log.len();
        let mut page = Page::new(s, c.log);
        page.fenced = !make_default;
        self.pages.insert(id.clone(), page);
        if make_default {
            self.default = Some(id.clone());
        }
        let world = match diverged {
            Some(d) if d.is_empty() => "server: not diverged (no non-safe request since the checkpoint)".to_string(),
            Some(d) => format!(
                "SERVER-DIVERGED: {} non-safe request(s) since the checkpoint reached the server and were NOT undone: [{}]",
                d.len(),
                d.iter().map(|x| x.short()).collect::<Vec<_>>().join(", ")
            ),
            None => "server: divergence unknown (the checkpoint was taken by another process)".to_string(),
        };
        let fence = if make_default {
            ""
        } else {
            "\nbranch write-fence ON: non-GET/HEAD/OPTIONS sends are held until `commit`"
        };
        Ok(Output::Text(format!(
            "{} {cid} [{}] into a new page ({})\n{world}{fence}\npage: {id}:{n}",
            if make_default { "restored" } else { "forked" },
            c.tag,
            c.url
        )))
    }

    /// `args.page`: absent = the default page; `"new"` = a fresh page; `"<id>:<seq>"` = resume/verify.
    pub async fn call(&mut self, tool: &str, args: &Map<String, Value>) -> Result<Output, String> {
        if matches!(tool, "restore" | "fork") {
            let cid = args
                .get("checkpoint")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!("{tool} needs `checkpoint` (an id returned by `checkpoint`)")
                })?
                .to_string();
            return self.from_ckpt(&cid, tool == "restore").await;
        }
        let want = args.get("page").and_then(Value::as_str).map(str::to_string);
        let id = match want.as_deref() {
            None => match &self.default {
                Some(d) => d.clone(),
                None => {
                    let id = self.fresh_id();
                    self.default = Some(id.clone());
                    id
                }
            },
            Some("new") => self.fresh_id(),
            Some(h) => {
                let (id, seq) = h
                    .rsplit_once(':')
                    .and_then(|(i, s)| Some((i, s.parse::<u64>().ok()?)))
                    .ok_or_else(|| format!("bad page handle {h:?}: expected \"<id>:<seq>\" as returned by the last call"))?;
                match self.pages.get(id).map(|p| p.log.len() as u64) {
                    Some(n) if n == seq => {}
                    Some(n) => {
                        return Err(format!(
                            "STALE page handle {h}: that page is at seq {n}; use {id}:{n}"
                        ))
                    }
                    None => self.resume(id, seq).await?,
                }
                id.to_string()
            }
        };
        if !self.pages.contains_key(&id) {
            let s = Session::new((self.make)());
            self.pages.insert(id.clone(), Page::new(s, Vec::new()));
        }
        if tool == "checkpoint" {
            return Ok(Output::Text(self.checkpoint(&id)));
        }
        if tool == "commit" {
            return Ok(Output::Text(self.commit(&id).await));
        }
        let logged = !UNLOGGED.contains(&tool);
        let a = strip_page(args);
        let page = self.pages.get_mut(&id).unwrap();
        // Take-since around the call: ids issued before it belong to someone else.
        let _ = manuk_net::take_request_ids();
        manuk_net::ledger::set_fence(page.fenced);
        let (r, h) = Self::run(&mut page.s, tool, &a).await;
        manuk_net::ledger::set_fence(false);
        page.held.extend(manuk_net::ledger::take_held());
        let held = if page.held.is_empty() {
            None
        } else {
            Some(held_line(&page.held))
        };
        let req = manuk_net::take_request_ids();
        let mut seq = page.log.len() as u64;
        if logged {
            seq += 1;
            let e = Entry {
                seq,
                tool: tool.to_string(),
                args: a,
                ok: r.is_ok(),
                diff_hash: h,
                req,
                storage_hash: Self::storage_hash(&page.s),
            };
            if let Some(p) = self.path(&id) {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(p)
                {
                    let _ = writeln!(f, "{}", e.to_json(&id));
                }
            }
            self.pages.get_mut(&id).unwrap().log.push(e);
        }
        let handle = format!("page: {id}:{seq}");
        let handle = match held {
            Some(h) => format!("{h}\n{handle}"),
            None => handle,
        };
        match r {
            Ok(Output::Text(t)) => Ok(Output::Text(format!("{t}\n{handle}"))),
            Ok(Output::Image { png, note }) => Ok(Output::Image {
                png,
                note: format!("{note}\n{handle}"),
            }),
            Err(e) => Err(format!("{e}\n{handle}")),
        }
    }
}
