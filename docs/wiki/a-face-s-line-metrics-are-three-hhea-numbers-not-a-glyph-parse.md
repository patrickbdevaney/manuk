# A face's line metrics are three hhea numbers, not a glyph-outline parse

> Landed PERF.1.3.b.

## Mechanism (engine/text/src/lib.rs, `FaceMetrics`)

- Layout only ever asked `fontdue::Font` for `horizontal_line_metrics(size)`. `Font::from_bytes` compiles every glyph outline,
  8-12 ms per distinct face (quotes.toscrape.com/js loads three).
- `FaceMetrics::read` now parses with `ttf-parser` 0.21.1 (the exact version fontdue uses, pinned `=0.21.1`) and keeps
  `ascender/descender/line_gap` as `i16 -> f32` plus `units_per_em`; scaling is `x * (px / upem)`, fontdue's own arithmetic, so values are bit-identical.
- `MANUK_EAGER_FONT_PARSE=1` re-adds the old eager fontdue parse (the gate's mutant).
- fontdue stays a dependency of manuk-text only for that mutant; paint's use is unrelated.

## Measured (5 runs, quotes.toscrape.com/js, trace)

- `layout` first line 4.6 ms (eager) -> 1.6 ms; `cascade+layout+blocking scripts` phase median 69.0 -> 49.6 ms.
