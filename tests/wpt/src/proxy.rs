//! # THE ONE-ORIGIN PROXY — the oracle's reference, loaded through a single real origin
//!
//! **The problem this exists for, stated once.** The fidelity oracle does not navigate Chrome at a
//! live URL: it `curl`s the document and renders a *snapshot* from `file://`, so the probe can be
//! injected and both engines can be handed the same bytes. That is sound for a document whose
//! subresources are ordinary — a `<script src>` from a foreign origin loads fine — and it is
//! **structurally fatal for a `type="module"` app**: a module script is *always* fetched in CORS
//! mode, a site has no reason to send `Access-Control-Allow-Origin` for its own bundle, so the
//! entry bundle never loads and the app never boots. The reference is then a shell of the
//! instrument's own making, and the oracle charges Chrome's missing page to us.
//!
//! t865 named the wall. t880 measured the fix with a throwaway proxy and **four of five sites
//! recovered their exact live `<div>` count**:
//!
//! ```text
//!                                      LIVE    PROXY    SNAPSHOT (what the oracle scored)
//!   pt88.app                            147      147       0
//!   booking.directferries.com             8        8       1
//!   portal.ensuretyfinance.com            8        8       0
//!   webfenix.movilidadbogota.gov.co      22       22       4
//!   allticketscol.com                   336     → 38 ←     0
//! ```
//!
//! ⚠⚠⚠ **AND THE FIFTH IS WHY THE ACCEPTANCE TEST SHIPS IN THE SAME FILE AS THE PROXY.**
//! `allticketscol.com` recovers 38 of 336 — a **half-boot**, which is the exact failure t865
//! measured and *refused* from a different fix (inlining the bundles): *"a HALF-BUILT reference is
//! worse than an honest shell — it clears the shell floor and the instrument starts charging
//! Chrome's missing half to us."* A proxy that half-boots does the same thing **silently**, and it
//! looks like progress while doing it. So the rule, and it is the load-bearing part of this module:
//!
//! > **A proxied render is usable as a reference only when it AGREES with the LIVE render.**
//! > Otherwise the row keeps its honest `oracle-module-shell` label and the denominator does not
//! > move.
//!
//! See [`renders_agree`]. The check costs one extra `--dump-dom` on a cohort of ~11 sites, it is
//! falsifiable in *both* directions, and it makes the half-boot a **detected** state rather than a
//! state the loop discovers three sweeps later while chasing "our" missing DOM.
//!
//! ## What "one origin" means here, and what it deliberately does NOT mean
//!
//! Chasing every host a page might pull from is unbounded — that is the bot-wall treadmill in a
//! different costume. This proxy covers **the site's own hosts**: the document host and anything
//! under the same registrable domain ([`is_same_site_host`]). `allticketscol.com` serving its
//! bundles from `static.allticketscol.com` is exactly that case, and it is the common one, because
//! a first-party asset host is a deployment convention rather than a third party. A genuine
//! third-party CDN is left alone: it either already sends `Access-Control-Allow-Origin` (which is
//! why it works in a real browser from any origin) or the acceptance test refuses the row.
//!
//! ## Implementation notes
//!
//! Blocking `std::net` + `curl` for the upstream leg, matching this crate's existing idiom
//! (`chrome::fetch_document` is a `curl` too — "zero new deps" is a standing choice here, and the
//! server only ever answers a single headless Chrome on loopback). One thread per connection,
//! capped; `Connection: close` on every response, so there is no keep-alive state machine.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// The path prefix under which a *related* foreign host is re-served. Chosen to be something no
/// real site routes: a collision would silently proxy the site's own page to the wrong upstream.
const FOREIGN_PREFIX: &str = "/__manuk_origin__/";

/// Never hold more than this many connection threads. Chrome opens ~6 sockets per host and every
/// host is this one, so the real number is small; the cap exists so a misbehaving page cannot turn
/// the instrument into a fork bomb.
const MAX_CONNS: usize = 64;

