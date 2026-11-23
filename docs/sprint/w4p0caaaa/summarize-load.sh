#!/usr/bin/env bash
# W4.P0.c.a.a.a.a.a.a — gate check + summary for load-trace.tsv.
# Usage: summarize-load.sh <trace.tsv>
# Prints one "# SUMMARY ..." line. Exit 1 (gate RED) if rows < 250 or span < 24 h.
# A quiet window = a run of consecutive samples with load1 < 2.0 whose first-to-last span is >= 900 s (15 min).
# Span is measured between sample epochs, so a window's true quiet length is at least what is printed.
set -u
TRACE=${1:?usage: summarize-load.sh <trace.tsv>}
awk -F'\t' '
  NR == 1 { next }                       # header
  /^#/ { next }                          # previous summary lines
  NF < 6 { next }
  {
    n++; utc[n] = $1; ep[n] = $2 + 0; l1[n] = $3 + 0
    if (n == 1 || l1[n] < minl) { minl = l1[n]; minutc = $1 }
  }
  END {
    if (n == 0) { print "# SUMMARY rows=0 GATE RED"; exit 1 }
    span = ep[n] - ep[1]
    wins = ""; nw = 0
    i = 1
    while (i <= n) {
      if (l1[i] < 2.0) {
        j = i
        while (j + 1 <= n && l1[j + 1] < 2.0) j++
        if (ep[j] - ep[i] >= 900) {
          nw++
          wmax = 0
          for (k = i; k <= j; k++) if (l1[k] > wmax) wmax = l1[k]
          wins = wins sprintf("%s..%s (%.0f min, max load1 %.2f); ", utc[i], utc[j], (ep[j] - ep[i]) / 60, wmax)
        }
        i = j + 1
      } else i++
    }
    if (nw == 0) wins = "none"
    printf("# SUMMARY rows=%d span_h=%.2f min_load1=%.2f at %s; quiet windows (load1<2.0, >=15 min): %s\n",
           n, span / 3600, minl, minutc, wins)
    gate_ok = (n >= 250 && span >= 86400)
    if (!gate_ok) { print "# GATE RED: need >=250 rows and >=24 h span"; exit 1 }
    exit 0
  }' "$TRACE"
