//! **G_SIZES_FIRST_MATCH — `sizes` read the LAST entry and ignored every media condition.**
//!
//! HTML's *"parse a sizes attribute"* is a list of `<source-size>`, each an optional
//! `<media-condition>` followed by a `<source-size-value>`, and **the FIRST entry whose condition
//! matches wins**. `sizes_slot_width` took the **last** entry — the opposite end of the list — and
//! understood exactly two units, `px` and `vw`.
//!
//! ⭐ **The old code was an honestly-labelled approximation, and its own estimate of the damage was
//! wrong.** It said: *"we take the LAST entry's length, which is that unconditional fallback… one
//! candidate step off at worst."* But the canonical responsive-image idiom on the real web is
//! `sizes="(max-width: 600px) 100vw, 50vw"` — *"full width on a phone, half width on a desktop"* —
//! and on a phone we answered **50vw** where the author said **100vw**. That is not one step off,
//! it is the narrow viewport getting the *smaller* file precisely because it is narrow.
//!
//! `sizes` is how a page states how big the image will BE. The wrong answer is a wrong intrinsic
//! size, and a wrong intrinsic size re-wraps everything below it.
//!
//! ⚠ **Scoped, and the boundary is measured rather than asserted.** This closes the LIST and VALUE
//! legs: first-match ordering, the full length-unit table, `calc()`/`min()`/`max()`/`clamp()`, and
//! parse errors falling back to `100vw`. It does NOT touch media-query *grammar* — `or`,
//! general-enclosed, unknown-feature handling — which is where the rest of WPT's
//! `the-img-element/sizes` remainder lives and is its own mechanism in its own crate.

use manuk_text::FontContext;

