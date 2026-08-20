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
var pre=document.createElement('pre');pre.id='__PARITY__';pre.style.display='none';
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
/// ⚠⚠⚠ **AND IT IS WHY THE SENTINEL MUST BE `display:none` (tick 781).** Deferring turned a probe
/// that measured ONCE and *then* wrote its answer into the page into one that writes its answer into
/// the page and then **measures again, three more times** — and `emit()` appends a `<pre>` holding
/// the entire result JSON as **one unwrapped line**. `<pre>` does not wrap, so that element's
/// max-content width is tens of thousands of px, and it is appended to `document.documentElement`,
/// i.e. as a SIBLING OF `<body>`. On any page whose root box is intrinsically sized rather than
/// stretched to the ICB, `<html>` then sizes to the sentinel and `<body>` inherits that width.
///
/// Measured on `www.naukri.com`, deterministically, on the harness's own instrumented copy:
/// **`<body>` 89,905px wide against a 1,200px viewport** — 75× — with the element population
/// unchanged (57 either way) and every height correct. Adding `pre.style.display='none'` and
/// changing nothing else returns it to **1,200**. Every `x` and every `width` the reference reported
/// for that site was a number the reference had created, and `shape_stats` charged the whole thing
/// to the engine: the row read `thin-overlap`, whose own text says *"this is OURS"*.
///
/// The general form, and the reason this is written at the deferral rather than at the sentinel:
/// **an instrument that writes into the thing it measures has to be re-checked when it starts
/// measuring more than once.** t674 changed WHEN `capture()` runs and was careful about the
/// population; the sentinel's inertness was a property of the OLD ordering and nobody re-derived it.
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
  // `display:none` is LOAD-BEARING, not tidiness: this sentinel is appended INTO the document the
  // probe is about to measure again, and it holds the whole result JSON on ONE unwrapped `<pre>`
  // line. See the doc comment above — without it the reference widens its own subject.
  if(!pre){pre=document.createElement('pre');pre.id='__PARITY__';pre.style.display='none';document.documentElement.appendChild(pre);}
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
/// by its selector-PATH (`tag.SIG:nth-of-type(n)/…` from the root) instead of its `id`, so
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
  // `display:none` is LOAD-BEARING, not tidiness: this sentinel is appended INTO the document the
  // probe is about to measure again, and it holds the whole result JSON on ONE unwrapped `<pre>`
  // line. See the doc comment above — without it the reference widens its own subject.
  if(!pre){pre=document.createElement('pre');pre.id='__PARITY__';pre.style.display='none';document.documentElement.appendChild(pre);}
  pre.textContent=JSON.stringify(o);
}
function fnv(str){var h=0x811c9dc5;for(var i=0;i<str.length;i++){h^=str.charCodeAt(i);h=Math.imul(h,0x01000193)>>>0;}return h>>>0;}
function sigOf(e){var cls=e.getAttribute('class');if(!cls)return '';var toks=cls.split(/[ \t\n\f\r]+/),a=[];for(var i=0;i<toks.length;i++){if(toks[i])a.push(toks[i].replace(/[A-Z]/g,function(c){return c.toLowerCase();}));}if(!a.length)return '';a.sort();var u=[];for(var j=0;j<a.length;j++){if(j===0||a[j]!==a[j-1])u.push(a[j]);}return '.'+('0000000'+fnv(u.join('.')).toString(16)).slice(-8);}
function pathOf(e){var p=[];while(e&&e.nodeType===1&&e.parentElement){var t=e.tagName.toLowerCase(),i=1,s=e;while((s=s.previousElementSibling)){if(s.tagName.toLowerCase()===t)i++;}p.unshift(t+sigOf(e)+':nth-of-type('+i+')');e=e.parentElement;}return p.join('/');}
function capture(){var out={};
var all=document.querySelectorAll('*');
var lim=Math.min(all.length,6000);
// ⚠⚠⚠ THE USED FACE, MEASURED RATHER THAN NAMED. `getComputedStyle` cannot report which face
// actually rasterized a box, and the field below used to say so and stop — which left the one
// question it was built for (t563: "is [74x16] vs [76x18] a different FACE or a different rule?")
// unanswerable, and t1151 found three sites where our box is WIDER *and* TALLER, a combination that
// cannot be a placement error, while this column printed {Raleway/18} against {Raleway/18}. Canvas
// IS the channel: `measureText` in the element's own resolved font returns the advance the USED
// face produces. It does not name the face; it measures it, which is what attribution needs.
// Cached per font string — the probe is fixed, so this is one measureText per distinct font, not
// per element. A rejected font string leaves `ctx.font` unchanged, so the sentinel round-trip
// reports 0 (ABSENCE) rather than the previous element's number.
var __fcx=document.createElement('canvas').getContext('2d');var __fmc={};
var __FPROBE='Hamburgefonstiv 0123';var __FSENT='7px monospace';
function __adv(cs){
  var f=(cs.fontStyle||'normal')+' '+(cs.fontWeight||'400')+' '+cs.fontSize+' '+cs.fontFamily;
  if(__fmc[f]!==undefined)return __fmc[f];
  __fcx.font=__FSENT; __fcx.font=f;
  var w=(__fcx.font===__FSENT&&f!==__FSENT)?0:Math.round(__fcx.measureText(__FPROBE).width);
  __fmc[f]=w; return w;}
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
  out[pathOf(e)]=[t,cs0.display,Math.round(r.x+window.scrollX),Math.round(r.y+window.scrollY),Math.round(r.width),Math.round(r.height),fam0+'/'+px0+'/'+__adv(cs0),cs0.position];}
emit(out);}"#,
    probe_defer_tail!()
);

