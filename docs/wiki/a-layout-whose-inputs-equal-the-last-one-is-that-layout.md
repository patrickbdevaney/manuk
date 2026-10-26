# A layout whose inputs equal the previous slow layout is that layout (restyle_and_layout memo)

> Landed PERF.1.3.a. Numbers: docs/sprint/PERF.md "PERF.1.3.a results".

## Mechanism (engine/page/src/lib.rs `restyle_and_layout`)

- comix.to runs ~13 whole cascade+layout passes of ~440 ms. A swiper carousel toggles `class` on its slides every script round, so
  the DOM, the dirty bits and `mutation_seq` all move - but the computed styles and everything else layout reads come out equal.
- After the cascade, `restyle_and_layout` compares the post-cascade `StyleMap` (whole, `PartialEq`) and a `layout_key` hash
  (tree shape, tags, text, attributes **except `class`/`style`**, shadow roots, sheet source hashes, viewport/ICB, font-set
  fingerprint, decoded-image sizes, hover/focus/active) with the last *slow* (>= 50 ms, >= 1000-node) layout. Equal -> clone the stored
  `(styles, root_box)` instead of calling `layout_document` again. The cascade still runs every time.
- Layout reads no `class`/`style` attribute (they act through the cascade, compared whole), so leaving them out of the key is what makes
  the carousel's churn a hit. Anything else layout reads must stay in the key.
- `layout_document` leaves thread-local side outputs (grid tracks, relative offsets, used end margins). A hit is valid only if
  `manuk_layout::layouts_run()` is unchanged since the stored layout, i.e. no other layout overwrote them.
- `MANUK_NO_PASS_MEMO=1` turns it off (the gate's mutant).

## Limits

Cascade (not skipped) and the per-pass sheet re-parse remain. A page whose script changes a layout-relevant style every round still
pays every pass; making min/max-content reusable across such passes is the unlanded half of the node.
