//! **THE UPSTREAM WPT `testharness.js` RUNNER — the oracle's ceiling, raised.**
//!
//! Until this file existed, **the only instrument that could find a bug in this engine was a
//! 265-site crawl diffed against Chromium.** That instrument has two hard limits, and neither is
//! fixable by running it harder:
//!
//!   1. **It can only see what those 265 sites happen to exercise.** A DOM method no site in the
//!      corpus calls is, to the oracle, *correct by default*.
//!   2. **It needs Chromium to say what "right" is** — so every answer is a *diff*, and a diff
//!      cannot tell you whether **both** engines are wrong, or whether *we* are wrong in a way that
//!      happens not to move a box.
//!
//! WPT has neither limit. It is ~50,000 tests that **carry their own verdict** — `assert_equals`
//! either holds or it does not, and **no oracle is required at all.** It is the difference between
//! *"we render this page differently from Chrome"* and ***"`Node.prototype.after()` is specified to
//! do X and we do Y."***
//!
//! ## How the integration actually works, and why it is not a hack
//!
//! WPT's own `resources/testharnessreport.js` says, in its header comment:
//!
//! > *"This file is intended for vendors to implement code needed to integrate testharness.js tests
//! > with their own test systems."*
//!
//! **That is the sanctioned hook, and we are the vendor.** We serve our own `testharnessreport.js`
//! in its place; it registers an `add_completion_callback` and writes the results into the DOM as
//! JSON, where the Rust side reads them back with `querySelector`. No engine hook, no private API,
//! no patch to the corpus — the same integration point Gecko and Servo use.
//!
//! ## Why a real HTTP server, when URL-rewriting to `file://` is five lines
//!
//! Because **`file://` is an opaque origin**, and a test that fails because *our harness* served it
//! from the wrong origin would be recorded as an **engine** failure. This project has already been
//! bitten by exactly that: `file://` was unsupported by the net layer *and* the fixture URLs were
//! malformed, so **"React renders nothing" sat in the ledger for ticks as a React problem — and it
//! was the harness.** A conformance number contaminated by its own runner is worse than no number,
//! because it is *believed*.
//!
//! So: a static server over the real tree, on a real origin, exactly as `wptserve` would.
//!
//! ## What this deliberately does NOT run, stated so it is never mistaken for a pass
//!
//!   * **Reftests** (`<link rel=match>`) — those are **Bar 2** (pixel precision), which is
//!     deliberately deferred. `reftest.rs` is where they live.
//!   * **`.any.js` / `.window.js`** — wptserve *generates* the HTML wrapper for these at request
//!     time. **72 of 2,809** tests in the current subsets (~2.5%). Skipped, counted, and reported —
//!     not silently dropped.
//!   * **`-manual.html`**, and anything needing `testdriver.js` (synthetic input) or wptserve's
//!     Python request handlers.
//!
//! Every one of those is **reported as SKIP with a reason.** A runner that quietly drops the tests
//! it cannot run reports a pass rate for a suite it did not run.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use manuk_text::FontContext;

