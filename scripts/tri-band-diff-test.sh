#!/usr/bin/env bash
# ── gate for docs/sprint/TRI-BAND-DIFF.md: every machine-checked row must cite a real TSV line that says what
# the row claims, and the two means must recompute to the figures the doc gives. Red on mutants.
# usage: tri-band-diff-test.sh [DIFF.md]
set -uo pipefail
R=/home/patrickd/manuk
MD=${1:-$R/docs/sprint/TRI-BAND-DIFF.md}
B=$R/docs/bench
fail=0
ok()  { echo "  ok   $*"; }
bad() { echo "  FAIL $*"; fail=1; }

# rows: | site | RUN:LINE | status | shape | RUN:LINE | status | shape |
rows=$(grep -E '^\| [a-z_]+ \| [0-9]{6}:[0-9]+ \|' "$MD")
n=$(printf '%s\n' "$rows" | grep -c .)
[ "$n" -eq 10 ] && ok "10 machine-checked rows" || bad "rows=$n (want 10)"

# check one cited cell: site, "RUN:LINE", want_status, want_shape
cite() {
  local site=$1 ref=$2 st=$3 sh=$4
  local run=${ref%%:*} line=${ref##*:}
  local f=$B/tri-sweep-20261025-$run.tsv
  [ -f "$f" ] || { bad "$site: no file $f"; return; }
  local got
  got=$(awk -F'\t' -v L="$line" 'NR==L{print $1"\t"$3"\t"$5}' "$f")
  local want="$site"$'\t'"$st"$'\t'"$sh"
  [ "$got" = "$want" ] && ok "$site $ref = $st ${sh:-–}" || bad "$site $ref: file has [$got] want [$want]"
}

cited_sum=0; cited_n=0; today_sum=0; today_n=0
while IFS='|' read -r _ site c cst csh t tst tsh _; do
  site=$(echo "$site" | xargs); c=$(echo "$c" | xargs); cst=$(echo "$cst" | xargs); csh=$(echo "$csh" | xargs)
  t=$(echo "$t" | xargs); tst=$(echo "$tst" | xargs); tsh=$(echo "$tsh" | xargs)
  cite "$site" "$c" "$cst" "$csh"
  cite "$site" "$t" "$tst" "$tsh"
  [ "$cst" = OK ] && [ -n "$csh" ] && { cited_sum=$(awk -v a="$cited_sum" -v b="$csh" 'BEGIN{print a+b}'); cited_n=$((cited_n+1)); }
  [ "$tst" = OK ] && [ -n "$tsh" ] && { today_sum=$(awk -v a="$today_sum" -v b="$tsh" 'BEGIN{print a+b}'); today_n=$((today_n+1)); }
done <<< "$rows"

cited_mean=$(awk -v s="$cited_sum" -v n="$cited_n" 'BEGIN{printf "%.1f", s/n}')
today_mean=$(awk -v s="$today_sum" -v n="$today_n" 'BEGIN{printf "%.1f", s/n}')
echo "  cited mean over $cited_n reachable sites: $cited_mean"
echo "  today mean over $today_n reachable sites: $today_mean"
[ "$cited_n" -eq 3 ] && [ "$cited_mean" = 85.5 ] && ok "cited 3-site mean recomputes to 85.5" || bad "cited mean $cited_mean over $cited_n (want 85.5 over 3)"
[ "$today_n" -eq 6 ] && [ "$today_mean" = 71.4 ] && ok "today 6-site mean recomputes to 71.4" || bad "today mean $today_mean over $today_n (want 71.4 over 6)"

# mutants re-run the gate on a broken copy; TDD_CHILD stops the children from recursing
if [ -z "${TDD_CHILD:-}" ]; then
  T=$(mktemp -d /tmp/tri-band-diff-test.XXXXXX); trap 'rm -rf "$T"' EXIT
  export TDD_CHILD=1
  # mutant: a cited SHAPE altered to a value the file does not hold must be refused
  sed 's/| gov_edu | 013937:9 | OK | 84.0 |/| gov_edu | 013937:9 | OK | 99.9 |/' "$MD" > "$T/m1.md"
  "$0" "$T/m1.md" >/dev/null 2>&1; rc=$?
  [ "$rc" -eq 1 ] && ok "mutant altered cited shape refused (rc=1)" || bad "mutant altered shape rc=$rc (want 1)"
  # mutant: a cited line that points at the wrong site must be refused
  sed 's/| blog | 013937:2 |/| blog | 013937:3 |/' "$MD" > "$T/m2.md"
  "$0" "$T/m2.md" >/dev/null 2>&1; rc=$?
  [ "$rc" -eq 1 ] && ok "mutant wrong-site line refused (rc=1)" || bad "mutant wrong-site rc=$rc (want 1)"
fi

[ "$fail" -eq 0 ] && echo "tri-band-diff-test: PASS" || { echo "tri-band-diff-test: FAIL"; exit 1; }