/// **WHICH VERSION OF THE ORACLE PRODUCED A ROW** — a stable fingerprint of the two live-site
/// probes, so a rows file can say that two readings of one site came from two different instruments.
///
/// Named by a measurement that would otherwise have been mis-read. Tick 674 deferred both live
/// probes to `load`, which is a change to the ORACLE'S POPULATION: on the HEAD-20 corpus keirin's
/// element count went 356 → 1036 and playhop's intersection went 550 → 5. The spread block then
/// printed those as *this site's own noise* — naukri Δ100.0 pts, agoda Δ58.6, keirin Δ52.6, playhop
/// Δ43.6 — and `repeat_plan` would have paid for three renders of each of those four sites on every
/// future sweep, forever, to re-measure a variance that is not variance. **A step change in the
/// instrument is not an error bar on the subject.**
///
/// It is the probes' OWN TEXT, so it cannot be forgotten: any edit to what the oracle collects
/// changes the tag, and no separate version constant has to be remembered. ⚠ It covers the
/// **population** half only — a change to the SCORING math (`shape_stats`'s tolerance, the sample
/// floor) moves numbers without moving this tag. That is a named limitation, not an oversight;
/// hashing the whole scoring module would discard the error bar on every comment edit, which costs
/// more than it buys.
pub fn instrument_tag() -> &'static str {
    static TAG: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TAG.get_or_init(|| {
        let both = format!("{PROBE_ALL_IDS_JS}\u{1}{PROBE_ALL_PATHS_JS}");
        stable_tag(&both)[..8].to_string()
    })
}

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
    // A path (`div.a1b2c3d4:nth-of-type(1)/main:nth-of-type(1)/p:nth-of-type(4)`) is computable
    // identically by both engines from the same
    // snapshot, and it names EVERY element rather than the handful an author chose to label. The
    // 6,000-element cap is a bound on probe cost, not on ambition, and it is reported so a truncated
    // page can never masquerade as a complete one.
    let probe = r#"<script>
(function(){
  var out = {};
  // Selector-path keying (tick 399 spec, RE-COUNTED t784): `tag.SIG:nth-of-type(N)`. N counts
  // the element siblings that share this element's TAG (1-based) — NOT all element siblings,
  // which is what `:nth-child` counted and what made one inserted `<div>` re-number the whole
  // page (t783: a1.ro matched 1 of 685 under nth-child, 685 of 685 under a tag-only key).
  // SIG is fnv1a-32 over the ASCII-lowercased, SORTED, deduped class
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
      var t = e.tagName.toLowerCase(), i = 1, s = e;
      while ((s = s.previousElementSibling)) { if (s.tagName.toLowerCase() === t) i++; }
      p.unshift(t + sigOf(e) + ':nth-of-type(' + i + ')');
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
                 Math.round(r.width), Math.round(r.height), '', cs.position];
  }
  // Health of the ORACLE ITSELF, not of the diff: is what Chromium rendered a real document, or a
  // bot wall / error page / no-script shell? Answered by what Chromium DREW, not by how many
  // elements happened to carry an id.
  out['__META__'] = ['', '', document.querySelectorAll('*').length,
                     (document.body ? document.body.innerText.length : 0), 0, 0];
  var pre = document.createElement('pre'); pre.id = '__ORACLE__'; pre.style.display = 'none';
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
  var pre = document.createElement('pre'); pre.id = '__G5__'; pre.style.display = 'none';
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
    let doc = format!("{}{PROBE_ALL_IDS_JS}", splice_head(&html, &base));
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
    let doc = format!("{}{PROBE_ALL_PATHS_JS}", splice_head(&html, &base));
    let tmp = std::env::temp_dir().join(format!("manuk-shape-{}.html", stable_tag(&doc)));
    std::fs::write(&tmp, &doc).map_err(|_| Unmeasurable::ProbeBlocked)?;
    let mut cmd = Command::new(&chrome);
    cmd.args(base_flags(vw, vh))
        .arg("--virtual-time-budget=6000")
        .arg("--dump-dom")
        .arg(format!("file://{}", tmp.display()));
    let secs = chrome_timeout_secs();
    let out = output_with_deadline(cmd, secs).ok_or(Unmeasurable::OracleTimeout(secs))?;
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
    let seen = parse_seen_probe_json(&String::from_utf8_lossy(&out.stdout))
        .map_err(|_| Unmeasurable::ProbeBlocked)?;
    // ⚠⚠⚠ **THE SNAPSHOT REFERENCE IS A SHELL — TRY ONE ORIGIN. THE CAUSE DOES NOT GATE THE FIX.**
    //
    // t865 named the wall and t880 built the fix for the `oracle-module-shell` cohort: a module
    // script is always CORS-fetched, a site does not send `Access-Control-Allow-Origin` for its own
    // bundle, so the app never boots for the ORACLE and the instrument charges Chrome's missing page
    // to us. Serving document and subresources through one loopback origin removes the wall by
    // construction.
    //
    // ⚠⚠⚠ **AND THE TRIGGER ALSO ASKED FOR `type="module"`, WHICH IS A SUFFICIENT CONDITION USED AS
    // A NECESSARY ONE (measured t903).** Modules are one way to reach the origin wall. They are not
    // the wall. Every ordinary root-relative subresource is `file:///…` from a snapshot, and so is
    // every same-document navigation. Five `shell-only` rows sat in the t898 sweep with the fix
    // built, wired, and withheld from them — four measured here, `curl` for the document and two
    // `--dump-dom` runs each:
    //
    // ```text
    //                                  ships type=module?   file:// snapshot   LIVE
    //   esaj.tjsp.jus.br                     NO                     30          300
    //   house.udn.com                        NO                     99          958
    //   merchant.upi9.pro                    NO                     20           68
    //   experiencia.pichincha.com            NO                     53          567
    // ```
    //
    // `merchant.upi9.pro` is Next.js with classic `<script src="/_next/…" defer>`; `house.udn.com`
    // is a 195-byte document whose whole body is `window.location.href="/house/index"`. Neither
    // involves a CORS-mode fetch, and one origin fixes both.
    //
    // **The cost gate is, and always was, the SHELL FLOOR — not the module test.** The comment this
    // replaces said so itself while the code did otherwise: *"gating on [modules] alone would double
    // this crate's Chrome bill … gating on the reference came in under the shell floor confines the
    // extra work to the ~11 rows that are unscored today."* A healthy site is over the floor and
    // pays nothing here; the module conjunct bought no budget and cost four rows.
    //
    // On refusal — or on any failure inside the proxy path — the snapshot's shell is returned
    // unchanged, and the row keeps its honest `shell-only` / `oracle-module-shell` label. See
    // [`crate::proxy::renders_agree`], which is what makes widening this safe: a proxied render is a
    // reference only when it AGREES with the live one, so a wider trigger can only convert rows the
    // acceptance test has already vouched for.
    if crate::fidelity::one_origin_worth_trying(seen.len(), &html) {
        if let Some(better) = one_origin_reference(url, &html, vw, vh) {
            return Ok(better);
        }
    }
    Ok(seen)
}