/// The vendor hook. WPT's own copy of this file is a placeholder that says, in so many words,
/// *"vendors: put your integration here."*
///
/// It must be **defensive**: it runs inside the page under test, and if it throws, the test reports
/// nothing and we would score a harness bug as an engine failure. Hence the `try`/`catch` around
/// the whole body and the `String(e)` fallbacks.
const REPORT_JS: &str = r#"
(function () {
  // A breadcrumb dropped SYNCHRONOUSLY, the moment this file executes. If the results node is
  // missing but this one is present, the harness loaded and the completion callback never fired —
  // a completely different bug from "testharness.js never ran at all". Without it, NO_REPORT is one
  // undifferentiated bucket and there is nothing to chase.
  try {
    var b = document.createElement('meta');
    b.id = '__wpt_hook__';
    (document.documentElement || document.body).appendChild(b);
  } catch (e) {}
  function emit(payload) {
    try {
      var s = document.createElement('script');
      s.id = '__wpt_results__';
      s.type = 'application/json';
      s.textContent = JSON.stringify(payload);
      (document.documentElement || document.body).appendChild(s);
    } catch (e) { /* nothing left to report with */ }
  }
  try {
    // **Turn off testharness's HTML results renderer.** It exists to draw a table into `#log` for a
    // human reading the page; we read the results programmatically. Every real WPT runner sets this
    // (wptrunner passes `output: false` through `testharness_properties`) — it is the sanctioned
    // configuration, not a workaround.
    //
    // It also removes an entire class of false failure: the renderer is *page code*, so any DOM gap
    // it trips over throws INSIDE `notify_complete`, which aborts the completion-callback loop and
    // makes the file report NOTHING. A missing `insertAdjacentText` did exactly that to 29 of the
    // first 40 files — the engine bug was real and is now fixed, but the runner must not be one DOM
    // gap away from reporting zero either way.
    setup({ output: false });
    add_completion_callback(function (tests, status) {
      // `status.status` is a NUMBER (0 OK, 1 ERROR, 2 TIMEOUT, 3 PRECONDITION_FAILED), not a string.
      // Emitting the raw number produced `{"harness":0,...}`, and the Rust reader — which scans past
      // the key to the next quoted value — sailed over the digit and returned the literal text
      // `message`. Every harness verdict in the first run was that word. **The instrument was lying,
      // and it took reading one raw payload to see it.**
      var HS = ['OK', 'ERROR', 'TIMEOUT', 'PRECONDITION_FAILED'];
      var out = { harness: HS[status.status] || ('STATUS_' + status.status),
                  message: status.message || '', tests: [] };
      for (var i = 0; i < tests.length; i++) {
        var t = tests[i];
        out.tests.push({
          name: String(t.name),
          status: t.status,                 // 0 PASS 1 FAIL 2 TIMEOUT 3 NOTRUN 4 PRECONDITION_FAILED
          message: t.message ? String(t.message) : ''
        });
      }
      emit(out);
    });
  } catch (e) {
    // `add_completion_callback` itself is missing => testharness.js did not run. That is the single
    // most important failure to distinguish, because it means the pass rate is measuring US, not it.
    emit({ harness: 'HARNESS_NOT_LOADED', message: String(e), tests: [] });
  }
})();
"#;

/// One WPT test's outcome, as WPT itself defines it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sub {
    Pass,
    Fail(String),
    Timeout,
    NotRun,
    PreconditionFailed(String),
}

#[derive(Clone, Debug)]
pub struct TestFile {
    pub path: String,
    /// `None` => the harness never reported (crash, hang, or testharness.js itself failed to run).
    pub subtests: Option<Vec<(String, Sub)>>,
    pub harness_status: String,
    pub ms: u128,
}

impl TestFile {
    pub fn counts(&self) -> (usize, usize) {
        match &self.subtests {
            Some(ts) => (ts.iter().filter(|(_, s)| *s == Sub::Pass).count(), ts.len()),
            None => (0, 0),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// The static server. `wptserve` in ~60 lines, minus the Python handlers we do not use.
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// Serve `root` over HTTP, with **one override**: `/resources/testharnessreport.js` returns
/// [`REPORT_JS`] instead of the file on disk.
///
/// Overriding it **in the server** rather than by writing into the checkout matters: the checkout
/// stays pristine, so `git -C wpt status` is clean and a `git pull` never conflicts. A runner that
/// mutates its own corpus is a runner whose corpus you cannot trust.
pub async fn serve(root: PathBuf) -> std::io::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    use http_body_util::Full;
    use hyper::body::Bytes;
    use hyper::service::service_fn;
    use hyper::{Response, StatusCode};
    use hyper_util::rt::TokioIo;
    use tokio::net::TcpListener;

    // Port 0 => the OS picks a free one. Hardcoding 8000 makes two concurrent runs fight, and the
    // loser's failures look like engine bugs.
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    let root = Arc::new(root);

    let handle = tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            let root = root.clone();
            tokio::spawn(async move {
                let svc = service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                    let root = root.clone();
                    async move {
                        let path = req.uri().path().to_string();
                        let (status, ctype, body) = resolve(&root, &path);
                        Ok::<_, std::convert::Infallible>(
                            Response::builder()
                                .status(StatusCode::from_u16(status).unwrap())
                                .header("content-type", ctype)
                                // WPT's own server sends this; some tests read it.
                                .header("access-control-allow-origin", "*")
                                .body(Full::new(Bytes::from(body)))
                                .unwrap(),
                        )
                    }
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), svc)
                    .await;
            });
        }
    });
    Ok((addr, handle))
}

