#!/usr/bin/env bash
# ── gate for docs/sprint/USAGOV-DRIFT.md: every cited usa.gov row must match its TSV line, the five fresh runs must
# be five distinct files, the min/max/spread/mean must recompute to the doc's figures, the signature table must be a
# 1:1 map from (cov, vis, paths) to SHAPE, and every cited row must carry the pinned chrome_ver. Red on mutants.
# usage: usagov-drift-test.sh [DRIFT.md]
set -uo pipefail
R=/home/patrickd/manuk
MD=${1:-$R/docs/sprint/USAGOV-DRIFT.md}
B=$R/docs/bench
PIN=${MANUK_CHROME_PIN:-145.0.7632.116}
fail=0
ok()  { echo "  ok   $*"; }
bad() { echo "  FAIL $*"; fail=1; }

# section rows: "| run | line | status | cov | shape | vis | paths |" (fresh) and "| run | line | cov | shape | vis | paths |" (all)
fresh=$(awk '/^## Five fresh runs/{s=1;next} /^## /{s=0} s && /^\| [0-9]{6} \| [0-9]+ \|/' "$MD")
all=$(awk '/^## Every usa.gov row/{s=1;next} /^## /{s=0} s && /^\| [0-9]{6} \| [0-9]+ \|/' "$MD")
nf=$(printf '%s\n' "$fresh" | grep -c .); na=$(printf '%s\n' "$all" | grep -c .)
[ "$nf" -eq 5 ] && ok "five fresh rows" || bad "fresh rows=$nf (want 5)"
[ "$na" -eq 11 ] && ok "eleven usa.gov rows on disk" || bad "all rows=$na (want 11)"

# check one cited row against its TSV line: run line cov shape vis paths status
check() {
  local run=$1 line=$2 st=$3 cov=$4 sh=$5 vis=$6 paths=$7
  local f=$B/tri-sweep-20261025-$run.tsv
  [ -f "$f" ] || { bad "$run: no file $f"; return 1; }
  local got
  got=$(awk -F'\t' -v L="$line" 'NR==L{print $1"\t"$2"\t"$3"\t"$4"\t"$5"\t"$6"\t"$7"\t"$20}' "$f")
  local want=$'gov_edu\thttps://usa.gov/\t'"$st"$'\t'"$cov"$'\t'"$sh"$'\t'"$vis"$'\t'"$paths"$'\t'"$PIN"
  if [ "$got" = "$want" ]; then ok "$run:$line usa.gov $st shape=$sh paths=$paths chrome=$PIN"; return 0
  else bad "$run:$line file has [$got] want [$want]"; return 1; fi
}

# ── fresh runs: five distinct files, each row matches its line, values for the stats ──
runs=$(printf '%s\n' "$fresh" | awk -F'|' '{gsub(/ /,"",$2); print $2}')
uniq_n=$(printf '%s\n' "$runs" | sort -u | grep -c .)
[ "$uniq_n" -eq 5 ] && ok "five distinct run files" || bad "distinct runs=$uniq_n (want 5)"
shapes=""
while IFS='|' read -r _ run line st cov sh vis paths _; do
  run=$(echo "$run" | xargs); line=$(echo "$line" | xargs); st=$(echo "$st" | xargs)
  cov=$(echo "$cov" | xargs); sh=$(echo "$sh" | xargs); vis=$(echo "$vis" | xargs); paths=$(echo "$paths" | xargs)
  check "$run" "$line" "$st" "$cov" "$sh" "$vis" "$paths"
  shapes="$shapes $sh"
done <<< "$fresh"