/// Render the reference through [`crate::proxy`] — ONE origin — and return it **only if it agrees
/// with the LIVE render**.
///
/// Two extra Chrome runs (the proxied page, and the live page for the acceptance test) on a cohort
/// of ~11 sites. `None` means "keep the snapshot's honest shell", and every failure inside is that
/// same answer: a proxy that cannot be shown to agree with the live page is not a reference.
fn one_origin_reference(
    url: &str,
    html: &str,
    vw: u32,
    vh: u32,
) -> Option<HashMap<String, crate::oracle::Seen>> {
    let chrome = chrome_bin()?;
    let (scheme, host, _) = crate::proxy::split_url(url)?;
    let bound = crate::proxy::Bound::bind(url)?;
    let root = bound.root();
    // No `<base>`: the whole point is that the document is served at its OWN path under a real
    // origin, so the author's relative URLs resolve exactly as the author wrote them. A `<base>`
    // pointing back at the site would undo the tick.
    let rewritten = crate::proxy::rewrite_document(html, scheme, host, &root);
    let doc = if let Some(i) = rewritten.find("<head>") {
        let (a, b) = rewritten.split_at(i + 6);
        format!("{a}{b}{PROBE_ALL_PATHS_JS}")
    } else {
        format!("{rewritten}{PROBE_ALL_PATHS_JS}")
    };
    let proxy = bound.serve(doc, PROBE_ALL_PATHS_JS.to_string());
    let secs = chrome_timeout_secs();

    let mut pcmd = Command::new(&chrome);
    pcmd.args(base_flags(vw, vh))
        .arg("--virtual-time-budget=6000")
        .arg("--dump-dom")
        .arg(proxy.document_url());
    let pout = output_with_deadline(pcmd, secs)?;
    if !pout.status.success() {
        return None;
    }
    let pdump = String::from_utf8_lossy(&pout.stdout).into_owned();

    let mut lcmd = Command::new(&chrome);
    lcmd.args(base_flags(vw, vh))
        .arg("--virtual-time-budget=6000")
        .arg("--dump-dom")
        .arg(url);
    let lout = output_with_deadline(lcmd, secs)?;
    if !lout.status.success() {
        return None;
    }
    let ldump = String::from_utf8_lossy(&lout.stdout).into_owned();
    let live_n = crate::proxy::count_open_tags(&ldump);
    let proxy_n = crate::proxy::count_open_tags(&pdump);
    if !crate::proxy::renders_agree(live_n, proxy_n) {
        // **A REFUSAL WITHOUT ITS EVIDENCE IS THE NEXT TICK'S GUESSWORK.** The loop's very next
        // question is always "what did the proxy miss (or invent)", and the answer is a two-line
        // diff of tag histograms — so it is printed here rather than rediscovered by hand. The
        // cohort is ~11 sites, so this is never noise on a healthy sweep.
        let (a, b) = crate::proxy::tag_delta(&ldump, &pdump);
        eprintln!(
            "  PROXY REFERENCE REFUSED: one-origin render carries {proxy_n} open tags against the \
             live page's {live_n} — a half-built reference is strictly WORSE than an honest shell, \
             so the row keeps `oracle-module-shell`\n    only LIVE has: {a}\n    only PROXY has: {b}"
        );
        return None;
    }
    let seen = parse_seen_probe_json(&pdump).ok()?;
    eprintln!(
        "  PROXY REFERENCE ACCEPTED: one-origin render agrees with live ({proxy_n} vs {live_n} open \
         tags) — scoring against {} probed elements instead of the snapshot's shell",
        seen.len()
    );
    Some(seen)
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

/// ⚠⚠⚠ **`--window-size` IS A WINDOW SIZE, NOT A VIEWPORT SIZE, AND THE DIFFERENCE WAS 87 PIXELS
/// OF UNCORRECTED ERROR AGAINST 73% OF THE CORPUS.**
///
/// Measured on this box (Chrome 145, `--headless=new`), asking the page for
/// `document.documentElement.clientHeight`:
///
/// ```text
///   --window-size=1200,600   ->  viewport 1200 x 513
///   --window-size=1200,800   ->  viewport 1200 x 713
///   --window-size=1200,1000  ->  viewport 1200 x 913
///   --window-size=800,800    ->  viewport  800 x 713
/// ```
///
/// A **constant 87px** on the block axis and **zero** on the inline one. So every reference capture
/// laid the page out in a viewport 87px shorter than the one our engine was told to use, and every
/// `vh` unit in the corpus was compared against a 12.2%-different height. `vh`/`vw` is declared by
/// **73.1%** of the burndown corpus and `min-height: 100vh` — the full-bleed hero section — by
/// **36.3%**: a hero measured 800 here and 713 there, and everything below it shifted by 87px.
///
/// This is the `--hide-scrollbars` lesson in a second subject: **the reference was not rendering the
/// page we asked for, and the divergence was charged to the engine.** The fix belongs here, not in
/// the engine.
///
/// **Measured, not hard-coded.** The offset is a property of the Chrome build and platform, so it is
/// probed once per process with a one-line document and cached — one extra launch per sweep, not one
/// per site. If the probe fails for any reason the offset is zero, which is exactly today's
/// behaviour: the instrument degrades to what it already did rather than to something new.
fn viewport_chrome_offset() -> (u32, u32) {
    static OFFSET: std::sync::OnceLock<(u32, u32)> = std::sync::OnceLock::new();
    *OFFSET.get_or_init(|| {
        const PROBE_W: u32 = 1000;
        const PROBE_H: u32 = 800;
        let Some(chrome) = chrome_bin() else {
            return (0, 0);
        };
        let tmp = std::env::temp_dir().join("manuk-viewport-probe.html");
        let html = "<!doctype html><html><body><pre id=o></pre><script>\
document.getElementById('o').textContent='VP:'+document.documentElement.clientWidth+'x'\
+document.documentElement.clientHeight;</script></body></html>";
        if std::fs::write(&tmp, html).is_err() {
            return (0, 0);
        }
        let out = Command::new(&chrome)
            .args([
                "--headless=new",
                "--disable-gpu",
                "--hide-scrollbars",
                "--force-device-scale-factor=1",
                "--no-sandbox",
                &format!("--window-size={PROBE_W},{PROBE_H}"),
                "--dump-dom",
            ])
            .arg(format!("file://{}", tmp.display()))
            .output();
        let _ = std::fs::remove_file(&tmp);
        let Ok(out) = out else { return (0, 0) };
        let text = String::from_utf8_lossy(&out.stdout);
        let Some(i) = text.find("VP:") else {
            return (0, 0);
        };
        let rest = &text[i + 3..];
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == 'x'))
            .unwrap_or(rest.len());
        let mut parts = rest[..end].split('x');
        let (Some(w), Some(h)) = (
            parts.next().and_then(|v| v.parse::<u32>().ok()),
            parts.next().and_then(|v| v.parse::<u32>().ok()),
        ) else {
            return (0, 0);
        };
        // Only ever a POSITIVE correction: if a future Chrome reports a viewport LARGER than the
        // window we asked for, that is not something to compensate by shrinking the window, and
        // saturating here keeps the instrument from inventing a new distortion out of a surprise.
        (PROBE_W.saturating_sub(w), PROBE_H.saturating_sub(h))
    })
}

