//! **G_SUPPORTS_HONESTY — `@supports` and `CSS.supports()` answer for what we RENDER, not for what
//! Stylo can parse.**
//!
//! A progressive-enhancement branch is a **bet on the browser's answer**. When a page writes
//!
//! ```css
//! @supports (backdrop-filter: blur(8px)) { .bar { background: rgba(255,255,255,.4) } }
//! ```
//!
//! a "yes" makes it throw away the opaque fallback it wrote for browsers that cannot blur — and if
//! the blur never happens, the user gets unreadable text over a photo. **A false "yes" is strictly
//! worse than a "no"**, because a "no" keeps a working page. That is the standing rule in this repo
//! (`honest-answer-is-not-a-fixed-answer`), and this gate is where it is enforced for CSS.
//!
//! ## The lie, and where it came from
//!
//! Stylo's servo build hides 35 longhands behind one shared pref, `layout.unimplemented`. The
//! cascade flips that pref on because **four** of the 35 are properties we really do render —
//! `user-select`, `color-scheme`, `mask-image`, `text-overflow` — and without the flip Stylo drops
//! them at parse time. The flip's comment claimed the other 31 were harmless: *"we consume a fixed
//! set of computed values via explicit `clone_*` calls, so enabling the other properties it also
//! ungates changes nothing we read."*
//!
//! **`@supports` reads them.** Enabling a property for the cascade also makes it *parse*, and both
//! `@supports` (via Stylo's `SupportsRule::enabled`) and `CSS.supports()` (via `supports_condition`)
//! answer "does this parse?". So the flip silently promised `backdrop-filter`, `view-transition-name`,
//! `offset-path`, `contain`, `zoom`, the eight `corner-*-shape`s and the whole `mask-*` family — 31
//! properties this engine does not read at all.
//!
//! Which four are real was **measured, not recalled**: a property is honest here only if it reaches a
//! `ComputedStyle` field. Three of the four arrive through the MinimalCascade recovery block rather
//! than a `clone_*` accessor, which is exactly why a grep for `clone_*` under-counts them and why the
//! list is asserted here rather than left as a comment.
//!
//! ## What each claim catches
//!
//! - **`vtn`/`bdf`/`op`** — the lie itself, on three different unread properties.
//! - **`us`/`csch`/`mi`/`to`** — the guard, and the reason this cannot be fixed by simply not
//!   flipping the pref: those four must keep answering **yes**, because we really do render them.
//! - **`flex`/`nope`** — ordinary properties are untouched; the fix is not a blanket "no".
//! - **`notvtn`** — negation still resolves through Stylo (`not (unsupported)` is **true**), which is
//!   the case a naive "does the text mention a banned property?" filter gets backwards.
//! - **`applied`/`skipped`** — the same verdict reaches the CASCADE. `CSS.supports()` and
//!   `@supports` disagreeing about one declaration is the tick-282 bug in a new place.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  #enh { color: rgb(1, 2, 3); }
  /* A property we DO render: the branch must be taken. */
  @supports (user-select: none) { #enh { color: rgb(0, 128, 0); } }
  /* A property we do NOT render: the branch must NOT be taken, or the page has thrown away a
     fallback that was working. */
  @supports (backdrop-filter: blur(4px)) { #enh { color: rgb(255, 0, 0); } }
</style></head><body>
<div id="enh">enhanced?</div>
<div id="out">-</div>
<script>
  var R = [];
  var s = function(c) { return CSS.supports(c); };
  // ── The lie: three properties Stylo parses under the pref and this engine never reads.
  R.push('vtn:' + s('view-transition-name: none'));
  R.push('bdf:' + s('backdrop-filter: blur(4px)'));
  R.push('op:'  + s('offset-path: none'));
  // ── The guard: the four that are genuinely rendered must still answer yes.
  R.push('us:'   + s('user-select: none'));
  R.push('csch:' + s('color-scheme: dark'));
  R.push('mi:'   + s('mask-image: url(a.svg)'));
  R.push('to:'   + s('text-overflow: ellipsis'));
  // ── Ordinary properties are untouched — the fix must not be a blanket "no".
  R.push('flex:' + s('display: flex'));
  R.push('nope:' + s('notaproperty: 1'));
  // ── Negation resolves through Stylo: `not (unsupported)` is TRUE.
  R.push('notvtn:' + s('not (view-transition-name: none)'));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

fn color_of(page: &manuk_page::Page, sel: &str) -> (u8, u8, u8) {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let c = page
        .styles_of(n)
        .unwrap_or_else(|| panic!("no style for {sel}"))
        .color;
    (c.r, c.g, c.b)
}

#[test]
fn supports_answers_for_what_we_render_not_for_what_stylo_parses() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://supports.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SUPPORTS HONESTY: {got}");

    for (claim, why) in [
        (
            "vtn:false",
            "`view-transition-name` is one of 35 longhands Stylo hides behind `layout.unimplemented`. \
             The cascade flips that pref for the FOUR of them we really render; the other 31 became \
             parseable as a side effect, and `CSS.supports()` reports parseability. A page told YES \
             here takes its view-transition branch against an engine that never reads the property",
        ),
        (
            "bdf:false",
            "`backdrop-filter` is the costliest instance: a page that believes us drops the OPAQUE \
             fallback it wrote for browsers that cannot blur, and its text lands unreadable over a \
             photo. A false yes is strictly worse than a no, because a no keeps a working page",
        ),
        ("op:false", "`offset-path` — same class, a third unread property"),
        (
            "us:true",
            "THE GUARD, and it is why this cannot be fixed by simply not flipping the pref: \
             `user-select` IS rendered here (tick 464 flipped the pref precisely for it), so it must \
             keep answering yes. A blanket no would trade one lie for a bigger one",
        ),
        (
            "csch:true",
            "`color-scheme` reaches a `ComputedStyle` field — rendered, so honest",
        ),
        (
            "mi:true",
            "`mask-image` is rendered (recovered through the MinimalCascade block, not a `clone_*` \
             accessor — which is why a grep for `clone_*` under-counts the honest set)",
        ),
        (
            "to:true",
            "`text-overflow` likewise — the `…` on a clipped single-line title",
        ),
        (
            "flex:true",
            "an ordinary implemented property is untouched by the fix",
        ),
        ("nope:false", "nonsense is still nonsense"),
        (
            "notvtn:true",
            "`not (<unsupported>)` is TRUE, and it must stay resolved by Stylo's own condition \
             parser. This is the case a naive \"does the condition text mention a banned property?\" \
             filter gets exactly backwards",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_SUPPORTS_HONESTY: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }

    // ── The same verdict must reach the CASCADE. `CSS.supports()` and `@supports` answering
    //    differently about one declaration is the tick-282 bug wearing new clothes: whichever the
    //    page consults, it gets a different browser.
    assert_eq!(
        color_of(&page, "#enh"),
        (0, 128, 0),
        "G_SUPPORTS_HONESTY: the `@supports (user-select: none)` branch must APPLY (green) and the \
         `@supports (backdrop-filter: blur(4px))` branch must NOT (red). Getting red means the \
         cascade took an enhancement branch for a property it does not render — the same lie as \
         `CSS.supports()` telling the page yes, but with the fallback already thrown away."
    );
}