/// **THE ACCEPTANCE TEST — a proxied render is a reference only when it agrees with the live one.**
///
/// `live` and `proxied` are element counts from two `chromium --dump-dom` runs of the same page:
/// one navigated at its real URL, one navigated at this proxy. Agreement is **symmetric** on
/// purpose — a proxy that renders *more* than the live page is just as divergent a reference as one
/// that renders less, and only a symmetric test can say so.
///
/// The 10% band is read off t880's five measured sites rather than guessed: the four that work
/// agree **exactly** (147/147, 8/8, 8/8, 22/22) and the one that half-boots lands at 38 of 336 —
/// 11%. There is no threshold anywhere between 0.11 and 1.00 that this data can distinguish, so the
/// number is set where a *deployment* difference (an ad slot, a consent banner, a lazily-hydrated
/// widget that raced) still passes and a **half-boot cannot**.
///
/// ⚠ Two zeroes are **not** agreement. A proxy that renders nothing and a live page that renders
/// nothing agree perfectly and prove nothing — that is t650's *"100% of nothing is 100%"*, and it
/// is the shape this function would take if it were written as a bare ratio.
pub fn renders_agree(live: usize, proxied: usize) -> bool {
    if live == 0 || proxied == 0 {
        return false;
    }
    let (lo, hi) = if live < proxied {
        (live, proxied)
    } else {
        (proxied, live)
    };
    lo * 10 >= hi * 9
}

/// How much DOM a `--dump-dom` carries: open tags, counted the way both sides are counted.
///
/// Deliberately crude. The acceptance test asks *"did the app boot"*, which is a question about
/// orders of magnitude (0, 38 or 336), and a crude count is the same crude count on both sides of
/// the comparison. Anything cleverer would be a second parser to keep honest.
///
/// ⚠⚠⚠ **`<script>` CONTENT IS SKIPPED, AND THAT IS NOT TIDINESS — THE FIRST VERSION OF THIS
/// FUNCTION COUNTED THE PROBE'S OWN SOURCE AS PAGE CONTENT.** Only one of the two renders carries
/// the injected probe, so every `i<lim`, `j<toks.length` and `i<str.length` in
/// `PROBE_ALL_PATHS_JS` became an "open tag" that the LIVE page could not have. Measured on
/// `portal.ensuretyfinance.com`: the proxy rendered a strict SUPERSET of live, the extra was
/// exactly `pre×2 a×1 lim×1 script×1 str×1 toks×1` — **seven artefacts, six of them fragments of
/// the probe's JavaScript** — and the acceptance test refused a reference that was byte-for-byte
/// as complete as the live page. That is t780-783's law aimed one layer up: *the probe's own
/// sentinel widened its subject*, and an instrument that measures itself reports its own presence
/// as the site's absence. A `<` inside a script is never a tag on either side, so skipping the
/// content is the SYMMETRIC fix, not a special case for our probe.
pub fn count_open_tags(dom: &str) -> usize {
    let b = dom.as_bytes();
    let lower = dom.to_ascii_lowercase();
    let mut n = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        // Raw-text elements: their content is DATA, not markup, on both sides of the comparison.
        for (open, close) in [("<script", "</script"), ("<style", "</style")] {
            if lower[i..].starts_with(open) {
                n += 1;
                i = lower[i..]
                    .find(close)
                    .map(|d| i + d + close.len())
                    .unwrap_or(b.len());
            }
        }
        if i >= b.len() {
            break;
        }
        if b[i] == b'<' && b.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic()) {
            // The probe's own sentinel is not the page's content — the probe skips it too.
            if !lower[i..].starts_with("<pre id=\"__parity__\"") {
                n += 1;
            }
        }
        i += 1;
    }
    n
}