/// ⚠⚠⚠ **THE REFERENCE BROWSER DECLARES ITSELF A DEVICE WITH NO POINTING DEVICE AT ALL, SO EVERY
/// `@media (hover: hover)` RULE ON THE CORPUS WAS SCORED AGAINST A LAYOUT NO DESKTOP USER SEES.**
///
/// Asked directly, with `matchMedia`, Chrome 145 `--headless=new` answers:
///
/// ```text
///   (hover: hover)      false      (hover: none)        true
///   (any-hover: hover)  false      (any-hover: none)    true
///   (pointer: fine)     false      (pointer: none)      true
///   (any-pointer: fine) false      (any-pointer: none)  true
/// ```
///
/// Not "coarse", not "unknown" — **`none`**, the value reserved for a device that cannot point.
/// Every other media feature this battery probed agrees with us exactly (`prefers-color-scheme`,
/// `prefers-reduced-motion`, `scripting`, `display-mode`, `color`, `min-resolution`, `forced-colors`,
/// `update`), which is what makes this one attributable rather than a general mismatch.
///
/// Our engine answers `hover: hover` / `pointer: fine`, and it is **right** — this is a desktop
/// browser with a mouse. So the divergence is the reference's, and correcting the ENGINE to match
/// would make the shipping browser wrong for every real user in order to make a number go up.
///
/// **This is the third subject of the mis-provisioned-reference class** (`--hide-scrollbars`,
/// `--window-size`, now the interaction family) and it is the branch of t1010's rule that says
/// *build it* rather than *do not build it*: `hyphens: auto` was unfixable because Chrome's
/// dictionaries are a separate component, but a pointing device is one flag away.
///
/// **Set, not probed, and the difference from `viewport_chrome_offset` is deliberate**: the offset
/// is a fact about the Chrome build that only measurement can supply, whereas this is a
/// CONFIGURATION we are choosing — the value we want is fixed by what Manuk is (a desktop browser
/// with a fine pointer), not by what Chrome happens to default to. What is falsifiable is whether
/// the flag still *takes effect*, and the gate for that is the parity fixture
/// `tests/wpt/corpus/media-interaction.html` — five probes that lay `hover`/`pointer` rules out
/// through this very capture path, in the wall for free because `parity` already runs there.
/// Commenting the flag out below gives `media-interaction 1/5`; `p-nohover` fails in the OPPOSITE
/// direction, so the fixture cannot be satisfied by one wrong constant.
///
/// ⚠ Run the CONTROL ARM before believing a flag: asking instead for `HoverType=1,PointerType=2`
/// yields `hover: none` / `pointer: coarse`, which is what proves the flag is doing the work rather
/// than coinciding with a default.
///
/// The Blink enum values: `HoverType::kHoverHoverType == 2`, `PointerType::kPointerFine == 4`.
const POINTING_DEVICE: &str = "--blink-settings=primaryHoverType=2,availableHoverTypes=2,\
primaryPointerType=4,availablePointerTypes=4";

