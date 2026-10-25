#!/usr/bin/env bash
# gate for R0.4.a.b.a.a.a.a.a.a: the HTML bytes Chrome itself received for usa.gov (CDP Network.getResponseBody, stored in
# docs/bench/chrome-bytes/) equal the engine-fetch HTML byte for byte in every stored capture, and every one of the 11
# paths-diff keys is in each post-JS Chrome DOM. A swapped/edited file must turn it red. chrome_ver=$MANUK_CHROME_PIN.
# usage: usagov-live-bytes-test.sh [BYTES_DIR]
set -uo pipefail
R=/home/patrickd/manuk; D=${1:-$R/docs/bench/chrome-bytes}; E=$R/docs/bench/engine-fetch/www.usa.gov.engine-fetch.html; fail=0
ok(){ echo "  ok   $*"; }; bad(){ echo "  FAIL $*"; fail=1; }
touch "$R/.git/sprint-working" 2>/dev/null || true
T=$(mktemp -d /tmp/usagov-live-bytes-test.XXXXXX); trap 'rm -rf "$T"' EXIT
"$R/scripts/usagov-paths-diff.sh" 2>/dev/null | awk -F'`' '/^\| `/{print $2}' > "$T/keys.txt"
[ "$(grep -c . "$T/keys.txt")" -eq 11 ] && ok "11 keys" || bad "key count != 11"
es=$(sha256sum "$E" | cut -c1-64); n=0
for f in "$D"/www.usa.gov.chrome-bytes.*.html; do
  n=$((n+1)); b=$(basename "$f" .html); i=${b##*.}
  [ "$(sha256sum "$f" | cut -c1-64)" = "$es" ] && ok "capture $i: Chrome-received bytes == engine-fetch ($es)" || bad "capture $i: Chrome bytes differ from engine-fetch"
  c=$(node "$R/scripts/usagov-fetch-has.js" "$D/www.usa.gov.chrome-dom.$i.html" "$T/keys.txt" 2>/dev/null | grep -c '^RAW')
  [ "$c" -eq 11 ] && ok "capture $i: 11/11 keys in post-JS Chrome DOM" || bad "capture $i: only $c/11 keys in post-JS DOM"
done
[ $n -ge 2 ] && ok "$n captures" || bad "fewer than 2 captures"
grep -q "chrome_ver=${MANUK_CHROME_PIN:-145.0.7632.116}" "$R/docs/sprint/USAGOV-ATTRIB.md" && ok "chrome_ver stated" || bad "chrome_ver missing in doc"
if [ -z "${TDD_CHILD:-}" ]; then export TDD_CHILD=1
  # mutant: a stored Chrome capture with one byte changed (stands in for a swapped file) -> must go red
  mkdir -p "$T/m"; cp "$D"/* "$T/m/"; printf ' ' >> "$T/m/www.usa.gov.chrome-bytes.1.html"
  "$R/scripts/usagov-live-bytes-test.sh" "$T/m" >/dev/null 2>&1 && bad "mutant (edited bytes) still green" || ok "mutant (edited bytes) red"
  # mutant: a DOM with a key node removed -> must go red
  rm -rf "$T/m"; mkdir -p "$T/m"; cp "$D"/* "$T/m/"; sed -i 's#<address#<addr-x#g' "$T/m/www.usa.gov.chrome-dom.2.html"
  "$R/scripts/usagov-live-bytes-test.sh" "$T/m" >/dev/null 2>&1 && bad "mutant (node removed) still green" || ok "mutant (node removed) red"
fi
[ $fail -eq 0 ] && echo "usagov-live-bytes-test: PASS" || { echo "usagov-live-bytes-test: FAIL"; exit 1; }