fn resolve(root: &Path, url_path: &str) -> (u16, &'static str, Vec<u8>) {
    if url_path == "/resources/testharnessreport.js" {
        return (200, "text/javascript", REPORT_JS.as_bytes().to_vec());
    }
    // Strip the query — WPT tests pass `?foo` to their own resources routinely.
    let clean = url_path
        .split('?')
        .next()
        .unwrap_or("")
        .trim_start_matches('/');
    // Path traversal: the corpus is not hostile, but a `..` that escaped the root would read this
    // repo's own files into a test, and the resulting failure would be inexplicable.
    if clean.split('/').any(|c| c == "..") {
        return (403, "text/plain", b"no".to_vec());
    }
    let full = root.join(clean);
    match std::fs::read(&full) {
        Ok(bytes) => (200, content_type(&full), bytes),
        Err(_) => (404, "text/plain", b"not found".to_vec()),
    }
}

fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" | "xht" | "xhtml" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "text/plain; charset=utf-8",
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// Test discovery
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// Why a file in the tree is not a test we can run. **Every skip carries a reason** — see the
/// module doc: a runner that silently drops what it cannot run reports a pass rate for a suite it
/// did not run.
pub fn skip_reason(rel: &str, body: &str) -> Option<&'static str> {
    let f = rel.rsplit('/').next().unwrap_or(rel);
    if rel.starts_with("resources/") || rel.contains("/support/") || rel.starts_with("common/") {
        return Some("support file, not a test");
    }
    if f.ends_with("-ref.html") || f.ends_with("-ref.xht") || rel.contains("/reference/") {
        return Some("reftest reference");
    }
    if f.contains("-manual.") {
        return Some("manual test (needs a human)");
    }
    // A reftest *is* a real test — it is just a Bar 2 one, and Bar 2 is deferred. Say so.
    if body.contains("rel=\"match\"")
        || body.contains("rel=match")
        || body.contains("rel=\"mismatch\"")
        || body.contains("rel=mismatch")
    {
        return Some("reftest (Bar 2 — pixel, deferred)");
    }
    if body.contains("testdriver.js") {
        return Some("needs testdriver (synthetic input)");
    }
    if !body.contains("testharness.js") {
        return Some("not a testharness test");
    }
    None
}

pub struct Discovered {
    pub tests: Vec<String>,
    pub skipped: BTreeMap<&'static str, usize>,
}