# recompute min, max, spread, mean over the five fresh SHAPE values
stats=$(printf '%s\n' $shapes | awk 'NF{n++; s+=$1; if(min==""||$1<min)min=$1; if(max==""||$1>max)max=$1}
  END{printf "%.1f %.1f %.1f %.1f %d", min, max, max-min, s/n, n}')
read -r mn mx sp mean n <<< "$stats"
echo "  fresh n=$n min=$mn max=$mx spread=$sp mean=$mean"
[ "$n" -eq 5 ] && [ "$mn" = 82.1 ] && [ "$mx" = 84.0 ] && [ "$sp" = 1.9 ] && [ "$mean" = 82.9 ] \
  && ok "fresh min/max/spread/mean recompute to 82.1/84.0/1.9/82.9" \
  || bad "fresh stats $mn/$mx/$sp/$mean over $n (want 82.1/84.0/1.9/82.9 over 5)"
# the doc must state the same figures in its text
grep -q "min \*\*82.1\*\*, max \*\*84.0\*\*, spread \*\*1.9 points\*\*, mean 82.9" "$MD" \
  && ok "doc states min/max/spread/mean" || bad "doc does not state min/max/spread/mean as recomputed"

# ── all eleven rows: each matches its line; the signature (cov/vis/paths) maps to one SHAPE ──
sigs=""
while IFS='|' read -r _ run line cov sh vis paths _; do
  run=$(echo "$run" | xargs); line=$(echo "$line" | xargs); cov=$(echo "$cov" | xargs)
  sh=$(echo "$sh" | xargs); vis=$(echo "$vis" | xargs); paths=$(echo "$paths" | xargs)
  check "$run" "$line" OK "$cov" "$sh" "$vis" "$paths"
  sigs="$sigs$cov/$vis/$paths=$sh"$'\n'
done <<< "$all"
bad_map=$(printf '%s' "$sigs" | awk -F= 'NF==2{ if(($1 in m) && m[$1]!=$2) print $1" -> "m[$1]" and "$2; m[$1]=$2 }')
nsig=$(printf '%s' "$sigs" | awk -F= 'NF==2{print $1}' | sort -u | grep -c .)
[ -z "$bad_map" ] && [ "$nsig" -eq 3 ] && ok "three signatures, each one SHAPE" || bad "signature map: sigs=$nsig conflicts=[$bad_map] (want 3, none)"

# ── mutants re-run the gate on a broken copy; TDD_CHILD stops the children from recursing ──
if [ -z "${TDD_CHILD:-}" ]; then
  T=$(mktemp -d /tmp/usagov-drift-test.XXXXXX); trap 'rm -rf "$T"' EXIT
  export TDD_CHILD=1
  # mutant: a cited SHAPE altered to a value the file does not hold must be refused
  sed 's/^| 061352 | 2 | OK | 99.7 | 84.0 |/| 061352 | 2 | OK | 99.7 | 99.9 |/' "$MD" > "$T/m1.md"
  "$0" "$T/m1.md" >/dev/null 2>&1; rc=$?
  [ "$rc" -eq 1 ] && ok "mutant altered shape refused (rc=1)" || bad "mutant altered shape rc=$rc (want 1)"
  # mutant: a cited line pointing at a different site line must be refused
  sed 's/^| 061412 | 2 |/| 061412 | 3 |/' "$MD" > "$T/m2.md"
  "$0" "$T/m2.md" >/dev/null 2>&1; rc=$?
  [ "$rc" -eq 1 ] && ok "mutant wrong line refused (rc=1)" || bad "mutant wrong line rc=$rc (want 1)"
  # mutant: the same run cited twice in the five fresh rows must be refused
  sed 's/^| 061431 | 2 |/| 061333 | 2 |/' "$MD" > "$T/m3.md"
  "$0" "$T/m3.md" >/dev/null 2>&1; rc=$?
  [ "$rc" -eq 1 ] && ok "mutant duplicate run refused (rc=1)" || bad "mutant duplicate run rc=$rc (want 1)"
  # mutant: a stated stat that does not recompute must be refused
  sed 's/spread \*\*1.9 points\*\*/spread **2.1 points**/' "$MD" > "$T/m4.md"
  "$0" "$T/m4.md" >/dev/null 2>&1; rc=$?
  [ "$rc" -eq 1 ] && ok "mutant wrong spread refused (rc=1)" || bad "mutant wrong spread rc=$rc (want 1)"
  # mutant: a foreign chrome pin must be refused (every cited row is measured under $MANUK_CHROME_PIN)
  MANUK_CHROME_PIN=144.0.0.0 "$0" "$MD" >/dev/null 2>&1; rc=$?
  [ "$rc" -eq 1 ] && ok "mutant wrong chrome pin refused (rc=1)" || bad "mutant wrong pin rc=$rc (want 1)"
fi

[ "$fail" -eq 0 ] && echo "usagov-drift-test: PASS" || { echo "usagov-drift-test: FAIL"; exit 1; }
