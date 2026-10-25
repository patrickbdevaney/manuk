#!/usr/bin/env bash
# gate for R0.4.a.b.a.a.a.a.a.a.a.a.a.a: usa.gov M1 SHAPE per oracle DOM (large 2286d97a1714 / small a6a6db2122b8), recounted from
# the stored tri-sweep tsv + attrib.tsv pairs, must equal the band table in docs/sprint/USAGOV-ATTRIB.md.
# usage: usagov-shape-band-test.sh [BENCHDIR] [ATTRIB.md]      chrome_ver=$MANUK_CHROME_PIN
set -uo pipefail
R=/home/patrickd/manuk; B=${1:-$R/docs/bench}; MD=${2:-$R/docs/sprint/USAGOV-ATTRIB.md}; fail=0; PIN=${MANUK_CHROME_PIN:-145.0.7632.116}
ok(){ echo "  ok   $*"; }; bad(){ echo "  FAIL $*"; fail=1; }
touch "$R/.git/sprint-working" 2>/dev/null || true
rows=$(for a in "$B"/tri-sweep-*.attrib.tsv "$B"/oracle-freq/*.attrib.tsv; do t=${a%.attrib.tsv}.tsv; [ -e "$t" ] || continue
  s=$(awk -F'\t' -v p="$PIN" '$2=="https://usa.gov/" && $20==p{print $5}' "$t"); sh=$(awk -F'\t' '$1=="https://usa.gov/"{print substr($2,1,12)}' "$a")
  [ -n "$s" ] && [ -n "$sh" ] && echo "$sh $s"; done)
band=$(echo "$rows" | awk '{n[$1]++; if(!($1 in lo)||$2<lo[$1])lo[$1]=$2; if(!($1 in hi)||$2>hi[$1])hi[$1]=$2}
  END{for(k in n) printf "| %s | %d | %s | %s |\n", k, n[k], lo[k], hi[k]}' | sort)
echo "$band"
[ "$(echo "$band" | grep -c .)" -eq 2 ] && ok "two oracle DOMs recounted" || bad "expected 2 oracle DOMs"
doc=$(sed -n '/<!-- shape-band:begin/,/<!-- shape-band:end/p' "$MD" | grep '^| [0-9a-f]\{12\} ')
[ "$band" = "$doc" ] && ok "doc band table == recount" || bad "doc band table differs from recount"
grep -q "chrome_ver=$PIN" "$MD" && ok "chrome_ver stated" || bad "chrome_ver missing"
if [ -z "${TDD_CHILD:-}" ]; then export TDD_CHILD=1; T=$(mktemp -d /tmp/shapeband.XXXXXX); trap 'rm -rf "$T"' EXIT
  sed 's/| 84.0 |$/| 83.0 |/;0,/| 2286d97a1714 |/s//| 2286d97a1714 |/' "$MD" > "$T/a.md"; sed -i '/shape-band:begin/,/shape-band:end/s/| 82.1 | 84.6 |/| 82.1 | 84.0 |/' "$T/a.md"
  "$0" "$B" "$T/a.md" >/dev/null 2>&1 && bad "mutant (altered band) still green" || ok "mutant (altered band) red"
fi
[ $fail -eq 0 ] && echo "usagov-shape-band-test: PASS" || { echo "usagov-shape-band-test: FAIL"; exit 1; }
