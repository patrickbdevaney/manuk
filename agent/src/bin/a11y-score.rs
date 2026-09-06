//! `a11y-score` — score Manuk's accessibility tree against Chrome's, with **precision, recall AND
//! F1**, on a real URL.
//!
//! ## Why this binary exists
//!
//! Track B's bar is *">=90% node match against Chrome's a11y tree"*, and until this tick **no
//! instrument in the repository could compute it.** Every number the loop has quoted for that bar
//! — 63.8%, 75.0%, 97.0%, and the per-site rows in `g_a11y_name_from_content_context` — was
//! produced by a throwaway script under `/tmp` that no longer exists. A gate whose number cannot be
//! recomputed is not a gate; it is a memory of one.
//!
//! ⭐⭐⭐ **AND THE NUMBER IT REMEMBERS IS RECALL.** A multiset match taken *over the oracle's nodes*
//! asks only "how many of Chrome's nodes did we produce" — it cannot see nodes we invent. The one
//! time both halves were computed by hand (martinfowler.com), they were **97.3% recall and 67.7%
//! precision**: nearly a third of what that tree offered an agent was not in Chrome's at all. A
//! screen reader reads those. An agent clicks them. Reporting the recall half as "node match" makes
//! the tree look a third better than it is, and it is the half that improves when the projection
//! gets *noisier*.
//!
//! So this tool reports all three, and F1 is the one to steer on.
//!
//! ```text
//!   a11y-score <url>...            # one row per url, then a TOTAL row over the pooled multisets
//! ```
//!
//! ## The oracle
//!
//! Headless Chrome over CDP `Accessibility.getFullAXTree`, which is the same oracle every
//! `agent/tests/g_ax_*.rs` gate is written against. Chrome is launched with its own throwaway
//! profile on an ephemeral debugging port, driven to the url, and read once the load event has
//! fired.
//!
//! ## What is compared, and what is dropped
//!
//! The comparison key is the pair **(role, accessible name)** as a multiset — not the tree shape.
//! Shape agreement is a stronger claim than either engine's tree can currently support (the two
//! disagree about how many `generic` wrappers a `<div>` soup deserves, which is a modelling
//! difference and not a defect), while (role, name) is exactly what an agent resolves a target by.
//!
//! Dropped from **both** sides before matching, each for a stated reason:
//!
//! | dropped | why |
//! |---|---|
//! | `generic` / `none` / `presentation` | carries no role and no name; a pure wrapper-count difference |
//! | `StaticText` / `InlineTextBox` | Chrome's text leaves; Manuk folds text into its parent's name, so counting them measures the modelling difference and nothing else |
//! | nodes with `ignored: true` | Chrome's own marker for "not exposed to an AT" |
//!
//! ⚠ Every one of those drops makes the score **kinder**, so they are listed in the output as
//! counts. A drop that flatters must stay visible or it stops being a modelling decision and starts
//! being a thumb on the scale.

use anyhow::{bail, Context, Result};
use manuk_agent::a11y_score::{self, collapse_ws, is_structural, normalize_chrome_role};
use manuk_agent::AgentBrowser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A blocking-free raw HTTP/1.1 GET against the local DevTools endpoint. Bringing in an HTTP client
/// for four requests to `127.0.0.1` would be a heavier dependency than the request it makes.
async fn http_get(port: u16, path: &str) -> Result<String> {
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
    // ⚠ **THE PORT IN `Host:` IS LOAD-BEARING.** Chrome builds each target's
    // `webSocketDebuggerUrl` by echoing back the request's `Host` header, so a bare
    // `Host: 127.0.0.1` yields `ws://127.0.0.1/devtools/page/...` with no port at all, and the
    // handshake then advertises the wrong authority.
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes())
        .await?;

    // ⚠⚠ **CHROME'S DEVTOOLS HTTP SERVER IGNORES `Connection: close` AND NEVER CLOSES THE
    //    SOCKET**, so reading to EOF blocks forever. The body has to be taken by `Content-Length`.
    //    Under a timeout this presents as a *connect* failure, which sent this tick chasing ports
    //    and stray processes for several attempts: an unbounded read inside a bounded call reports
    //    the deadline, never the reason.
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let (mut head_end, mut want) = (None, None);
    loop {
        if head_end.is_none() {
            if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&buf[..p]).to_ascii_lowercase();
                want = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok());
                head_end = Some(p + 4);
            }
        }
        if let (Some(h), Some(w)) = (head_end, want) {
            if buf.len() >= h + w {
                return Ok(String::from_utf8_lossy(&buf[h..h + w]).to_string());
            }
        }
        let n = s.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8_lossy(&buf).to_string();
    Ok(text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or(text))
}

