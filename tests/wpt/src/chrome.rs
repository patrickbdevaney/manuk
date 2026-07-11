//! Reference-browser capture via **headless Chrome/Chromium** — the "parity to Chromium"
//! ground truth.
//!
//! Two signals per page:
//!
//! - **Box geometry** ([`capture_boxes`]) — the *font-agnostic, rigorous* signal. We
//!   instrument the page with a tiny probe script that reads `getBoundingClientRect()` for
//!   every element whose `id` starts with `p-`, serialize the rects to JSON, and read them
//!   back from `--dump-dom`. Comparing box positions/sizes measures layout correctness
//!   without being confused by font-rasterization differences between engines.
//! - **Screenshot** ([`capture_screenshot_png`]) — for human eyeballing side-by-side; not
//!   used for a pass/fail number, because cross-engine text anti-aliasing makes pixel-exact
//!   parity meaningless.
//!
//! Chrome is located at runtime; if none is installed the harness reports "unavailable"
//! rather than failing, so the box-geometry parity is opt-in on machines that have Chrome.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

/// A probe element's border-box, in CSS px, rounded — `[x, y, width, height]`.
pub type Box4 = [i32; 4];

/// The JS injected before capture: collect `getBoundingClientRect` for every `#p-*` element
/// into a `<pre id="__PARITY__">` the DOM dump then carries back to us. Runs synchronously at
/// end of body, after layout, so it needs no load event.
const PROBE_JS: &str = r#"<script>
(function(){var out={};
document.querySelectorAll('[id^="p-"]').forEach(function(e){
  var r=e.getBoundingClientRect();
  out[e.id]=[Math.round(r.x),Math.round(r.y),Math.round(r.width),Math.round(r.height)];
});
var pre=document.createElement('pre');pre.id='__PARITY__';
pre.textContent=JSON.stringify(out);document.documentElement.appendChild(pre);})();
</script>"#;

/// **Structural probe** (the benchmark's rigorous half). Reports `getBoundingClientRect` for
/// every element carrying an `id` — real sites have hundreds — plus its tag. This is what catches
/// what the visual score keeps missing: a MISSING element is a missing BOX, and a whole absent
/// sidebar barely moves a pixel score but is glaring here.
const PROBE_ALL_IDS_JS: &str = r#"<script>
(function(){var out={};
document.querySelectorAll('[id]').forEach(function(e){
  var r=e.getBoundingClientRect();
  if (r.width===0 && r.height===0) return;   // not rendered: don't demand Manuk render it either
  out[e.id]=[Math.round(r.x),Math.round(r.y),Math.round(r.width),Math.round(r.height)];
});
var pre=document.createElement('pre');pre.id='__PARITY__';
pre.textContent=JSON.stringify(out);document.documentElement.appendChild(pre);})();
</script>"#;