/// **What the two renders disagree about, by tag name** — the evidence a refusal must carry.
///
/// Returns `(only-in-a, only-in-b)` as `tag×n` summaries, most-different first. A bare "38 against
/// 336" says a proxy half-booted; this says *which* half, which is the difference between the next
/// tick knowing where to look and guessing.
pub fn tag_delta(a: &str, b: &str) -> (String, String) {
    fn hist(dom: &str) -> std::collections::BTreeMap<String, i64> {
        let mut m = std::collections::BTreeMap::new();
        let bytes = dom.as_bytes();
        let mut i = 0;
        while i + 1 < bytes.len() {
            if bytes[i] == b'<' && bytes[i + 1].is_ascii_alphabetic() {
                let s = i + 1;
                let mut j = s;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
                    j += 1;
                }
                *m.entry(dom[s..j].to_ascii_lowercase()).or_insert(0) += 1;
                i = j;
                continue;
            }
            i += 1;
        }
        m
    }
    let (ha, hb) = (hist(a), hist(b));
    let mut only_a: Vec<(String, i64)> = Vec::new();
    let mut only_b: Vec<(String, i64)> = Vec::new();
    for k in ha.keys().chain(hb.keys()).collect::<BTreeSet<_>>() {
        let d = ha.get(k).copied().unwrap_or(0) - hb.get(k).copied().unwrap_or(0);
        if d > 0 {
            only_a.push((k.clone(), d));
        } else if d < 0 {
            only_b.push((k.clone(), -d));
        }
    }
    let fmt = |mut v: Vec<(String, i64)>| -> String {
        v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        if v.is_empty() {
            "—".to_string()
        } else {
            v.iter()
                .take(12)
                .map(|(t, n)| format!("{t}×{n}"))
                .collect::<Vec<_>>()
                .join(" ")
        }
    };
    (fmt(only_a), fmt(only_b))
}

/// Split a URL into `(scheme, host[:port], path-with-query)`. `None` for anything not http(s).
pub fn split_url(url: &str) -> Option<(&str, &str, &str)> {
    let (scheme, rest) = if let Some(r) = url.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = url.strip_prefix("http://") {
        ("http", r)
    } else {
        return None;
    };
    let cut = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (host, path) = rest.split_at(cut);
    if host.is_empty() {
        return None;
    }
    Some((scheme, host, if path.is_empty() { "/" } else { path }))
}

/// The registrable-ish suffix of a host: the last two labels, or the last three when the last two
/// look like a country-code second-level domain (`co.uk`, `com.au`, `gov.co`, …).
///
/// ⚠ **This is a heuristic and it is bounded on the safe side.** Getting it wrong *narrow* costs a
/// site the proxy (the acceptance test then refuses the row and the honest label stands); getting
/// it wrong *wide* would proxy an unrelated origin, which is the failure that produces a subtly
/// wrong reference rather than an obviously broken one. There is no Public Suffix List in this
/// crate and adding one for an instrument's host test is not the trade.
pub fn registrable_suffix(host: &str) -> String {
    let host = host.split(':').next().unwrap_or(host);
    let labels: Vec<&str> = host.split('.').filter(|s| !s.is_empty()).collect();
    if labels.len() <= 2 {
        return labels.join(".");
    }
    let last = labels[labels.len() - 1];
    let second = labels[labels.len() - 2];
    const SLD: [&str; 8] = ["co", "com", "net", "org", "gov", "edu", "ac", "or"];
    if last.len() <= 3 && SLD.contains(&second) && labels.len() >= 3 {
        labels[labels.len() - 3..].join(".")
    } else {
        labels[labels.len() - 2..].join(".")
    }
}

/// Is `host` one of the SITE's own hosts — same registrable domain as `site_host`?
pub fn is_same_site_host(host: &str, site_host: &str) -> bool {
    let h = host.split(':').next().unwrap_or(host);
    let s = site_host.split(':').next().unwrap_or(site_host);
    if h.eq_ignore_ascii_case(s) {
        return true;
    }
    let suffix = registrable_suffix(s);
    if suffix.is_empty() || !suffix.contains('.') {
        return false;
    }
    let hl = h.to_ascii_lowercase();
    hl == suffix || hl.ends_with(&format!(".{suffix}"))
}