/// ⚠⚠⚠ **THE REFERENCE'S SCROLLBAR POLICY — AND FOR THE WHOLE PROJECT IT WAS APPLIED TO ONE
/// ENGINE ONLY, WHICH IS WHY IT WAS A BUG AND NOT A SETTING.**
///
/// The comment this replaces read: *"`--hide-scrollbars` matters: a visible scrollbar would shrink
/// the layout viewport and shift every box."* Every word of that is true, and it named the exact
/// defect it went on to cause — because it shrinks the layout viewport of the **reference** only.
/// Our engine reserves a classic 15px gutter, as a desktop browser does; the reference was told not
/// to. Measured, on this box:
///
/// ```text
///     google-chrome --headless=new --window-size=1200,887   (document 5000px tall)
///        --hide-scrollbars     documentElement.clientWidth = 1200
///        (no flag)             documentElement.clientWidth = 1185   ← what our engine computes
/// ```
///
/// ⭐ **Our 15px is Blink's 15px to the pixel.** The engines agree; the instrument did not. On
/// `ticket.jfa.jp` — one of the near-miss band's own anchors — that is a `width: 90%` container
/// measured 1067 against 1080 and its `5%` margin at 59 against 60, on every element, which then
/// re-wraps prose and pushes everything below it down. That is precisely the *width launders into
/// dy* shape `docs/loop/PHASE0-RENDER-BURNDOWN.md` §11 ranks the band by, and it was the
/// instrument's.
///
/// So the policy is now **one constant that decides it for BOTH engines**: it selects the Chrome
/// flag below, and [`match_reference_scrollbar_policy`] puts our engine in the same mode. They
/// cannot drift, because there is only one of them.
///
/// It stays `true` — hiding scrollbars is the honest choice for a *comparison*: it removes the
/// scrollbar's own painted strip from the visual diff and takes a platform UA metric out of the
/// geometry, leaving the layout math, which is what this instrument is for. ⚠ It does mean the
/// `overflow: auto`-and-actually-overflows reservation (our engine's documented residue — it
/// reserves only for the deterministic `overflow: scroll`) stays **unmeasured by this instrument**;
/// that gap needs its own WPT coverage and must not be read as absent because the sweep is quiet.
pub const REFERENCE_HIDES_SCROLLBARS: bool = true;

/// Put OUR engine in the same scrollbar mode the reference is launched with. A host that captures
/// against Chrome must call this before it lays anything out — see [`REFERENCE_HIDES_SCROLLBARS`]
/// for what a mismatch costs.
pub fn match_reference_scrollbar_policy() {
    manuk_layout::set_scrollbars_hidden(REFERENCE_HIDES_SCROLLBARS);
}

