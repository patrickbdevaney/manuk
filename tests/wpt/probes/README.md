# Probe pages — reproducible measurements, NOT gates

These are **local pages written to answer one question each**, run through
`manuk-wpt fidelity --urls "file:///abs/path.html"` so the same box-probe comparison the corpus sweep uses
answers it against live Chromium. They are deliberately **not** in `tests/wpt/corpus/` — that directory
feeds the `parity` gate, and a probe whose whole purpose is to FIND a divergence would turn the gate red
for a divergence we already know about and are tracking elsewhere.

A probe earns its place here when its answer changed a decision. Each one names the question in its
`<style>` comments, and the answer lives in the journal entry for the tick that ran it.

| page | question | answer |
|---|---|---|
| `font-family-resolution.html` | does an explicitly named, installed family resolve to that family? | **t556: NO for sans faces.** `"DejaVu Sans"`, `"Noto Sans"` and a deliberately non-existent `"NoSuchFontXYZ"` all render **330px** wide here, where Chromium gives 374 / 348 / 299 — three different faces. `"DejaVu Serif"` DOES resolve (299 vs Chromium 380), so name resolution is partial, not absent. |
| `font-stack-metrics.html` | do the generic stacks (`sans-serif`/`serif`/`monospace`) measure like Chromium's? | **t556: YES, within the 8px tolerance** — which is what makes the finding above so specific: our *advance computation* is fine, our *face selection* is not. |
| `font-local-vs-webfont-name.html` | with no `@font-face` in play, does a locally-installed named family measure exactly like Chromium's? | **t560: YES — 100% SHAPE, 0 of 7 misplaced.** `"Open Sans"`, `sans-serif`, and `"Open Sans",sans-serif` all agree. So the t557/t558 resolution is correct, and the `martinfowler.com` regression is NOT resolution: it is the **webfont-shadowing rule** (below). |
| `line-box-height.html` | with `line-height: normal`, does our line BOX match Chromium's for the same face — and do wrap points agree? | **t562: YES, exactly — 100% SHAPE, 0 of 12 misplaced, and absolute placement 100% (dx=dy=dw=dh=0).** `Open Sans`, `DejaVu Sans`, the generic stack, `line-height:1.5`, `line-height:24px`, and a 400px-wide wrapping paragraph all agree. So the line-box derivation is NOT a general defect, and the residual 2px anchor-height divergence on `martinfowler.com` is **face- or size-specific to that page**, not a metrics formula error. |
| `grid-template-areas.html` | martinfowler lays sections side by side with CSS Grid + NAMED AREAS. Is the missing half the column TRACKS or the named-area PLACEMENT? | **t565: NEITHER — 100% SHAPE, 0 of 13 misplaced, absolute placement 100%.** All four shapes match Chromium exactly: auto placement on `1fr 1fr`, `grid-template-areas:"l r"` + `grid-area`, explicit `grid-column` lines, and a `1fr 200px` mix. So grid placement is NOT the defect; the site's container is simply **not receiving `display:grid`**, which makes it a cascade/selector/media question. |

