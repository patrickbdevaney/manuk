//! # G_IS_WHERE_SELECTORS — `:is()` / `:where()` in `querySelector`, and a dropped selector matches NOTHING
//!
//! **`document.querySelectorAll('.a :is(.b, .c)')` returned an EMPTY LIST.** Not a partial answer —
//! nothing at all.
//!
//! `manuk_css`'s selector matcher (the one behind `querySelector`, `querySelectorAll`, `matches`
//! and `closest`) had a `Pseudo` enum with `Not` and `Has` and **no `Is`/`Where`**, so both fell
//! through to the parser's `_ => return None` arm — which drops the WHOLE selector, not just the
//! unknown part. One unsupported pseudo anywhere in a selector silently deletes the query.
//!
//! `:is()` and `:where()` are Baseline and are how every modern stylesheet and component library
//! writes a grouped rule — `.card :is(h1, h2, h3)`, `:where(.prose) a`. So the silence was broad,
//! and silent: no error, no warning, just an empty NodeList.
//!
//! ## The members are COMPLEX selectors, and that is the part a compound-only fix gets wrong
//!
//! `:is(.e + .f, .g > .b)` is legal — the list holds *complex* selectors, not compounds. `:not()`
//! here takes a single `Compound`, so copying its shape would have handled `:is(.b, .c)` and failed
//! `:is(.g > .b)` while looking finished. This reuses `parse_selector`/`selector_matches`, which is
//! exactly "does this complex selector match with that node as the subject".
//!
//! ## Forgiving, and why that matters more than it sounds
//!
//! An unparsable member is DROPPED and the rest still apply, so `:is(.a, 123)` still matches `.a`.
//! That is the whole point of a forgiving list: `:is()` **cannot take the query down** the way an
//! unknown pseudo does. Only an entirely unusable list fails the selector.
//!
//! ⚠ **`:where()` is folded into the same variant ON PURPOSE.** The two differ only in
//! SPECIFICITY — `:where()` contributes zero — and this matcher answers *"does it match"* for
//! `querySelector`/`matches`/`closest`, where specificity is never consulted. The live cascade is
//! Stylo's and computes specificity itself. Collapsing them anywhere specificity IS read would be
//! wrong, and the claim `whereZeroSpecificityNotOurJob` below records that boundary rather than
//! leaving it implicit.
//!
//! ## RED probes run against this gate
//!
//! Both were run, quoted with the values they produced.
//!
//! | mutation | result |
//! |---|---|
//! | restore the naive `text.split(',')` in `parse_selector_list` | RED — `isSimple:2` (not 3), `isComplex:1`, `nested:2` … and, most instructively, **`notComma:3` instead of `1`**: `:not(.b, .c, .e)` silently degrades to `:not(.b)` and matches **MORE** elements. A dropped `:is()` member matches less and reads as "unsupported"; a dropped `:not()` member INVERTS, which is why `:not()` fails closed and `:is()` does not |
//! | parse `:is()` members with `parse_compound` (the old `:not()` shape) instead of `parse_selector` | RED — **`isComplex:0` alone**, every other claim green. The plausible half-fix: it handles `:is(.b, .c)` and silently drops `:is(.g > .b)` while looking finished |

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <div class="a">
    <span class="b" id="b1">b1</span>
    <span class="c" id="c1">c1</span>
    <div class="g" id="g1"><span class="b" id="b2">b2</span></div>
    <span class="e" id="e1">e1</span><span class="f" id="f1">f1</span>
  </div>
  <span class="b" id="outside">outside</span>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    function ids(sel) {
      var n = document.querySelectorAll(sel), a = [];
      for (var i = 0; i < n.length; i++) { a.push(n[i].id); }
      return a.length + (a.length ? ':' + a.join(',') : '');
    }

    // ── 1. THE LOAD-BEARING CLAIM: a grouped list after a descendant combinator.
    p('isSimple:' + ids('.a :is(.b, .c)'));
    p('whereSimple:' + ids('.a :where(.b, .c)'));

    // ── 2. COMPLEX members — the half a compound-only fix gets wrong.
    p('isComplex:' + ids('.a :is(.g > .b, .e + .f)'));

    // ── 3. Nesting, and `:where()` inside `:is()`.
    p('nested:' + ids('.a :is(:where(.b), .c)'));

    // ── 4. FORGIVING: an unusable member must not take the list with it.
    p('forgiving:' + ids('.a :is(.b, 123)'));

    // ── 5. The other matcher entry points share the parser.
    p('matchesApi:' + document.getElementById('b2').matches(':is(.b, .c)'));
    p('closestApi:' + (document.getElementById('b2').closest(':is(.g, .zzz)') || {}).className);

    // ── 6. THE RATCHET CLAUSE — scoping and the plain selectors must not move.
    p('scoped:' + ids('.a :is(.b)'));
    p('plainStillWorks:' + ids('.a .c'));
    p('notStillWorks:' + ids('.a span:not(.b):not(.e):not(.f)'));
    // ⚠⚠⚠ **t1346 — THIS PROBE USED TO END HERE, SILENTLY.** `ids()` calls `querySelectorAll`,
    // which now THROWS `SyntaxError` on a genuinely invalid selector (the spec's answer, and
    // Chrome's — measured on this fixture: `THREW:SyntaxError`). The throw aborted the script
    // mid-sentence, so `#out` simply stopped one key early and the assertion for a MISSING key
    // reported "expected `unknownStillDrops:0`" while the real fact was that the whole rest of the
    // probe had vanished. An expectation written as an ABSENT value cannot tell "the value is wrong"
    // from "nothing ran".
    p('unknownThrows:' + (function () {
      try { return 'no:' + ids('.a :totally-unknown-pseudo(x)'); } catch (e) { return e.name; }
    })());
    p('whereZeroSpecificityNotOurJob:' + (typeof getComputedStyle(document.getElementById('b1')).color === 'string'));

    // ── 7. THE ROOT CAUSE, which is NOT about `:is()` at all: the top-level list split was
    //    `text.split(',')` and did not see parentheses, so a comma inside ANY functional pseudo cut
    //    the selector in half. These two never involved `:is()` and were broken the same way.
    p('hasComma:' + ids('.a :has(> .b, > .zzz)'));
    p('notComma:' + ids('.a span:not(.b, .c, .e)'));
    p('listWithFn:' + ids('.a :is(.b), .a .e'));
  </script>
