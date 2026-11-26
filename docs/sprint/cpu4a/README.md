# CPU.4.a — remove cascade/layout passes that recompute an unchanged result — chrome_ver=145.0.7632.116

Links, not duplicates: CPU.4 (attribution, `docs/sprint/cpu4/`), CPU.2.a (`note_cascade_current`), PERF.1.3.a.a.

Two changes in `engine/page/src/lib.rs`:
1. **Post-script restyle skipped when the scripts left the DOM untouched** (`dom.mutation_seq()` unchanged across `load_document`).
   It was a cascade + root-scrollbar re-cascade + layout over inline sheets only, reproducing the initial pass.
2. **`relayout_zoomed`'s tree-grew restyle calls `note_cascade_current`**, so the `apply_stylesheets` in `finish_loading: external CSS`
   that follows sees identical inputs and skips instead of redoing a full cascade+layout.

Escape hatches restoring the old structure (used as the "before" arm): `MANUK_NO_SKIP_CLEAN_RESTYLE`, `MANUK_NO_CASCADE_NOTE`.

## Gate: `python3 docs/sprint/cpu4a/gate.py` (3 interleaved before/after rounds, real manuk-mcp navigate+observe, medians; host load ~20-26)
| page | before cascade/layout passes / cpu-s | after |
|---|---|---|
| mdn-css | 7 / 6 / 4.991 | 4 / 4 / 2.995 (-40%) |
| rfc-editor | 3 / 3 / 0.561 | 2 / 2 / 0.560 |

observe output byte-identical before vs after on both pages. `--mutant` (after also gets the escape hatches): mdn 7/6 passes, cpu-s -6%,
gate exits 1. Attribution (`cpu4/attrib.py`, `phases-mcp.tsv`): phase rows sum to 99.0-99.9% of wait4 total in all 6 runs.
rfc-editor's remaining layout (2 passes, ~0.2 of 0.56) is first-layout cost, not a repeat; its saved pass was cheap, so cpu-s is flat within noise.
