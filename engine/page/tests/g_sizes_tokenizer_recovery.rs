//! **G_SIZES_TOKENIZER_RECOVERY — `sizes` was parsed as a GRAMMAR over raw bytes, and three
//! TOKENIZER rules had already run before the grammar was ever entitled to look.**
//!
//! `sizes_slot_width` implements HTML's *"parse a sizes attribute"*, and that algorithm is defined
//! over a **token stream**, not over the attribute's characters. t1275 built the list-and-value
//! grammar (`g_sizes_first_match`) directly on the bytes, which is correct for every attribute a
//! human writes by hand and wrong for three constructs WPT writes ~20 times each. In all three the
//! failure is the same shape and it is SILENT: the entry is discarded or swallowed, the slot
//! collapses to the `100vw` default, and a **different bitmap is fetched** — on a narrow viewport,
//! the larger one, which is the exact inversion responsive images exist to prevent.
//!
//! ```text
//!   sizes=…                        chrome   before   the tokenizer rule that had already run
//!   "/* */1px/* */"                 1px     100vw    comments are REMOVED before the grammar
//!   "\(,1px"                        1px     100vw    an ESCAPED bracket is an ident char
//!   "min(1px, 200vw"                1px     100vw    EOF CLOSES an open function
//! ```
//!
//! ## ⭐⭐⭐ AN UNCLOSED OPENER IS CLOSED BY EOF — IT IS NOT A PARSE ERROR
//!
//! The old code returned `None` here and *reasoned it out in a comment*: "an unclosed `(` makes the
//! whole `<source-size>` a parse error". `g_sizes_first_match` then pinned that conclusion as a row
//! (`unclosed=b.png`) — a **prose-derived reference value**, and headless Chrome answers the
//! opposite. CSS's *consume a function* step is explicit: reaching EOF ends the function, flags a
//! parse error, **and returns the block anyway**. So `min(1px, 200vw` resolves exactly as
//! `min(1px, 200vw)` does. That gate row is corrected under this tick; it was the only one of its
//! 22 that disagreed with Chrome.
//!
//! ⭐ **And the bug hid behind the spelling that has no space.** `calc(1px` passed the whole time,
//! which is what made the area look finished. `split_trailing_component` walks RIGHT-TO-LEFT and
//! stops at the first top-level whitespace it crosses — for `calc(1px` there is none, so it reached
//! the `(` and recovered; for `min(1px, 200vw` it crosses the space after the comma **before it has
//! seen the `(` at all**, because the `(` is to its left. It split into a condition `min(1px,` and a
//! value `200vw`, the bogus condition failed to match, and the entry vanished. Hence the fix's
//! shape: **measure the balance BEFORE the right-to-left walk, not during it.**
//!
//! ## ⚠⚠ A COMMENT IS A TOKEN BOUNDARY, NOT NOTHING
//!
//! Stripping `/* … */` to the empty string is the obvious implementation and it is wrong:
//! `1/**/px` would become `1px`. Chrome answers `100vw` for it — `1` and `px` are two tokens with a
//! comment between them, which is not a `<length>`. The strip therefore emits a **space**, and
//! `c_boundary` is the row that can tell the two implementations apart. `c_unterm` pins the other
//! half of the tokenizer's rule: an unterminated `/*` is ended by EOF, so `1px/*` is `1px`.
//!
//! ## ⚠ THE THREE NEGATIVE CONTROLS ARE THE POINT
//!
//! Each mechanism ships with the row that must STILL fall back, because "recover from a malformed
//! attribute" is one edit away from "never reject anything":
//! `c_boundary` (comment is a boundary) · `e_none` (an UNescaped `(` really does swallow the comma)
//! · `u_cond_skip` (the media condition is still parsed and can still fail on the unclosed path).
//!
//! Every expectation below is headless Chrome's own answer, read off `currentSrc` at an 800px
//! viewport — not derived from spec prose, which is what put the wrong value in the sibling gate.
//!
//! ⚠ ONE `#[test]` in this binary: two SpiderMonkey contexts in one manuk-page binary tear each
//! other down and the second test reads the first one's empty output.

use manuk_text::FontContext;