/// The flags every headless invocation shares.
fn base_flags(vw: u32, vh: u32) -> Vec<String> {
    // See `viewport_chrome_offset`: the requested LAYOUT viewport, expressed as the WINDOW size
    // that produces it.
    let (dw, dh) = viewport_chrome_offset();
    let (vw, vh) = (vw + dw, vh + dh);
    let mut v = vec![
        "--headless=new".into(),
        "--disable-gpu".into(),
        "--force-device-scale-factor=1".into(),
        "--no-sandbox".into(),
        "--disable-extensions".into(),
        "--disable-lcd-text".into(),
        POINTING_DEVICE.into(),
        format!("--window-size={vw},{vh}"),
        "--virtual-time-budget=2000".into(),
    ];
    if REFERENCE_HIDES_SCROLLBARS {
        v.push("--hide-scrollbars".into());
    }
    v
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

/// **Splice `<base>` into a document without pushing Chrome into QUIRKS MODE.**
///
/// ⚠⚠⚠ **THE ORACLE WAS RENDERING HEADLESS FIXTURES IN QUIRKS MODE AND SCORING US AGAINST IT.**
/// Three probes assembled their document as *"insert after `<head>`, or else PREPEND"*, and the
/// `else` branch is the bug: HTML's own parser rule is that a `<!DOCTYPE>` is only a doctype **when
/// it is the first thing in the document**. Prepending `<base href=…>` puts a tag in front of it,
/// the doctype degrades to a comment-like token, and Chrome switches to `CSS1Compat`'s opposite —
/// `BackCompat`. Every page that does not spell a literal `<head>` was affected, which is nearly
/// every hand-written fixture and every page whose `<head>` the author let the parser imply.
///
/// **What it cost, measured (t1247).** A `height:100%` box inside an auto-height parent:
///
/// ```text
///                                        oracle's Chrome   REAL Chrome    ours
///   inline-block, height:100%              50x800            50x16       50x16
///   block child,  height:100%              50x800            50x16       50x16
///   inline-block, height:50%               50x400            50x16       50x16
/// ```
///
/// In quirks mode a percentage height walks up through auto-height ancestors to the initial
/// containing block; in standards mode CSS2 §10.5 computes it to `auto`. So the oracle reported a
/// **784px divergence on a row where we are exactly right**, and it would have reported it forever —
/// a fixture with no `<head>` cannot be scored honestly by an instrument that deletes its doctype.
/// (`document.compatMode` from the same file, loaded directly: `CSS1Compat`. Through the probe:
/// quirks behaviour.)
///
/// One function rather than a fourth copy of the three-line match: the three call sites had written
/// the same `else` branch three times and it was wrong all three times, which is this repository's
/// standing *one rule, N implementations* failure in its usual shape.
fn splice_head(html: &str, insert: &str) -> String {
    if insert.is_empty() {
        return html.to_string();
    }
    // Preferred seam, unchanged: immediately inside an explicit `<head>`.
    if let Some(i) = html.find("<head>") {
        let (a, b) = html.split_at(i + 6);
        return format!("{a}{insert}{b}");
    }
    // No `<head>`: go in AFTER the doctype so the doctype stays first. Case-insensitive, because
    // `<!DOCTYPE html>` is the spelling most of the web ships.
    let lower = html.to_ascii_lowercase();
    if lower.trim_start().starts_with("<!doctype") {
        let lead = html.len() - html.trim_start().len();
        if let Some(gt) = html[lead..].find('>') {
            let cut = lead + gt + 1;
            let (a, b) = html.split_at(cut);
            return format!("{a}{insert}{b}");
        }
    }
    // No doctype at all — the document is quirks-mode by the author's own choice, and prepending
    // changes nothing about that. Faithful.
    format!("{insert}{html}")
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
    let doc = splice_head(&html, &base);
    let tmp = std::env::temp_dir().join(format!("manuk-shot-{}.html", stable_tag(&doc)));
    std::fs::write(&tmp, &doc).map_err(|_| Unmeasurable::ProbeBlocked)?;
    let mut cmd = Command::new(&chrome);
    cmd.args(base_flags(vw, vh))
        .arg("--virtual-time-budget=6000") // let the page settle (webfonts, JS) before the shot
        .arg(format!("--screenshot={}", dest.display()))
        .arg(format!("file://{}", tmp.display()));
    let secs = chrome_timeout_secs();
    let out = output_with_deadline(cmd, secs).ok_or(Unmeasurable::OracleTimeout(secs))?;
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
        // 6 (pre-t563) · 7 (the computed font joined the tuple) · 8 (the computed `position`, t1084).
        // Older shapes stay accepted so a cached probe output parses with an EMPTY trailing field
        // rather than being dropped — an absent datum must not silently remove the element.
        //
        // ⚠⚠⚠ **AND THE EXACT-LIST FORM OF THAT GUARD BROKE THE INSTRUMENT THE MOMENT THE TUPLE
        // GREW.** t1084 added an 8th field to the probe and this line dropped **every element of
        // every page** — and the failure did not look like a parser failure: the sweep reported
        // `UNMEASURABLE [oracle-module-shell-0]: the ORACLE rendered only 0 element(s) — and this
        // document is a type="module" SPA`, i.e. it named a *cause on the page* for a defect in the
        // reader. A guard written to be forward-tolerant was enumerating lengths instead of taking
        // a MINIMUM, so it was tolerant in exactly one direction and silently fatal in the other.
        if a.len() < 6 {
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
                position: a.get(7).and_then(|f| f.as_str()).unwrap_or("").to_string(),
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

    /// **G_ORACLE_STAYS_IN_STANDARDS_MODE — the `<base>` splice must never displace the doctype.**
    ///
    /// HTML's parser treats a `<!DOCTYPE>` as a doctype **only when it is the first thing in the
    /// document**. Three probes assembled their document as *"after `<head>`, or else PREPEND"*, and
    /// the prepend put `<base href=…>` in front of the doctype — so Chrome parsed every
    /// `<head>`-less page in **quirks mode** while the real page is standards mode, and scored us
    /// against it.
    ///
    /// Measured (t1247): a `height:100%` box in an auto-height parent read **50x800** through the
    /// probe and **50x16** in the same Chrome loading the same file directly — which is also our own
    /// answer. The oracle reported a 784px divergence on a row where the engine is exactly right,
    /// and **9 of 183 live corpus documents** ship a doctype with no literal `<head>`, so this was
    /// live on the real sweep and not only on fixtures.
    ///
    /// **To watch it go RED:** change `splice_head`'s no-`<head>` arm back to
    /// `format!("{insert}{html}")` unconditionally.
    #[test]
    fn splice_head_never_puts_anything_in_front_of_the_doctype() {
        let base = "<base href=\"https://x.test/\">";

        // No `<head>`: the doctype must still be first, and the base must still be present.
        let out = splice_head("<!doctype html>\n<div>hi</div>", base);
        assert!(
            out.trim_start()
                .to_ascii_lowercase()
                .starts_with("<!doctype"),
            "the doctype must remain the first thing in the document, got: {out}"
        );
        assert!(
            out.contains(base),
            "the <base> must still be spliced in: {out}"
        );

        // The spelling the web actually ships.
        let out = splice_head("<!DOCTYPE HTML>\n<p>x</p>", base);
        assert!(
            out.trim_start()
                .to_ascii_lowercase()
                .starts_with("<!doctype"),
            "case-insensitive: got {out}"
        );
        assert!(out.contains(base));

        // An explicit `<head>` keeps the original, preferred seam.
        let out = splice_head(
            "<!doctype html><html><head><title>t</title></head></html>",
            base,
        );
        assert!(out
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("<!doctype"));
        assert!(
            out.find(base).unwrap() < out.find("<title>").unwrap(),
            "the base belongs immediately inside <head>: {out}"
        );

        // CONTROL — no doctype at all. The document is quirks by the author's own choice and
        // prepending changes nothing, so the faithful thing is to leave that alone.
        let out = splice_head("<div>hi</div>", base);
        assert!(
            out.starts_with(base),
            "no doctype: prepending is faithful, got {out}"
        );
    }

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

    /// **G_PROBE_INERT (source half) — every sentinel this file appends into a page it measures
    /// must be `display:none`.**
    ///
    /// Asserted over this file's OWN SOURCE rather than over the two probe constants, because there
    /// are FIVE sentinels and only two of them are constants: `PROBE_JS`, `PROBE_ALL_IDS_JS`,
    /// `PROBE_ALL_PATHS_JS`, the `__ORACLE__` probe built inside `capture_seen_all_paths_from_html`,
    /// and the `__G5__` probe built inside the interaction capture. A test that named the two
    /// constants would pass while the other three rotted — the "one rule, N implementations" shape
    /// this project loses fixes to, and the exact reason the widening survived in the differential
    /// crawl's probe as well as in the certificate's.
    ///
    /// The rule is positional and mechanical: a `createElement('pre')` must be followed, before the
    /// `appendChild` that puts it in the document, by a `style.display` set to none.
    #[test]
    fn every_probe_sentinel_is_display_none() {
        let src = include_str!("chrome.rs");
        // Split so this line is not itself an instance of what it looks for.
        let needle = concat!("createElement", "('pre')");
        let mut seen = 0usize;
        for line in src.lines() {
            // Prose ABOUT the rule is not an instance of it; only real code counts.
            if line.trim_start().starts_with("///") || !line.contains(needle) {
                continue;
            }
            seen += 1;
            assert!(
                line.contains("style.display") && line.contains("none"),
                "a probe sentinel is appended into the measured document WITHOUT display:none.\n\
                 It carries the whole result JSON on one unwrapped `<pre>` line, so its max-content \
                 width is tens of thousands of px; appended as a sibling of <body> it re-sizes any \
                 root that is intrinsically sized rather than stretched to the ICB.\n\
                 Measured (t781): naukri.com's Chrome reference read <body> 89905px wide at a 1200px \
                 viewport — 75× — and shape_stats charged every one of those x/width values to the \
                 engine.\n\
                 offending line: {}",
                line.trim()
            );
        }
        assert!(
            seen >= 5,
            "expected at least 5 probe sentinels in this file, found {seen} — either a probe was \
             removed (then drop the count) or the detection above stopped matching, which would \
             make this gate vacuous"
        );
    }

    /// **G_PROBE_INERT (live half) — the reference must return the same geometry with and without
    /// its own sentinel in the page.**
    ///
    /// The source assertion above cannot show that the rule MATTERS; this one does, hermetically and
    /// with no network. The fixture is a root that is intrinsically sized (`html{width:max-content}`)
    /// wrapping a single 300px block, served from `file://` — so `<body>` must be exactly 300px wide.
    ///
    /// **Proven red (t781):** delete `pre.style.display='none'` from `PROBE_ALL_PATHS_JS` and this
    /// reports `<body>` at **1221px** — the width of the probe's own JSON line — against the 300 it
    /// must be. The margin is not a tolerance question: the failure IS the sentinel's width, so it
    /// scales with how much the probe found (naukri.com: 89,905px).
    #[test]
    fn the_reference_probe_does_not_widen_an_intrinsically_sized_root() {
        if !available() {
            eprintln!("skipped: no Chrome/Chromium on this box");
            return;
        }
        let fixture = "<!doctype html><html style=\"width:max-content\"><head><style>\
                       body{margin:0}</style></head><body>\
                       <div id=\"a\" style=\"width:300px;height:20px\">hello</div></body></html>";
        let path =
            std::env::temp_dir().join(format!("manuk-probe-inert-{}.html", stable_tag(fixture)));
        std::fs::write(&path, fixture).expect("write fixture");
        let url = format!("file://{}", path.display());
        let seen = capture_seen_all_paths(&url, 1200, 800).expect("probe the fixture");
        let _ = std::fs::remove_file(&path);
        let body = seen
            .iter()
            .find(|(k, v)| v.tag == "body" && !k.contains('/'))
            .map(|(_, v)| v)
            .unwrap_or_else(|| panic!("no <body> in the reference snapshot: {:?}", seen.keys()));
        assert_eq!(
            body.rect[2], 300,
            "the reference widened its own subject: <body> should be 300px (the max-content width \
             of its only child) and came back {}px. The extra width is the probe's `<pre>` sentinel \
             being laid out as a sibling of <body> under an intrinsically-sized <html>.",
            body.rect[2]
        );
    }

    /// **THE INSTRUMENT'S VERSION MUST BE THE PROBES' OWN TEXT, NOT A CONSTANT SOMEONE BUMPS.**
    ///
    /// A hand-maintained version number is the thing this project has been bitten by every time:
    /// the edit lands, the bump does not, and downstream pools two instruments' readings while
    /// believing it did not. So the assertion is not "the tag exists" — it is that the tag is a
    /// FUNCTION OF the probe sources, which is what makes forgetting impossible.
    #[test]
    fn the_instrument_tag_is_derived_from_the_probes_not_declared() {
        let tag = instrument_tag();
        assert_eq!(tag.len(), 8, "the tag must be a short stable digest: {tag}");
        assert!(
            tag.chars().all(|c| c.is_ascii_hexdigit()),
            "the tag must be hex so it is safe in a TSV column: {tag}"
        );
        // Derived: the digest of the two probes IS the tag...
        let both = format!("{PROBE_ALL_IDS_JS}\u{1}{PROBE_ALL_PATHS_JS}");
        assert_eq!(
            &stable_tag(&both)[..8],
            tag,
            "the tag is not the digest of the probes — it has become a declaration, and a \
             declaration is a thing that can be forgotten on the tick that changes the probe"
        );
        // ...and ANY edit to what the oracle collects moves it. This is the property that makes the
        // spread block's version filter mechanical rather than remembered.
        let edited = format!("{both}// one more line of probe\n");
        assert_ne!(
            &stable_tag(&edited)[..8],
            tag,
            "editing a probe did not change the tag, so two oracles would share one version"
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
        let dom = r#"<html><body><pre id="__PARITY__">{"button.abc:nth-of-type(2)":["button","flex",30,0,100,40],"div:nth-of-type(1)/p:nth-of-type(3)":["p","block",0,40,60,20]}</pre></body></html>"#;
        let seen = parse_seen_probe_json(dom).unwrap();
        let b = &seen["button.abc:nth-of-type(2)"];
        assert_eq!(b.tag, "button");
        assert_eq!(b.display, "flex");
        assert_eq!(b.rect, [30, 0, 100, 40]);
        let p = &seen["div:nth-of-type(1)/p:nth-of-type(3)"];
        assert_eq!(p.tag, "p");
        assert_eq!(p.display, "block");
        assert_eq!(p.rect, [0, 40, 60, 20]);
        // A malformed entry (a bare 4-tuple from a stale probe) is skipped, never mis-parsed as a Seen.
        let bad = r#"<html><body><pre id="__PARITY__">{"x:nth-of-type(1)":[1,2,3,4]}</pre></body></html>"#;
        assert!(parse_seen_probe_json(bad).unwrap().is_empty());
    }

    /// **G_REFERENCE_VIEWPORT_MATCHES — the two engines must lay out in the SAME viewport, and for
    /// the whole life of this instrument they did not.**
    ///
    /// `base_flags` passes `--hide-scrollbars`, which makes the reference's scrollbars zero-width.
    /// Our engine reserved a classic 15px gutter, as a desktop browser does. Nobody had ever asked
    /// the two for the same number, so a **1.25% inline deficit** rode under every sweep: on
    /// `ticket.jfa.jp` a `width: 90%` container measured 1067 against Chrome's 1080, its `5%`
    /// margin 59 against 60, and the narrower column re-wrapped prose and pushed everything below
    /// it down — the *width launders into dy* shape the near-miss band is ranked by. Correcting it
    /// moved that one site's SHAPE from **66.4% to 82.1%** and its parent-relative misses from 216
    /// to 115, with no engine layout change at all.
    ///
    /// ⚠ **The gate asserts AGREEMENT, not a value.** Flipping
    /// [`REFERENCE_HIDES_SCROLLBARS`] to `false` must keep it green (both sides go to 1185): the
    /// defect was never the policy, it was the policy applied to one side. **To watch it go RED:**
    /// delete the [`match_reference_scrollbar_policy`] call below — that is exactly the state every
    /// sweep before t1319 ran in.
    ///
    /// The document must OVERFLOW: a short one has no scrollbar to hide, which is why
    /// `viewport_chrome_offset`'s own one-line probe measured this axis at zero and never saw it.
    #[test]
    fn the_reference_lays_out_in_the_same_viewport_our_engine_does() {
        let Some(chrome) = chrome_bin() else {
            eprintln!("skip: no Chrome/Chromium — reference viewport gate not run");
            return;
        };
        match_reference_scrollbar_policy();

        const VW: u32 = 1200;
        const VH: u32 = 800;
        let tmp = std::env::temp_dir().join("manuk-reference-viewport-gate.html");
        // 5000px tall: the reference MUST want a vertical scrollbar, or there is nothing to hide.
        let html = "<!doctype html><html><body style=\"margin:0\"><div style=\"height:5000px\">t</div>\
<pre id=o></pre><script>document.getElementById('o').textContent='VP:'\
+document.documentElement.clientWidth+'x'+document.documentElement.clientHeight;</script></body></html>";
        std::fs::write(&tmp, html).expect("write probe");

        let out = Command::new(&chrome)
            .args(base_flags(VW, VH))
            .arg("--dump-dom")
            .arg(format!("file://{}", tmp.display()))
            .output()
            .expect("run chrome");
        let dom = String::from_utf8_lossy(&out.stdout);
        let vp = dom
            .split("VP:")
            .nth(1)
            .and_then(|r| r.split('<').next())
            .map(str::trim)
            .and_then(|r| r.split_once('x'))
            .and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)));
        let Some((cw, ch)) = vp else {
            // An unreadable probe is an unmeasured gate, and it must say so rather than pass.
            panic!(
                "the reference did not report a viewport — this gate measured NOTHING, which is not \
                 the same as agreeing. dump was {} bytes",
                dom.len()
            );
        };

        // What OUR engine offers a `width:100%` child of the initial containing block at the same
        // viewport: the requested width, less whatever gutter the UA metric reserves.
        let ours_w = VW as f32 - manuk_layout::scrollbar_gutter(manuk_css::ScrollbarWidth::Auto);
        assert_eq!(
            cw as f32,
            ours_w,
            "the reference lays out at {cw}px of inline space and our engine at {ours_w}px. A \
             {}px difference in the INITIAL CONTAINING BLOCK is charged to the engine on every \
             percentage width, every text wrap and every `dy` beneath it, on every site in the \
             corpus — see REFERENCE_HIDES_SCROLLBARS.",
            (cw as f32 - ours_w).abs()
        );
        assert_eq!(
            ch, VH,
            "the reference's LAYOUT viewport is {ch}px tall, not the {VH}px it was asked for — \
             `viewport_chrome_offset` is meant to convert the window size into exactly this and \
             has stopped doing so"
        );
    }
}
