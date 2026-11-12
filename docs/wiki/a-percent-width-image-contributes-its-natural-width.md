# A percentage-width image contributes its natural width, not zero

**Mechanism.** `content_right_extent` (engine/layout/src/lib.rs) measures a shrink-to-fit subtree at a 1e6 width and discards any box that filled it (FILL_SENTINEL). An `<img style="width:100%">` always fills 1e6, so it was discarded and contributed 0. A `flex: 0 1 auto` pane holding it measured 0 wide.

**Chrome.** A percentage width is cyclic in an intrinsic measure and behaves as `auto` (CSS Sizing 5.2.2), so the bitmap's own width is the max-content contribution. The flex algorithm then shrinks it to the free space (zilloft-2 map pane: 530 = 1280 - 750; chrome_ver=145.0.7632.116).

**Fix.** `ComputedStyle::natural_width` keeps the decoded bitmap width even when an author width overrides it (`fill_natural_size`); `Ctx::min_width_floor_border` returns it for a `Dim::Percent` width, which `visit` already counts for filled boxes.

**Gate.** engine/page/tests/g_flex_shrink_img_pane.rs (RED p1=0, GREEN p1=530). Follow-up (RG.2.E1.a.a.a.a.a.a): the real page stayed 0 wide because `Page::settle_async` (what runs after a click) never fetched images, so an `<img>` a click inserted had no bitmap/size and `naturalWidth` 0. It now calls `fetch_and_apply_images` once the rounds finish (already-tried images are skipped; lean profile gets header dims). Gate: docs/sprint/rg/E1aaaaaa-gate.py (RED nat=0, GREEN nat=120) and E1aaaaa-probe.py (0 -> 530, chrome_ver=145.0.7632.116).
