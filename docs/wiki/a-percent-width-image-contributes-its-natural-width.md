# A percentage-width image contributes its natural width, not zero

**Mechanism.** `content_right_extent` (engine/layout/src/lib.rs) measures a shrink-to-fit subtree at a 1e6 width and discards any box that filled it (FILL_SENTINEL). An `<img style="width:100%">` always fills 1e6, so it was discarded and contributed 0. A `flex: 0 1 auto` pane holding it measured 0 wide.

**Chrome.** A percentage width is cyclic in an intrinsic measure and behaves as `auto` (CSS Sizing 5.2.2), so the bitmap's own width is the max-content contribution. The flex algorithm then shrinks it to the free space (zilloft-2 map pane: 530 = 1280 - 750; chrome_ver=145.0.7632.116).

**Fix.** `ComputedStyle::natural_width` keeps the decoded bitmap width even when an author width overrides it (`fill_natural_size`); `Ctx::min_width_floor_border` returns it for a `Dim::Percent` width, which `visit` already counts for filled boxes.

**Gate.** engine/page/tests/g_flex_shrink_img_pane.rs (RED p1=0, GREEN p1=530). Limit: only a DECODED image has a natural width; in an MCP session images on zilloft-2 are never loaded, so the real page is still 0 wide (see RG.2.E1.a.a.a.a.a.a).