</body></html>"##;

#[test]
fn is_and_where_match_in_the_query_apis() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://iswhere.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("IS/WHERE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_IS_WHERE_SELECTORS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "isSimple:3:b1,c1,b2",
        "THE LOAD-BEARING CLAIM. `.a :is(.b, .c)` returned an EMPTY LIST — an unknown pseudo drops \
         the WHOLE selector, so the query was silently deleted rather than partially answered. It \
         must find all three descendants and NOT the `.b` outside `.a`",
    ),
    (
        "whereSimple:3:b1,c1,b2",
        "`:where()` is the same match — it differs only in specificity, which this matcher never \
         consults",
    ),
    (
        "isComplex:2:b2,f1",
        "⚠ THE CLAIM A COMPOUND-ONLY FIX FAILS. `:is()` members are COMPLEX selectors — `.g > .b` \
         and `.e + .f` are legal. `:not()` here takes a single Compound, so copying its shape would \
         have passed `isSimple` and failed this while looking finished. ⚠ TWO, not three: `.e + .f` \
         selects the `.f` that FOLLOWS `.e`, so `f1` matches and `e1` does not — my first draft of \
         this claim said three and the engine was right",
    ),
    (
        "nested:3:b1,c1,b2",
        "`:where()` nested inside `:is()` — the members are parsed by the same entry point, so \
         nesting works or the recursion is wrong",
    ),
    (
        "forgiving:2:b1,b2",
        "FORGIVING. `123` is not a selector; it is DROPPED and `.b` still matches. This is what \
         stops `:is()` taking the query down the way an unknown pseudo does",
    ),
    (
        "matchesApi:true",
        "`Element.matches` shares the parser — a fix wired only into querySelectorAll would leave \
         `matches`/`closest` still silently empty",
    ),
    ("closestApi:g", "and `closest`, which walks ancestors through the same matcher"),
    (
        "scoped:2:b1,b2",
        "a single-member `:is()` still respects the descendant combinator before it — it must not \
         degrade into a document-wide match (`outside` is `.b` and must NOT appear)",
    ),
    (
        "plainStillWorks:1:c1",
        "THE RATCHET CLAUSE. Ordinary selectors are untouched",
    ),
    (
        "notStillWorks:1:c1",
        "and `:not()`, the neighbouring arm this change sits beside, still matches",
    ),
    (
        // ⚠⚠⚠ RE-PINNED AT t1346, AND IT IS A CORRECTION. This row read `unknownStillDrops:0` when
        // an invalid selector returned an empty list. It now THROWS `SyntaxError`, which is what
        // Selectors says and what Chrome does — measured on this exact selector:
        // `document.querySelectorAll('.a :totally-unknown-pseudo(x)')` → `THREW:SyntaxError`.
        //
        // The INTENT is unchanged and is the reason the row survives rather than being deleted: an
        // unrecognised pseudo must fail CLOSED, never match everything. A throw is fail-closed and
        // is strictly more useful than silence, because try/catch around a selector is how the web
        // feature-detects selector support — an engine that never throws answers "supported" for
        // every selector it cannot match.
        "unknownThrows:SyntaxError",
        "an unrecognised pseudo fails CLOSED — it throws `SyntaxError` rather than matching \
         everything. `no:0` means the selector was silently dropped to an empty list (the old \
         behaviour, which lies to feature detection); `no:` with matches means it matched, which is \
         the dangerous direction",
    ),
    (
        "hasComma:1:g",
        "⚠⚠⚠ THE ROOT CAUSE, AND IT IS NOT ABOUT `:is()`. `parse_selector_list` was \
         `text.split(',')` — blind to parentheses — so `:has(> .b, > .zzz)` was cut into \
         `:has(> .b` and `> .zzz)`. The first fragment has an unbalanced paren and parses as though \
         the list held only its first member; the second is garbage and is dropped. The selector \
         therefore did not fail loudly, it QUIETLY MATCHED A SUBSET — which is why it survived: \
         `:is(.b, .c)` returned the `.b` elements and looked like it worked",
    ),
    (
        "notComma:1:f1",
        "the same cut applied to `:not()` with a selector list, which never involved `:is()` \
         either — one naive `split(',')` was mis-parsing every functional pseudo that takes a list. \
         This ALSO needed `:not()` to hold a complex-selector LIST rather than one `Compound`; \
         `:not(.a, .b)` is Selectors 4 and Baseline, and a single compound could not represent it. \
         ⚠ `f1` alone: it is the only `span` in `.a` that is none of `.b`/`.c`/`.e`",
    ),
    (
        "listWithFn:3:b1,b2,e1",
        "and a REAL top-level list whose first branch contains a function — the case that proves \
         the paren-aware split still splits where it genuinely should, rather than swallowing the \
         whole string as one selector",
    ),
    (
        "whereZeroSpecificityNotOurJob:true",
        "the boundary, stated so it is not implicit: the LIVE cascade is Stylo's and computes \
         specificity itself, so folding `:where()` into `:is()` here is safe. Reading a computed \
         style still works, which is the check that the cascade path was not disturbed",
    ),
];
