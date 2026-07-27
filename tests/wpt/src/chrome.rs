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

use crate::fidelity::Unmeasurable;

/// A probe element's border-box, in CSS px, rounded — `[x, y, width, height]`.
pub type Box4 = [i32; 4];

/// The JS injected before capture: collect `getBoundingClientRect` for every `#p-*` element
/// into a `<pre id="__PARITY__">` the DOM dump then carries back to us. Runs synchronously at
/// end of body, after layout, so it needs no load event.
///
/// **This one is deliberately NOT deferred, and the exception is named rather than inherited.** It
/// probes the committed `docs/bench/` parity fixtures — static local HTML with no scripts — where the
/// DOM at end-of-parse provably *is* the final DOM, so [`PROBE_DEFER_TAIL`] would buy nothing and
/// would put a 72/72 green gate at risk for it. The sentence above is true here. It was **false** for
/// the two live-site probes below, which inherited it, and that cost the certificate two rows
/// (tick 674).
const PROBE_JS: &str = r#"<script>
(function(){var out={};
document.querySelectorAll('[id^="p-"]').forEach(function(e){
  var r=e.getBoundingClientRect();
  out[e.id]=[Math.round(r.x),Math.round(r.y),Math.round(r.width),Math.round(r.height)];
});
var pre=document.createElement('pre');pre.id='__PARITY__';
pre.textContent=JSON.stringify(out);document.documentElement.appendChild(pre);})();
</script>"#;

/// **The shared tail that makes a live-site probe wait for the page to exist.**
///
/// Every probe below defines `capture()` and `emit()`; this drives them. It is one string shared by
/// both live-site probes because *"one rule, N implementations"* is how this project loses a fix —
/// the deferral has to be a single definition, not a pattern two constants happen to follow.
///
/// **The measurement it exists for (tick 674).** The probes ran synchronously at end-of-parse, so
/// they reported the DOM *before* DOMContentLoaded — before any deferred script, module, or
/// hydration. On a JS-rendered site that is the shell:
///
/// ```text
///                       PARSE     DCL     LOAD   T+2000   T+5000
///   comix.to                3       4        5        6        7
///   www.naukri.com          4      37       59       60       61
///   www.welt.de          3199    3200     3177     3201     3176
/// ```
///
/// `naukri` gains **15×** and crosses the certificate's sample floor; `welt.de` — server-rendered,
/// and one of the five rows currently carrying the certificate — does not move. That asymmetry is
/// the whole case: the deferral converts the unscoreable population and leaves the scored one alone.
///
/// **Monotone by construction.** `capture()` runs at parse FIRST and each later event overwrites the
/// same `<pre>`, so a page whose `load` never fires still emits exactly what it emits today. The
/// probe can get better and cannot get worse — a `__PARITY__` that went missing would read as
/// [`Unmeasurable::ProbeBlocked`] and silently cost a row.
///
/// The `setTimeout` is belt-and-braces for a page that never fires `load` (a hanging subresource);
/// under `--virtual-time-budget` its 3s costs no real time.
///
/// A macro rather than a `const` only because `concat!` takes literals; the point is that there is
/// exactly ONE copy of this text and both probes paste it.
macro_rules! probe_defer_tail {
    () => {
        r#"
capture();
if(document.readyState!=='complete'){
  window.addEventListener('DOMContentLoaded',capture,false);
  window.addEventListener('load',capture,false);
}
setTimeout(capture,3000);
})();
</script>"#
    };
}