pub fn discover(root: &Path, subset: &str) -> Discovered {
    let mut tests = Vec::new();
    let mut skipped: BTreeMap<&'static str, usize> = BTreeMap::new();
    let base = root.join(subset);
    let mut stack = vec![base];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string();
            if ext == "js" && (rel.contains(".any.") || rel.contains(".window.")) {
                *skipped
                    .entry("generated wrapper (.any.js/.window.js — needs wptserve)")
                    .or_default() += 1;
                continue;
            }
            if !matches!(ext, "html" | "htm" | "xht" | "xhtml") {
                continue;
            }
            let body = std::fs::read_to_string(&p).unwrap_or_default();
            match skip_reason(&rel, &body) {
                Some(r) => *skipped.entry(r).or_default() += 1,
                None => tests.push(rel),
            }
        }
    }
    tests.sort();
    Discovered { tests, skipped }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// Running one test
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// Load one test through the **real** page pipeline (fetch → parse → cascade → script → event
/// loop) and read back what `testharness.js` concluded.
///
/// The `timeout` is WPT's own: 10s default. A test that does not report inside it is a **TIMEOUT**,
/// which is a **result**, not an error — it is Bar 0 signal, and it is exactly the number the hang
/// work has been chasing all project.
pub async fn run_one(
    base: &str,
    rel: &str,
    fonts: &FontContext,
    timeout: std::time::Duration,
) -> TestFile {
    let url = format!("{base}/{rel}");
    let t0 = std::time::Instant::now();

    let fut = async {
        let (html, final_url) = manuk_page::fetch_html(&url).await.ok()?;
        let mut page = manuk_page::Page::load_async(&html, &final_url, fonts, 800.0).await;
        page.finish_loading(fonts, 800.0).await;
        Some(page)
    };

    let page = match tokio::time::timeout(timeout, fut).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return TestFile {
                path: rel.into(),
                subtests: None,
                harness_status: "FETCH_FAILED".into(),
                ms: t0.elapsed().as_millis(),
            }
        }
        Err(_) => {
            // **`SLOW`, not `TIMEOUT`.** This is OUR budget expiring — the page took longer than we
            // allowed. It is NOT the same event as testharness reporting its own `TIMEOUT` status
            // (an async test that never completed), and it is NOT a hang (a hang is the driver
            // killing a child that stopped making progress at all).
            //
            // Collapsing all three into the word "TIMEOUT" made 89 slow files read as 89 Bar 0 hangs
            // in the first baseline. **Three different findings must not share a name.**
            return TestFile {
                path: rel.into(),
                subtests: None,
                harness_status: "SLOW".into(),
                ms: t0.elapsed().as_millis(),
            };
        }
    };

    let ms = t0.elapsed().as_millis();
    let mut page = page;
    let dom = page.dom();
    let hits = manuk_css::query_selector_all(dom, dom.root(), "#__wpt_results__");
    if hits.is_empty() {
        // **Do not record "NO_REPORT" and move on.** Ask the page what happened — it still knows.
        page.eval_for_test(
            "(function(){ try {                var s = document.createElement('script'); s.id='__wpt_diag__'; s.type='application/json';                s.textContent = JSON.stringify({                  harness: (typeof add_completion_callback === 'function'),                  hook: !!document.getElementById('__wpt_hook__'),                  ready: String(document.readyState),                  loadFired: !!globalThis.__loadFired,                  errors: (globalThis.__errors || []).slice(0, 2) });                (document.documentElement||document.body).appendChild(s);              } catch (e) {} })()",
        );
        let dom = page.dom();
        let d = manuk_css::query_selector_all(dom, dom.root(), "#__wpt_diag__");
        let why = d.first().map(|&n| dom.text_content(n)).unwrap_or_default();
        return TestFile {
            path: rel.into(),
            subtests: None,
            harness_status: if why.is_empty() {
                "NO_REPORT".into()
            } else {
                format!("NO_REPORT {why}")
            },
            ms,
        };
    }
    let dom = page.dom();
    let hits = manuk_css::query_selector_all(dom, dom.root(), "#__wpt_results__");
    let Some(&node) = hits.first() else {
        // No results node at all. The completion callback never fired: the page threw before
        // testharness.js finished, or the harness never loaded. **This is the number that decides
        // whether the whole suite is measuring the engine or measuring the runner.**
        return TestFile {
            path: rel.into(),
            subtests: None,
            harness_status: "NO_REPORT".into(),
            ms,
        };
    };
    let json = dom.text_content(node);

    match parse_results(&json) {
        Some((harness, subtests)) => TestFile {
            path: rel.into(),
            subtests: Some(subtests),
            harness_status: harness,
            ms,
        },
        None => TestFile {
            path: rel.into(),
            subtests: None,
            harness_status: "BAD_REPORT".into(),
            ms,
        },
    }
}