/// Every host the document names that belongs to the SITE — the set this proxy will re-serve.
pub fn same_site_hosts(html: &str, site_host: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let lower_site = site_host.split(':').next().unwrap_or(site_host).to_string();
    out.insert(lower_site.to_ascii_lowercase());
    let b = html.as_bytes();
    let mut i = 0;
    while i + 2 < b.len() {
        // Both `scheme://host` and protocol-relative `//host` start here.
        if b[i] == b'/' && b[i + 1] == b'/' {
            let start = i + 2;
            let mut j = start;
            while j < b.len()
                && !matches!(
                    b[j],
                    b'/' | b'?'
                        | b'#'
                        | b'"'
                        | b'\''
                        | b'\\'
                        | b')'
                        | b' '
                        | b'\t'
                        | b'\n'
                        | b'\r'
                        | b'<'
                        | b'>'
                        | b','
                )
            {
                j += 1;
            }
            if j > start {
                let host = &html[start..j];
                if host.contains('.') && is_same_site_host(host, site_host) {
                    out.insert(host.to_ascii_lowercase());
                }
            }
            i = j.max(i + 2);
            continue;
        }
        i += 1;
    }
    out
}

/// **Point every SAME-SITE absolute URL in the document at the proxy.** Foreign hosts are left
/// exactly as written — see the module header for why that is a decision and not an omission.
///
/// The DOCUMENT's own host maps to the proxy root, so the author's paths stay the author's paths.
/// The site's *other* hosts (`static.example.com`) map under [`FOREIGN_PREFIX`], which carries the
/// upstream scheme and host in the path — because collapsing two hosts onto one root would send
/// `static.example.com/main.js` to `example.com/main.js`, and a proxy that answers the wrong
/// upstream produces a *subtly* wrong reference rather than an obviously broken one. That is the
/// failure mode this module is built to refuse.
///
/// Also removes `<meta http-equiv="content-security-policy">`: the served document's origin is
/// `http://127.0.0.1:PORT`, so a policy naming the site's own hosts would refuse every rewritten
/// URL *and* the injected probe. (t865 measured the probe being refused for want of a CSP nonce and
/// the row printing `—`, which is the same failure one layer up.) The response headers this proxy
/// writes carry no CSP either.
pub fn rewrite_document(
    html: &str,
    site_scheme: &str,
    site_host: &str,
    proxy_root: &str,
) -> String {
    let own = site_host
        .split(':')
        .next()
        .unwrap_or(site_host)
        .to_ascii_lowercase();
    let mut hosts: Vec<String> = same_site_hosts(html, site_host).into_iter().collect();
    // Longest first: a shorter host can never be a `//`-prefixed substring of a longer one, but
    // ordering by length keeps that true by construction rather than by argument.
    hosts.sort_by_key(|h| std::cmp::Reverse(h.len()));
    let mut doc = strip_csp_meta(html);
    for h in &hosts {
        let (to_https, to_http, to_rel) = if *h == own {
            (
                proxy_root.to_string(),
                proxy_root.to_string(),
                proxy_root.to_string(),
            )
        } else {
            (
                format!("{proxy_root}{FOREIGN_PREFIX}https/{h}"),
                format!("{proxy_root}{FOREIGN_PREFIX}http/{h}"),
                format!("{proxy_root}{FOREIGN_PREFIX}{site_scheme}/{h}"),
            )
        };
        // Scheme'd first — otherwise the `//host` pass would eat the `//host` inside `https://host`
        // and leave a dangling `https:` behind.
        doc = doc.replace(&format!("https://{h}"), &to_https);
        doc = doc.replace(&format!("http://{h}"), &to_http);
        doc = doc.replace(&format!("//{h}"), &to_rel);
    }
    doc
}

