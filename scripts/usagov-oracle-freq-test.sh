#!/usr/bin/env bash
# gate for R0.4.a.b.a.a.a.a.a.a.a.a: the frequency of the small usa.gov oracle DOM INSIDE the tri-sweep oracle. Recounts, from the
# stored sidecars + oracle path files in docs/bench/oracle-freq/ (N>=10 fresh tri-sweep runs of usa.gov alone), how many runs hit
# oracle sha a6a6db2122b8 and how many of the 11 paths-diff keys each run's oracle file holds; docs/sprint/USAGOV-ATTRIB.md must match.
# usage: usagov-oracle-freq-test.sh [DIR] [ATTRIB.md]      --print: emit the table rows only
# chrome_ver=$MANUK_CHROME_PIN. A dropped sidecar, or an altered count, must turn it red.
set -uo pipefail
R=/home/patrickd/manuk; PRINT=0; [ "${1:-}" = "--print" ] && { PRINT=1; shift; }
D=${1:-$R/docs/bench/oracle-freq}; MD=${2:-$R/docs/sprint/USAGOV-ATTRIB.md}; fail=0; PIN=${MANUK_CHROME_PIN:-145.0.7632.116}
ok(){ echo "  ok   $*"; }; bad(){ echo "  FAIL $*"; fail=1; }
touch "$R/.git/sprint-working" 2>/dev/null || true
T=$(mktemp -d /tmp/usagov-oracle-freq-test.XXXXXX); trap 'rm -rf "$T"' EXIT
"$R/scripts/usagov-paths-diff.sh" 2>/dev/null | awk -F'`' '/^\| `/{print $2}' > "$T/keys.txt"
[ "$(grep -c . "$T/keys.txt")" -eq 11 ] || { echo "key count != 11"; exit 1; }
rows=""; n=0; small=0; badver=0
for a in "$D"/tri-sweep-*.attrib.tsv; do
  [ -e "$a" ] || continue; n=$((n+1)); st=$(basename "$a" .attrib.tsv); st=${st#tri-sweep-}
  line=$(grep -F 'usa.gov' "$a" | head -1); osha=$(echo "$line" | cut -f2 | cut -c1-12); cv=$(echo "$line" | cut -f5)
  [ "$cv" = "$PIN" ] || badver=$((badver+1))
  of=$(ls "$D"/oracle-paths/tri-sweep-$st.*.oracle-paths.tsv 2>/dev/null | head -1)
  if [ -n "$of" ]; then [ "$(sha256sum "$of" | cut -c1-12)" = "$osha" ] || osha="$osha!MISMATCH"
    k=$(awk -F'\t' 'FNR==NR{s[$1]=1;next} $1 in s{n++} END{print n+0}' "$T/keys.txt" "$of"); else k=NOFILE; fi
  [ "${osha%%!*}" = a6a6db2122b8 ] && small=$((small+1))
  rows="$rows| $st | $osha | $k |"$'\n'
done
rows=$(printf '%s' "$rows")
if [ $PRINT -eq 1 ]; then echo "$rows"; exit 0; fi
[ "$n" -ge 10 ] && ok "$n fresh sweep sidecars (>=10)" || bad "$n sidecars, need >=10"
[ "$badver" -eq 0 ] && ok "every sidecar chrome_ver=$PIN" || bad "$badver sidecars with another chrome_ver"
echo "$rows" | grep -qE 'NOFILE|MISMATCH' && bad "a run has no/mismatching oracle path file" || ok "every run has a matching oracle path file"
doc=$(sed -n '/<!-- oracle-freq:begin/,/<!-- oracle-freq:end/p' "$MD" | grep '^| [0-9]')
[ "$(echo "$rows" | sed 's/ *$//')" = "$(echo "$doc" | sed 's/ *$//')" ] && ok "doc table == recount row for row" || bad "doc table differs from the recount"
grep -q "small DOM (a6a6db2122b8) in ${small}/${n} fresh tri-sweep runs" "$MD" && ok "doc summary matches recount ($small/$n)" || bad "doc summary does not state ${small}/${n}"
grep -q "chrome_ver=$PIN" "$MD" && ok "chrome_ver stated" || bad "chrome_ver missing in doc"
if [ -z "${TDD_CHILD:-}" ]; then export TDD_CHILD=1
  rm -rf "$T/m1"; cp -r "$D" "$T/m1"; rm "$(ls "$T"/m1/tri-sweep-*.attrib.tsv | head -1)"
  "$0" "$T/m1" "$MD" >/dev/null 2>&1 && bad "mutant (dropped sidecar) still green" || ok "mutant (dropped sidecar) red"
  rm -rf "$T/m2"; cp -r "$D" "$T/m2"; f=$(ls "$T"/m2/tri-sweep-*.attrib.tsv | head -1); sed -i 's/2286d97a171496f1c2ad9f2aa59f8b42cacbec113f9df2bead8413c46e0043e5/a6a6db2122b80ea114db806fb04b0196288f8e4db22fb6b7aef1acb2adf80063/' "$f"
  "$0" "$T/m2" "$MD" >/dev/null 2>&1 && bad "mutant (altered sha) still green" || ok "mutant (altered sha) red"
fi
[ $fail -eq 0 ] && echo "usagov-oracle-freq-test: PASS" || { echo "usagov-oracle-freq-test: FAIL"; exit 1; }