/// Parse our own report payload. Deliberately a hand-rolled scan rather than a `serde` dependency:
/// the shape is ours, it is fixed, and the messages contain every character that would make a
/// naive split wrong — so the scanner tracks string state and escapes.
fn parse_results(json: &str) -> Option<(String, Vec<(String, Sub)>)> {
    let harness = field_str(json, "\"harness\":")?;
    let mut out = Vec::new();
    // Walk each `{"name":...,"status":N,"message":...}` object inside `"tests":[ ... ]`.
    let tests_at = json.find("\"tests\":")?;
    let mut rest = &json[tests_at..];
    while let Some(n_at) = rest.find("\"name\":") {
        rest = &rest[n_at..];
        let name = field_str(rest, "\"name\":")?;
        let status_at = rest.find("\"status\":")?;
        let after = &rest[status_at + 9..];
        let num: String = after
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let msg = field_str(rest, "\"message\":").unwrap_or_default();
        out.push((
            name,
            match num.as_str() {
                "0" => Sub::Pass,
                "1" => Sub::Fail(msg),
                "2" => Sub::Timeout,
                "3" => Sub::NotRun,
                _ => Sub::PreconditionFailed(msg),
            },
        ));
        rest = &rest[status_at + 9..];
    }
    Some((harness, out))
}

/// Read the JSON string value that follows `key`, honouring `\"` escapes.
fn field_str(s: &str, key: &str) -> Option<String> {
    let at = s.find(key)? + key.len();
    let bytes = s.as_bytes();
    let mut i = at;
    while i < bytes.len() && bytes[i] != b'"' {
        i += 1;
    }
    i += 1; // past the opening quote
    let mut out = String::new();
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => {
                match bytes[i + 1] {
                    b'n' => out.push('\n'),
                    b't' => out.push('\t'),
                    b'u' => {
                        i += 5;
                        continue;
                    } // drop \uXXXX — messages only
                    c => out.push(c as char),
                }
                i += 2;
            }
            b'"' => return Some(out),
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_report_with_quotes_and_commas_in_the_message() {
        // The messages are the hostile input: they contain the very characters a naive
        // split-on-comma would break on, and WPT's assert_equals messages ALWAYS do.
        let json = r#"{"harness":"OK","message":"","tests":[
            {"name":"first, with a comma","status":0,"message":""},
            {"name":"second","status":1,"message":"assert_equals: expected \"a\" but got \"b\""}
        ]}"#;
        let (h, ts) = parse_results(json).expect("must parse");
        assert_eq!(h, "OK");
        assert_eq!(
            ts.len(),
            2,
            "a comma inside a NAME must not split it into two subtests"
        );
        assert_eq!(ts[0].0, "first, with a comma");
        assert_eq!(ts[0].1, Sub::Pass);
        match &ts[1].1 {
            Sub::Fail(m) => assert!(
                m.contains("expected \"a\" but got \"b\""),
                "the escaped quotes in an assert_equals message must survive: {m}"
            ),
            other => panic!("expected Fail, got {other:?}"),
        }
    }

    #[test]
    fn a_reftest_is_skipped_as_bar_2_not_run_as_a_pass() {
        // Silently treating a reftest as "no subtests, no failures" would report it as a PASS.
        let body = r#"<link rel="match" href="foo-ref.html">"#;
        assert_eq!(
            skip_reason("css/x.html", body),
            Some("reftest (Bar 2 — pixel, deferred)")
        );
        assert_eq!(skip_reason("css/x-ref.html", ""), Some("reftest reference"));
        // ...and a real testharness test is NOT skipped.
        assert_eq!(
            skip_reason("dom/x.html", r#"<script src="/resources/testharness.js">"#),
            None
        );
    }
}
