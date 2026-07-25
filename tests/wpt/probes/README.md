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
