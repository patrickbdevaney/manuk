#!/usr/bin/env bash
# gate for R0.4.a.b.a.a.a.a.a.a.a.a.a: what differs at the oracle's capture point between the small (a6a6db2122b8) and large
# (2286d97a1714) usa.gov oracle DOMs. Recounts from the stored oracle-side post-JS DOM dumps in docs/bench/oracle-dom/
# (>=1 small, >=3 large; captured with MANUK_ORACLE_DUMP_DIR): data-contact-callout="hidden" count per DOM, the 11 paths-diff
# keys present in that DOM's path file, and that the stored page JS holds the Math.random()<0.8 hide branch.
# usage: usagov-oracle-capture-test.sh [DIR] [ATTRIB.md]      chrome_ver=$MANUK_CHROME_PIN
set -uo pipefail
R=/home/patrickd/manuk; D=${1:-$R/docs/bench/oracle-dom}; MD=${2:-$R/docs/sprint/USAGOV-ATTRIB.md}; fail=0; PIN=${MANUK_CHROME_PIN:-145.0.7632.116}
ok(){ echo "  ok   $*"; }; bad(){ echo "  FAIL $*"; fail=1; }
touch "$R/.git/sprint-working" 2>/dev/null || true
T=$(mktemp -d /tmp/usagov-oracle-capture-test.XXXXXX); trap 'rm -rf "$T"' EXIT
"$R/scripts/usagov-paths-diff.sh" 2>/dev/null | awk -F'`' '/^\| `/{print $2}' > "$T/keys.txt"
[ "$(grep -c . "$T/keys.txt")" -eq 11 ] || { echo "key count != 11"; exit 1; }
rows=""; small=0; large=0
for f in "$D"/oracle-dom.r*.html; do [ -e "$f" ] || continue; r=${f##*oracle-dom.}; r=${r%.html}
  h=$(grep -o 'data-contact-callout="hidden"' "$f" | wc -l); v=$(grep -o 'data-contact-callout="visible"' "$f" | wc -l)
  p="$D/oracle-paths.$r.tsv"; [ -e "$p" ] || { rows="$rows| $r | NOPATHS |"$'\n'; continue; }
  k=$(awk -F'\t' 'FNR==NR{s[$1]=1;next} $1 in s{n++} END{print n+0}' "$T/keys.txt" "$p")
  sh=$(sha256sum "$p" | cut -c1-12); [ "$sh" = a6a6db2122b8 ] && small=$((small+1)); [ "$sh" = 2286d97a1714 ] && large=$((large+1))
  rows="$rows| $r | $sh | $h | $v | $k |"$'\n'
done
rows=$(printf '%s' "$rows")
[ "$small" -ge 1 ] && ok "$small small DOM(s) (>=1)" || bad "$small small DOMs, need >=1"
[ "$large" -ge 3 ] && ok "$large large DOM(s) (>=3)" || bad "$large large DOMs, need >=3"
echo "$rows" | grep -q NOPATHS && bad "a DOM has no path file" || ok "every DOM has its path file"
# the rule under test: small <=> 3 hidden callouts and 0/11 keys; large <=> 0 hidden, 3 visible, 11/11 keys
bd=$(echo "$rows" | awk -F'|' '{gsub(/ /,"",$3);gsub(/ /,"",$4);gsub(/ /,"",$5);gsub(/ /,"",$6)}
  ($3=="a6a6db2122b8" && !($4==3 && $5==0 && $6==0)) || ($3=="2286d97a1714" && !($4==0 && $5==3 && $6==11)) {n++} END{print n+0}')
[ "$bd" -eq 0 ] && ok "small = 3 hidden callouts + 0/11 keys; large = 3 visible + 11/11 keys" || bad "$bd DOM(s) break the hidden-callout rule"
doc=$(sed -n '/<!-- oracle-capture:begin/,/<!-- oracle-capture:end/p' "$MD" | grep '^| r')
[ "$(echo "$rows" | sed 's/ *$//')" = "$(echo "$doc" | sed 's/ *$//')" ] && ok "doc table == recount row for row" || bad "doc table differs from the recount"
grep -q 'shouldDisplay(){const randomNumber=Math.random();return randomNumber<0.8;}' "$D/callout-js-excerpt.txt" && grep -q 'injectCSS' "$D/callout-js-excerpt.txt" && ok "stored page JS has the random hide branch" || bad "stored JS lacks the Math.random()<0.8 branch"
grep -q "hides all three \`data-contact-callout\`" "$MD" && ok "doc names the capture-time difference" || bad "doc does not name the capture-time difference"
grep -q "chrome_ver=$PIN" "$MD" && ok "chrome_ver stated" || bad "chrome_ver missing"
if [ -z "${TDD_CHILD:-}" ]; then export TDD_CHILD=1
  rm -rf "$T/m1"; cp -r "$D" "$T/m1"; rm "$T/m1/oracle-dom.r20.html"
  "$0" "$T/m1" "$MD" >/dev/null 2>&1 && bad "mutant (dropped small dump) still green" || ok "mutant (dropped small dump) red"
  rm -rf "$T/m2"; cp -r "$D" "$T/m2"; sed -i 's/data-contact-callout="hidden"/data-contact-callout="visible"/' "$T/m2/oracle-dom.r26.html"
  "$0" "$T/m2" "$MD" >/dev/null 2>&1 && bad "mutant (edited dump) still green" || ok "mutant (edited dump) red"
  rm -rf "$T/m3"; mkdir "$T/m3"; sed 's/oracle-capture:begin/oracle-capture:xx/' "$MD" > "$T/m3/a.md"
  "$0" "$D" "$T/m3/a.md" >/dev/null 2>&1 && bad "mutant (doc table removed) still green" || ok "mutant (doc table removed) red"
fi
[ $fail -eq 0 ] && echo "usagov-oracle-capture-test: PASS" || { echo "usagov-oracle-capture-test: FAIL"; exit 1; }
