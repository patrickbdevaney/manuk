//! **G_IMG_CURRENT_SRC — the engine picked the candidate and the DOM published `src`.**
//!
//! `<img>.currentSrc` is *"the URL of the resource the element is displaying"*. It was a getter that
//! returned `this.src`, under a comment asserting the engine *"does not yet do srcset/`<picture>`
//! candidate selection for the bitmap"*. That stopped being true at tick 582: `select_image_url`
//! runs the full `<picture>` → `srcset` → `src` selection and **both** the fetch worklist and the
//! decode worklist consume it. So the engine has known which file it chose all along, and the DOM
//! answered with a different one — or, for `<img srcset>` with **no `src`** (legal, and what
//! WordPress, every CMS and every image CDN emit), with the **empty string**.
//!
//! ⭐ **The empty string is the expensive part, because it is a wrong answer of the right TYPE.**
//! WPT's `the-img-element/sizes` files read `expect = referenceImg.currentSrc` once per paragraph
//! and `assert_unreached` every sibling when it is falsy, so one empty string at the top of each
//! group failed the whole group: the directory read **0 of 795**. Publishing the selection took it
//! to 472.
//!
//! ⚠⚠⚠ **AND IT EXPOSED 57 SUBTESTS THAT HAD BEEN PASSING ON THE EMPTY STRING.** The sibling
//! directory `the-img-element/srcset` went **188 → 131** the moment `currentSrc` became real, and
//! that drop is the honest shape of the bug rather than a new one: those rows feed a MALFORMED
//! `srcset` (`1x 1x`, `1w 1w`, `1w 1x`, `0w`, `-1w`) and assert the element selects **nothing**.
//! Our parser accepted every one of them, but `currentSrc` returned `''` for all images, which is
//! the same `''` an invalid list is supposed to produce — a wrong mechanism agreeing with the right
//! answer, exactly the vacuous-pass class t1270 named. Tightening `parse_srcset` to HTML's own
//! descriptor rules did not merely repair the 57: the directory finished at **241/252**, above
//! where it started.
//!
//! ⚠⚠ **The table is keyed by `(arena, NodeId)` and that is load-bearing.** The fixture WPT cares
//! about most is *iframed*, and a `NodeId` means nothing outside its own document — keyed on the id
//! alone, a child's node #7 answers with the parent's node #7, confidently and wrongly. That is the
//! one-arena bug `node_and_dom` was written to close, and this gate's `frame` row is what stops it
//! being reintroduced here.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head></head><body>
 <!-- THE CORE CLAIM: `srcset` with NO `src` at all. This is the shape that reported the empty
      string, and it is the common one on the real web. 800px viewport, 1x density: the smallest
      candidate that still covers 800 wins, so 900w — not the first listed, and not the largest. -->
 <img id="a" srcset="/s/small.png 300w, /s/mid.png 900w, /s/big.png 1600w">
 <!-- A `w` list ALONGSIDE a `src`: the selection must WIN over the `src`, which is the whole point
      (on a `w` list the `src` is frequently the smallest file, so publishing it is a thumbnail). -->
 <img id="b" src="/s/fallback.png" srcset="/s/small.png 300w, /s/mid.png 900w">
 <!-- DENSITY descriptors at dpr 1: `1x` wins over `2x`. -->
 <img id="c" srcset="/s/one.png 1x, /s/two.png 2x">
 <!-- `<picture>`: the first matching `<source>` beats the `<img>`'s own src entirely. -->
 <picture><source srcset="/s/wide.png"><img id="d" src="/s/inline.png"></picture>

 <!-- ── THE INVALID-DESCRIPTOR ROWS. Each must select NOTHING, and each is a rule the parser used
      to ignore because it only ever read the FIRST descriptor token. These are the 57 subtests
      that had been passing on the empty string. -->
 <img id="e1" srcset="/s/a.png 1x 1x">      <!-- density twice -->
 <img id="e2" srcset="/s/a.png 1w 1w">      <!-- width twice -->
 <img id="e3" srcset="/s/a.png 1w 1x">      <!-- width AND density -->
 <img id="e4" srcset="/s/a.png 0w">         <!-- zero is not a valid width -->
 <img id="e5" srcset="/s/a.png -1w">        <!-- nor is a negative one -->
 <img id="e6" srcset="/s/a.png 1h">         <!-- `h` alone is meaningless; it pairs with `w` -->
 <img id="e7" srcset="/s/a.png bogus">      <!-- an unrecognised descriptor -->
 <!-- …and `h` WITH a `w` is valid, so the pair above is a real rule and not a blanket rejection. -->
 <img id="f" srcset="/s/a.png 1h 900w">

 <!-- ⚠ THE PARENTHESIS RULE, which decides where the next CANDIDATE begins. Every comma between
      `(` and `)` belongs to one descriptor token, so this is TWO candidates and the first is
      invalid — the answer is `c.png`. Splitting at the first comma instead reads `b.png` as a
      candidate and selects it, which is what the old parser did. -->
 <img id="g" srcset="/s/a.png ( , /s/b.png 1x, ), /s/c.png">
 <!-- …and an UNCLOSED paren swallows the rest of the attribute, so there is no candidate at all. -->
 <img id="h" srcset="/s/a.png (, /s/b.png">

 <!-- ── CONTROLS. These bound the claim to the source-set path. -->
 <!-- (1) A plain `src` with no `srcset`: the element makes no SELECTION, so the published table
      must not contain it and the getter must fall through to the resolved `src`, byte for byte as
      it did before this tick existed. -->
 <img id="n1" src="/s/plain.png">
 <!-- (2) No source of any kind: the empty string is the CORRECT answer here, and it must stay
      distinguishable from the empty string the bug used to produce everywhere. -->
 <img id="n2">
 <!-- (3) Not an image. `currentSrc` is IMG-guarded and must read `undefined` on anything else,
      rather than growing a stray URL from the shared prototype. -->
 <div id="n3"></div>

 <div id="out">-</div>
 <iframe id="fr" srcdoc="<img id='i' srcset='/s/f1.png 300w, /s/f2.png 900w'>"></iframe>
 <script>
 window.addEventListener('load', function(){
   var ids=['a','b','c','d','e1','e2','e3','e4','e5','e6','e7','f','g','h','n1','n2','n3'], r=[];
   function tail(v){
     if (v === undefined) { return 'undefined'; }
     if (v === '') { return 'EMPTY'; }
     return v.replace(/^https?:\/\/[^/]+/, '');
   }
   for (var i=0;i<ids.length;i++){
     r.push(ids[i]+'='+tail(document.getElementById(ids[i]).currentSrc));
   }
   // ⚠ THE CROSS-ARENA ROW. An iframed `<img>` is the case the WPT fixture is built from, and a
   // table keyed on a bare NodeId answers this one with the PARENT's node of the same number.
   var d = document.getElementById('fr').contentDocument;
   r.push('frame=' + tail(d ? d.getElementById('i').currentSrc : 'NODOC'));
   document.getElementById('out').textContent=r.join(' ');
 });
 </script></body></html>"##;

