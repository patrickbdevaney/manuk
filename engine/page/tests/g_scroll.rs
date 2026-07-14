//! **G_SCROLL — `element.scrollTop` is real: it reports the truth, it clamps, it moves pixels, it fires.**
//!
//! This was the roadmap's #2 item, and it was the worst kind of gap: **not missing — lying.**
//!
//! * Reading `element.scrollTop` gave **`undefined`**.
//! * Writing it quietly created a plain JavaScript own-property. It scrolled nothing, and it threw
//!   nothing.
//!
//! So a virtualised list would set `scrollTop`, read it back, get **its own value**, and conclude it had
//! worked. **The failure was silent on both sides of the API**, which is the only kind that ships.
//!
//! And `scrollHeight` / `clientHeight` were absent too — which matters more than it looks, because
//! `scrollHeight - clientHeight` is precisely the number every virtualised list divides by to decide
//! which slice of the data to render. `undefined - undefined` is `NaN`, and a list that renders `NaN`
//! rows renders none.
//!
//! What this gate demands, and each one is a thing real code depends on:
//!
//! 1. the geometry is **truthful** (`clientHeight` is the visible window; `scrollHeight` the content);
//! 2. a write is **clamped** — a script that assigns `1e9` to reach the bottom must read back the real
//!    maximum, or *"am I at the bottom?"* is false forever;
//! 3. it survives a **re-layout** — layout starts from zero every time, so without care the user types
//!    in a chat box and the list jumps back to the top;
//! 4. it **moves the actual pixels**;
//! 5. it **fires `scroll`** — an infinite feed listens for that to fetch the next page. A scroll that
//!    moves the pixels and fires nothing is half a scroll.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<div id="box" style="height:100px;width:200px;overflow:auto">
  <div id="tall" style="height:1000px;background:#f00">
    <div id="marker" style="position:relative;top:500px;height:120px;background:#00f"></div>
  </div>
</div>
<script>
  var R = [], fired = 0;
  var box = document.getElementById('box');
  box.addEventListener('scroll', function(){ fired++; });

  // ── 1. The geometry must be TRUE, not merely present.
  R.push('clientH:' + box.clientHeight);      // the visible window: 100
  R.push('scrollH:' + box.scrollHeight);      // the content: 1000
  R.push('top0:' + box.scrollTop);            // starts at the top, and is a NUMBER, not undefined

  // ── 2. A write must take effect and be readable on the very next line.
  box.scrollTop = 300;
  R.push('top1:' + box.scrollTop);

  // ── 3. …and must CLAMP. A list that scrolls to 1e9 to reach the bottom must read back the real
  //       maximum (scrollHeight - clientHeight = 900), or "am I at the bottom?" is false forever.
  box.scrollTop = 1e9;
  R.push('clamped:' + box.scrollTop);

  box.scrollTop = -50;
  R.push('floor:' + box.scrollTop);

  box.scrollTop = 200;                        // settle somewhere checkable
  R.push('final:' + box.scrollTop);

  globalThis.__report = function(){
    R.push('fired:' + (fired > 0));
    document.getElementById('out').textContent = R.join(' ');
  };
</script></body></html>"##;

#[test]
fn element_scrolling_reports_the_truth_clamps_moves_pixels_and_fires() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://scroll.test/", &fonts, 400.0);
    page.eval_for_test("globalThis.__report && __report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "clientH:100",
            "clientHeight is the VISIBLE window. It was undefined, and `scrollHeight - clientHeight` is \
             the number every virtualised list divides by — `NaN` rows is no rows",
        ),
        ("scrollH:1000", "scrollHeight is the CONTENT's full extent, not the box's height"),
        (
            "top0:0",
            "scrollTop must be a NUMBER at rest. It read `undefined`, and `undefined` fails every \
             comparison silently",
        ),
        (
            "top1:300",
            "a write must take effect and be readable on the next line — this is what a virtualised \
             list does, every frame",
        ),
        (
            "clamped:900",
            "CLAMPED to scrollHeight - clientHeight. Assigning 1e9 to reach the bottom is idiomatic; \
             reading back 1e9 makes every `atBottom` check false forever",
        ),
        ("floor:0", "and clamped at zero — a negative scroll offset is not a thing"),
        ("final:200", "and it settles where it was put"),
        (
            "fired:true",
            "the `scroll` event must FIRE. An infinite feed listens for it to fetch the next page; a \
             sticky header listens for it to pin. A scroll that moves pixels and fires nothing is half \
             a scroll",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_SCROLL: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }

    // ── 4. AND IT MUST MOVE THE ACTUAL PIXELS.
    //
    // Everything above could be satisfied by a number in a map. The blue marker sits 500px down inside a
    // container only 100px tall — so at scrollTop=200 it is still far below the fold and the container
    // shows red. Scroll to 500 and the marker must appear. A scrollTop that JavaScript can read but the
    // user cannot see is exactly the bug this gate exists to have killed.
    page.eval_for_test("document.getElementById('box').scrollTop = 500");
    let canvas = page.paint(&fonts, 220, 140);
    let rgba = canvas.rgba_bytes();
    // Inside the 200x100 container. The marker is 120px tall and, at scrollTop=500, starts at the
    // container's top edge — so it fills the whole visible window and no sample can land on an edge.
    let (x, y) = (100usize, 60usize);
    let i = (y * 220 + x) * 4;
    let (r, g, b) = (rgba[i], rgba[i + 1], rgba[i + 2]);

    assert!(
        b > 150 && r < 100,
        "G_SCROLL: after `scrollTop = 500` the blue marker (500px down a 1000px column, in a 100px \
         window) must be VISIBLE — the pixel at ({x},{y}) is `{r},{g},{b}`.\n\n  \
         Everything else this gate asserts could be satisfied by a number in a hash map. A `scrollTop` \
         that JavaScript can read but the user cannot see is the bug, not the fix."
    );
}