/// Remove any `<meta http-equiv="content-security-policy" ...>` element from a document.
pub fn strip_csp_meta(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    let mut scan = 0;
    while let Some(rel) = lower[scan..].find("<meta") {
        let start = scan + rel;
        let Some(end_rel) = lower[start..].find('>') else {
            break;
        };
        let end = start + end_rel + 1;
        if lower[start..end].contains("content-security-policy") {
            out.push_str(&html[cursor..start]);
            cursor = end;
        }
        scan = end;
    }
    out.push_str(&html[cursor..]);
    out
}

/// A bound-but-not-yet-serving proxy. Two steps because the document body has to name the port and
/// the port is only known after the bind.
pub struct Bound {
    listener: TcpListener,
    port: u16,
    scheme: String,
    host: String,
    doc_path: String,
}

impl Bound {
    /// Bind a loopback socket for `site_url`. `None` if the URL is not http(s) or the bind fails.
    pub fn bind(site_url: &str) -> Option<Bound> {
        let (scheme, host, path) = split_url(site_url)?;
        let listener = TcpListener::bind("127.0.0.1:0").ok()?;
        let port = listener.local_addr().ok()?.port();
        Some(Bound {
            listener,
            port,
            scheme: scheme.to_string(),
            host: host.to_string(),
            doc_path: path.to_string(),
        })
    }

