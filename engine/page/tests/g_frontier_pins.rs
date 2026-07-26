//! **G_FRONTIER_PINS — the last eight unknowns, measured and pinned.**
//!
//! Surface audit #33 established that probing the unknowns is the only genuinely open CO-#1 letter.
//! t601 took 17 → 8; this takes 8 → 0. The value of a pin is not the verdict, it is that **nobody
//! re-derives it**: three of t601's unknowns turned out to be already built, and the only way that
//! stays known is a gate that fails when it stops being true.
//!
//! ## What is pinned, and how each was measured
//!
//! **Honestly ABSENT, and the `CSS.supports` answer already says so** — `shape()`, multicol L2
//! (`column-wrap`/`column-height`), `animation-composition`, and the 2026 frontier bundle (`if()`,
//! `text-fit`, `progress()`, `@function`). These are pinned as **no** in both directions: absent
//! *and* honestly reported. If one lands without the answer changing, this gate goes red — which is
//! the failure mode t601 caught on `zoom`, where a working capability was denied for hundreds of
//! ticks.
//!
//! **`shape()` is the interesting one of that group**, because t593 landed `clip-path` for real. The
//! basic shapes clip; `shape()` maps to `None` (honestly unclipped) and `CSS.supports` says no. That
//! is the *narrower* lie t593's own row named as residue, now pinned rather than remembered.
//!
//! **FedCM: absent — and the naive feature-detect is MISLEADING.** `navigator.credentials.get` **is**
//! a function (it arrived with WebAuthn at t484), so a page probing `navigator.credentials` alone
//! concludes FedCM is available. `IdentityCredential` and `DigitalCredential` are the honest signal
//! and both are `undefined`. This is the same shape as t601's OPFS finding — `navigator.storage`
//! exists, `getDirectory` does not — and it is worth stating as a class: **a namespace existing is
//! not the capability existing, and the sub-capability check is the only honest one.**
//!
//! **Per-glyph font fallback across scripts: measured WORKING, at the advance level.** At 20px:
//! Latin `Hello` 44px, CJK `日本語` **60px = exactly 3em** (the correct full-width advance), emoji
//! 50px, Arabic `مرحبا` 42px (shaped — joined forms are narrower than five isolated glyphs), and an
//! empty control at 0. Four scripts, four script-appropriate answers, none of them a uniform
//! `.notdef` box. That is strong evidence a script-capable face is selected per run — the t557/t558
//! font arc holding — and it is deliberately recorded as **partial**, not `works`: distinct advances
//! prove *different faces with different metrics*, not that every glyph is the right glyph. Pinning
//! the claim I actually measured is the whole point.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="font-size:20px">
<span id="lat">Hello</span><br><span id="cjk">日本語</span><br><span id="emoji">😀😀</span><br>
<span id="rtl">مرحبا</span><br><span id="empty"></span>
<div id="out">-</div>
<script>
  var R = [], w = function(id){ return Math.round(document.getElementById(id).getBoundingClientRect().width); };
  // ── The frontier set: absent, and honestly reported as absent.
  R.push('shape=' + CSS.supports('clip-path','shape(from 0% 0%, line to 100% 0%)'));
  R.push('colWrap=' + CSS.supports('column-wrap','wrap'));
  R.push('colHeight=' + CSS.supports('column-height','50px'));
  R.push('animComp=' + CSS.supports('animation-composition','add'));
  R.push('cssIf=' + CSS.supports('width','if(style(--x): 1px; else: 2px)'));
  R.push('textFit=' + CSS.supports('text-fit','per-line'));
  R.push('atFunction=' + (typeof CSSFunctionRule));
  // ── FedCM: the namespace exists (WebAuthn), the capability does not.
  R.push('credGet=' + (navigator.credentials && typeof navigator.credentials.get));
  R.push('identityCred=' + (typeof IdentityCredential));
  R.push('digitalCred=' + (typeof DigitalCredential));
  // ── Per-script advances: four scripts, four different script-appropriate widths.
  R.push('lat=' + w('lat'));
  R.push('cjk=' + w('cjk'));
  R.push('emoji=' + w('emoji'));
  R.push('rtl=' + w('rtl'));
  R.push('empty=' + w('empty'));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn the_last_unknowns_are_measured_not_guessed() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://frontier.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("FRONTIER PINS: {got}");

    for (claim, why) in [
        (
            "shape=false",
            "`clip-path: shape()` is absent AND honestly reported. t593 landed the basic shapes and \
             mapped `shape()`/`path()` to None — this pins the narrower residue that row named, so \
             that if `shape()` ever lands and the answer does not follow, it goes red HERE. That is \
             exactly the state `zoom` was in for hundreds of ticks",
        ),
        ("colWrap=false", "multicol L2 `column-wrap` — absent, honestly"),
        ("colHeight=false", "…and `column-height`"),
        ("animComp=false", "`animation-composition` — absent, honestly"),
        ("cssIf=false", "CSS `if()` — the 2026 frontier bundle, absent"),
        ("textFit=false", "`text-fit` — same bundle"),
        ("atFunction=undefined", "`@function` / `CSSFunctionRule` — same bundle"),
        (
            "credGet=function",
            "**THE MISLEADING PART, PINNED DELIBERATELY.** `navigator.credentials.get` IS a function \
             — it arrived with WebAuthn (t484) — so a page probing `navigator.credentials` alone \
             concludes FedCM is available. Recording that this is TRUE is what makes the next two \
             claims meaningful rather than a shrug",
        ),
        (
            "identityCred=undefined",
            "…and FedCM proper is absent. `IdentityCredential` is the honest signal. Same class as \
             t601's OPFS finding (`navigator.storage` exists, `getDirectory` does not): **a \
             namespace existing is not the capability existing**",
        ),
        ("digitalCred=undefined", "Digital Credentials likewise absent"),
        (
            "cjk=60",
            "**CJK gets its correct full-width advance: 3 glyphs at 20px = exactly 60px.** A run \
             rendered with `.notdef` boxes from a Latin face would not land on 3em. This is the \
             t557/t558 named-family fix still holding",
        ),
        (
            "lat=44",
            "Latin `Hello` at 20px — a proportional advance, and notably NOT 5em, so the CJK number \
             above is not simply 'every glyph is one em'",
        ),
        (
            "rtl=42",
            "Arabic `مرحبا` is SHAPED — five characters render narrower than five isolated glyphs, \
             which is what joining forms do and what a fallback-to-boxes would not do",
        ),
        (
            "empty=0",
            "the empty control — without it, four similar numbers could be four instances of one \
             constant rather than four measurements",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_FRONTIER_PINS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
