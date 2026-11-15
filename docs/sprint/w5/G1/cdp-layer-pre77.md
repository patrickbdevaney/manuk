# W5.G1.b.a.a.a.a: the tree effect of 77b97d40, measured directly

chrome_ver=145.0.7632.116 (all three sets). Suite real-v1-det, Haiku, `cdp-layer` arm, the same 10 tasks x 3 runs as v2 and v3
(dashdish-10, dashdish-7, fly-unified-6, gomail-3, gomail-5, gomail-8, opendining-10, opendining-4, staynb-2, staynb-4),
`real_run.py run --arm cdp-layer --runs 3 --jobs 1 --timeout 420` (the default timeout).

## Setup

- Pre-77 tree: `git worktree` at `0d02a367` (= `77b97d40^`), detached, at `/home/patrickd/manuk-pre77`. Its own release
  build of `manuk-mcp` (`MOZJS_FROM_SOURCE=1`, `CARGO_TARGET_DIR` inside the worktree). Run with
  `MANUK_MCP_BIN=/home/patrickd/manuk-pre77/target/release/manuk-mcp`.
- HEAD side: the v2 and v3 sets already in this directory (2026-11-14).
- Build identity is recorded in `pre77-build.tsv`: the pre-77 binary has sha256 `f978c551…1cad` and no `drop-form-containers`
  marker string; the HEAD binary (`eb38fea0…ab99`) has the marker. The two binaries differ.

## Result

| task | pre-77 (2026-11-15) | v2 HEAD (2026-11-14) | v3 HEAD (2026-11-14) |
|---|---|---|---|
| dashdish-10 | 3/3 | 2/3 | 3/3 |
| dashdish-7 | 2/3 | 2/3 | 2/3 |
| fly-unified-6 | 0/3 | 0/3 | 0/3 |
| gomail-3 | 0/3 | 3/3 | 2/3 |
| gomail-5 | 0/3 | 1/3 | 1/3 |
| gomail-8 | 1/3 | 0/3 | 1/3 |
| opendining-10 | 0/3 | 0/3 | 0/3 |
| opendining-4 | 2/3 | 1/3 | 2/3 |
| staynb-2 | 3/3 | 2/3 | 3/3 |
| staynb-4 | 1/3 | 2/3 | 2/3 |
| **total runs passed** | **12/30** | **13/30** | **16/30** |

Gate output: `w5g1_v3_compare.py --v2 cdp-layer-pre77.tsv --v3 cdp-layer-v3.tsv` exits 0, GATE PASS (`pre77-compare.md`).

## Band reading (fixed before the pre-77 numbers were read)

The v2-v3 band at HEAD is |13 - 16| = 3 runs. The pre-77 total is OUTSIDE the band only if it is more than 3 runs from both
HEAD totals. It is 12/30: 1 run from v2 and 4 from v3. **It is INSIDE the band.** So the total does not show a tree effect of
77b97d40 beyond the run-to-run noise measured at HEAD.

Per task, the one difference that is larger than the HEAD band is gomail-3: 0/3 pre-77 against 3/3 (v2) and 2/3 (v3) at HEAD.
That matches the claim in 77b97d40's own commit (gomail-3 0/3 before the fix). Other per-task gaps are at most 2 runs and
come with 3 runs per cell, so they are not separable from noise. Gomail-3 is a 3-run cell, so this is one signal, not a
measured effect size.

## What this does not settle

- **The 2026-11-07 set (9/30) is still unattributed.** The pre-77 total (12/30, 2026-11-15) sits between 9 and 13. So the
  9-to-13 gap is not a single step at 77b97d40 either; it may be run noise or a change at another commit. Not attributed here.
- **The v2 and v3 binary is not the one hashed here.** Per `cdp-layer-v3.md`, the `target/release/manuk-mcp` that produced
  the v2 and v3 rows was built at 20:57 on 2026-11-14. That binary was overwritten by a later build (00:14 on 2026-11-15)
  and cannot be re-hashed. The 77b97d40 marker in the 00:14 binary is recorded, but the 20:57 one is not verified.
- **One row has no result event.** `opendining-10 r1` hit the 420 s timeout (`seconds` 420.1, pass 0). The transcript has no
  `result` line, so `w5g1_transcript_check.py --tsv cdp-layer-pre77.tsv` reports 29/30 tied and exits 1. The row is kept
  as recorded (a timeout is a failed run), not re-run and not dropped. The check's rule does not yet account for timeouts;
  that is a follow-up.

## Checks

- `w5g1_v3_compare.py` mutants (in a temp directory): dropped row → RED (`staynb-4: v2 has 2 rows`); pin mismatch → RED;
  equal pre-77 and HEAD shas → RED; marker present in the pre-77 build → RED; duplicate run number in one task → RED.
  The real files → GATE PASS, exit 0.
- Transcript check: 29/30 rows tied (see the timeout row above).
