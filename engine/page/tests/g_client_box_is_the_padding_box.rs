//! **G_CLIENT_BOX_IS_THE_PADDING_BOX — `clientWidth`/`clientHeight` reported the BORDER box for
//! every element that is not a scroll container, which is nearly all of them.**
//!
//! `scroll_geometry_of` computes the padding box correctly (`rect.width - bw.left - bw.right`) — and
//! it only maps `overflow: auto|scroll|hidden` containers. Everything else took the getter's
//! *fallback*, which handed back `rect.width`: the border box. So the right answer was computed for
//! the minority and the fallback answered for everyone else.
//!
//! The fallback's own comment was half right and that is why it survived: *"a plain `<div>` still has
//! a `clientHeight`, and it is its own box."* True — and **its own box is the BORDER box, while
//! `client*` is the PADDING box.**
//!
//! Chrome-measured, `width:200px; height:100px`:
//!
//! ```text
//!                                    CHROME          BEFORE          AFTER
//!   plain                            200/100         200/100         200/100
//!   padding:10px                     220/120         220/120         220/120
//!   border:2px                       200/100         204/104   ✗     200/100
//!   padding:10px + border:2px        220/120         224/124   ✗     220/120
//!   display:inline + border          0/0             4/16      ✗     0/0
//!   offsetWidth (border box)         224/124         224/124         224/124
//! ```
//!
//! **Why it matters more than four pixels.** `clientHeight` is the viewport half of every
//! virtualised list (`scrollHeight - clientHeight` is what you divide by), every "is this element
//! overflowing?" test (`scrollWidth > clientWidth`), every sticky-header offset and every carousel
//! page-size. Overstating it by the border makes an overflow check answer *no* on a bordered box
//! that is in fact overflowing — the failure is a scrollbar that never appears and a list that
//! renders the wrong slice.
//!
//! ⚠ **A non-replaced INLINE box reports 0**, per CSSOM — it has no padding box. Returning the
//! border-subtracted box there (`4/16`) is the same mistake as returning the border box for a block:
//! a plausible number where the spec says zero, and `if (!el.clientHeight)` is a standard *"is this
//! laid out?"* guard that a plausible number defeats.
//!
//! ⚠ Named residual, pinned by assertion (5): a scroll container reports `220` where Chrome reports
//! `205`. Chrome subtracts a 15px classic scrollbar gutter from the padding box; this engine does not
//! reserve one. That is a scrollbar-model difference, not a box-model one, and it is the honest
//! current state.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>
 body { margin:0; font: 16px/1.5 sans-serif }
 .base { width:200px; height:100px }
 #pad  { padding:10px }
 #bord { border:2px solid #000 }
 #both { padding:10px; border:2px solid #000 }
 #scr  { padding:10px; border:2px solid #000; overflow:auto }
 #scr > div { height:400px }
 #inl  { display:inline; border:2px solid #000 }
</style></head><body>
 <div class="base" id="plain">a</div>
 <div class="base" id="pad">b</div>
 <div class="base" id="bord">c</div>
 <div class="base" id="both">d</div>
 <div class="base" id="scr"><div>e</div></div>
 <span id="inl">f</span>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     function m(i) {
       var e = document.getElementById(i);
       return i + ':' + e.clientWidth + 'x' + e.clientHeight + '/' + e.offsetWidth + 'x' + e.offsetHeight;
     }
     document.getElementById('out').textContent =
       [m('plain'), m('pad'), m('bord'), m('both'), m('scr'), m('inl')].join(' ');
   });
 </script>
</body></html>"#;

fn out(page: &manuk_page::Page) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn client_width_and_height_are_the_padding_box() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://clientbox.test/",
        &fonts,
        800.0,
    ));
    let got = out(&page);
    println!("CLIENT-BOX {got}");
    let has = |s: &str| got.contains(s);

    // (1) **No border, no padding** — the control. `client*` and `offset*` agree, so a fix that
    // subtracted something from every element would break here first.
    assert!(
        has("plain:200x100/200x100"),
        "a plain box: client and offset must both be 200x100 — got {got:?}"
    );

    // (2) **Padding is INSIDE the client box.** RED: subtract padding as well → 200x100.
    assert!(
        has("pad:220x120/220x120"),
        "padding must be INCLUDED in clientWidth/Height — got {got:?}"
    );

    // (3) **Border is OUTSIDE it — the bug.** RED: revert the fallback to `r[2]`/`r[3]` → 204x104,
    // which is what shipped. Chrome measures 200x100.
    assert!(
        has("bord:200x100/204x104"),
        "border must be EXCLUDED from clientWidth/Height and INCLUDED in offsetWidth/Height — got \
         {got:?}. `204x104` for the client box is the border box, which is what `offset*` is for."
    );
    assert!(
        has("both:220x120/224x124"),
        "padding in, border out, both at once — got {got:?}"
    );

    // (4) **A non-replaced INLINE box is 0**, per CSSOM. RED: drop the inline arm → `4x16`, a
    // plausible number that defeats `if (!el.clientHeight)`.
    assert!(
        has("inl:0x0/"),
        "an inline box has no padding box: clientWidth/Height must be 0 (Chrome: 0/0) while \
         offset* still reports the border box — got {got:?}"
    );

    // (5) **THE NAMED RESIDUAL, PINNED.** Chrome reports `205` here because it reserves a 15px
    // classic scrollbar gutter inside the padding box; this engine reserves none, so it reports the
    // full `220`. A scrollbar-model difference, not a box-model one. If this ever reads `205`, the
    // gutter landed — update the assertion to Chrome's value and say so in the journal.
    assert!(
        has("scr:220x120/224x124"),
        "a scroll container currently reports the full padding box (220) because no scrollbar \
         gutter is reserved; Chrome reports 205. If this changed, the gutter landed — got {got:?}"
    );
}