/// Capture Chrome's box for every `[id]` element of a LIVE url (structural benchmark half).
pub fn capture_boxes_all_ids(url: &str, vw: u32, vh: u32) -> Result<HashMap<String, Box4>> {
    let chrome = chrome_bin().ok_or_else(|| anyhow!("no Chrome/Chromium found"))?;
    // Inject the probe by navigating, then re-serialising the DOM with --dump-dom after the
    // script has run. Chrome evaluates page scripts before dump-dom, so we ship the probe as a
    // `javascript:`-free approach: fetch the page, append the probe, serve from a temp file with a
    // <base> so subresources still resolve to the real origin.
    let html = ureq_get(url)?;
    let base = format!("<base href=\"{url}\">");
    let doc = if let Some(i) = html.find("<head>") {
        let (a, b) = html.split_at(i + 6);
        format!("{a}{base}{b}{PROBE_ALL_IDS_JS}")
    } else {
        format!("{base}{html}{PROBE_ALL_IDS_JS}")
    };
    let tmp = std::env::temp_dir().join(format!("manuk-struct-{}.html", stable_tag(&doc)));
    std::fs::write(&tmp, &doc)?;
    let mut cmd = Command::new(&chrome);
    cmd.args(base_flags(vw, vh))
        .arg("--virtual-time-budget=6000")
        .arg("--dump-dom")
        .arg(format!("file://{}", tmp.display()));
    let out = cmd.output().context("chrome --dump-dom (structural probe)")?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        bail!("chrome --dump-dom failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    parse_probe_json(&String::from_utf8_lossy(&out.stdout))
}

/// Minimal blocking GET (the harness already links reqwest-free; use curl for zero new deps).
fn ureq_get(url: &str) -> Result<String> {
    let out = Command::new("curl")
        .args(["-sL", "--max-time", "25", "-A", "Mozilla/5.0 (X11; Linux x86_64) Manuk/0.1", url])
        .output()
        .context("curl")?;
    if !out.status.success() {
        bail!("curl failed for {url}");
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Find an installed Chrome/Chromium binary, preferring stable Chrome.
pub fn chrome_bin() -> Option<PathBuf> {
    const CANDIDATES: &[&str] = &[
        "google-chrome-stable",
        "google-chrome",
        "chromium",
        "chromium-browser",
    ];
    for name in CANDIDATES {
        if let Ok(out) = Command::new("which").arg(name).output() {
            if out.status.success() {
                let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !p.is_empty() {
                    return Some(PathBuf::from(p));
                }
            }
        }
    }
    None
}

/// Whether a reference browser is available on this machine.
pub fn available() -> bool {
    chrome_bin().is_some()
}

/// The flags every headless invocation shares. `--hide-scrollbars` matters: a visible
/// scrollbar would shrink the layout viewport and shift every box.
fn base_flags(vw: u32, vh: u32) -> Vec<String> {
    vec![
        "--headless=new".into(),
        "--disable-gpu".into(),
        "--hide-scrollbars".into(),
        "--force-device-scale-factor=1".into(),
        "--no-sandbox".into(),
        "--disable-extensions".into(),
        "--disable-lcd-text".into(),
        format!("--window-size={vw},{vh}"),
        "--virtual-time-budget=2000".into(),
    ]
}

/// Capture Chrome's box geometry for a local HTML file at the given viewport.
///
/// The original file is left untouched; we write an *instrumented* copy (original HTML +
/// probe script) to a temp file next to it and dump that, so the corpus stays clean.
pub fn capture_boxes(html: &str, vw: u32, vh: u32) -> Result<HashMap<String, Box4>> {
    let chrome = chrome_bin().ok_or_else(|| anyhow!("no Chrome/Chromium found"))?;

    // Instrument: append the probe just before </body> (or at the end).
    let instrumented = inject_probe(html);
    let tmp = std::env::temp_dir().join(format!("manuk-parity-{}.html", stable_tag(html)));
    std::fs::write(&tmp, instrumented).with_context(|| format!("writing {}", tmp.display()))?;
    let url = format!("file://{}", tmp.display());

    let mut cmd = Command::new(&chrome);
    cmd.args(base_flags(vw, vh)).arg("--dump-dom").arg(&url);
    let out = cmd.output().context("running headless Chrome --dump-dom")?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        bail!(
            "chrome --dump-dom exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let dom = String::from_utf8_lossy(&out.stdout);
    parse_probe_json(&dom)
}

/// G1 — screenshot a **live URL** in headless Chrome, so Chromium fetches the page's own CSS,
/// images and fonts exactly as it would for a user. (The file:// variant below can't do that for a
/// real site: relative subresource URLs would resolve against the temp file.)
pub fn capture_url_screenshot(url: &str, vw: u32, vh: u32, dest: &Path) -> Result<()> {
    let chrome = chrome_bin().ok_or_else(|| anyhow!("no Chrome/Chromium found"))?;
    let mut cmd = Command::new(&chrome);
    cmd.args(base_flags(vw, vh))
        .arg("--virtual-time-budget=6000") // let the page settle (webfonts, JS) before the shot
        .arg(format!("--screenshot={}", dest.display()))
        .arg(url);
    let out = cmd.output().context("running headless Chrome --screenshot <url>")?;
    if !out.status.success() {
        bail!(
            "chrome --screenshot exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// Capture a PNG screenshot of a local HTML file at the given viewport (for eyeballing).
pub fn capture_screenshot_png(html: &str, vw: u32, vh: u32, dest: &Path) -> Result<()> {
    let chrome = chrome_bin().ok_or_else(|| anyhow!("no Chrome/Chromium found"))?;
    let tmp = std::env::temp_dir().join(format!("manuk-parity-shot-{}.html", stable_tag(html)));
    std::fs::write(&tmp, html).with_context(|| format!("writing {}", tmp.display()))?;
    let url = format!("file://{}", tmp.display());

    let mut cmd = Command::new(&chrome);
    cmd.args(base_flags(vw, vh))
        .arg(format!("--screenshot={}", dest.display()))
        .arg(&url);
    let out = cmd.output().context("running headless Chrome --screenshot")?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        bail!(
            "chrome --screenshot exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// Append the probe script to a page. If there is a `</body>`, inject before it; else append.
fn inject_probe(html: &str) -> String {
    if let Some(pos) = html.rfind("</body>") {
        let mut s = String::with_capacity(html.len() + PROBE_JS.len());
        s.push_str(&html[..pos]);
        s.push_str(PROBE_JS);
        s.push_str(&html[pos..]);
        s
    } else {
        format!("{html}{PROBE_JS}")
    }
}

/// Pull the `#__PARITY__` JSON out of a dumped DOM and parse it. Reuses our own HTML parser
/// so entity-escaping in the serialization is handled correctly.
fn parse_probe_json(dumped_dom: &str) -> Result<HashMap<String, Box4>> {
    let dom = manuk_html::parse(dumped_dom);
    let mut json = None;
    for n in dom.descendants(dom.root()) {
        if let Some(el) = dom.element(n) {
            if el.id() == Some("__PARITY__") {
                json = Some(dom.text_content(n));
                break;
            }
        }
    }
    let json = json.ok_or_else(|| anyhow!("no __PARITY__ probe output in dumped DOM (did Chrome run the script?)"))?;
    let map: HashMap<String, Box4> =
        serde_json::from_str(json.trim()).with_context(|| format!("parsing probe JSON: {json}"))?;
    Ok(map)
}

/// A deterministic short tag for a temp filename (FNV-1a of the HTML) — no clock, no RNG.
fn stable_tag(html: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in html.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_probe_places_before_body_close() {
        let out = inject_probe("<body><p>hi</p></body>");
        assert!(out.contains("__PARITY__"));
        assert!(out.find("__PARITY__").unwrap() < out.rfind("</body>").unwrap());
    }

    #[test]
    fn parse_probe_json_reads_boxes() {
        let dom = r#"<html><body><pre id="__PARITY__">{"p-a":[30,0,100,40],"p-b":[0,40,60,20]}</pre></body></html>"#;
        let boxes = parse_probe_json(dom).unwrap();
        assert_eq!(boxes["p-a"], [30, 0, 100, 40]);
        assert_eq!(boxes["p-b"], [0, 40, 60, 20]);
    }
}
