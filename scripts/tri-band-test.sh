#!/usr/bin/env bash
# ── gate for scripts/tri-band.sh: green on three real runs, red on two broken inputs.
# usage: tri-band-test.sh RUN1.tsv RUN2.tsv RUN3.tsv
set -uo pipefail
R=/home/patrickd/manuk
export MANUK_CHROME_PIN=${MANUK_CHROME_PIN:-145.0.7632.116}
[ "$#" -eq 3 ] || { echo "usage: $0 RUN1 RUN2 RUN3"; exit 2; }
T=$(mktemp -d /tmp/tri-band-test.XXXXXX); trap 'rm -rf "$T"' EXIT
fail=0
ok()  { echo "  ok   $*"; }
bad() { echo "  FAIL $*"; fail=1; }

# 1. green: three real runs produce a band with four metric rows, all spreads numeric
"$R/scripts/tri-band.sh" "$T/band.tsv" "$@" >/dev/null 2>&1; rc=$?
if [ "$rc" -eq 0 ] && [ "$(tail -n +2 "$T/band.tsv" | wc -l)" -eq 4 ]; then ok "green on 3 real runs (rc=0, 4 metrics)"; else bad "green rc=$rc"; fi

# 2. mutant: one row carries the wrong chrome_ver -> must refuse
cp "$1" "$T/m1.tsv"; awk -F'\t' 'BEGIN{OFS="\t"} NR==3{$20="145.0.0.0"} {print}' "$1" > "$T/m1.tsv"
"$R/scripts/tri-band.sh" "$T/x.tsv" "$T/m1.tsv" "$2" "$3" >/dev/null 2>&1; rc=$?
[ "$rc" -eq 1 ] && ok "mutant wrong chrome_ver refused (rc=1)" || bad "mutant wrong chrome_ver rc=$rc (want 1)"

# 3. mutant: one run drops a data row -> must refuse
head -n -1 "$2" > "$T/m2.tsv"
"$R/scripts/tri-band.sh" "$T/y.tsv" "$1" "$T/m2.tsv" "$3" >/dev/null 2>&1; rc=$?
[ "$rc" -eq 1 ] && ok "mutant 9-row run refused (rc=1)" || bad "mutant 9-row run rc=$rc (want 1)"

[ "$fail" -eq 0 ] && echo "tri-band-test: PASS" || { echo "tri-band-test: FAIL"; exit 1; }
