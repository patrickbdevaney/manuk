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

## A flex/grid item's content height depends on its resolved WIDTH, not on the probe that asked (RG.S1.a.a.a.b.a)

zilloft `/homes` (4.7k nodes): one layout cost ~1.18 s, ~98% inside one taffy solve whose subtree
(2740 descendants, 186 flex + 361 grid containers) was measured ~15k times per layout. `measure_cache`
keys on `(node, avail, mode)`; taffy probes the same item at several avails/modes that
shrink-to-fit to the SAME width, and each re-laid-out the subtree. `Ctx::measure_by_width`
(`(node, width bits) -> (height, baseline)`) serves those. Measured with
`docs/sprint/rg/S1aaab-drain.py 3` (chrome_ver=145.0.7632.116): Beds & Baths click 9.9-10.0 s -> 7.3-7.4 s,
Start search 7.9-8.2 s -> 6.4-6.6 s. NOT enough for the 5 s gate; the rest is real nested solves
(widths in one node differ by the 16 px padding) and 2-9 relayouts per click whose styles differ
(overflow-y toggles, opacity steps), so the whole-layout memo above misses them.
`MANUK_NO_MEASURE_BY_WIDTH=1` turns it off (the fixture's mutant).

## An opacity-only restyle reuses the memoed fragment tree (RG.S1.a.a.a.b.a.a)
zilloft's panel fades elements in/out: forced reflows at equal `layout_key` whose styles differ only in effective `opacity` (and nothing else) used to miss the memo and pay a ~1 s relayout. `restyle_and_layout` now calls `manuk_layout::reuse_for_opacity_only(styles_in, styles, root_box)`: every style pair must be equal after masking `opacity`, then the cached tree is cloned with each box's `opacity` set from the new style (anonymous boxes take their nearest named ancestor's, or the reuse bails). Only when the `@container` re-pass changed nothing. Any other differing property (e.g. `overflow-y`) still lays out. `MANUK_NO_OPACITY_PATCH=1` disables it (the fixture's mutant).

## font-family resolution is memoed per list (RG.S1.a.a.a.b.a.b)
A gdb-sampled profile (1200 samples, symbols built into a side target dir) of the zilloft /homes clicks put ~78% of every 4.7k-node layout inside `text_style` -> `FontContext::resolve_family` / `first_non_generic_family` -> `fontdb::Database::query`, a linear scan of every installed face by string compare, repeated per text node per taffy probe. Both answers are pure in (the font-family list, the registered fonts), so `FontContext::family_memo` keys them by the exact list and drops itself whenever `FontContext::fingerprint` moves (a webfont registered/declared). Per-layout `total_ms` of the 4.7k-node page 820 -> 165 (median of 56 layouts), Beds & Baths click 7.4 s -> 2.9 s. Fixture `family_memo_agrees_with_uncached_and_sees_new_webfonts` goes red if the fingerprint sync is removed.

## Literal-1.0 boxes under an opacity-0 ancestor (RG.S1.a.a.a.b.a.a.a)
`reuse_for_opacity_only` rejected a restyle when an anonymous box (e.g. the inline-run wrapper in a block with mixed text/block children) carried `opacity: 1.0` under an ancestor whose old opacity was 0: layout builds those boxes with a literal 1.0, not the folded value, so "equal to the ancestor's" is the wrong test. They now keep 1.0 (a fresh layout builds the same). Measured on the zilloft Beds & Baths click (chrome_ver=145.0.7632.116, 3 runs): 8 -> 7 SLOW RESTYLE+LAYOUT. The other misses are real: DOM growth, body scroll-lock overflow-y, a baked transform-origin, and attr/text mutations (cascade-only hover/focus/active state is in `layout_key` although layout never reads it, but a direct-path memo without it still missed on the attr/text changes).
