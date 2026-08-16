//! **G_MEDIA_GRAMMAR — `not (anything-we-do-not-know)` was TRUE, and `or` was never a thing.**
//!
//! Media Queries Level 4 evaluates in more than two states, and the engine had one `bool`:
//!
//! * **`<general-enclosed>`** — a well-formed `( … )` block naming a feature this UA does not
//!   recognise — is **Unknown**, not false. MQ4 §3.2 gives it Kleene logic, so `not unknown` is
//!   **Unknown**, which is `false` at the top. Folding Unknown into `false` first and negating
//!   second answers **true**, and `@media not (some-feature-from-2029) { … }` applies a sheet
//!   written for a browser we are not. That is the whole bug in one line.
//! * **A grammar failure** is *"replaced with `not all`"* at the **whole-query** level, so it
//!   survives an enclosing `not`. `not )` is false; it is not the negation of a false thing.
//! * **`or` did not exist.** `split_media_terms` split on ` and ` only, so
//!   `(min-width:0) or (min-width:99999px)` was handed to the feature lookup as the single term
//!   `(min-width:0) or (min-width:99999px)`, whose outer parens were stripped by
//!   `strip_prefix('(') + strip_suffix(')')` into the nonsense `min-width:0) or (min-width:99999px`.
//!   Every `or` query in the world evaluated FALSE. Nested parens failed identically, because
//!   `((a) or (b)) and (c)` has the same shape.
//!
//! ⭐ **AND A `<media-condition>` IS NOT A `<media-query>`.** `sizes` takes the *condition*
//! production, which **cannot contain a media type** — so `sizes="not print 100vw, 1px"` is a **1px**
//! slot (the first entry is a grammar error and is discarded) where the identical text after
//! `@media` is a query that matches on screen. One string, two correct-and-different answers, and
//! `sizes` was asking the wrong production and fetching a different bitmap because of it.
//!
//! ⭐ **AND THE BOOLEAN CONTEXT ASKS A DIFFERENT QUESTION** (tick 1277). `( feature )` with no value
//! means *"is this feature ENGAGED?"* — MQ4 §2.4 — not *"does its default value match?"*. Reading
//! the two as one question inverted five features at once, and the loudest is the near-universal
//!
//! ```css
//! @media (prefers-reduced-motion) { *, *::before, *::after { animation: none !important } }
//! ```
//!
//! which we answered **true**, disabling every animation on the page on a browser that has no
//! reduced-motion preference at all. Its mirror is just as wrong in the other direction:
//! `(orientation)` and `(scripting)` have no "false" value in their sets and must match
//! unconditionally, and we answered false. **`(prefers-reduced-motion: reduce)` was always right,
//! which is precisely why this hid** — the common spelling works, so the rare one looks fine too.
//!
//! Why it is render-moving rather than conformance trivia: `@media` decides which stylesheet a real
//! site gets, `sizes` decides which bitmap an `<img>` gets, and `matchMedia` decides which branch
//! the page's own JS takes. All three are this one function.

use manuk_text::FontContext;

