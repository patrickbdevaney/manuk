#!/bin/sh
# W3.20.b.a.a verdict: french-bistro native tool count across repeat runs.
# usage: verdict.sh <RESULT.tsv>...   prints counts and STABLE iff every run has the same native_n (and chrome_ver is one value); exits 1 otherwise or on missing rows.
awk -F'\t' '
  FNR==1 { next }
  $1=="french-bistro" { n++; c[$3]++; cv[$11]++ ; print FILENAME": native_n="$3" verdict="$2 }
  END {
    if (n<3) { print "FLAKY-OR-INCOMPLETE: only "n" french-bistro rows"; exit 1 }
    k=0; for (x in c) k++; m=0; for (x in cv) m++
    if (k==1 && m==1) { for (x in c) { for (v in cv) cvv=v; print "STABLE: native_n="x" in "n"/"n" runs; chrome_ver="cvv } ; exit 0 }
    print "FLAKY: native_n values differ across runs"; for (x in c) print "  native_n="x" x"c[x]; exit 1
  }' "$@"