// 800px viewport. Candidates are 1w and 900w on every row, so the assertion is a clean binary:
// a SMALL slot selects `/s/a.png` (1w covers it), a LARGE one selects `/s/b.png`.
const HTML: &str = r##"<!doctype html><html><head></head><body>
 <!-- THE CORE CLAIM, and it is the real web's idiom. `(min-width:0)` always matches, so the FIRST
      entry wins and the slot is 1px — the old code took `100vw` from the end of the list. -->
 <img id="first" srcset="/s/a.png 1w, /s/b.png 900w" sizes="(min-width:0) 1px, 100vw">
 <!-- …and the mirror: a condition that CANNOT match must be skipped, leaving the fallback. If the
      two rows agreed, the condition would not be being consulted at all. -->
 <img id="skip" srcset="/s/a.png 1w, /s/b.png 900w" sizes="(min-width:99999px) 1px, 100vw">
 <!-- Order among SEVERAL matching entries: the first still wins, not the narrowest or the last. -->
 <img id="order" srcset="/s/a.png 1w, /s/b.png 900w" sizes="(min-width:0) 1px, (min-width:0) 100vw, 100vw">

 <!-- ── UNITS. The old table was `px` and `vw`; everything else silently became the viewport. -->
 <img id="em"   srcset="/s/a.png 1w, /s/b.png 900w" sizes="0.05em">
 <img id="rem"  srcset="/s/a.png 1w, /s/b.png 900w" sizes="0.05rem">
 <img id="pt"   srcset="/s/a.png 1w, /s/b.png 900w" sizes="0.5pt">
 <img id="cm"   srcset="/s/a.png 1w, /s/b.png 900w" sizes="0.01cm">
 <img id="zero" srcset="/s/a.png 1w, /s/b.png 900w" sizes="0">
 <!-- ⚠ `vmin` must not be matched by the `in` suffix — `1vmin` is not 96 pixels. Longest-first. -->
 <img id="vmin" srcset="/s/a.png 1w, /s/b.png 900w" sizes="0.1vmin">

 <!-- ── MATH FUNCTIONS, which is the reason the value resolver recurses. -->
 <img id="calc"  srcset="/s/a.png 1w, /s/b.png 900w" sizes="calc(1px)">
 <img id="min"   srcset="/s/a.png 1w, /s/b.png 900w" sizes="min(1px, 100px)">
 <img id="max"   srcset="/s/a.png 1w, /s/b.png 900w" sizes="max(-100px, 1px)">
 <!-- ⚠ `clamp(min, val, max)` resolves to **1px**, NOT to the negative middle argument: the
      clamping happens before the sign is judged. Written expecting otherwise, and corrected. -->
 <img id="clamp" srcset="/s/a.png 1w, /s/b.png 900w" sizes="clamp(1px, -50px, 100px)">
 <!-- A comma INSIDE a math function belongs to the function, not to the sizes list. Split
      naively, `min(1px` is the first entry and ` 100px)` is a second that looks parseable. -->
 <img id="comma" srcset="/s/a.png 1w, /s/b.png 900w" sizes="min(1px, 100px), 100vw">

 <!-- ── PARSE ERRORS. Each must fall back to 100vw (the large slot), never to a guess. -->
 <img id="unclosed" srcset="/s/a.png 1w, /s/b.png 900w" sizes="min(1px, 100px">
 <img id="junk"     srcset="/s/a.png 1w, /s/b.png 900w" sizes="foo bar">
 <img id="bare"     srcset="/s/a.png 1w, /s/b.png 900w" sizes="1">
 <img id="pct"      srcset="/s/a.png 1w, /s/b.png 900w" sizes="50%">
 <!-- A NEGATIVE slot is an invalid value, not a slot of zero — skip the entry, take the next. -->
 <img id="neg"      srcset="/s/a.png 1w, /s/b.png 900w" sizes="-1px, 1px">

 <!-- ── CONTROLS. -->
 <!-- (1) NO `sizes` at all: the slot is 100vw, which is the spec's default and what this
      function returned before any of it existed. -->
 <img id="n1" srcset="/s/a.png 1w, /s/b.png 900w">
 <!-- (2) A `sizes` on an `x`-descriptor list is IGNORED — `sizes` only means anything against
      `w` descriptors, so this must select by density and not by slot. -->
 <img id="n2" srcset="/s/a.png 1x, /s/b.png 2x" sizes="1px">
 <!-- (3) A plain `src`: no selection happens, so no slot is computed at all. -->
 <img id="n3" src="/s/plain.png">

 <div id="out">-</div>
 <script>
 window.addEventListener('load', function(){
   var ids=['first','skip','order','em','rem','pt','cm','zero','vmin','calc','min','max','clamp',
            'comma','unclosed','junk','bare','pct','neg','n1','n2','n3'], r=[];
   for (var i=0;i<ids.length;i++){
     var v=document.getElementById(ids[i]).currentSrc;
     r.push(ids[i]+'='+(v===''?'EMPTY':v.replace(/^https?:\/\/[^/]+\/s\//,'')));
   }
   document.getElementById('out').textContent=r.join(' ');
 });
 </script></body></html>"##;

/// **One test, on purpose** — see `g_defer`.
#[test]
fn sizes_resolves_the_first_matching_source_size_not_the_last() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://sizes.test/",
        &fonts,
        800.0,
    ));
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    let got = page.dom().text_content(hits[0]);
    println!("SIZES-FIRST-MATCH {got}");

    // RED, run 1 — restore the old `sizes.rsplit(',').next()`: every value row collapses onto
    // `b.png`, because the last entry is `100vw` (or the unit is unknown, which also fell through
    // to the viewport). `skip` is unchanged — its last entry was ALREADY the right answer — which
    // is exactly why the gate needs both directions of the condition rather than `first` alone.
    //
    // ⚠ `neg` is unchanged too, and it was written expecting otherwise. `-1px, 1px` under the old
    // parser reads the LAST entry, `1px`, and is right **by accident**: two wrong rules cancelling.
    // Recorded rather than quietly re-worded, because "the mutation did not move this row" is the
    // only thing that distinguishes a row that pins a rule from a row that pins a coincidence.
    // Every control stays byte-identical.
    //
    // RED, run 2 — drop the unit table back to `px`/`vw`: `em`, `rem`, `pt`, `cm` and `vmin`
    // become `b.png`, because an unrecognised unit fell through to the viewport width.
    //
    // RED, run 3 — remove the math-function arm: `calc`, `min`, `max`, `clamp` and `comma` become
    // `b.png`. (`unclosed` does NOT move — an unclosed function is a parse error under BOTH, and
    // that is the row that says the fallback is reached deliberately rather than by accident.)
    assert_eq!(
        got,
        "first=a.png skip=b.png order=a.png em=a.png rem=a.png pt=a.png cm=a.png zero=a.png \
         vmin=a.png calc=a.png min=a.png max=a.png clamp=a.png comma=a.png unclosed=b.png \
         junk=b.png bare=b.png pct=b.png neg=a.png n1=b.png n2=a.png n3=plain.png",
        "`sizes` is a list whose FIRST matching `<source-size>` wins. `first` is the core claim and \
         the real web's idiom — `(max-width:600px) 100vw, 50vw` means *full width on a phone*, and \
         reading the last entry hands the narrow viewport the SMALLER file precisely because it is \
         narrow. `skip` is the mirror (a condition that cannot match is passed over) and `first` vs \
         `skip` must DIFFER, or the condition is not being consulted at all. `order` proves the \
         FIRST match wins rather than the narrowest or the last. The unit rows were all silently \
         the viewport before, because the table knew only `px`/`vw`; `vmin` is there because a \
         shortest-suffix-first table matches `in` inside it and turns 0.1vmin into 96 pixels. \
         `clamp` CORRECTED ITS AUTHOR — `clamp(1px, -50px, 100px)` is 1px, the clamping happens \
         before the sign is judged. `comma` proves the list split is math-function-aware. The \
         parse-error rows must reach the 100vw FALLBACK rather than a guess, and `neg` proves a \
         negative value is an invalid entry to skip, not a slot of zero. CONTROLS: `n1` has no \
         `sizes`, `n2` is an `x`-descriptor list where `sizes` is meaningless, `n3` makes no \
         selection at all"
    );
}
