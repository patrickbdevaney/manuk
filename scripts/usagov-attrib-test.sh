#!/usr/bin/env bash
# ── gate for docs/sprint/USAGOV-ATTRIB.md: each cited run must match its TSV line and its .attrib.tsv sidecar (full sha
# prefix, chrome pin), same oracle sha <-> same (cov,vis,paths,shape), >=2 distinct oracle shas. Mutants: altered sha,
# dropped sidecar, one sha for all, wrong pin.
# usage: usagov-attrib-test.sh [ATTRIB.md]
set -uo pipefail
R=/home/patrickd/manuk; MD=${1:-$R/docs/sprint/USAGOV-ATTRIB.md}; B=${BENCH:-$R/docs/bench}
PIN=${MANUK_CHROME_PIN:-145.0.7632.116}; fail=0
ok(){ echo "  ok   $*"; }; bad(){ echo "  FAIL $*"; fail=1; }
rows=$(awk '/^\| [0-9]{6} \|/' "$MD"); n=$(printf '%s\n' "$rows"|grep -c .)
[ "$n" -ge 5 ] && ok "$n runs cited" || bad "runs=$n (want >=5)"
sigs=""; oshas=""
while IFS='|' read -r _ run st cov sh vis paths os es _; do
  run=$(echo $run); st=$(echo $st); cov=$(echo $cov); sh=$(echo $sh); vis=$(echo $vis); paths=$(echo $paths); os=$(echo $os); es=$(echo $es)
  f=$B/tri-sweep-20261025-$run.tsv; a=$B/tri-sweep-20261025-$run.attrib.tsv
  [ -f "$f" ] && [ -f "$a" ] || { bad "$run: missing TSV or attrib sidecar"; continue; }
  got=$(awk -F'\t' 'NR==2{print $2"\t"$3"\t"$4"\t"$5"\t"$6"\t"$7"\t"$20}' "$f")
  want=$'https://usa.gov/\t'"$st"$'\t'"$cov"$'\t'"$sh"$'\t'"$vis"$'\t'"$paths"$'\t'"$PIN"
  [ "$got" = "$want" ] && ok "$run row matches TSV" || bad "$run TSV [$got] want [$want]"
  ag=$(awk -F'\t' 'NR==2{print substr($2,1,12)"\t"substr($3,1,12)"\t"$4"\t"$5; if(length($2)!=64||length($3)!=64)print "SHORT"}' "$a")
  aw=$os$'\t'"$es"$'\t'"$paths"$'\t'"$PIN"
  [ "$ag" = "$aw" ] && ok "$run sidecar sha/paths/pin match" || bad "$run sidecar [$ag] want [$aw]"
  sigs="$sigs$os=$cov/$vis/$paths/$sh"$'\n'; oshas="$oshas$os"$'\n'
done <<< "$rows"
conf=$(printf '%s' "$sigs"|awk -F= 'NF==2{if(($1 in m)&&m[$1]!=$2)print $1; m[$1]=$2}')
[ -z "$conf" ] && ok "one signature per oracle sha" || bad "oracle sha with two signatures: $conf"
d=$(printf '%s' "$oshas"|sort -u|grep -c .)
[ "$d" -ge 2 ] && ok "$d distinct oracle shas" || bad "distinct oracle shas=$d (want >=2)"
if [ -z "${TDD_CHILD:-}" ]; then
  T=$(mktemp -d /tmp/usagov-attrib.XXXXXX); trap 'rm -rf "$T"' EXIT; export TDD_CHILD=1
  sed 's/a6a6db2122b8/2286d97a1714/' "$MD" > "$T/m1.md"; "$0" "$T/m1.md" >/dev/null 2>&1; rc=$?
  [ $rc -eq 1 ] && ok "mutant one-sha-for-all refused" || bad "mutant one-sha rc=$rc"
  sed 's/34f9e0bd13c4 |$/aaaaaaaaaaaa |/;' "$MD" | sed '0,/aaaaaaaaaaaa/s//34f9e0bd13c4/' > "$T/m2.md"; "$0" "$T/m2.md" >/dev/null 2>&1; rc=$?
  [ $rc -eq 1 ] && ok "mutant altered fetch sha refused" || bad "mutant fetch sha rc=$rc"
  mkdir "$T/b"; cp "$B"/tri-sweep-20261025-06*.tsv "$T/b/"; rm -f "$T/b"/tri-sweep-20261025-062051.attrib.tsv
  BENCH="$T/b" "$0" "$MD" >/dev/null 2>&1; rc=$?
  [ $rc -eq 1 ] && ok "mutant dropped sidecar refused" || bad "mutant dropped sidecar rc=$rc"
  sed 's/| 84.0 | 29.0 | 319 | 2286d97a1714/| 83.0 | 29.0 | 319 | 2286d97a1714/' "$MD" > "$T/m4.md"; "$0" "$T/m4.md" >/dev/null 2>&1; rc=$?
  [ $rc -eq 1 ] && ok "mutant altered shape refused" || bad "mutant shape rc=$rc"
  MANUK_CHROME_PIN=144.0.0.0 "$0" "$MD" >/dev/null 2>&1; rc=$?
  [ $rc -eq 1 ] && ok "mutant wrong pin refused" || bad "mutant pin rc=$rc"
fi
[ $fail -eq 0 ] && echo "usagov-attrib-test: PASS" || { echo "usagov-attrib-test: FAIL"; exit 1; }
