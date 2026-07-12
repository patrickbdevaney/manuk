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

/// **The oracle's Chromium half.** Render an *already-fetched snapshot* and report every `[id]`
/// element's tag, computed `display`, and box.
///
/// It takes the HTML rather than a URL on purpose: the oracle must feed **one identical document**
/// to both engines. Fetching independently per engine compares two different documents and calls
/// the difference a bug — which is exactly what pinned a metric at 5,122px across four correct
/// fixes, because the live origin injected a banner the `file://` copy never saw.
pub fn oracle_probe(html: &str, base_url: &str, vw: u32, vh: u32) -> Result<HashMap<String, (String, String, [i64; 4])>> {
    let chrome = chrome_bin().ok_or_else(|| anyhow!("no Chrome/Chromium found"))?;
    let base = format!("<base href=\"{base_url}\">");
    // **Key on STRUCTURAL PATH, not on `id`.**
    //
    // The probe used to diff only elements carrying an `id`. Widening the crawl frame exposed what
    // that costs immediately: text.npr.org reported **one probed element**, because most of the web
    // does not put ids on things. Across 265 sites the oracle was about to be very nearly blind —
    // and, worse, it would have reported "no divergences" with complete confidence.
    //
    // A path (`div[0]/main[0]/p[3]`) is computable identically by both engines from the same
    // snapshot, and it names EVERY element rather than the handful an author chose to label. The
    // 6,000-element cap is a bound on probe cost, not on ambition, and it is reported so a truncated
    // page can never masquerade as a complete one.
    let probe = r#"<script>
(function(){
  var out = {};
  function pathOf(e){
    var p = [];
    while (e && e.nodeType === 1 && e.parentElement) {
      var i = 0, s = e;
      while ((s = s.previousElementSibling)) { if (s.tagName === e.tagName) i++; }
      p.unshift(e.tagName.toLowerCase() + '[' + i + ']');
      e = e.parentElement;
    }
    return p.join('/');
  }
  var all = document.querySelectorAll('*');
  var n = Math.min(all.length, 6000);
  for (var k = 0; k < n; k++) {
    var e = all[k];
    var t = e.tagName.toLowerCase();
    if (t === 'script' || t === 'style' || t === 'head' || t === 'meta' || t === 'link' ||
        t === 'base' || t === 'title' || t === 'noscript' || t === 'template' || t === 'html') continue;
    var r = e.getBoundingClientRect();
    var cs = getComputedStyle(e);
    if (r.width === 0 && r.height === 0 && cs.display !== 'none') continue;
    out[pathOf(e)] = [t, cs.display,
                 Math.round(r.x + window.scrollX), Math.round(r.y + window.scrollY),
                 Math.round(r.width), Math.round(r.height)];
  }
  // Health of the ORACLE ITSELF, not of the diff: is what Chromium rendered a real document, or a
  // bot wall / error page / no-script shell? Answered by what Chromium DREW, not by how many
  // elements happened to carry an id.
  out['__META__'] = ['', '', document.querySelectorAll('*').length,
                     (document.body ? document.body.innerText.length : 0), 0, 0];
  var pre = document.createElement('pre'); pre.id = '__ORACLE__';
  pre.textContent = JSON.stringify(out);
  document.documentElement.appendChild(pre);
})();
</script>"#;
    let doc = match html.find("<head>") {
        Some(i) => {
            let (a, b) = html.split_at(i + 6);
            format!("{a}{base}{b}{probe}")
        }
        None => format!("{base}{html}{probe}"),
    };
    let tmp = std::env::temp_dir().join(format!("manuk-oracle-{}.html", stable_tag(&doc)));
    std::fs::write(&tmp, &doc)?;
    let mut cmd = Command::new(&chrome);
    cmd.args(base_flags(vw, vh))
        .arg("--virtual-time-budget=8000")
        .arg("--dump-dom")
        .arg(format!("file://{}", tmp.display()));
    let out = cmd.output().context("chrome --dump-dom (oracle probe)")?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        bail!("chrome --dump-dom failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let dumped = String::from_utf8_lossy(&out.stdout);
    let dom = manuk_html::parse(&dumped);
    let mut json = None;
    for n in dom.descendants(dom.root()) {
        if dom.element(n).and_then(|e| e.id()) == Some("__ORACLE__") {
            json = Some(dom.text_content(n));
            break;
        }
    }
    let json = json.ok_or_else(|| anyhow!("oracle probe did not run in Chromium"))?;
    let v: serde_json::Value = serde_json::from_str(json.trim()).context("parsing oracle JSON")?;
    let mut map = HashMap::new();
    if let Some(o) = v.as_object() {
        for (id, arr) in o {
            let Some(a) = arr.as_array() else { continue };
            if a.len() < 6 {
                continue;
            }
            let tag = a[0].as_str().unwrap_or("").to_string();
            let disp = a[1].as_str().unwrap_or("").to_string();
            let rect = [
                a[2].as_i64().unwrap_or(0),
                a[3].as_i64().unwrap_or(0),
                a[4].as_i64().unwrap_or(0),
                a[5].as_i64().unwrap_or(0),
            ];
            map.insert(id.clone(), (tag, disp, rect));
        }
    }
    Ok(map)
}

/// Capture Chrome's `[id]` boxes **before and after** a scripted interaction — the G5 half.
///
/// The interaction JS runs between the two probes, in the same document. Running it in a second
/// navigation would compare two different pages and call the difference "the interaction".
pub fn capture_boxes_interaction(
    url: &str,
    vw: u32,
    vh: u32,
    steps_js: &str,
) -> Result<(HashMap<String, Box4>, HashMap<String, Box4>)> {
    let chrome = chrome_bin().ok_or_else(|| anyhow!("no Chrome/Chromium found"))?;
    let html = ureq_get(url)?;
    let base = format!("<base href=\"{url}\">");
    let probe = format!(
        r#"<script>
(function(){{
  var snap = function(){{
    var out = {{}};
    document.querySelectorAll('[id]').forEach(function(e){{
      var r = e.getBoundingClientRect();
      if (r.width === 0 && r.height === 0) return;
      // Document coordinates: a scroll must not look like every box moving.
      out[e.id] = [Math.round(r.x + window.scrollX), Math.round(r.y + window.scrollY),
                   Math.round(r.width), Math.round(r.height)];
    }});
    return out;
  }};
  var before = snap();
  try {{ {steps_js} }} catch (e) {{}}
  var after = snap();
  var pre = document.createElement('pre'); pre.id = '__G5__';
  pre.textContent = JSON.stringify({{before: before, after: after}});
  document.documentElement.appendChild(pre);
}})();
</script>"#
    );
    let doc = match html.find("<head>") {
        Some(i) => {
            let (a, b) = html.split_at(i + 6);
            format!("{a}{base}{b}{probe}")
        }
        None => format!("{base}{html}{probe}"),
    };
    let tmp = std::env::temp_dir().join(format!("manuk-g5-{}.html", stable_tag(&doc)));
    std::fs::write(&tmp, &doc)?;
    let mut cmd = Command::new(&chrome);
    cmd.args(base_flags(vw, vh))
        .arg("--virtual-time-budget=8000")
        .arg("--dump-dom")
        .arg(format!("file://{}", tmp.display()));
    let out = cmd.output().context("chrome --dump-dom (G5 probe)")?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        bail!("chrome --dump-dom failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    // Read the probe's payload back through the HTML parser — it is the thing that already knows
    // how to undo the entity escaping Chrome applied on the way out.
    let dumped = String::from_utf8_lossy(&out.stdout);
    let dom = manuk_html::parse(&dumped);
    let mut json = None;
    for n in dom.descendants(dom.root()) {
        if dom.element(n).and_then(|e| e.id()) == Some("__G5__") {
            json = Some(dom.text_content(n));
            break;
        }
    }
    let json = json.ok_or_else(|| anyhow!("G5 probe did not run (no __G5__ in the dumped DOM)"))?;
    let v: serde_json::Value =
        serde_json::from_str(json.trim()).context("parsing G5 probe JSON")?;
    let take = |k: &str| -> HashMap<String, Box4> {
        v[k].as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(id, arr)| {
                        let a = arr.as_array()?;
                        Some((
                            id.clone(),
                            [
                                a.first()?.as_i64()? as i32,
                                a.get(1)?.as_i64()? as i32,
                                a.get(2)?.as_i64()? as i32,
                                a.get(3)?.as_i64()? as i32,
                            ],
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    Ok((take("before"), take("after")))
}

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
    // Screenshot the SAME page the box probe measures: the fetched HTML, served from a temp file
    // with a `<base>` so subresources still resolve to the real origin.
    //
    // Pointing Chrome at the live URL instead looks more faithful and is in fact a trap: the two
    // Chrome captures then render *different pages*. Wikipedia's CentralNotice injects a 350px
    // fundraising banner on the real origin and not on a `file://` page, so the screenshot had a
    // banner the box probe never saw — and the visual score and the structural score were measuring
    // two different documents. One page, two probes.
    let html = ureq_get(url)?;
    let base = format!("<base href=\"{url}\">");
    let doc = match html.find("<head>") {
        Some(i) => {
            let (a, b) = html.split_at(i + 6);
            format!("{a}{base}{b}")
        }
        None => format!("{base}{html}"),
    };
    let tmp = std::env::temp_dir().join(format!("manuk-shot-{}.html", stable_tag(&doc)));
    std::fs::write(&tmp, &doc)?;
    let mut cmd = Command::new(&chrome);
    cmd.args(base_flags(vw, vh))
        .arg("--virtual-time-budget=6000") // let the page settle (webfonts, JS) before the shot
        .arg(format!("--screenshot={}", dest.display()))
        .arg(format!("file://{}", tmp.display()));
    let out = cmd.output().context("running headless Chrome --screenshot <url>")?;
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