/// **One test, on purpose** — see `g_defer`.
#[test]
fn current_src_reports_the_source_set_selection_not_the_src() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://img.test/",
        &fonts,
        800.0,
    ));
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    let got = page.dom().text_content(hits[0]);
    println!("IMG-CURRENT-SRC {got}");

    // RED, run 1 — delete the `__selectedSrc` consultation from the `currentSrc` getter in
    // `event_loop.rs` (i.e. restore `return this.src`): every selecting row collapses onto its
    // `src`, so `a=EMPTY b=/s/fallback.png c=EMPTY d=/s/inline.png f=EMPTY g=EMPTY frame=EMPTY`
    // — and EVERY CONTROL stays byte-identical, which is what says this gate measures the
    // selection path rather than image handling in general.
    //
    // RED, run 2 — revert `parse_srcset`'s descriptor VALIDATION to reading only its first token:
    // `e1..e5` stop being empty and select `/s/a.png`, and `f` (a valid `1h 900w`) goes EMPTY
    // because the old code cannot parse `1h` at all. Nothing else moves. That is the arm that had
    // been scoring 57 WPT subtests on the empty string.
    //
    // ⚠ **`g`/`h` do NOT move under run 2, and that is the point of listing them separately.**
    // They are governed by the paren-aware descriptor SCAN — where the descriptor list *ends* —
    // not by the validation of the tokens inside it, so they need their own mutation (make the
    // phase-2 loop `break` on any `,`): then `g` selects `/s/b.png` from inside the parenthesised
    // descriptor and `h` selects `/s/b.png` after the unclosed paren. Two arms, two mutations —
    // asserting them under one and calling it proven is how a gate ends up pinning half of what
    // its message claims.
    assert_eq!(
        got,
        "a=/s/mid.png b=/s/mid.png c=/s/one.png d=/s/wide.png \
         e1=EMPTY e2=EMPTY e3=EMPTY e4=EMPTY e5=EMPTY e6=EMPTY e7=EMPTY \
         f=/s/a.png g=/s/c.png h=EMPTY \
         n1=/s/plain.png n2=EMPTY n3=undefined frame=/s/f2.png",
        "`currentSrc` must report the candidate the engine actually SELECTED, not the element's \
         `src`. `a` is the core claim — `srcset` with no `src` used to publish the empty string, \
         and the empty string is what made WPT's `the-img-element/sizes` read 0 of 795. `b` proves \
         the selection BEATS a present `src` (on a `w` list the `src` is often the smallest file). \
         `d` is the `<picture>` path. The `e*` rows are the invalid-descriptor family that HTML \
         requires to select nothing — they were passing before this tick only because every image \
         published `''`, which is the same `''` an invalid list produces; `f` is the same `h` \
         descriptor made VALID by a `w` beside it, so `e6` is a real rule and not a blanket \
         rejection. `g`/`h` are the parenthesis rule, which decides where the next CANDIDATE \
         starts. CONTROLS: `n1` makes no selection and must fall through to the resolved `src` \
         exactly as before; `n2` has no source at all, where the empty string is CORRECT; `n3` is \
         not an `<img>` and must read `undefined`. `frame` is the cross-arena row — the table is \
         keyed by `(arena, NodeId)` because a bare NodeId resolves an iframed image against the \
         PARENT's arena and answers about a different element entirely"
    );
}