// 800px viewport. Every `<img>` offers a 1w and a 900w candidate, so the slot is a clean binary:
// a SMALL slot selects `a.png`, the 100vw fallback selects `b.png`.
const HTML: &str = r##"<!doctype html><html><head><style>
  /* The CSS path must reach the SAME evaluator as `matchMedia` — a page that branches in CSS and
     in JS on one query and gets two answers renders a layout no designer specified. */
  @media (min-width:0) or (min-width:99999px) { #m_or { visibility: hidden } }
  @media not (unknown-mf-name) { #m_notunknown { visibility: hidden } }
  @media screen { #m_screen { visibility: hidden } }
</style></head><body>
 <div id="m_or">a</div><div id="m_notunknown">b</div><div id="m_screen">c</div>

 <!-- ── THE CONDITION PRODUCTION. A media TYPE is a grammar error here, so the entry is dropped. -->
 <img id="s_notprint" srcset="/s/a.png 1w, /s/b.png 900w" sizes="not print 100vw, 1px">
 <img id="s_all"      srcset="/s/a.png 1w, /s/b.png 900w" sizes="all 100vw, 1px">
 <!-- `or`, and Kleene: True or Unknown is TRUE, so the 1px entry wins. -->
 <img id="s_or"       srcset="/s/a.png 1w, /s/b.png 900w" sizes="(min-width:0) or (unknown-mf-name) 1px">
 <!-- The core claim: `not <unknown>` is Unknown, so this entry is SKIPPED and 1px is taken. -->
 <img id="s_notunk"   srcset="/s/a.png 1w, /s/b.png 900w" sizes="not (unknown-mf-name) 100vw, 1px">
 <!-- …and the FUNCTION spelling of general-enclosed, `ident( … )`, which reaches Unknown by a
      different branch than the parenthesised-block spelling above. -->
 <img id="s_notfn"    srcset="/s/a.png 1w, /s/b.png 900w" sizes="not unknown-general-enclosed(foo) 100vw, 1px">
 <!-- CONTROLS from the tick before this one: first-match ordering must be untouched. -->
 <img id="s_first"    srcset="/s/a.png 1w, /s/b.png 900w" sizes="(min-width:0) 1px, 100vw">
 <img id="s_skip"     srcset="/s/a.png 1w, /s/b.png 900w" sizes="(min-width:99999px) 1px, 100vw">

 <div id="out">-</div>
 <script>
 window.addEventListener('load', function(){
   var Q=[
     // ── `or`, which never evaluated true before.
     ['or',      '(min-width:0) or (min-width:99999px)'],
     ['orf',     '(min-width:99999px) or (min-width:99998px)'],
     ['oru',     '(min-width:0) or (unknown-mf-name)'],
     ['uorf',    '(unknown-mf-name) or (min-width:99999px)'],
     ['andu',    '(min-width:0) and (unknown-mf-name)'],
     ['nest',    '((min-width:0) or (min-width:99999px)) and (min-width:0)'],
     // ⚠ Mixing `and` and `or` at one level without parens is a SYNTAX ERROR, not a precedence
     //   question — `a and b or c` has no agreed reading, so the spec refuses to guess.
     ['mix',     '(min-width:0) and (min-width:0) or (min-width:0)'],
     // ── `not` over things we do not understand. All four answered TRUE before.
     ['notu',    'not (unknown-mf-name)'],
     ['notge',   'not (unknown "general-enclosed")'],
     ['notfn',   'not unknown-general-enclosed(foo)'],
     ['notparen','not )'],
     ['notbang', 'not !'],
     // ── An out-of-range or unitless value makes the FEATURE invalid — i.e. Unknown, not false.
     ['negmin',  '(min-width:-1px)'],
     ['unitless','(min-width:600)'],
     // ⚠ …and a keyword outside the feature's own value set is invalid the same way. It only
     //   SHOWS under `not`: answering plain false here negates to a positive match.
     ['badkw',   '(orientation: sideways)'],
     ['notbadkw','not (orientation: sideways)'],
     // ── THE BOOLEAN CONTEXT: `( feature )` asks "is it ENGAGED?", not "does its default match?".
     //    `brm` is the loud one — `@media (prefers-reduced-motion) { * { animation: none } }` is a
     //    near-universal idiom and we answered TRUE, disabling every animation on the page.
     ['brm',     '(prefers-reduced-motion)'],
     ['bfc',     '(forced-colors)'],
     ['bcontr',  '(prefers-contrast)'],
     ['binv',    '(inverted-colors)'],
     //    …and its mirror: features with no "false" value in their set match unconditionally.
     ['borient', '(orientation)'],
     ['bscheme', '(prefers-color-scheme)'],
     ['bscript', '(scripting)'],
     ['bwidth',  '(width)'],
     //    A `min-`/`max-` prefix is a RANGE; with no value it is not a boolean feature at all.
     ['bminw',   '(min-width)'],
     //    A colon with nothing after it is a grammar error, NOT the boolean form.
     ['emptyval','(hover:)'],
     // ── CONTROLS. The query production keeps media types, the ordinary breakpoints that decide
     //    every real layout must answer exactly as they did before, and the VALUE form of the
     //    features whose BOOLEAN form moved must be untouched — `(prefers-reduced-motion: reduce)`
     //    was always right, which is exactly why the boolean bug hid.
     ['notprint','not print'],
     ['screen',  'screen'],
     ['zero',    '(min-width:0)'],
     ['wide',    '(max-width:99999px)'],
     ['vrm',     '(prefers-reduced-motion: reduce)'],
     ['vrmno',   '(prefers-reduced-motion: no-preference)'],
     ['vscheme', '(prefers-color-scheme: light)'],
     ['vhover',  '(hover: hover)']
   ], r=[];
   for (var i=0;i<Q.length;i++) r.push('q_'+Q[i][0]+'='+(matchMedia(Q[i][1]).matches?'1':'0'));
   var I=['s_notprint','s_all','s_or','s_notunk','s_notfn','s_first','s_skip'];
   for (var j=0;j<I.length;j++){
     var v=document.getElementById(I[j]).currentSrc;
     r.push(I[j]+'='+(v===''?'EMPTY':v.replace(/^https?:\/\/[^/]+\/s\//,'')));
   }
   document.getElementById('out').textContent=r.join(' ');
 });
 </script></body></html>"##;

fn hidden(page: &manuk_page::Page, sel: &str) -> &'static str {
    let n = manuk_css::query_selector_all(page.dom(), page.dom().root(), sel)[0];
    match page.styles_of(n).map(|s| s.visibility) {
        Some(manuk_css::Visibility::Visible) => "visible",
        _ => "hidden",
    }
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn media_conditions_evaluate_in_four_states_and_or_exists() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://mq.test/",
        &fonts,
        800.0,
    ));
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    let got = format!(
        "{} m_or={} m_notunknown={} m_screen={}",
        page.dom().text_content(hits[0]),
        hidden(&page, "#m_or"),
        hidden(&page, "#m_notunknown"),
        hidden(&page, "#m_screen"),
    );
    println!("MEDIA-GRAMMAR {got}");

    // ── PROVEN RED on EIGHT mutations. The moved-row sets below are the ones actually OBSERVED,
    // not the ones predicted; three predictions were wrong and are recorded as such, because
    // "the mutation did not move this row" is the only thing separating a row that pins a RULE
    // from a row that pins a COINCIDENCE.
    //
    // RED, run 1 — `<general-enclosed>` and an unknown feature return `False` instead of `Unknown`
    // (the old two-state answer). MOVED: `q_notu` `q_notge` `s_notunk` `s_notfn` `m_notunknown`.
    //   ⚠ `q_oru` did NOT move, and the prediction that it would was wrong: `True or False` and
    //     `True or Unknown` are both `True`, so an `or` row cannot see this leg at all.
    //   ⚠ `q_notfn` did NOT move either — the FUNCTION spelling reaches `Invalid` through the
    //     query production's ident check, never through `is_enclosed_function`. `s_notfn` was
    //     added to cover that branch once the run showed the hole.
    //   `q_uorf` and `q_andu` are unmoved by construction: their Kleene and two-state results
    //     coincide, which is what makes them the rows that say which other rows *can* discriminate.
    //
    // RED, run 2 — delete the ` or ` split from `eval_condition` (restore ` and `-only splitting).
    // MOVED: `q_or` `q_oru` `q_nest` `s_or` `m_or`. Overlaps run 1 nowhere and run 3 nowhere.
    //
    // RED, run 3 — let a grammar failure decay to `False` instead of staying absorbing `Invalid`.
    // MOVED: `q_notfn` `q_notparen` `q_notbang` `s_notprint`.
    //   ⚠ `s_all` did NOT move: `all` carries no `not`, so an absorbing and a plain false are the
    //     same answer there. It is run 4's row, not run 3's.
    //
    // RED, run 4 — route `sizes` back through `media_matches` (the QUERY production). MOVED:
    // `s_notprint` `s_all` — and NOTHING in the `q_*` block, which is precisely what proves the
    // condition/query split is a real distinction rather than a restatement of the same rule.
    //
    // RED, run 5 — drop the value-range filters (accept a negative length, accept a unitless
    // non-zero). MOVED: `q_negmin` `q_unitless`, and nothing else.
    //
    // RED, run 6 — the BOOLEAN context of the `no-preference`/`none` family answers `True` (the
    // pre-tick-1277 behaviour, where the boolean form was read as the value form's default).
    // MOVED: `q_brm` `q_bfc` `q_bcontr` `q_binv`. Every `v*` control holds, which is the point:
    // `(prefers-reduced-motion: reduce)` was ALWAYS right, and that is why the boolean bug hid.
    //
    // RED, run 7 — the boolean context of the features with no "false" value answers `False`.
    // MOVED: `q_borient` `q_bscheme` `q_bscript` `q_bwidth`. Disjoint from run 6 — the two halves
    // of the boolean context are independently pinned rather than one rule counted twice.
    //
    // RED, run 8 — `kw` drops its allowed-set check and compares the value directly.
    // MOVED: `q_notbadkw` alone.
    //   ⚠ `q_badkw` does NOT move, and it is kept for exactly that: an invalid keyword and a
    //     non-matching keyword are the same answer until a `not` is in front of them. The pair is
    //     the assertion; either row alone proves nothing.
    //
    // ⚠ `q_bminw` and `q_emptyval` are moved by NONE of the eight. They pin the two grammar edges
    // of the boolean form — a range prefix with no value, and a colon with nothing after it — and
    // no mutation here reaches them. Said out loud rather than left to look like coverage.
    //
    // ⚠⚠ RUN 1 HAD TO BE RE-APPLIED. Written as a replacement of the LAST `_ => Mq::Unknown` arm,
    // it went red on one row and left `q_notu`/`q_notge`/`m_notunknown` untouched — because this
    // tick added a SECOND such arm in the boolean block, and the scripted edit only hit one of
    // them. A partial mutation still goes red, and a red that is smaller than expected is the only
    // evidence that the mutation did not fully apply (t1239).
    assert_eq!(
        got,
        "q_or=1 q_orf=0 q_oru=1 q_uorf=0 q_andu=0 q_nest=1 q_mix=0 q_notu=0 q_notge=0 q_notfn=0 \
         q_notparen=0 q_notbang=0 q_negmin=0 q_unitless=0 q_badkw=0 q_notbadkw=0 q_brm=0 q_bfc=0 \
         q_bcontr=0 q_binv=0 q_borient=1 q_bscheme=1 q_bscript=1 q_bwidth=1 q_bminw=0 \
         q_emptyval=0 q_notprint=1 q_screen=1 q_zero=1 q_wide=1 q_vrm=0 q_vrmno=1 q_vscheme=1 \
         q_vhover=1 s_notprint=a.png s_all=a.png s_or=a.png s_notunk=a.png s_notfn=a.png \
         s_first=a.png s_skip=b.png m_or=hidden m_notunknown=visible m_screen=hidden",
        "MQ4 evaluates in four states and the engine had a `bool`. `q_notu`/`q_notge`/`q_notfn` are \
         the core claim: an unrecognised feature is UNKNOWN, so negating it is still unknown and \
         still false — they all answered TRUE before, which is a browser applying a stylesheet \
         written for a UA it is not. `q_or`/`q_nest` are the second: `or` and nested parens were \
         parsed by stripping the first and last characters, so every `or` query on the web was \
         false. `q_oru` needs both legs at once. `q_notparen`/`q_notbang` pin the ABSORBING \
         invalid state — a grammar failure is `not all` at the whole-query level and does not \
         un-negate. `q_mix` pins the refusal to guess a precedence the spec does not define. \
         `q_negmin`/`q_unitless` say an out-of-range value invalidates the FEATURE rather than \
         merely failing to match it. The `s_*` rows are the CONDITION production, which forbids \
         media types: `not print` and `all` are grammar errors inside `sizes` and the entry is \
         dropped, while `q_notprint` shows the same text is a matching QUERY after `@media` — the \
         two must DISAGREE or the productions are not being distinguished. `m_*` proves the CSS \
         cascade reaches the same evaluator as `matchMedia`. CONTROLS: `q_screen`, `q_zero`, \
         `q_wide` are the ordinary breakpoints every real layout turns on, and `s_first`/`s_skip` \
         are the previous tick's first-match rows held byte-identical"
    );
}
