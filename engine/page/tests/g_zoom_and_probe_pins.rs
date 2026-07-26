//! **G_ZOOM_AND_PROBE_PINS — one probe found a false NO and a false YES, in opposite directions.**
//!
//! Surface audit #33 established that probing the remaining unknowns is the only genuinely open
//! CO-#1 letter. This is that work, and the two findings it produced are mirror images:
//!
//! **`zoom` was a FALSE NO.** It sat on `PARSE_ONLY_LONGHANDS`, so `CSS.supports('zoom', '2')`
//! answered **false** — and `zoom` has worked the whole time. **Stylo applies it inside its own
//! length computation** (`effective_zoom`), so it takes effect without this engine reading a `zoom`
//! field at all. Measured end to end: a `zoom: 2` 50px box lays out at 100px, its `font-size: 10px`
//! computes to 20px, and a 20px CHILD comes out at 40px — geometry, typography and inheritance.
//!
//! **A false NO is not the harmless direction.** This project has spent four ticks on the danger of a
//! false yes (a page drops the fallback it shipped). The mirror costs a page its *enhancement*: it
//! feature-detects, is told no, and takes a degraded path against an engine that implements the
//! thing. Both are the answer not matching the engine, which is the only thing
//! `honest-answer-is-not-a-fixed-answer` actually forbids.
//!
//! **`text-justify` was a FALSE YES**, found by the same probe: parsed natively by Stylo's servo
//! build, read by nothing here, and `CSS.supports` said yes. The t591 category exactly.
//!
//! ## What else the probe pinned, so the next audit does not re-derive it
//!
//! ```text
//! window.screen + screen.orientation   ALREADY WORKS  (800/800/24, "landscape-primary")
//! display: inline flex (multi-keyword) ALREADY WORKS  (computes to inline-flex)
//! reportError                          WORKS          — but ReportingObserver is absent (partial)
//! counter-increment / counter-reset    genuinely parse-only: `content: counter(x)` renders nothing
//! @counter-style                       at-rule NOT honoured — the marker stays `disc`
//! OPFS / File System Access            absent
//! Speculation Rules / prerendering     absent
//! ```
//!
//! Two of those were carried as `unknown` and **already worked** — which is the standing lesson that
//! constellation rows run *stale-pessimistic*, collected again.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  #z  { zoom: 2; width: 50px; height: 50px; font-size: 10px }
  #zk { width: 20px; height: 20px }
  #n  { width: 50px; height: 50px; font-size: 10px }
</style></head><body>
<div id="z"><div id="zk"></div></div>
<div id="n"></div>
<div id="out">-</div>
<script>
  var R = [], rect = function(id){ return document.getElementById(id).getBoundingClientRect(); };
  var cs = function(id, p){ return getComputedStyle(document.getElementById(id))[p]; };
  // ── `zoom` is REAL: geometry, typography, and inheritance into the child.
  R.push('zW=' + Math.round(rect('z').width));
  R.push('nW=' + Math.round(rect('n').width));
  R.push('zFont=' + cs('z','fontSize'));
  R.push('nFont=' + cs('n','fontSize'));
  R.push('zKidW=' + Math.round(rect('zk').width));
  // ── …so the honest answer is YES, and `text-justify` (read by nothing) is NO.
  R.push('supZoom=' + CSS.supports('zoom','2'));
  R.push('supTj=' + CSS.supports('text-justify','inter-word'));
  // ── The pins: capabilities measured this tick, so a later audit does not re-derive them.
  R.push('screen=' + (screen && screen.width > 0 && screen.colorDepth > 0));
  R.push('orient=' + (screen && screen.orientation && screen.orientation.type));
  R.push('mkDisplay=' + (function(){ var d=document.createElement('div');
      d.style.display='inline flex'; document.body.appendChild(d);
      return getComputedStyle(d).display; })());
  R.push('reportError=' + (typeof reportError));
  R.push('ReportingObserver=' + (typeof ReportingObserver));
  R.push('opfs=' + (navigator.storage && typeof navigator.storage.getDirectory));
  R.push('prerender=' + (typeof document.prerendering));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn zoom_is_real_and_the_supports_answer_now_matches_the_engine() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://zoom.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("ZOOM/PINS: {got}");

    for (claim, why) in [
        // ── The unzoomed control FIRST: without it, "zW=100" could be any layout bug at all.
        (
            "nW=50",
            "the UNZOOMED control must be its authored 50px — otherwise `zW=100` proves nothing \
             about zoom and only that some box is 100px wide",
        ),
        (
            "nFont=10px",
            "…and its font-size must be the authored 10px, for the same reason",
        ),
        (
            "zW=100",
            "`zoom: 2` must DOUBLE the box: 50px lays out at 100px. This is Stylo's own \
             `effective_zoom` inside length computation — no `zoom` field is read by this engine, \
             which is exactly why the capability was invisible to a source-grep and had to be \
             measured",
        ),
        (
            "zFont=20px",
            "zoom scales TYPOGRAPHY too, not just boxes — `font-size: 10px` computes to 20px. A \
             geometry-only zoom would pass the box claim and render every zoomed page with \
             correctly-sized boxes full of wrongly-sized text",
        ),
        (
            "zKidW=40",
            "and it INHERITS: a 20px child inside a `zoom: 2` parent is 40px. This is the claim that \
             separates a real zoom from a one-element transform",
        ),
        (
            "supZoom=true",
            "**THE FALSE NO, CORRECTED.** `zoom` was on the parse-only denylist, so `CSS.supports` \
             said no about a capability that has worked the whole time. A false no costs a page its \
             ENHANCEMENT branch exactly as a false yes costs it its fallback — both are the answer \
             not matching the engine",
        ),
        (
            "supTj=false",
            "**THE FALSE YES, CORRECTED**, found by the same probe and pointing the other way. \
             `text-justify` is parsed natively by Stylo and read by nothing here",
        ),
        // ── Pins: measured this tick so the next audit does not re-derive them.
        (
            "screen=true",
            "`window.screen` was carried as UNKNOWN and ALREADY WORKS (width, colorDepth). \
             Constellation rows run stale-PESSIMISTIC — collected again",
        ),
        (
            "orient=landscape-primary",
            "`screen.orientation.type` likewise — the Screen Orientation half of the same unknown \
             row already answers",
        ),
        (
            "mkDisplay=inline-flex",
            "multi-keyword `display: inline flex` (CSS Display 3) already computes correctly — a \
             third unknown that was already built",
        ),
        (
            "reportError=function",
            "`reportError` works, which is why the Reporting API row is PARTIAL and not missing — \
             the two halves needed splitting rather than one flat verdict",
        ),
        (
            "ReportingObserver=undefined",
            "…and the observer half is genuinely absent. Pinning both is what makes `partial` a \
             measurement instead of a shrug",
        ),
        (
            "opfs=undefined",
            "OPFS / File System Access absent — `navigator.storage` exists but `getDirectory` does \
             not, which is the shape that makes a feature-detect on the namespace alone wrong",
        ),
        (
            "prerender=undefined",
            "Speculation Rules / `document.prerendering` absent",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_ZOOM_AND_PROBE_PINS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
