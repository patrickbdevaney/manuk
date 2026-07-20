//! **G_MATCH_MEDIA_AGREES — `matchMedia` and the `@media` cascade are one evaluator.**
//!
//! This is a **consistency** gate, not a coverage gate, and the distinction is the whole point. It
//! does not check that `matchMedia` returns something, or even that it returns the *right* thing in
//! isolation. It checks that **the answer JS gets and the answer the cascade acts on are the same
//! answer**, by styling an element with a query and asking JS about the identical query.
//!
//! A browser is allowed to be unusual — it may legitimately report no hover, a coarse pointer, a
//! dark scheme. It is **not** allowed to disagree with itself, and a page cannot route around a
//! browser that does. The idiom on the real web is a component that reads
//! `matchMedia('(max-width: 700px)').matches` to decide whether to mount the mobile tree, while the
//! stylesheet decides the layout with the same breakpoint. When those two disagree the page renders
//! a combination no designer ever specified — the desktop grid holding the mobile component, or a
//! drawer that is open in JS and off-screen in CSS. Nothing throws.
//!
//! ## The two evaluators that produced it
//!
//! The JS prelude carried its own media-query implementation: a small feature table, no `not`, no
//! `only`, no range syntax, and — the load-bearing difference — `default: return true` for any
//! feature it did not recognise. The CSS side answers `false` for an unknown feature, per CSS's own
//! error handling. **The two defaults were exact opposites**, so every feature the prelude had not
//! heard of was a guaranteed disagreement: `matchMedia('(hover: none)')` said `true` while
//! `@media (hover: none)` did not apply.
//!
//! `matchMedia` is now `__matchMedia`, a host binding onto the same `manuk_css::media_matches` the
//! cascade calls. This is the third time this repository has been bitten by a second source of
//! truth for one question (`UA_CSS` vs `apply_ua_defaults`, the Stylo/Minimal property split), and
//! each time the second copy is the one that goes stale. The fix is not to synchronise them.
//!
//! ## The RED probe (run, not imagined)
//!
//! Restoring the prelude's own `__evalMediaFeature` flips `agreeHover` and `agreeUnknown` — the two
//! claims carrying features its table did not list — while every width claim stays green. Those
//! width claims are exactly what a hand-written second evaluator gets right, which is why a gate
//! that only tested widths could not see this.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  @media (max-width: 700px)         { #a { visibility: hidden } }
  @media (min-width: 700px)         { #b { visibility: hidden } }
  @media (hover: none)              { #c { visibility: hidden } }
  @media (nonsense-feature: 3)      { #d { visibility: hidden } }
  @media not print                  { #e { visibility: hidden } }
  @media (width >= 640px)           { #f { visibility: hidden } }
</style></head><body>
<div id="a"></div><div id="b"></div><div id="c"></div>
<div id="d"></div><div id="e"></div><div id="f"></div>
<div id="out">-</div>
<script>
  var R = [], Q = {
    a: '(max-width: 700px)', b: '(min-width: 700px)', c: '(hover: none)',
    d: '(nonsense-feature: 3)', e: 'not print', f: '(width >= 640px)'
  };
  for (var k in Q) R.push(k + '=' + (matchMedia(Q[k]).matches ? '1' : '0'));
  // `media` must echo the query back, and the object must be MediaQueryList-shaped.
  R.push('shape:' + (typeof matchMedia('screen').addEventListener === 'function' &&
                     matchMedia('screen').media === 'screen'));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn matchmedia_and_the_media_cascade_give_the_same_answer() {
    let fonts = FontContext::new();
    // 1280px wide: max-width:700 is false, min-width:700 and width>=640 are true.
    let page = manuk_page::Page::load(HTML, "https://mm.test/", &fonts, 1280.0);
    let dom = page.dom();
    let got = dom.text_content(manuk_css::query_selector_all(dom, dom.root(), "#out")[0]);

    assert!(
        got.contains("shape:true"),
        "G_MATCH_MEDIA_AGREES: matchMedia must return a MediaQueryList-shaped object that echoes \
         its query — got: {got}"
    );

    for (id, why) in [
        (
            "a",
            "`(max-width: 700px)` at a 1280px viewport — the plain breakpoint case, which any \
             hand-written second evaluator also gets right",
        ),
        (
            "b",
            "`(min-width: 700px)` — the other half of the same breakpoint",
        ),
        (
            "c",
            "`(hover: none)` — a feature the JS prelude's table did not list, so its `unknown → \
             true` default answered the OPPOSITE of the cascade's `unknown → false`. This is the \
             claim the bug lived in",
        ),
        (
            "d",
            "an unrecognised feature must be false on BOTH sides — 'we don't know' is never a \
             reason to apply a stylesheet",
        ),
        (
            "e",
            "`not print` — negation, which the prelude could not parse at all",
        ),
        (
            "f",
            "`(width >= 640px)` range syntax — modern CSS, and the prelude's regex rejected it",
        ),
    ] {
        // What the CASCADE did: the rule hides the element, so hidden == the query matched.
        let node = manuk_css::query_selector_all(dom, dom.root(), &format!("#{id}"))[0];
        let css_matched =
            page.styles_of(node).map(|s| s.visibility) != Some(manuk_css::Visibility::Visible);
        // What JS was TOLD.
        let js_matched = got.contains(&format!("{id}=1"));
        assert_eq!(
            js_matched,
            css_matched,
            "G_MATCH_MEDIA_AGREES agree{}: matchMedia says {js_matched} but the @media cascade \
             acted on {css_matched} for the identical query.\n  got: {got}\n\n  {why}.",
            id.to_uppercase()
        );
    }
}
