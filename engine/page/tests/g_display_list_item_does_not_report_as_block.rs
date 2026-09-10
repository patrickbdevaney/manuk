//! **G_DISPLAY_LIST_ITEM_DOES_NOT_REPORT_AS_BLOCK — a value that LAYS OUT as one thing and REPORTED
//! as another.**
//!
//! `display: list-item` is block-level; the marker is generated elsewhere. So **both** cascades map
//! the keyword to `Display::Block` on purpose, and every layout path in this engine is correct as
//! written. The collapse then leaked into the *computed value*: `getComputedStyle(li).display`
//! answered `block` where Chrome answers `list-item` — on every `<li>` and every `<summary>` on the
//! web.
//!
//! ⚠ **THE RULE THIS BREAKS WAS ALREADY WRITTEN DOWN, THREE ARMS AWAY IN THE SAME `match`.** The
//! `flow-root` arm's own comment says it: *"`getComputedStyle(el).display` must round-trip the
//! specified keyword — a feature-detect that reads back `block` for `flow-root` concludes the value
//! is unsupported and falls back to a clearfix or `overflow:hidden`, both of which have side effects
//! the author avoided."* Identical failure, one keyword over, live the whole time.
//!
//! ## How it was found — a survey that refused its own hypothesis
//!
//! t1488 surveyed the 28-site near-bar cohort (shape 55–75%) and found **71% of shape misses are on
//! the BLOCK axis**, which suggested containers in the wrong layout mode (a flex row falling back to
//! a column stacks N items and is exactly N× too tall). Printing the `display` each side computed —
//! a value the oracle **already carried and the dump discarded** — refused it:
//!
//! ```text
//!   1,352 shape misses across six near-bar sites · only 50 disagree about display at all
//!   and 40 of those 50 are one keyword:  list-item -> block
//! ```
//!
//! So the near-bar shape gap is *inside agreed layout modes* — and the one real display defect in the
//! cohort is this. **A plausible rule refused in one run, and the refusal named the actual bug.**
//!
//! ## Arbitrated against headless Chrome, eight rows
//!
//! ```text
//!   li=list-item  over=block  dt=block  dd=block  sum=list-item  oli=list-item
//!   explicit=list-item  two=list-item
//! ```
//!
//! * **`dt`/`dd` are NOT list items.** The UA table had `"li" | "dd" | "dt"` on one arm, so the
//!   obvious fix is to make all three `list-item` and it is wrong for two of them.
//! * **`sum=list-item`** — `<summary>` is a list item in Chrome, which is not obvious and is why it
//!   is measured rather than assumed.
//! * **`over=block`** — an author's `display: block` on an `<li>` MUST stop reporting `list-item`.
//!   A flag that is only ever set is a latch, and a latch on a cascaded property is wrong for every
//!   element that overrides it.
//! * **`two=list-item`** — the two-value syntax `display: block flow list-item` normalises to the
//!   same keyword, so the flag is read from the NORMALISED value and not the raw declaration.
//!
//! ⚠ **A SIDE FLAG, NOT A NEW `Display` VARIANT.** `legacy_webkit_box` beside it is the precedent.
//! Adding a variant would force twenty layout sites to handle a mode that behaves exactly like
//! `Block`, and the first one that forgot would be a real rendering regression bought for a
//! serialization fix.
//!
//! ⚠ **BOTH CASCADES.** The MinimalCascade tracks the keyword beside the collapse; the Stylo path
//! recovers it the way it already recovers `appearance`, `display_in_flow` and `counter_set` —
//! *"Stylo cannot answer it and the other cascade can"*. A keyword one cascade knows and the other
//! does not is the two-cascades trap this file has been bitten by before.
//!
//! Mutations that must turn this red:
//!   1. drop the `list_item` UA default          -> `li` and `sum` report `block`
//!   2. add `dd`/`dt` to it                      -> `dt`/`dd` report `list-item`
//!   3. set the flag without clearing it         -> `over` reports `list-item`
//!   4. drop the stylo-side recovery             -> every row reports `block`
//!   5. drop the serializer arm                  -> every row reports `block`

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
#over { display: block }
#two  { display: block flow list-item }
</style></head><body>
<ul><li id="li">x</li><li id="over">o</li></ul>
<dl><dt id="dt">t</dt><dd id="dd">d</dd></dl>
<details id="det"><summary id="sum">s</summary>b</details>
<ol><li id="oli">y</li></ol>
<div id="explicit" style="display:list-item">e</div>
<div id="two">t</div>
<div id="out">-</div>
<script>
var g = function (i) { return i + '=' + getComputedStyle(document.getElementById(i)).display; };
document.getElementById('out').textContent =
  [g('li'), g('over'), g('dt'), g('dd'), g('sum'), g('oli'), g('explicit'), g('two')].join(' ');
</script></body></html>"##;

#[test]
fn display_list_item_round_trips_through_getcomputedstyle() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, "https://li.test/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("DISPLAY: {got}");

    // ── VACUITY, AND IT CUTS BOTH WAYS. `dt=block` and `over=block` are satisfied by an engine that
    //    never reports `list-item` at all — which is the bug — so a positive row must be asserted
    //    first, and separately.
    assert!(
        got.contains("li=list-item"),
        "VACUOUS: `list-item` is not reported at all, so every `=block` row below is passing on the \
         very defect this gate exists to catch — {got:?}"
    );
    assert!(
        got.contains("dt=block"),
        "VACUOUS in the other direction: if EVERYTHING reports `list-item` the positive row above \
         proves nothing — {got:?}"
    );

    // Chrome headless, every row.
    let want = "li=list-item over=block dt=block dd=block sum=list-item oli=list-item \
                explicit=list-item two=list-item";
    assert_eq!(
        got,
        want.split_whitespace().collect::<Vec<_>>().join(" "),
        "\n  `display` must round-trip the specified keyword — the rule the `flow-root` arm of the \
         same match already states\n  want: {want}\n  got:  {got}"
    );

    // …and LAYOUT is unchanged: a list item is still block-level and still stacks. If this fix had
    // given layout a new mode to mishandle, the two <li> would no longer be one above the other.
    let a = manuk_css::query_selector_all(page.dom(), root, "#li")[0];
    let b = manuk_css::query_selector_all(page.dom(), root, "#over")[0];
    let rects = page.root_box.node_rects(page.dom());
    let (ra, rb) = (rects.get(&a), rects.get(&b));
    if let (Some(ra), Some(rb)) = (ra, rb) {
        assert!(
            rb.y > ra.y,
            "a list item is BLOCK-LEVEL and must still stack — this was a serialization fix and \
             must not have moved a box: #li at y={} #over at y={}",
            ra.y,
            rb.y
        );
    }
}