/// Launch headless Chrome on `port`, drive it to `url`, and return its `(role, name)` multiset.
async fn chrome_ax(url: &str, port: u16) -> Result<(Vec<(String, String)>, usize)> {
    let profile = std::env::temp_dir().join(format!("manuk-a11y-score-{port}"));
    let _ = std::fs::remove_dir_all(&profile);
    let mut child = std::process::Command::new("google-chrome")
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--force-renderer-accessibility",
            &format!("--remote-debugging-port={port}"),
            &format!("--user-data-dir={}", profile.display()),
            "about:blank",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("launching headless Chrome — is `google-chrome` on PATH?")?;

    let result = chrome_ax_inner(url, port).await;
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&profile);
    result
}

async fn chrome_ax_inner(url: &str, port: u16) -> Result<(Vec<(String, String)>, usize)> {
    // The endpoint takes a moment to bind; poll rather than sleep a guessed constant.
    let mut ws_url = None;
    for _ in 0..100 {
        // ⚠ Every request to the endpoint is bounded. A DevTools server that accepts the connection
        // and then answers nothing is indistinguishable from a slow one until a deadline says so.
        let got = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            http_get(port, "/json/list"),
        )
        .await;
        if let Ok(Ok(body)) = got {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(t) = v.as_array().and_then(|a| {
                    a.iter()
                        .find(|t| t["type"] == "page" && t["webSocketDebuggerUrl"].is_string())
                }) {
                    ws_url = Some(t["webSocketDebuggerUrl"].as_str().unwrap().to_string());
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let ws_url = ws_url.context("Chrome's DevTools endpoint never offered a page target")?;

    use futures_util::{SinkExt, StreamExt};
    // `client_async` over a socket we opened ourselves rather than `connect_async`: the endpoint is
    // always `127.0.0.1:<port>`, so the `connect` feature's DNS + TLS stack would be dead weight.
    let stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .context("connecting to the DevTools port")?;
    let (mut ws, _) = tokio_tungstenite::client_async(&ws_url, stream)
        .await
        .context("connecting to the DevTools websocket")?;

    let mut id = 0u64;
    let mut send = |method: &str, params: serde_json::Value| {
        id += 1;
        (
            id,
            serde_json::json!({ "id": id, "method": method, "params": params }).to_string(),
        )
    };

    for (method, params) in [
        ("Page.enable", serde_json::json!({})),
        ("Accessibility.enable", serde_json::json!({})),
        ("Page.navigate", serde_json::json!({ "url": url })),
    ] {
        let (_, msg) = send(method, params);
        ws.send(tokio_tungstenite::tungstenite::Message::Text(msg))
            .await?;
    }

    // Wait for the load event, but never forever — a site that never fires one still has a tree.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            break;
        }
        match tokio::time::timeout(left, ws.next()).await {
            Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t)))) => {
                if t.contains("\"Page.loadEventFired\"") {
                    break;
                }
            }
            Ok(Some(Ok(_))) => {}
            _ => break,
        }
    }
    // Let post-load script settle the same way the manuk side does.
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let (want, msg) = send("Accessibility.getFullAXTree", serde_json::json!({}));
    ws.send(tokio_tungstenite::tungstenite::Message::Text(msg))
        .await?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            bail!("Chrome never answered Accessibility.getFullAXTree");
        }
        let Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t)))) =
            tokio::time::timeout(left, ws.next()).await
        else {
            bail!("the DevTools websocket closed before the tree arrived");
        };
        let v: serde_json::Value = match serde_json::from_str(&t) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v["id"].as_u64() != Some(want) {
            continue;
        }
        let nodes = v["result"]["nodes"]
            .as_array()
            .context("getFullAXTree returned no `nodes`")?;
        let mut out = Vec::new();
        let mut dropped = 0usize;
        for n in nodes {
            if n["ignored"].as_bool() == Some(true) {
                dropped += 1;
                continue;
            }
            let role = normalize_chrome_role(n["role"]["value"].as_str().unwrap_or(""));
            if is_structural(&role) {
                dropped += 1;
                continue;
            }
            let name = n["name"]["value"].as_str().unwrap_or("").trim().to_string();
            out.push((role, collapse_ws(&name)));
        }
        return Ok((out, dropped));
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error")),
        )
        .with_writer(std::io::stderr)
        .init();

    let mut urls: Vec<String> = std::env::args().skip(1).collect();
    let diff = urls.first().map(|a| a == "--diff").unwrap_or(false);
    if diff {
        urls.remove(0);
    }
    if urls.is_empty() {
        bail!("usage: a11y-score [--diff] <url>...");
    }

    println!(
        "{:<38} {:>7} {:>7} {:>7} {:>8} {:>8} {:>8}",
        "url", "manuk", "chrome", "match", "prec", "recall", "F1"
    );

    let (mut pool_m, mut pool_c) = (Vec::new(), Vec::new());
    let mut pooled_hit = 0usize;

    for (i, url) in urls.iter().enumerate() {
        let port = 9500 + i as u16;
        let chrome = match chrome_ax(url, port).await {
            Ok(c) => c,
            Err(e) => {
                println!("{url:<38} ORACLE FAILED: {e}");
                continue;
            }
        };
        let mut browser = AgentBrowser::new(1024, 768);
        let manuk = match browser
            .navigate(url)
            .await
            .and_then(|_| browser.a11y_tree())
        {
            Ok(t) => a11y_score::manuk_bag(&t),
            Err(e) => {
                println!("{url:<38} MANUK FAILED: {e}");
                continue;
            }
        };

        let hit = a11y_score::multiset_overlap(&manuk.0, &chrome.0);
        let precision = if manuk.0.is_empty() {
            0.0
        } else {
            hit as f64 / manuk.0.len() as f64
        };
        let recall = if chrome.0.is_empty() {
            0.0
        } else {
            hit as f64 / chrome.0.len() as f64
        };
        println!(
            "{:<38} {:>7} {:>7} {:>7} {:>7.1}% {:>7.1}% {:>7.1}%   (dropped structural: manuk {} / chrome {})",
            url.chars().take(38).collect::<String>(),
            manuk.0.len(),
            chrome.0.len(),
            hit,
            precision * 100.0,
            recall * 100.0,
            a11y_score::f1(precision, recall) * 100.0,
            manuk.1,
            chrome.1,
        );
        if diff {
            // ⭐ The phantoms, named and ranked — precision is the binding half and a percentage
            //    cannot say what to fix.
            println!("  ── OURS IN EXCESS OF CHROME (top 25 of the multiset difference)");
            for (n, role, name) in a11y_score::excess(&manuk.0, &chrome.0).into_iter().take(25) {
                println!(
                    "     {n:>5}  {role:<16} {:?}",
                    name.chars().take(60).collect::<String>()
                );
            }
            println!("  ── CHROME IN EXCESS OF OURS (top 15)");
            for (n, role, name) in a11y_score::excess(&chrome.0, &manuk.0).into_iter().take(15) {
                println!(
                    "     {n:>5}  {role:<16} {:?}",
                    name.chars().take(60).collect::<String>()
                );
            }
        }
        pooled_hit += hit;
        pool_m.extend(manuk.0);
        pool_c.extend(chrome.0);
    }

    if !pool_m.is_empty() {
        let precision = pooled_hit as f64 / pool_m.len() as f64;
        let recall = pooled_hit as f64 / pool_c.len() as f64;
        println!(
            "{:<38} {:>7} {:>7} {:>7} {:>7.1}% {:>7.1}% {:>7.1}%",
            "TOTAL (pooled, per-site matches)",
            pool_m.len(),
            pool_c.len(),
            pooled_hit,
            precision * 100.0,
            recall * 100.0,
            a11y_score::f1(precision, recall) * 100.0,
        );
    }
    Ok(())
}
