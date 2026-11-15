# W5.G1.b.a.a: cdp-layer re-run, every row tied to its transcript

chrome_ver=145.0.7632.116 (both sets). Suite real-v1-det, Haiku, 10 tasks x 3 runs, `real_run.py run --arm cdp-layer --jobs 1`
with the `archive_prior` guard from W5.G1.b.a in place.

Rows: `cdp-layer-v2.tsv` (new, 30 rows, dates 2026-11-14). The original `cdp-layer.tsv` (2026-11-07) is kept unchanged and
is NOT merged with v2. Its transcripts were overwritten by later runs and cannot be used for the G1 causes.

## Checker

    python3 -I bench/agent-tasks/w5g1_transcript_check.py --tsv docs/sprint/w5/G1/cdp-layer-v2.tsv
    30/30 rows tied to their transcript
    CHECK PASS (exit 0)

    same checker on the original cdp-layer.tsv: 0/30 rows tied (exit 1)
    mutant (one row's answer altered in a temp copy): 29/30, CHECK FAIL, exit 1

## Pass counts (3 runs each, not merged)

| task | cdp-layer.tsv (2026-11-07) | cdp-layer-v2.tsv (2026-11-14) | v2 runs |
|---|---|---|---|
| dashdish-10 | 3/3 | 2/3 | 1,0,1 |
| dashdish-7 | 0/3 | 2/3 | 1,0,1 |
| fly-unified-6 | 0/3 | 0/3 | 0,0,0 |
| gomail-3 | 0/3 | 3/3 | 1,1,1 |
| gomail-5 | 0/3 | 1/3 | 0,1,0 |
| gomail-8 | 0/3 | 0/3 | 0,0,0 |
| opendining-10 | 0/3 | 0/3 | 0,0,0 |
| opendining-4 | 0/3 | 1/3 | 0,0,1 |
| staynb-2 | 3/3 | 2/3 | 1,1,0 |
| staynb-4 | 3/3 | 2/3 | 1,1,0 |
| **total runs passed** | **9/30** | **13/30** | |
| tasks passing in >=2 of 3 | 3 | 5 | |

## Reading

- The two sets were taken a week apart, and the tree changed in between (for example 77b97d40 changed the CDP shield,
  which the cdp-layer arm uses). The difference (9 vs 13 runs, gomail-3 0/3 vs 3/3) is therefore NOT attributed to any
  one cause here. Attribution needs a repeat at a single tree, which is a follow-up node.
- The archive guard is verified by the checker, not by these pass counts.
- Any G1 causes or classes that rely on the cdp-layer arm must be recomputed from v2 (node W5.G1.b.b).
