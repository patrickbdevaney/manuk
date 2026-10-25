# PERF.1.1.b paired gate: null, mutant, re-judge

chrome_ver=145.0.7632.116 · 22 URLs listed in docs/sprint/perf1/urls.txt; the Σ uses the 20 URLs that are OK in both arms on all runs (mkp.gem.gov.in and virtual.lapampa.edu.ar return FAIL rc != 0 in both arms on every run, so they are excluded from every sum) · 3 runs · each run interleaves arm A and arm B per URL, order flips each run.

Judged with: `python3 -I scripts/perf1-gate.py --paired <paired.jsonl>` (per-run Σ ratio B/A, median and min-max band). Per-arm sums: `summ.py` = per-URL median over runs, then Σ over URLs.

## null
```
run1 pairs=20 sum_A=26.42s sum_B=22.41s B/A=0.848
run2 pairs=20 sum_A=22.41s sum_B=21.34s B/A=0.952
run3 pairs=20 sum_A=23.74s sum_B=23.40s B/A=0.986
PAIRED phase=subres_fetch median_sum_A=23.74s median_sum_B=22.41s ratio_B/A median=0.952 band=[0.848-0.986] n_runs=3
PAIRED GREEN (bar 0.90 <= B/A <= 1.10)
manuk urls=20 subres_fetch=23.41s doc_fetch=4.84s settle=15.17s
manuk-b urls=20 subres_fetch=20.86s doc_fetch=4.65s settle=15.25s
```

## mutant
```
run1 pairs=20 sum_A=20.04s sum_B=50.23s B/A=2.506
run2 pairs=20 sum_A=24.50s sum_B=74.02s B/A=3.022
run3 pairs=20 sum_A=22.92s sum_B=63.03s B/A=2.750
PAIRED phase=subres_fetch median_sum_A=22.92s median_sum_B=63.03s ratio_B/A median=2.750 band=[2.506-3.022] n_runs=3
PAIRED RED (bar 0.90 <= B/A <= 1.10)
manuk urls=20 subres_fetch=19.54s doc_fetch=4.08s settle=15.14s
manuk-b urls=20 subres_fetch=50.96s doc_fetch=4.09s settle=15.13s
```

## rejudge
```
run1 pairs=20 sum_A=20.27s sum_B=37.96s B/A=1.873
run2 pairs=20 sum_A=23.38s sum_B=47.33s B/A=2.024
run3 pairs=20 sum_A=27.90s sum_B=51.21s B/A=1.836
PAIRED phase=subres_fetch median_sum_A=23.38s median_sum_B=47.33s ratio_B/A median=1.873 band=[1.836-2.024] n_runs=3
PAIRED RED (bar 0.90 <= B/A <= 1.10)
manuk urls=20 subres_fetch=22.20s doc_fetch=4.49s settle=15.20s
manuk-b urls=20 subres_fetch=41.01s doc_fetch=4.07s settle=22.05s
```

Arms: null = HEAD vs HEAD; mutant = HEAD vs HEAD with MANUK_MAX_CONNS_PER_HOST=1 on B; rejudge = HEAD (A) vs 5dd98b69 pre-fix binary (B).

## PERF.1.1 bar on arm A (rejudge), 3 runs
```
urls=20 sum_subres_fetch=22.20s sum_doc_fetch=4.49s
duplicate wire requests (all traces) = 0
GATE GREEN
```

PERF.1.2 bar on arm A: Σ settle (per-URL median) = 15.20 s vs bar <= 11 s: NOT met; arm B (pre-fix) 22.05 s.
