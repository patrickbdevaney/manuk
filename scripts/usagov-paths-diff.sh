#!/usr/bin/env bash
# R0.4.a.b.a.a.a.a: recompute, from the stored docs/bench/oracle-paths/*.oracle-paths.tsv files, which path keys exist in
# the larger usa.gov oracle DOM (sha 2286d97a1714) and not in the smaller (a6a6db2122b8). Prints one markdown row per key.
# usage: usagov-paths-diff.sh [DIR]   (DIR defaults to docs/bench/oracle-paths)
set -uo pipefail
D=${1:-/home/patrickd/manuk/docs/bench/oracle-paths}
big=$(cd "$D" && sha256sum *usa.gov.oracle-paths.tsv | awk '$1 ~ /^2286d97a1714/{print $2; exit}')
small=$(cd "$D" && sha256sum *usa.gov.oracle-paths.tsv | awk '$1 ~ /^a6a6db2122b8/{print $2; exit}')
[ -n "$big" ] && [ -n "$small" ] || { echo "missing a stored file for one of the two oracle shas" >&2; exit 1; }
echo "big=$big small=$small" >&2
awk -F'\t' 'FNR==NR{s[$1]=1; next} !($1 in s){print "| `" $1 "` | " $2 " | " $3 " | " $4 " | " $5 " |"}' "$D/$small" "$D/$big"
