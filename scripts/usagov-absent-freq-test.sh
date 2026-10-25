#!/usr/bin/env bash
# gate for R0.4.a.b.a.a.a.a.a.a.a: the frequency of the small usa.gov oracle DOM. Re-counts, from the stored Chrome captures
# in docs/bench/chrome-freq/ (N runs of usagov-chrome-bytes.py), how many of the 11 paths-diff keys are absent from each
# post-JS DOM, and requires docs/sprint/USAGOV-ATTRIB.md's absent-freq table to equal that recount row for row.
# usage: usagov-absent-freq-test.sh [DIR] [ATTRIB.md]      --print: emit the table rows only (to paste into the doc)
# chrome_ver=$MANUK_CHROME_PIN. A dropped stored DOM, or a DOM with a key node removed, must turn it red.
set -uo pipefail
R=/home/patrickd/manuk; PRINT=0; [ "${1:-}" = "--print" ] && { PRINT=1; shift; }
D=${1:-$R/docs/bench/chrome-freq}; MD=${2:-$R/docs/sprint/USAGOV-ATTRIB.md}; E=34f9e0bd13c46c13; fail=0
ok(){ echo "  ok   $*"; }; bad(){ echo "  FAIL $*"; fail=1; }
touch "$R/.git/sprint-working" 2>/dev/null || true
T=$(mktemp -d /tmp/usagov-absent-freq-test.XXXXXX); trap 'rm -rf "$T"' EXIT
"$R/scripts/usagov-paths-diff.sh" 2>/dev/null | awk -F'`' '/^\| `/{print $2}' > "$T/keys.txt"
[ "$(grep -c . "$T/keys.txt")" -eq 11 ] || { echo "key count != 11"; exit 1; }
# recount: one row per run N (1..) that has a DOM file; a run with a DOM but no bytes file is a failure
table(){ local dir=$1 n=0
  for f in "$dir"/www.usa.gov.chrome-dom.*.html; do n=$((n+1)); i=$(basename "$f" .html); i=${i##*.}
    raw=$(node "$R/scripts/usagov-fetch-has.js" "$f" "$T/keys.txt" 2>/dev/null | grep -c '^RAW')
    bs=$(sha256sum "$dir/www.usa.gov.chrome-bytes.$i.html" 2>/dev/null | cut -c1-12 || true); [ -n "$bs" ] || bs=MISSING
    echo "| $i | $raw | $((11-raw)) | $(sha256sum "$f" | cut -c1-12) | $bs |"
  done; echo "$n"; }
out=$(table "$D"); n=$(echo "$out" | tail -1); rows=$(echo "$out" | sed '$d')
if [ $PRINT -eq 1 ]; then echo "$rows"; exit 0; fi
[ "$n" -eq 20 ] && ok "20 stored DOM captures" || bad "$n stored DOM captures, expected 20"
[ "$(echo "$rows" | grep -c 'MISSING')" -eq 0 ] && ok "every DOM has its bytes capture" || bad "a DOM has no bytes capture"
bo=0; for f in "$D"/www.usa.gov.chrome-bytes.*.html; do [ "$(sha256sum "$f" | cut -c1-16)" = "${E}" ] && bo=$((bo+1)); done
[ "$bo" -eq "$n" ] && ok "all $bo Chrome-received byte files == engine-fetch sha $E" || bad "$bo of $n byte files match engine-fetch sha"
want=$(echo "$rows" | awk -F'|' '{s+=$4} END{print s+0}')
doc=$(sed -n '/<!-- absent-freq:begin/,/<!-- absent-freq:end/p' "$MD" | grep '^| [0-9]')
[ "$(echo "$doc" | grep -c .)" -eq "$n" ] && ok "doc table has $n rows" || bad "doc table rows != $n"
[ "$(echo "$rows" | sed 's/ *$//')" = "$(echo "$doc" | sed 's/ *$//')" ] && ok "doc table == recount row for row" || bad "doc table differs from the recount"
grep -q "absent: ${want}/$((n*11)) key-slots over $n runs" "$MD" && ok "doc summary matches recount ($want absent key-slots)" || bad "doc summary does not state the recount ($want absent key-slots over $n runs)"
grep -q "chrome_ver=${MANUK_CHROME_PIN:-145.0.7632.116}" "$MD" && ok "chrome_ver stated" || bad "chrome_ver missing in doc"
if [ -z "${TDD_CHILD:-}" ]; then export TDD_CHILD=1
  # mutant: one stored DOM dropped -> run count != 20 -> must go red
  rm -rf "$T/m1"; mkdir -p "$T/m1"; cp "$D"/* "$T/m1/"; rm "$T/m1/www.usa.gov.chrome-dom.7.html"
  "$0" "$T/m1" "$MD" >/dev/null 2>&1 && bad "mutant (dropped DOM) still green" || ok "mutant (dropped DOM) red"
  # mutant: a DOM with the address nodes renamed (keys absent) -> the recount moves, doc table no longer matches -> red
  rm -rf "$T/m2"; mkdir -p "$T/m2"; cp "$D"/* "$T/m2/"; sed -i 's#<address#<addr-x#g' "$T/m2/www.usa.gov.chrome-dom.7.html"
  "$0" "$T/m2" "$MD" >/dev/null 2>&1 && bad "mutant (key node removed) still green" || ok "mutant (key node removed) red"
fi
[ $fail -eq 0 ] && echo "usagov-absent-freq-test: PASS" || { echo "usagov-absent-freq-test: FAIL"; exit 1; }