// 800px viewport; every row carries the same two candidates, so the assertion is a clean binary —
// a SMALL slot takes `/s/a.png` (1w covers it), the 100vw fallback takes `/s/b.png`.
const HTML: &str = r##"<!doctype html><html><body>
 <!-- ── (1) CSS COMMENTS. Removed by the TOKENIZER, so nothing downstream can see one. -->
 <img id="c_lead_trail" srcset="/s/a.png 1w, /s/b.png 900w" sizes="/* */1px/* */">
 <img id="c_ws_runs"    srcset="/s/a.png 1w, /s/b.png 900w" sizes=" /**/ /**/ 1px /**/ /**/ ">
 <!-- ⚠ NEGATIVE: a comment SEPARATES tokens. `1` `px` is not a length, so this must fall back —
      the row that distinguishes "strip to a space" from "strip to nothing". -->
 <img id="c_boundary"   srcset="/s/a.png 1w, /s/b.png 900w" sizes="1/**/px">
 <!-- …inside the media condition, and between the condition and the value. -->
 <img id="c_in_cond"    srcset="/s/a.png 1w, /s/b.png 900w" sizes="(min-width:/**/0) 1px">
 <img id="c_between"    srcset="/s/a.png 1w, /s/b.png 900w" sizes="(min-width:0)/**/1px">
 <!-- An UNTERMINATED comment is ended by EOF — the tokenizer's own rule, same as the block one. -->
 <img id="c_unterm"     srcset="/s/a.png 1w, /s/b.png 900w" sizes="1px/*">

 <!-- ── (2) ESCAPES. `\( ` is an IDENT character, not an opener, so the comma after it is
      TOP-LEVEL and the list has two entries. Counting it as depth swallowed the comma. -->
 <img id="e_paren"      srcset="/s/a.png 1w, /s/b.png 900w" sizes="\(,1px">
 <img id="e_brace"      srcset="/s/a.png 1w, /s/b.png 900w" sizes="\{,1px">
 <img id="e_bracket"    srcset="/s/a.png 1w, /s/b.png 900w" sizes="\[,1px">
 <img id="e_prefixed"   srcset="/s/a.png 1w, /s/b.png 900w" sizes="x\(,1px">
 <!-- ⚠ NEGATIVE: without the backslash the `(` IS an opener, it swallows the comma, and the one
      resulting entry is unparseable. If this matched `e_paren` the escape would be doing nothing. -->
 <img id="e_none"       srcset="/s/a.png 1w, /s/b.png 900w" sizes="(,1px">

 <!-- ── (3) EOF CLOSES AN OPEN BLOCK. Each of these must resolve as its CLOSED form does. -->
 <!-- ⭐ the one that was broken: the space after the comma is crossed BEFORE the `(` is seen. -->
 <img id="u_space"      srcset="/s/a.png 1w, /s/b.png 900w" sizes="min(1px, 200vw">
 <!-- ⭐ …and the one that hid it: no whitespace, so the right-to-left walk reached the `(`. -->
 <img id="u_nospace"    srcset="/s/a.png 1w, /s/b.png 900w" sizes="calc(1px">
 <img id="u_cond"       srcset="/s/a.png 1w, /s/b.png 900w" sizes="(min-width:0) min(1px, 200vw">
 <!-- ⚠ NEGATIVE: the condition is STILL parsed on the recovery path and this one cannot match, so
      the entry is skipped and the list runs out. Recovery is not "accept everything". -->
 <img id="u_cond_skip"  srcset="/s/a.png 1w, /s/b.png 900w" sizes="(min-width:99999px) calc(1px">
 <img id="u_nested"     srcset="/s/a.png 1w, /s/b.png 900w" sizes="min(max(1px, 0px">
 <img id="u_clamp"      srcset="/s/a.png 1w, /s/b.png 900w" sizes="clamp(1px, 0px, 100px">

 <!-- ── CONTROLS: the closed form of the ⭐ row, a plain length, and a real parse error. -->
 <img id="k_closed"     srcset="/s/a.png 1w, /s/b.png 900w" sizes="min(1px, 200vw)">
 <img id="k_plain"      srcset="/s/a.png 1w, /s/b.png 900w" sizes="1px">
 <img id="k_junk"       srcset="/s/a.png 1w, /s/b.png 900w" sizes="foo bar">

 <div id="out">-</div>
 <script>
 window.addEventListener('load', function(){
   var ids=['c_lead_trail','c_ws_runs','c_boundary','c_in_cond','c_between','c_unterm',
            'e_paren','e_brace','e_bracket','e_prefixed','e_none',
            'u_space','u_nospace','u_cond','u_cond_skip','u_nested','u_clamp',
            'k_closed','k_plain','k_junk'], r=[];
   for (var i=0;i<ids.length;i++){
     var v=document.getElementById(ids[i]).currentSrc;
     r.push(ids[i]+'='+(v===''?'EMPTY':v.replace(/^https?:\/\/[^/]+\/s\//,'')));
   }
   document.getElementById('out').textContent=r.join(' ');
 });
 </script></body></html>"##;

/// **One test, on purpose** — see `g_defer`.
#[test]
fn sizes_applies_the_css_tokenizer_before_the_source_size_grammar() {
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
    println!("SIZES-TOKENIZER {got}");

    assert_eq!(
        got,
        "c_lead_trail=a.png c_ws_runs=a.png c_boundary=b.png c_in_cond=a.png c_between=a.png \
         c_unterm=a.png e_paren=a.png e_brace=a.png e_bracket=a.png e_prefixed=a.png \
         e_none=b.png u_space=a.png u_nospace=a.png u_cond=a.png u_cond_skip=b.png \
         u_nested=a.png u_clamp=a.png k_closed=a.png k_plain=a.png k_junk=b.png",
        "`sizes` is parsed over a TOKEN STREAM, and three tokenizer rules run before the \
         `<source-size>` grammar is entitled to look. (1) COMMENTS are removed — `/* */1px/* */` \
         is exactly `1px` — but a comment is a token BOUNDARY, so `c_boundary` (`1/**/px`) must \
         still fall back, and `c_unterm` pins that EOF ends an unterminated one. (2) An ESCAPED \
         bracket is an IDENT character, so the comma in `\\(,1px` is TOP-LEVEL and the list has two \
         entries; `e_none` is the unescaped mirror where the `(` really does swallow it. (3) EOF \
         CLOSES an open block — `min(1px, 200vw` resolves as `min(1px, 200vw)` does, which is why \
         `u_space` and `k_closed` must AGREE; `u_nospace` is the spelling that already worked and \
         hid the bug, because with no whitespace the right-to-left walk reaches the `(`. \
         `u_cond_skip` proves the media condition is still parsed and can still fail on the \
         recovery path. EVERY value here is headless Chrome's own `currentSrc` at 800px, not spec \
         prose — prose is what put the wrong `unclosed` value in `g_sizes_first_match`"
    );
}
