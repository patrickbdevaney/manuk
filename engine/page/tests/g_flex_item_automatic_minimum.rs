//! **G_FLEX_ITEM_AUTOMATIC_MINIMUM — a flex item with a definite `width` could never shrink, and the
//! reason was that we answered taffy's own question with the answer taffy was about to clamp.**
//!
//! CSS Box Sizing §5.1 / Flexbox §4.5: `min-width: auto` on a flex item resolves to its
//! **content-based minimum size**, which is
//! `min(specified size suggestion, content size suggestion)` — the item's declared width against its
//! *content's* min-content size. taffy implements exactly that and applies the `min` itself
//! (`taffy-0.12.1/src/compute/flexbox.rs`: `min_content_main_size.maybe_min(child.size.main(dir))`),
//! so what it asks the engine for is the **content** suggestion alone.
//!
//! ⚠⚠⚠ **We answered with the item's declared width, which makes taffy's `min` VACUOUS.** The
//! automatic minimum of every fixed-width flex item became its own width, so **no such item could
//! ever shrink** — the single most load-bearing behaviour in flexbox, absent, on the default value of
//! the property. Two 200px items in a 300px row stayed 200 where every browser gives 150.
//!
//! ⭐ **AND IT IS WHY THE FOLKLORE WORKS.** *"My flex row won't shrink — add `min-width: 0`"* and
//! *"...add `overflow: hidden`"* are on every CSS forum on the web, and both were already correct
//! here: each sets taffy's `style_min_main_size` to a definite zero, which never reaches the measure
//! at all. Only the DEFAULT was broken, which is precisely the configuration nobody files a bug
//! about because the workaround is folklore. `n1`/`n2` are those two rows and they are controls: they
//! were green before this fix and must stay green.
//!
//! ⚠ The diagnosis took an instrumented measure callback, not reasoning: every input handed to taffy
//! was **correct** — `container_width=300`, `available_space=Definite(300)`, `flex_shrink=1`,
//! `min_size=auto`, `display=Flex`, `direction=Row` — and taffy's own unit-level test in
//! `taffy_tree` shrinks to 150/150 with those same inputs. Two cache layers were disabled and
//! exonerated first. The only thing that told the truth was logging what the measure was ASKED and
//! what it ANSWERED: asked *"how narrow can you get?"* (`known.width = None`,
//! `available = MinContent`), it said **200**.
//!
//! **Every expected number below is headless Chrome on this platform** — `google-chrome --headless
//! --disable-gpu --dump-dom` on this exact fixture.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
.p{display:flex;width:300px}
.rail{overflow:auto;display:flex;width:300px}
.card{display:inline-block;width:200px;height:100px}
.noshrink .card{flex-shrink:0}
</style></head><body>
<div class=p><div id=a style="width:200px;height:20px">a</div><div style="width:200px;height:20px">b</div></div>
<div class=p><div id=b style="width:200px;height:20px;min-width:0">a</div><div style="width:200px;height:20px;min-width:0">b</div></div>
<div class=p><div id=c style="width:200px;height:20px;overflow:hidden">a</div><div style="width:200px;height:20px;overflow:hidden">b</div></div>
<div class=p><div id=d style="height:20px">aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa</div><div style="height:20px">bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb</div></div>
<div class=rail id=r1><div class=card>1</div><div class=card>2</div><div class=card>3</div><div class=card>4</div><div class=card>5</div></div>
<div class="rail noshrink" id=r2><div class=card>1</div><div class=card>2</div><div class=card>3</div><div class=card>4</div><div class=card>5</div></div>
<div id=out></div><script>
var s='';
['a','b','c','d'].forEach(function(k){s+=document.getElementById(k).offsetWidth+';';});
s+=document.getElementById('r1').scrollWidth+';'+document.getElementById('r2').scrollWidth;
document.getElementById('out').textContent=s;
</script></body></html>"##;

#[test]
fn a_flex_item_with_a_definite_width_shrinks_to_its_content_based_minimum() {
    let fonts = FontContext::new();
    let p = manuk_page::Page::load(HTML, "http://x/", &fonts, 800.0);
    let root = p.dom().root();
    let hits = manuk_css::query_selector_all(p.dom(), root, "#out");
    assert!(!hits.is_empty(), "fixture is missing #out");
    let got = p.dom().text_content(hits[0]);
    assert!(
        got.contains(';'),
        "the fixture's script must run, or this gate measures nothing — got {got:?}"
    );
    let vals: Vec<i32> = got.split(';').map(|v| v.parse().unwrap_or(-1)).collect();

    // (label, Chrome's answer)
    let expect: [(&str, i32); 6] = [
        ("t1 two 200px items in a 300px row shrink to 150", 150),
        ("n1 CONTROL min-width:0 — already correct before", 150),
        ("n2 CONTROL overflow:hidden — already correct before", 150),
        ("n3 CONTROL auto-width items, long text — untouched", 312),
        (
            "t2 the carousel rail's scrollWidth collapses to the rail",
            300,
        ),
        ("n4 CONTROL flex-shrink:0 rail really does overflow", 1000),
    ];
    for (i, (label, want)) in expect.iter().enumerate() {
        assert_eq!(
            vals[i], *want,
            "{label}: expected {want} (headless Chrome's own number for this fixture), got {}. \
             A 200 at t1 means the item's automatic minimum is still its own declared width, so \
             taffy's `min(specified, content)` is vacuous and nothing can shrink. A 150 at n4 would \
             mean the fix shrinks items the author pinned with `flex-shrink: 0`.",
            vals[i]
        );
    }

    // ⭐ n4 above is the half that stops this from being satisfiable by "always shrink": the
    // `flex-shrink: 0` rail MUST still overflow to 1000, which is the whole point of a carousel.
    assert_ne!(
        vals[4], vals[5],
        "the shrinking rail and the flex-shrink:0 rail must not report the SAME scrollWidth — if \
         they do, one of the two behaviours has been made unreachable."
    );
}