    /// `http://127.0.0.1:PORT` — the one origin, with no trailing slash.
    pub fn root(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The site's own host, for [`rewrite_document`].
    pub fn site_host(&self) -> &str {
        &self.host
    }

    /// Start serving `document` at the site's own path under this origin.
    pub fn serve(self, document: String) -> OneOrigin {
        let stop = Arc::new(AtomicBool::new(false));
        let doc_url = format!("{}{}", self.root(), self.doc_path);
        let upstream = format!("{}://{}", self.scheme, self.host);
        let doc_path = self.doc_path.clone();
        let listener = self.listener;
        let _ = listener.set_nonblocking(true);
        let stop_t = Arc::clone(&stop);
        let live = Arc::new(AtomicUsize::new(0));
        let live_t = Arc::clone(&live);
        let doc = Arc::new(document);
        let join = std::thread::spawn(move || {
            while !stop_t.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((sock, _)) => {
                        if live_t.load(Ordering::Relaxed) >= MAX_CONNS {
                            continue;
                        }
                        live_t.fetch_add(1, Ordering::Relaxed);
                        let doc = Arc::clone(&doc);
                        let doc_path = doc_path.clone();
                        let upstream = upstream.clone();
                        let live_c = Arc::clone(&live_t);
                        std::thread::spawn(move || {
                            let _ = serve_one(sock, &doc, &doc_path, &upstream);
                            live_c.fetch_sub(1, Ordering::Relaxed);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        OneOrigin {
            doc_url,
            stop,
            join: Some(join),
        }
    }
}

/// A running one-origin proxy. Stops on drop.
pub struct OneOrigin {
    doc_url: String,
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl OneOrigin {
    /// The URL to navigate Chrome at.
    pub fn document_url(&self) -> &str {
        &self.doc_url
    }
}

impl Drop for OneOrigin {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Answer one request. Errors are swallowed into a 502 — a proxy that panics takes the accept loop
/// with it and the sweep would read that as the *site* failing.
fn serve_one(
    mut sock: TcpStream,
    document: &str,
    doc_path: &str,
    upstream: &str,
) -> std::io::Result<()> {
    let _ = sock.set_read_timeout(Some(std::time::Duration::from_secs(20)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    // Headers only: everything this proxy forwards is a GET, and a body it never reads cannot
    // stall the loop.
    loop {
        let n = sock.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 64 * 1024 {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let Some(line) = head.lines().next() else {
        return write_response(&mut sock, 400, "text/plain", b"bad request");
    };
    let mut parts = line.split_whitespace();
    let _method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");

    if path == doc_path || (doc_path == "/" && path == "/") {
        return write_response(
            &mut sock,
            200,
            "text/html; charset=utf-8",
            document.as_bytes(),
        );
    }

    let target = if let Some(rest) = path.strip_prefix(FOREIGN_PREFIX) {
        // `/__manuk_origin__/<scheme>/<host>/<rest>`
        let mut it = rest.splitn(3, '/');
        match (it.next(), it.next(), it.next()) {
            (Some(s), Some(h), tail) if !s.is_empty() && !h.is_empty() => {
                format!("{s}://{h}/{}", tail.unwrap_or(""))
            }
            _ => return write_response(&mut sock, 404, "text/plain", b"bad proxy path"),
        }
    } else {
        format!("{upstream}{path}")
    };

    match fetch_upstream(&target) {
        Some((status, ctype, body)) => write_response(&mut sock, status, &ctype, &body),
        None => write_response(&mut sock, 502, "text/plain", b"upstream failed"),
    }
}

/// Pull one upstream resource. Returns `(status, content-type, body)`.
///
/// No `--compressed`: without an `Accept-Encoding` the upstream answers identity, so the bytes
/// forwarded need no `Content-Encoding` header and cannot disagree with the `Content-Length` this
/// proxy writes. Dropping a header is cheap; forwarding one that no longer describes the body is
/// the kind of "subtly wrong reference" this whole module is built to avoid.
fn fetch_upstream(url: &str) -> Option<(u16, String, Vec<u8>)> {
    let tag = format!(
        "{:x}",
        url.bytes().fold(0x811c9dc5u32, |h, c| (h ^ c as u32)
            .wrapping_mul(0x01000193))
    );
    let body_path = std::env::temp_dir().join(format!("manuk-proxy-{tag}.body"));
    let out = std::process::Command::new("curl")
        .args([
            "-sL",
            "--max-time",
            "20",
            "-A",
            "Mozilla/5.0 (X11; Linux x86_64) Manuk/0.1",
            "-o",
            &body_path.to_string_lossy(),
            "-w",
            "%{http_code}\t%{content_type}",
            url,
        ])
        .output()
        .ok()?;
    let meta = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let mut it = meta.split('\t');
    let status: u16 = it.next().unwrap_or("0").parse().unwrap_or(0);
    let ctype = it.next().unwrap_or("").trim().to_string();
    let body = std::fs::read(&body_path).unwrap_or_default();
    let _ = std::fs::remove_file(&body_path);
    if !out.status.success() {
        return None;
    }
    let ctype = if ctype.is_empty() {
        "application/octet-stream".to_string()
    } else {
        ctype
    };
    Some((if status == 0 { 502 } else { status }, ctype, body))
}

fn write_response(
    sock: &mut TcpStream,
    status: u16,
    ctype: &str,
    body: &[u8],
) -> std::io::Result<()> {
    // `Access-Control-Allow-Origin: *` on EVERY response is the point of the whole module: it is
    // what makes the site's own hosts loadable as modules from this origin. No CSP is written, and
    // none is forwarded — see `rewrite_document`.
    let head = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    sock.write_all(head.as_bytes())?;
    sock.write_all(body)?;
    sock.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚠⚠⚠ **THE GATE THE WHOLE MODULE EXISTS BEHIND.** t880's five sites, as the assertion t865's
    /// refusal earned: the four that recover are accepted, and the **half-boot is refused**.
    ///
    /// **Proven red:** widen the band to `lo * 10 >= hi` and `allticketscol` (38 against 336) is
    /// accepted — which is precisely the reference the loop must never score against, because it
    /// clears the shell floor and then charges Chrome's missing 298 elements to us.
    #[test]
    fn a_proxied_render_is_a_reference_only_when_it_agrees_with_the_live_one() {
        // The four that worked, exactly as measured at t880.
        assert!(renders_agree(147, 147), "pt88.app");
        assert!(renders_agree(8, 8), "booking.directferries.com");
        assert!(renders_agree(8, 8), "portal.ensuretyfinance.com");
        assert!(renders_agree(22, 22), "webfenix.movilidadbogota.gov.co");
        // The half-boot. THIS is the one the acceptance test exists for.
        assert!(
            !renders_agree(336, 38),
            "allticketscol.com half-boots (38 of 336) and a half-built reference is strictly \
             WORSE than an honest shell"
        );
        // Symmetric: a proxy that renders MORE than live is just as divergent a reference.
        assert!(!renders_agree(38, 336), "the test must be symmetric");
        // A deployment difference still passes.
        assert!(renders_agree(300, 290));
        assert!(!renders_agree(300, 200));
        // Two zeroes are not agreement (t650: 100% of nothing is 100%).
        assert!(!renders_agree(0, 0));
        assert!(!renders_agree(0, 12));
        assert!(!renders_agree(12, 0));
    }

    /// The site's own hosts are re-served; a third party is left alone. The `allticketscol` shape —
    /// bundles on `static.<site>` — is the case that made the naive proxy half-boot.
    #[test]
    fn the_sites_own_hosts_are_one_origin_and_a_third_party_is_not() {
        assert!(is_same_site_host("allticketscol.com", "allticketscol.com"));
        assert!(is_same_site_host(
            "static.allticketscol.com",
            "allticketscol.com"
        ));
        assert!(is_same_site_host(
            "www.allticketscol.com",
            "allticketscol.com"
        ));
        assert!(!is_same_site_host("cdn.jsdelivr.net", "allticketscol.com"));
        assert!(!is_same_site_host(
            "www.googletagmanager.com",
            "allticketscol.com"
        ));
        // ⚠ The ccTLD guard: without it the registrable suffix of a `gov.co` host is `gov.co` and
        // EVERY Colombian government domain becomes "the same site" — the wide failure that
        // produces a subtly wrong reference.
        assert_eq!(
            registrable_suffix("webfenix.movilidadbogota.gov.co"),
            "movilidadbogota.gov.co"
        );
        assert!(!is_same_site_host(
            "otra.entidad.gov.co",
            "webfenix.movilidadbogota.gov.co"
        ));
        assert!(is_same_site_host(
            "cdn.movilidadbogota.gov.co",
            "webfenix.movilidadbogota.gov.co"
        ));
    }

    /// Rewriting points the site's own absolute URLs — scheme'd and protocol-relative — at the
    /// proxy, and leaves a foreign host and an XML namespace **untouched**.
    #[test]
    fn rewriting_moves_the_sites_own_urls_and_nothing_else() {
        let html = r#"<html><head>
<script type="module" src="https://static.example.com/main.js"></script>
<script src="//www.example.com/legacy.js"></script>
<script src="https://cdn.jsdelivr.net/x.js"></script>
<link href="http://example.com/app.css">
<svg xmlns="http://www.w3.org/2000/svg"></svg>
</head></html>"#;
        let out = rewrite_document(html, "https", "example.com", "http://127.0.0.1:9");
        // ⚠⚠⚠ THE ONE THE NAIVE PROXY GOT WRONG: `static.example.com` is the SITE's host and must
        // be re-served — but under its OWN name. Collapsing it onto the root would fetch
        // `example.com/main.js`, which is a 404 or, worse, a DIFFERENT file.
        assert!(
            out.contains("http://127.0.0.1:9/__manuk_origin__/https/static.example.com/main.js"),
            "{out}"
        );
        assert!(
            out.contains("http://127.0.0.1:9/__manuk_origin__/https/www.example.com/legacy.js"),
            "{out}"
        );
        assert!(out.contains("http://127.0.0.1:9/app.css"), "{out}");
        // ⚠ THE ONE THAT MUST NOT MOVE: rewriting the SVG namespace would make every inline icon
        // an unknown element, which is a whole-page rendering change dressed as a URL fix.
        assert!(out.contains("http://www.w3.org/2000/svg"), "{out}");
        assert!(out.contains("https://cdn.jsdelivr.net/x.js"), "{out}");
    }

    /// A `<meta>` CSP is removed, and nothing else is.
    #[test]
    fn a_meta_csp_is_removed_and_the_rest_of_the_head_survives() {
        let html = "<head><meta charset=\"utf-8\">\
                    <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'\">\
                    <title>t</title></head>";
        let out = strip_csp_meta(html);
        assert!(!out.to_ascii_lowercase().contains("content-security-policy"));
        assert!(out.contains("charset=\"utf-8\""));
        assert!(out.contains("<title>t</title>"));
    }

    /// ⚠⚠⚠ **THE INSTRUMENT MUST NOT COUNT ITSELF.** Only the proxied render carries the injected
    /// probe, so a counter that reads `<` inside a script sees the probe's own `i<lim` /
    /// `j<toks.length` as page content the live render "lost". Measured on
    /// `portal.ensuretyfinance.com`: proxy 55 against live 48, and the whole seven-tag difference
    /// was `pre×2 a×1 lim×1 script×1 str×1 toks×1` — the probe, reported as the site's absence.
    ///
    /// **Proven red:** delete the `<script`/`<style` skip and the last assertion counts 5 instead
    /// of 1, which is exactly the false refusal this found.
    #[test]
    fn open_tags_are_counted_the_same_way_on_both_sides() {
        assert_eq!(count_open_tags("<div><span></span></div>"), 2);
        assert_eq!(count_open_tags("a < b and 1<2"), 0);
        assert_eq!(count_open_tags(""), 0);
        // A script's CONTENT is data on both sides. One `<script>` tag, and nothing from its body.
        assert_eq!(
            count_open_tags(
                "<script>for(var i=0;i<lim;i++){if(j<toks.length&&k<str.length&&m<a.length){}}</script>"
            ),
            1,
            "the probe's own comparisons are not elements of the page"
        );
        // The probe's sentinel is not the page's content either.
        assert_eq!(
            count_open_tags("<div></div><pre id=\"__PARITY__\" style=\"display: none;\">{}</pre>"),
            1
        );
    }

    #[test]
    fn a_url_splits_into_scheme_host_and_path() {
        assert_eq!(
            split_url("https://pt88.app/x?y=1"),
            Some(("https", "pt88.app", "/x?y=1"))
        );
        assert_eq!(
            split_url("https://pt88.app"),
            Some(("https", "pt88.app", "/"))
        );
        assert_eq!(split_url("file:///tmp/a.html"), None);
    }

    /// End-to-end on loopback with no network: the proxy serves the document at the site's own
    /// path under its own origin.
    #[test]
    fn the_proxy_serves_the_document_at_the_sites_own_path() {
        let bound = Bound::bind("https://example.com/app/index.html").expect("bind");
        let root = bound.root();
        let proxy = bound.serve("<html><body>hello</body></html>".into());
        assert_eq!(
            proxy.document_url(),
            format!("{root}/app/index.html"),
            "the document keeps its own path, so relative URLs resolve as the author wrote them"
        );
        let mut s = TcpStream::connect(format!("127.0.0.1:{}", root.rsplit(':').next().unwrap()))
            .expect("connect");
        s.write_all(b"GET /app/index.html HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut got = String::new();
        let _ = s.read_to_string(&mut got);
        assert!(got.contains("hello"), "{got}");
        assert!(
            got.contains("Access-Control-Allow-Origin: *"),
            "every response must be CORS-open — that is the entire point: {got}"
        );
    }
}