/// **Structural probe** (the benchmark's rigorous half). Reports `getBoundingClientRect` for
/// every element carrying an `id` — real sites have hundreds — plus its tag. This is what catches
/// what the visual score keeps missing: a MISSING element is a missing BOX, and a whole absent
/// sidebar barely moves a pixel score but is glaring here.
const PROBE_ALL_IDS_JS: &str = concat!(
    r#"<script>
(function(){
var pre=null;
function emit(o){
  if(!pre){pre=document.createElement('pre');pre.id='__PARITY__';document.documentElement.appendChild(pre);}
  pre.textContent=JSON.stringify(o);
}
function capture(){var out={};
document.querySelectorAll('[id]').forEach(function(e){
  if(e.id==='__PARITY__')return;             // the probe must not measure its own sentinel
  var r=e.getBoundingClientRect();
  if (r.width===0 && r.height===0) return;   // not rendered: don't demand Manuk render it either
  out[e.id]=[Math.round(r.x),Math.round(r.y),Math.round(r.width),Math.round(r.height)];
});
emit(out);}"#,
    probe_defer_tail!()
);

/// **Structural probe, selector-path keyed** (the fidelity redesign §3a producer). Keys every element
/// by its selector-PATH (`tag.SIG:nth-child(n)/…` from the root) instead of its `id`, so
/// `fidelity::shape_stats` has real `/`-ancestry to subtract a constant page offset against. Modern
/// React/Tailwind pages barely use ids (39% of the corpus was unmeasurable on `[id]` keys); a path is
/// present on every element.
///
/// Since tick 537 (brick 4b) each entry is a **`Seen`-shaped tuple** `[tag, display, x, y, w, h]`, and since tick 563 a
/// 7th slot `"<family>/<px>"` carrying the COMPUTED FONT that produced the box
/// — the SAME shape as the differential `oracle_probe` — not the bare 4-tuple box it emitted before.
/// The extra tag+display let the G1 fidelity probe carry `oracle::Seen` maps and call the four jarring
/// invariants (`jarring_h_overflow`/`jarring_overlap`/`jarring_reading_order`/`jarring_collapsed_target`)
/// DIRECTLY, instead of through Box4 mirror cores that had to re-derive the tag from the key. One
/// definition of every invariant, shared by the oracle and the exit gate.
///
/// The `fnv`/`sigOf`/`pathOf` here are a BYTE-IDENTICAL contract with the oracle probe's `pathOf`
/// (above) and with `path_of`/`sig_of` in main.rs: fnv-1a over UTF-16 code units (`charCodeAt`), the
/// same `[ \t\n\f\r]+` whitespace split, the same ASCII-lowercase + sort + dedup of the class list,
/// and — the easy-to-get-wrong part — an element whose parent is not an element (i.e. `<html>`)
/// contributes NO component, because `e.parentElement` is null there. A path built two different
/// ways is two different keys, and the diff would then compare strangers.
const PROBE_ALL_PATHS_JS: &str = concat!(
    r#"<script>
(function(){
var pre=null;
function emit(o){
  if(!pre){pre=document.createElement('pre');pre.id='__PARITY__';document.documentElement.appendChild(pre);}
  pre.textContent=JSON.stringify(o);
}
function fnv(str){var h=0x811c9dc5;for(var i=0;i<str.length;i++){h^=str.charCodeAt(i);h=Math.imul(h,0x01000193)>>>0;}return h>>>0;}
function sigOf(e){var cls=e.getAttribute('class');if(!cls)return '';var toks=cls.split(/[ \t\n\f\r]+/),a=[];for(var i=0;i<toks.length;i++){if(toks[i])a.push(toks[i].replace(/[A-Z]/g,function(c){return c.toLowerCase();}));}if(!a.length)return '';a.sort();var u=[];for(var j=0;j<a.length;j++){if(j===0||a[j]!==a[j-1])u.push(a[j]);}return '.'+('0000000'+fnv(u.join('.')).toString(16)).slice(-8);}
function pathOf(e){var p=[];while(e&&e.nodeType===1&&e.parentElement){var i=1,s=e;while((s=s.previousElementSibling))i++;p.unshift(e.tagName.toLowerCase()+sigOf(e)+':nth-child('+i+')');e=e.parentElement;}return p.join('/');}
function capture(){var out={};
var all=document.querySelectorAll('*');
var lim=Math.min(all.length,6000);
for(var k=0;k<lim;k++){var e=all[k];var t=e.tagName.toLowerCase();
  if(t==='script'||t==='style'||t==='head'||t==='meta'||t==='link'||t==='base'||t==='title'||t==='noscript'||t==='template'||t==='html')continue;
  if(e.id==='__PARITY__')continue;         // the probe must not measure its own sentinel
  var r=e.getBoundingClientRect();
  if(r.width===0&&r.height===0)continue;   // not rendered: don't demand Manuk render it either
  var cs0=getComputedStyle(e);
  // The COMPUTED FONT that produced this box: first declared family (unquoted) + used px size. A rect
  // cannot say which face or size made it, and by t562 every remaining text-metric lead was blocked on
  // exactly that — `[74x16] vs [76x18]` is unattributable without it (t563).
  var fam0=(cs0.fontFamily||'').split(',')[0].trim().replace(/^["']|["']$/g,'');
  var px0=Math.round(parseFloat(cs0.fontSize)||0);
  out[pathOf(e)]=[t,cs0.display,Math.round(r.x+window.scrollX),Math.round(r.y+window.scrollY),Math.round(r.width),Math.round(r.height),fam0+'/'+px0];}
emit(out);}"#,
    probe_defer_tail!()
);

/// **The oracle's Chromium half.** Render an *already-fetched snapshot* and report every `[id]`
/// element's tag, computed `display`, and box.
///
/// It takes the HTML rather than a URL on purpose: the oracle must feed **one identical document**
/// to both engines. Fetching independently per engine compares two different documents and calls
/// the difference a bug — which is exactly what pinned a metric at 5,122px across four correct
/// fixes, because the live origin injected a banner the `file://` copy never saw.
pub fn oracle_probe(
    html: &str,
    base_url: &str,
    vw: u32,
    vh: u32,
) -> Result<HashMap<String, (String, String, [i64; 4])>> {
    let chrome = chrome_bin().ok_or_else(|| anyhow!("no Chrome/Chromium found"))?;
    let base = format!("<base href=\"{base_url}\">");
    // **Key on STRUCTURAL PATH, not on `id`.**
    //
    // The probe used to diff only elements carrying an `id`. Widening the crawl frame exposed what
    // that costs immediately: text.npr.org reported **one probed element**, because most of the web
    // does not put ids on things. Across 265 sites the oracle was about to be very nearly blind —
    // and, worse, it would have reported "no divergences" with complete confidence.
    //
    // A path (`div.a1b2c3d4:nth-child(1)/main:nth-child(2)/p:nth-child(4)`) is computable
    // identically by both engines from the same
    // snapshot, and it names EVERY element rather than the handful an author chose to label. The
    // 6,000-element cap is a bound on probe cost, not on ambition, and it is reported so a truncated
    // page can never masquerade as a complete one.
    let probe = r#"<script>
(function(){
  var out = {};
  // Selector-path keying (tick 399 spec): `tag.SIG:nth-child(N)`. N counts ALL element
  // siblings (1-based); SIG is fnv1a-32 over the ASCII-lowercased, SORTED, deduped class
  // list joined with '.'. Sorted so framework class-shuffling keeps identity; hashed so a
  // 40-class Tailwind string cannot bloat the key (or smuggle a '/' into the path). An
  // element whose class list differs from its positional counterpart FAILS the lookup and
  // books as tree drift — instead of minting a phantom style diff between two strangers.
  // This function and Rust's `sig_of`/`path_of` (main.rs) are a byte-identical contract:
  // fnv over UTF-16 code units there too (encode_utf16), same whitespace split, same sort.
  function fnv(str){
    var h = 0x811c9dc5;
    for (var i = 0; i < str.length; i++) {
      h ^= str.charCodeAt(i);
      h = Math.imul(h, 0x01000193) >>> 0;
    }
    return h >>> 0;
  }
  function sigOf(e){
    var cls = e.getAttribute('class');
    if (!cls) return '';
    var toks = cls.split(/[ \t\n\f\r]+/), a = [];
    for (var i = 0; i < toks.length; i++) {
      if (toks[i]) a.push(toks[i].replace(/[A-Z]/g, function(c){ return c.toLowerCase(); }));
    }
    if (!a.length) return '';
    a.sort();
    var u = [];
    for (var j = 0; j < a.length; j++) { if (j === 0 || a[j] !== a[j-1]) u.push(a[j]); }
    return '.' + ('0000000' + fnv(u.join('.')).toString(16)).slice(-8);
  }
  function pathOf(e){
    var p = [];
    while (e && e.nodeType === 1 && e.parentElement) {
      var i = 1, s = e;
      while ((s = s.previousElementSibling)) i++;
      p.unshift(e.tagName.toLowerCase() + sigOf(e) + ':nth-child(' + i + ')');
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
        bail!(
            "chrome --dump-dom failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
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
        bail!(
            "chrome --dump-dom failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
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
    let out = cmd
        .output()
        .context("chrome --dump-dom (structural probe)")?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        bail!(
            "chrome --dump-dom failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    parse_probe_json(&String::from_utf8_lossy(&out.stdout))
}

/// Capture Chrome's `Seen` (tag + display + box) for every rendered element of a LIVE url, keyed by
/// SELECTOR-PATH — the fidelity redesign's Layer-1 (SHAPE) + Layer-2 (jarring) producer. The path-keyed
/// sibling of `capture_boxes_all_ids`: `fidelity::shape_stats` needs `/`-ancestry to cancel a constant
/// page offset, which `[id]` keys (no `/`) never carry, and the jarring invariants need the tag/display
/// the `Seen` shape carries. Same fetch → `<base>` → `--dump-dom` flow, only the injected probe differs.
///
/// Returns `oracle::Seen` maps so the G1 fidelity probe scores placement AND the four jarring invariants
/// through the SAME oracle functions the differential crawl uses — no Box4 mirror in between (brick 4b).
///
/// **The error type is [`Unmeasurable`], not `anyhow`, and that is the load-bearing part.** Its one
/// caller used to write `if let Ok(cseen) = capture_seen_all_paths(...)`, which threw every failure
/// on the floor: the row printed a bare `—`, the certificate counted an UNSCORED site, and the
/// reason — the thing §0 of the certification design requires and t602 explicitly asked for — was
/// gone. A typed error cannot be discarded by an `if let Ok`; the caller has to say what it did with
/// it.
pub fn capture_seen_all_paths(
    url: &str,
    vw: u32,
    vh: u32,
) -> std::result::Result<HashMap<String, crate::oracle::Seen>, Unmeasurable> {
    let chrome = chrome_bin().ok_or(Unmeasurable::Unreachable)?;
    let html = fetch_document(url)?;
    let base = format!("<base href=\"{url}\">");
    let doc = if let Some(i) = html.find("<head>") {
        let (a, b) = html.split_at(i + 6);
        format!("{a}{base}{b}{PROBE_ALL_PATHS_JS}")
    } else {
        format!("{base}{html}{PROBE_ALL_PATHS_JS}")
    };
    let tmp = std::env::temp_dir().join(format!("manuk-shape-{}.html", stable_tag(&doc)));
    std::fs::write(&tmp, &doc).map_err(|_| Unmeasurable::ProbeBlocked)?;
    let mut cmd = Command::new(&chrome);
    cmd.args(base_flags(vw, vh))
        .arg("--virtual-time-budget=6000")
        .arg("--dump-dom")
        .arg(format!("file://{}", tmp.display()));
    let secs = chrome_timeout_secs();
    let out = output_with_deadline(cmd, secs).ok_or(Unmeasurable::Timeout(secs))?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        return Err(Unmeasurable::ProbeBlocked);
    }
    // **A missing `__PARITY__` means the page stopped our script, not that the page is empty.**
    // Measured on `fdown.net`: Cloudflare's interstitial ships
    // `<meta http-equiv="content-security-policy" content="default-src 'none'; script-src 'nonce-…'">`,
    // so Chrome parses the injected probe, refuses to execute it for want of the nonce, and dumps a
    // DOM in which the probe is present as TEXT and its output never existed. `parse_seen_probe_json`
    // already asks exactly the right question — *"did Chrome run the script?"* — and for five ticks
    // nobody heard it, because the caller discarded the error.
    parse_seen_probe_json(&String::from_utf8_lossy(&out.stdout))
        .map_err(|_| Unmeasurable::ProbeBlocked)
}

/// Minimal blocking GET (the harness already links reqwest-free; use curl for zero new deps).
fn ureq_get(url: &str) -> Result<String> {
    fetch_document(url).map_err(|r| anyhow!("{url}: {}", r.explain()))
}

/// **Fetch a document AND look at what the server actually said.**
///
/// The function this replaces checked `out.status.success()` — **`curl`'s process exit code, not the
/// HTTP status.** `curl -sL` without `-f` exits 0 on a 403, so a Cloudflare *"Just a moment…"*
/// interstitial was returned to the caller as if it were the page, and an `imdb.com` answer of **202
/// with zero bytes** was returned as an empty document. Six of the certification corpus's 20 HEAD
/// sites answer non-2xx or empty to this client; all six were indistinguishable from a site that
/// simply renders badly.
///
/// That is why the certificate's fixed-denominator rule could count an unscored site but never say
/// **why** — the reason was destroyed here, before any layer that reports existed. Classification
/// itself lives in [`crate::fidelity::classify_fetch`] so it is exercisable without a network.
pub fn fetch_document(url: &str) -> std::result::Result<String, Unmeasurable> {
    let body_path = std::env::temp_dir().join(format!("manuk-fetch-{}.body", stable_tag(url)));
    let out = Command::new("curl")
        .args([
            "-sL",
            "--max-time",
            "25",
            "-A",
            "Mozilla/5.0 (X11; Linux x86_64) Manuk/0.1",
            "-o",
            &body_path.to_string_lossy(),
            "-w",
            "%{http_code}",
            url,
        ])
        .output()
        .map_err(|_| Unmeasurable::Unreachable)?;
    // A non-zero curl exit is a TRANSPORT failure (DNS, TLS, connect, timeout) — there is no status
    // to report, and that is itself the distinguishing fact.
    if !out.status.success() {
        let _ = std::fs::remove_file(&body_path);
        return Err(Unmeasurable::Unreachable);
    }
    let status: u32 = String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    let body = std::fs::read(&body_path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    let _ = std::fs::remove_file(&body_path);
    // **`000` MEANS "NO HTTP STATUS", WHICH IS NOT THE SAME AS "REFUSED".**
    //
    // This first read `status == 0 => Unreachable`, and G1 went red on the very next wall: the gate's
    // own corpus is `file://` SNAPSHOTS, and `curl -w '%{http_code}'` reports `000` for every
    // non-HTTP scheme because there is no status to report. curl exited 0 and the bytes are right
    // there — the fetch worked perfectly. A rule written for one scheme had quietly condemned all the
    // others, and the two static pages the fidelity floor is measured on became "unreachable".
    //
    // The transport failure that `000` *can* also indicate is already caught above, by curl's exit
    // code. So at this point the only honest reading of `000` is "this scheme has no HTTP status",
    // and the body decides.
    if status == 0 {
        return if body.trim().is_empty() {
            Err(Unmeasurable::Unreachable)
        } else {
            Ok(body)
        };
    }
    match crate::fidelity::classify_fetch(status, &body) {
        Some(reason) => Err(reason),
        None => Ok(body),
    }
}

/// **How long a single Chrome invocation may take before it is killed.**
///
/// **This deadline exists to make an INFINITE wait finite. It is not a latency budget**, and the
/// difference decides the number: any value comfortably above the legitimate worst case and far below
/// "the whole sweep" achieves the goal, so it should be set at the generous end and left there.
///
/// The first draft used 90s and **the wall failed on it immediately** — G1's own two `file://`
/// snapshots came back `2×timeout-90s`. Chrome is not slow on those pages; the WALL runs ~25 test
/// binaries in parallel, and a Chrome that takes seconds on an idle box takes minutes under that.
/// Which is precisely the trap this comment already warned about in its first version — *a deadline
/// that fires on a working site turns a fidelity measurement into a timing one* — written by me, and
/// then walked into on the very next verify.
///
/// 300s is ~6x the slowest legitimate run observed on an idle box and ~9x under the wall's own load,
/// while still turning the observed failure (a sweep stalled ~45 minutes, losing nine completed
/// sites) into one counted row. `MANUK_CHROME_TIMEOUT_SECS` overrides.
fn chrome_timeout_secs() -> u64 {
    std::env::var("MANUK_CHROME_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300)
}

/// **Run a child process with a deadline. `Command::output()` has none.**
///
/// Every Chrome invocation in this file used `output()`, which blocks forever if the child does. The
/// sweep runs sites in ONE process, one after another, so a single stuck child stalls the entire
/// corpus — and the run then produces no certificate at all, losing the sites that already finished.
/// Observed: a 20-site sweep killed by its outer `timeout` after ~45 minutes on the ninth site, with
/// its nine completed rows discarded.
///
/// `None` means the deadline fired and the child was killed; the caller turns that into a COUNTED
/// [`Unmeasurable::Timeout`] row rather than losing the run.
///
/// Polling rather than a `wait_timeout` crate: it is a dozen lines, adds no dependency, and the
/// resolution that matters here is seconds. **The kill is not optional** — returning without it would
/// leak a headless Chrome per stuck site, and the sweep's whole problem is that it runs long.
fn output_with_deadline(mut cmd: Command, secs: u64) -> Option<std::process::Output> {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    // **THE PIPES MUST BE DRAINED WHILE WE WAIT, AND THE FIRST VERSION OF THIS DID NOT.**
    //
    // `Command::output()` reads stdout and stderr concurrently with the wait. A poll loop that only
    // calls `try_wait` does not — so when the child fills the 64KB pipe buffer it BLOCKS on write and
    // never exits, and the "timeout" then fires on a process that was working perfectly. Chrome's
    // `--dump-dom` emits hundreds of KB, so this is not an edge case: it is every real page.
    //
    // The wall caught it in the most pointed way available. The first draft timed out on G1's own two
    // snapshot pages; I read that as "the deadline is too tight", raised it — and they simply took the
    // longer deadline instead. **A bound that turns a working process into a timeout is a worse bug
    // than the unbounded wait it replaced**, because the unbounded wait at least never lied about a
    // healthy site.
    let mut so = child.stdout.take()?;
    let mut se = child.stderr.take()?;
    let t_out = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = so.read_to_end(&mut v);
        v
    });
    let t_err = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = se.read_to_end(&mut v);
        v
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {}
            Err(_) => return None,
        }
        if std::time::Instant::now() >= deadline {
            // The kill is not optional: returning without it leaks a headless Chrome per stuck site,
            // and the sweep's whole problem is that it runs long. Killing also closes the pipes, so
            // the reader threads finish and can be joined rather than detached.
            let _ = child.kill();
            let _ = child.wait();
            let _ = t_out.join();
            let _ = t_err.join();
            tracing::warn!(
                secs,
                "a Chrome invocation exceeded its deadline and was killed"
            );
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };

    Some(std::process::Output {
        status,
        stdout: t_out.join().ok()?,
        stderr: t_err.join().ok()?,
    })
}

#[cfg(test)]
mod deadline_tests {
    use super::*;

    /// **G_SUBPROCESS_DEADLINE — a child that never returns must be killed, not waited on forever.**
    ///
    /// `Command::output()` has no timeout. The certification sweep runs sites in ONE process, one
    /// after another, so a single stuck Chrome stalls the whole corpus — and the run then yields no
    /// certificate at all, so the sites that already completed are lost with it. Observed: a 20-site
    /// sweep killed by its outer `timeout` after ~45 minutes on the ninth site, its nine finished rows
    /// discarded.
    ///
    /// The hang itself was NOT reproducible in isolation (`ebay.com` alone completes in 32s), which is
    /// exactly why this gates the MECHANISM rather than the site: the defect is that an unbounded wait
    /// exists at all, and that is true whether or not any particular page triggers it today.
    #[test]
    fn a_child_that_never_returns_is_killed_at_the_deadline() {
        // Well under any real Chrome run, so the assertion cannot pass by the child finishing.
        let mut c = Command::new("sleep");
        c.arg("120");
        let t = std::time::Instant::now();
        let out = output_with_deadline(c, 1);
        let elapsed = t.elapsed();
        assert!(
            out.is_none(),
            "a child that outlives its deadline must report None, so the caller can COUNT the site as \
             Unmeasurable::Timeout instead of blocking the corpus"
        );
        assert!(
            elapsed < std::time::Duration::from_secs(15),
            "the deadline must actually fire — returned after {elapsed:?}, which means the wait is \
             still unbounded and one stuck child still costs the whole sweep"
        );

        // …and the ordinary case must be untouched: a child that finishes returns its output.
        let mut ok = Command::new("echo");
        ok.arg("alive");
        let got = output_with_deadline(ok, 30).expect("a fast child returns its output");
        assert!(
            String::from_utf8_lossy(&got.stdout).contains("alive"),
            "bounding the wait must not change what a healthy child returns"
        );

        // ── **A LARGE OUTPUT, which is what the first version of this deadlocked on.**
        //
        // A poll loop that does not drain the pipes lets the child fill the 64KB buffer and BLOCK on
        // write; `try_wait` then never reports exit and the deadline fires on a healthy process.
        // Chrome's `--dump-dom` emits hundreds of KB, so this is the normal case, not an edge one —
        // and the failure it produces (a working page reported as a timeout) is worse than the
        // unbounded wait this replaced. 4MB is ~64 pipe buffers: nothing survives that without
        // concurrent draining.
        let mut big = Command::new("sh");
        big.arg("-c")
            .arg("yes ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz | head -c 4000000");
        let t2 = std::time::Instant::now();
        let out2 = output_with_deadline(big, 30).expect(
            "a child producing 4MB must COMPLETE, not hit the deadline — if this is None the pipes \
             are not being drained while we wait, and every real Chrome run will 'time out' while \
             working perfectly",
        );
        assert_eq!(
            out2.stdout.len(),
            4_000_000,
            "the whole of a large stdout must come back — a truncated read is a silently wrong probe"
        );
        assert!(
            t2.elapsed() < std::time::Duration::from_secs(25),
            "4MB should stream in well under the deadline; {:?} means it is being throttled by the \
             poll interval rather than read concurrently",
            t2.elapsed()
        );
    }
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
///
/// Errors as [`Unmeasurable`] for the same reason `capture_seen_all_paths` does: this is the first
/// step that touches the origin, so it is where a refusal is discovered, and its caller has to
/// COUNT that site rather than skip it. A `continue` here is exactly the silent drop §0 forbids.
pub fn capture_url_screenshot(
    url: &str,
    vw: u32,
    vh: u32,
    dest: &Path,
) -> std::result::Result<(), Unmeasurable> {
    let chrome = chrome_bin().ok_or(Unmeasurable::Unreachable)?;
    // Screenshot the SAME page the box probe measures: the fetched HTML, served from a temp file
    // with a `<base>` so subresources still resolve to the real origin.
    //
    // Pointing Chrome at the live URL instead looks more faithful and is in fact a trap: the two
    // Chrome captures then render *different pages*. Wikipedia's CentralNotice injects a 350px
    // fundraising banner on the real origin and not on a `file://` page, so the screenshot had a
    // banner the box probe never saw — and the visual score and the structural score were measuring
    // two different documents. One page, two probes.
    let html = fetch_document(url)?;
    let base = format!("<base href=\"{url}\">");
    let doc = match html.find("<head>") {
        Some(i) => {
            let (a, b) = html.split_at(i + 6);
            format!("{a}{base}{b}")
        }
        None => format!("{base}{html}"),
    };
    let tmp = std::env::temp_dir().join(format!("manuk-shot-{}.html", stable_tag(&doc)));
    std::fs::write(&tmp, &doc).map_err(|_| Unmeasurable::ProbeBlocked)?;
    let mut cmd = Command::new(&chrome);
    cmd.args(base_flags(vw, vh))
        .arg("--virtual-time-budget=6000") // let the page settle (webfonts, JS) before the shot
        .arg(format!("--screenshot={}", dest.display()))
        .arg(format!("file://{}", tmp.display()));
    let secs = chrome_timeout_secs();
    let out = output_with_deadline(cmd, secs).ok_or(Unmeasurable::Timeout(secs))?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        return Err(Unmeasurable::ProbeBlocked);
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
    let out = cmd
        .output()
        .context("running headless Chrome --screenshot")?;
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
    let json = json.ok_or_else(|| {
        anyhow!("no __PARITY__ probe output in dumped DOM (did Chrome run the script?)")
    })?;
    let map: HashMap<String, Box4> =
        serde_json::from_str(json.trim()).with_context(|| format!("parsing probe JSON: {json}"))?;
    Ok(map)
}

/// Pull the `#__PARITY__` JSON out of a dumped DOM and parse it as `Seen` entries — the enriched
/// path producer (brick 4b) whose values are `[tag, display, x, y, w, h]` plus (since t563) a 7th `"<family>/<px>"`, the same shape
/// `oracle_probe` emits. Reuses our own HTML parser so entity-escaping is handled correctly. Skips any
/// entry that is not a well-formed 6- or 7-tuple rather than failing the whole page.
fn parse_seen_probe_json(dumped_dom: &str) -> Result<HashMap<String, crate::oracle::Seen>> {
    use crate::oracle::Seen;
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
    let json = json.ok_or_else(|| {
        anyhow!("no __PARITY__ probe output in dumped DOM (did Chrome run the script?)")
    })?;
    let raw: HashMap<String, serde_json::Value> = serde_json::from_str(json.trim())
        .with_context(|| format!("parsing seen probe JSON: {json}"))?;
    let mut map = HashMap::new();
    for (id, v) in raw {
        let Some(a) = v.as_array() else { continue };
        // 7 since t563 (the computed font joined the tuple); 6 is still accepted so a cached probe
        // output from before that change parses with an EMPTY font rather than being dropped — an
        // absent datum must not silently remove the element from the diff.
        if a.len() != 6 && a.len() != 7 {
            continue;
        }
        let (Some(tag), Some(display)) = (a[0].as_str(), a[1].as_str()) else {
            continue;
        };
        let mut rect = [0i64; 4];
        let mut ok = true;
        for (i, slot) in rect.iter_mut().enumerate() {
            match a[i + 2].as_i64() {
                Some(x) => *slot = x,
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        map.insert(
            id,
            Seen {
                tag: tag.to_string(),
                display: display.to_string(),
                rect,
                font: a.get(6).and_then(|f| f.as_str()).unwrap_or("").to_string(),
            },
        );
    }
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

    /// **G_PROBE_WAITS_FOR_THE_PAGE** — the live-site probes must not report the DOM at parse time.
    ///
    /// Tick 674's measurement, and the reason this is asserted on the SOURCE rather than left to a
    /// live run: both live probes ran synchronously at end-of-parse, so on a JS-rendered site they
    /// reported the pre-hydration shell. Same page, five moments:
    ///
    /// ```text
    ///                       PARSE     DCL     LOAD   T+2000   T+5000
    ///   comix.to                3       4        5        6        7
    ///   www.naukri.com          4      37       59       60       61
    ///   www.welt.de          3199    3200     3177     3201     3176
    /// ```
    ///
    /// A live assertion would need the network and would be exactly the flaky gate this project
    /// refuses to build. What CAN be asserted hermetically is that the deferral is present, that it
    /// is the SAME text in both probes, and that the probe skips its own sentinel — the three ways
    /// this fix silently rots.
    #[test]
    fn live_probes_defer_to_load_and_skip_their_own_sentinel() {
        for (name, probe) in [
            ("PROBE_ALL_IDS_JS", PROBE_ALL_IDS_JS),
            ("PROBE_ALL_PATHS_JS", PROBE_ALL_PATHS_JS),
        ] {
            for needle in [
                "addEventListener('load',capture",
                "addEventListener('DOMContentLoaded',capture",
                "setTimeout(capture,3000)",
            ] {
                assert!(
                    probe.contains(needle),
                    "{name} lost `{needle}`. A live-site probe that runs only at parse reports the \
                     pre-hydration shell — naukri.com read 4 elements instead of 61."
                );
            }
            // Monotone: the parse-time capture still runs FIRST, so a page whose `load` never fires
            // emits what it emits today rather than nothing. A missing `__PARITY__` reads as
            // ProbeBlocked and costs a whole row.
            assert!(
                probe.contains("\ncapture();"),
                "{name} no longer captures at parse time. The deferral must ADD later readings, \
                 never replace the one a hanging page would otherwise give us."
            );
            assert!(
                probe.contains("'__PARITY__'") && probe.contains("__PARITY__')continue;")
                    || probe.contains("e.id==='__PARITY__')return;"),
                "{name} does not skip its own sentinel. Once the probe re-runs, the `<pre>` it \
                 already appended is a rendered element with a box and it would measure itself."
            );
        }
        // One definition, pasted — not two that happen to agree today.
        let tail = probe_defer_tail!();
        assert!(
            PROBE_ALL_IDS_JS.ends_with(tail) && PROBE_ALL_PATHS_JS.ends_with(tail),
            "a live probe stopped using the shared deferral tail. 'One rule, N implementations' is \
             how this project loses a fix: the next edit would land in one probe and not the other."
        );
        // ...and the static-fixture probe is deliberately NOT deferred. If that ever changes it
        // should be a decision, not a copy-paste.
        assert!(
            !PROBE_JS.contains("setTimeout(capture"),
            "PROBE_JS was deferred. It probes committed static fixtures where end-of-parse IS the \
             final DOM; deferring it risks a 72/72 green gate to buy nothing."
        );
    }

    #[test]
    fn parse_probe_json_reads_boxes() {
        let dom = r#"<html><body><pre id="__PARITY__">{"p-a":[30,0,100,40],"p-b":[0,40,60,20]}</pre></body></html>"#;
        let boxes = parse_probe_json(dom).unwrap();
        assert_eq!(boxes["p-a"], [30, 0, 100, 40]);
        assert_eq!(boxes["p-b"], [0, 40, 60, 20]);
    }

    #[test]
    fn parse_seen_probe_json_reads_tag_display_and_box() {
        // The enriched path producer (brick 4b) emits `[tag, display, x, y, w, h]` — the SAME 6-tuple
        // as the differential `oracle_probe`. Prove the tag and display survive the round-trip, not
        // just the box (a Box4-only parse would silently drop them and the jarring invariants that
        // read `Seen.tag` — collapsed-target — would misfire).
        let dom = r#"<html><body><pre id="__PARITY__">{"button.abc:nth-child(2)":["button","flex",30,0,100,40],"div:nth-child(1)/p:nth-child(3)":["p","block",0,40,60,20]}</pre></body></html>"#;
        let seen = parse_seen_probe_json(dom).unwrap();
        let b = &seen["button.abc:nth-child(2)"];
        assert_eq!(b.tag, "button");
        assert_eq!(b.display, "flex");
        assert_eq!(b.rect, [30, 0, 100, 40]);
        let p = &seen["div:nth-child(1)/p:nth-child(3)"];
        assert_eq!(p.tag, "p");
        assert_eq!(p.display, "block");
        assert_eq!(p.rect, [0, 40, 60, 20]);
        // A malformed entry (a bare 4-tuple from a stale probe) is skipped, never mis-parsed as a Seen.
        let bad =
            r#"<html><body><pre id="__PARITY__">{"x:nth-child(1)":[1,2,3,4]}</pre></body></html>"#;
        assert!(parse_seen_probe_json(bad).unwrap().is_empty());
    }
}
