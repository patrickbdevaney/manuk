//! **G_SUPPORTS_HONESTY — `@supports` and `CSS.supports()` answer for what we RENDER, not for what
//! Stylo can parse.**
//!
//! A progressive-enhancement branch is a **bet on the browser's answer**. When a page writes
//!
//! ```css
//! @supports (isolation: isolate) { .layer { /* enhancement that needs an unread property */ } }
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
  @supports (isolation: isolate) { #enh { color: rgb(255, 0, 0); } }
</style></head><body>
<div id="enh">enhanced?</div>
<div id="out">-</div>
<script>
  var R = [];
  var s = function(c) { return CSS.supports(c); };
  // ── The lie: three properties Stylo parses under the pref and this engine never reads.
  R.push('vtn:' + s('view-transition-name: none'));
  R.push('op:'  + s('offset-path: none'));
  // ── The SECOND category (t591): parsed NATIVELY, behind no pref, still not rendered.
  R.push('wm:' + s('writing-mode: vertical-rl'));
  // ── The guard: the ones that are genuinely rendered must still answer yes.
  R.push('flt:'  + s('filter: blur(4px)'));
  R.push('clip:' + s('clip-path: circle(50%)'));
  R.push('mbm:'  + s('mix-blend-mode: multiply'));
  R.push('bdf:'  + s('backdrop-filter: blur(4px)'));
  R.push('us:'   + s('user-select: none'));
  R.push('csch:' + s('color-scheme: dark'));
  R.push('mi:'   + s('mask-image: url(a.svg)'));
  R.push('to:'   + s('text-overflow: ellipsis'));
  // ── Ordinary properties are untouched — the fix must not be a blanket "no".
  R.push('flex:' + s('display: flex'));
  R.push('nope:' + s('notaproperty: 1'));
  // ── Negation resolves through Stylo: `not (unsupported)` is TRUE.
  R.push('notvtn:' + s('not (view-transition-name: none)'));

  // ══ THE MIRROR IMAGE (tick 1180) — a false NO about a property we DO render. ══════════════
  // NEGATIVE ROWS FIRST. An allowlist keyed on the property NAME would pass every row below by
  // answering yes to anything, so these are what make it a capability claim rather than a
  // blanket promise — and the value half is the half a setter will trust (tick 1177).
  R.push('sw-bad:'   + s('scrollbar-width: banana'));
  R.push('sc-one:'   + s('scrollbar-color: red'));          // needs BOTH thumb and track
  R.push('sc-bad:'   + s('scrollbar-color: red banana'));
  R.push('sst-bad:'  + s('scroll-snap-type: diagonal'));
  R.push('sst-two:'  + s('scroll-snap-type: x banana'));    // good axis, bad strictness
  R.push('ssa-bad:'  + s('scroll-snap-align: middle'));
  R.push('lc-bad:'   + s('-webkit-line-clamp: banana'));
  R.push('lc-zero:'  + s('-webkit-line-clamp: 0'));         // `<integer>` must be >= 1
  R.push('lc-empty:' + s('-webkit-line-clamp:'));
  // Still-honest NOs: the constellation says `missing`/`unknown`, so these must NOT be swept in
  // by a fix aimed at their neighbours.
  R.push('tw:'  + s('text-wrap: balance'));
  R.push('cv:'  + s('content-visibility: auto'));
  R.push('bo:'  + s('-webkit-box-orient: vertical'));
  R.push('an:'  + s('anchor-name: --a'));
  // ── THE SUBJECT: five properties this engine renders through the MinimalCascade merge.
  R.push('sw:'  + s('scrollbar-width: thin'));
  R.push('sc:'  + s('scrollbar-color: red blue'));
  R.push('scf:' + s('scrollbar-color: rgb(255 0 0) rgb(0 0 255)'));   // functional notation
  R.push('sst:' + s('scroll-snap-type: x mandatory'));
  R.push('sstn:'+ s('scroll-snap-type: none'));             // the INITIAL value is still a value
  R.push('ssa:' + s('scroll-snap-align: start'));
  R.push('lc:'  + s('-webkit-line-clamp: 3'));
  R.push('lcu:' + s('line-clamp: 3'));                      // the unprefixed spelling too
  R.push('lcn:' + s('-webkit-line-clamp: none'));
  R.push('swi:' + s('scrollbar-width: initial'));           // CSS-wide keywords are always valid
  // ── COMPOSITION: the allowlist goes through the same tree rewrite as the denylist, so Stylo —
  //    not hand-rolled boolean logic — resolves `not`/`and`/`or`. A text filter gets these
  //    backwards, which is the exact bug the denylist half was written to avoid.
  R.push('notsw:'    + s('not (scrollbar-width: thin)'));       // false — we DO support it
  R.push('notswbad:' + s('not (scrollbar-width: banana)'));     // true  — we do not
  R.push('andsw:'    + s('(scrollbar-width: thin) and (display: flex)'));
  R.push('andmix:'   + s('(scrollbar-width: thin) and (isolation: isolate)'));
  R.push('orsw:'     + s('(scrollbar-width: banana) or (scroll-snap-align: start)'));
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
            "bdf:true",
            "**THE ONE THIS GATE WAS WRITTEN AROUND, retired at tick 595.** `backdrop-filter` was the \
             costliest possible false yes — a page told yes drops the OPAQUE fallback it shipped for \
             engines that cannot blur and its text lands unreadable over a photo — and it stayed an \
             honest NO for three ticks while `filter`, `clip-path` and `mix-blend-mode` landed \
             around it, because it needs a different INPUT (what is painted behind the element) and \
             not merely a different operation. It is now genuinely rendered (G_BACKDROP_FILTER), so \
             the yes is true. If the rendering is ever lost this must go back to `false` in the same \
             commit — the whole point of this file is that the answer follows the capability",
        ),
        ("op:false", "`offset-path` — same class, a third unread property"),
        (
            "flt:true",
            "**THE ENTRY THAT CHANGED SIDES.** `filter` was the costliest member of the second \
             category — parsed natively, never read, 51.9% of page loads told YES about a blur we \
             could not draw. t591 made it an honest no; **tick 592 made it a true yes** (the \
             computed list reaches `manuk-paint`, which runs it over an offscreen group — see \
             G_FILTER_RENDER for the pixels). It sits with the guards now, and if the rendering is \
             ever lost this goes red HERE as well as there. `backdrop-filter` stays a no: it filters \
             what is painted BEHIND the element, a different input",
        ),
        (
            "clip:true",
            "`clip-path` (43.8% of page loads) followed `filter` across at tick 593 — the four basic \
             shapes clip the group's offscreen surface (G_CLIP_PATH). It is the same shape of move \
             and, notably, the same MECHANISM: the offscreen group t592 built for `filter` is \
             exactly the surface a clip mask applies to, which is why the second capability cost a \
             fraction of the first",
        ),
        (
            "mbm:true",
            "`mix-blend-mode` (12.9%) crossed at tick 594 — and it crossed CHEAPLY, out of the same \
             offscreen group t592 built for `filter`: a blend needs the group's own pixels separate \
             from the backdrop beneath them, which is exactly what that surface is. See \
             G_MIX_BLEND_MODE",
        ),
        ("wm:false", "`writing-mode` — the same, and the axis the CJK story stops short of"),
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
        // ══ THE MIRROR IMAGE (tick 1180) ══════════════════════════════════════════════════════
        //
        // Everything above subtracts a false YES. These add back a false NO — properties this
        // engine RENDERS that stylo 0.19's servo build cannot parse at all (`engine="gecko"`), so
        // Stylo's verdict was not merely wrong, it was uninformed. Measured by asking the engine
        // about its OWN computed value (`CSS.supports(p, cs[p])` over all 139 properties the
        // computed snapshot answers to), which needs no expectation column and so cannot repeat
        // t1177's error of writing *Chrome's* answer into a question about *this* engine.
        //
        // ⚠ This direction is a PRECONDITION, not a nicety. Tick 1177 stopped one step short of
        // validating `el.style`'s setter through this seam precisely because of these rows: with
        // the seam still answering no, `el.style.webkitLineClamp = 3` would become a **silent
        // no-op** and delete a shipped, gated capability.
        (
            "sw-bad:false",
            "**THE NEGATIVE ROW THAT MAKES THE ALLOWLIST A CAPABILITY CLAIM.** An allowlist keyed \
             on the property NAME alone would answer yes to `scrollbar-width: banana` — which is \
             t1177's `el.style.color = \"yelow\"` lie moved one layer down, and worse here because \
             a setter is about to trust this answer. ⚠ The value grammar is written out rather \
             than routed through `apply_declaration`: those arms are LENIENT BY DESIGN (a cascade \
             must not abort a page over a bad value), so reusing them would make every value valid",
        ),
        (
            "sc-one:false",
            "`scrollbar-color` takes TWO colours (thumb then track); one is a parse error",
        ),
        ("sc-bad:false", "…and a non-colour in either slot is too"),
        ("sst-bad:false", "`diagonal` is not a snap axis"),
        (
            "sst-two:false",
            "a GOOD axis with a BAD strictness keyword — the row that catches a validator which \
             stops reading after the first token",
        ),
        ("ssa-bad:false", "`middle` is `center`'s common misspelling, and it is invalid"),
        ("lc-bad:false", "`-webkit-line-clamp` takes `none` or an integer"),
        (
            "lc-zero:false",
            "`<integer>` must be >= 1. Zero is the value the LENIENT cascade arm accepts and \
             silently treats as unclamped — so this row is exactly where `supports` and the \
             cascade must be allowed to differ, and it is why the grammar is written twice on \
             purpose rather than shared",
        ),
        ("lc-empty:false", "an empty value is not a value"),
        (
            "tw:false",
            "**THE HONEST NOs, AND THEY ARE THE REASON THIS LIST IS FOUR ENTRIES AND NOT SIX.** \
             t1177 named six properties `CSS.supports` denies that Chrome supports. Four of the \
             six were the instrument being CORRECT: `CSS.supports` is a question about THIS \
             engine, and the constellation says `missing`/`unknown` for `text-wrap`, \
             `content-visibility`, `-webkit-box-orient` and `anchor-name`. A fix aimed at their \
             neighbours must not sweep them in",
        ),
        ("cv:false", "`content-visibility` — constellation says `missing`"),
        (
            "bo:false",
            "`-webkit-box-orient` — constellation says `missing`. ⚠ Its sibling `display: \
             -webkit-box` IS applied by the same merge (`legacy_webkit_box`), so the constellation \
             row is probably stale; recorded rather than acted on, because widening an allowlist \
             on a row I have not measured is how the false YES gets back in",
        ),
        ("an:false", "`anchor-name` — constellation says `unknown`, which is not a yes"),
        (
            "sw:true",
            "`scrollbar-width: thin` — rendered, gated by G_SCROLLBAR_THEME, and denied by \
             `CSS.supports` until this tick. A dark-mode page that feature-detects it keeps a \
             bright scrollbar on a dark UI",
        ),
        ("sc:true", "`scrollbar-color`, the other half of G_SCROLLBAR_THEME"),
        (
            "scf:true",
            "the SAME value in functional notation, with spaces INSIDE the parens — the row that \
             catches a validator splitting the value on every space instead of at top level",
        ),
        ("sst:true", "`scroll-snap-type: x mandatory` — gated by G_SCROLL_SNAP"),
        (
            "sstn:true",
            "⚠ `none` is the INITIAL value of `scroll-snap-type` and it is still a value. This row \
             kills the obvious cheap validator — *'apply it and see whether the computed style \
             changed'* — which would answer NO for every property asked about its own initial value",
        ),
        ("ssa:true", "`scroll-snap-align: start`"),
        (
            "lc:true",
            "`-webkit-line-clamp: 3`. ⚠ It is on this list by a DIFFERENT route from the other \
             four: it never reaches the computed snapshot, so the self-calibrating probe reported \
             it ABSENT rather than false. Its evidence is \
             `line_clamp_recovers_through_the_stylo_cascade`, not the probe",
        ),
        (
            "lcu:true",
            "the unprefixed `line-clamp` spelling answers the same. A page asks with whichever it \
             writes — the shorthand lesson surface audit #34 learned on the denylist half",
        ),
        ("lcn:true", "`none` unclamps, and is a valid value"),
        (
            "swi:true",
            "a CSS-wide keyword is valid on every property, and pages really do write \
             `@supports (scrollbar-width: initial)`",
        ),
        (
            "notsw:false",
            "**COMPOSITION, THE HALF A TEXT FILTER GETS BACKWARDS.** `not (<supported>)` is FALSE. \
             The allowlist goes through the SAME condition-tree rewrite as the denylist — the \
             declaration is replaced with one Stylo certainly supports and Stylo resolves the \
             surrounding operator — so `and`/`or`/`not` cost no new boolean logic",
        ),
        ("notswbad:true", "…and `not (<unsupported>)` is TRUE, on the same property"),
        ("andsw:true", "an allowlisted YES composes with an ordinary YES"),
        (
            "andmix:false",
            "an allowlisted YES `and` a denylisted NO is NO — the row where both halves of the \
             rewrite meet in one condition, which is the only place a precedence bug could hide",
        ),
        ("orsw:true", "an invalid VALUE `or` a valid one is YES"),
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
         `@supports (isolation: isolate)` branch must NOT (red). Getting red means the \
         cascade took an enhancement branch for a property it does not render — the same lie as \
         `CSS.supports()` telling the page yes, but with the fallback already thrown away."
    );
}
