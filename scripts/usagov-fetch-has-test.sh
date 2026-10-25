#!/usr/bin/env bash
# gate for R0.4.a.b.a.a.a.a.a: the RAW/JS_ONLY table in docs/sprint/USAGOV-ATTRIB.md must equal usagov-fetch-has.sh's output
# for the 11 paths-diff keys, the classifier must go red when a key is dropped, and it must call an element that a script
# writes (fixture) JS_ONLY, which proves the no-JS DOM is what it reads. chrome_ver is stated in the doc.
# usage: usagov-fetch-has-test.sh [ATTRIB.md]
set -uo pipefail
R=/home/patrickd/manuk; MD=${1:-$R/docs/sprint/USAGOV-ATTRIB.md}; fail=0
ok(){ echo "  ok   $*"; }; bad(){ echo "  FAIL $*"; fail=1; }
touch "$R/.git/sprint-working" 2>/dev/null || true
T=$(mktemp -d /tmp/usagov-fetch-has-test.XXXXXX); trap 'rm -rf "$T"' EXIT
# the table in the doc, as "class<TAB>key" pairs
got=$(awk '/raw-has:begin/{s=1;next} /raw-has:end/{s=0} s && /^\| `/' "$MD" | awk -F'|' '{gsub(/^ +| +$/,"",$3); gsub(/`/,"",$2); gsub(/^ +| +$/,"",$2); printf "%s\t%s\n", $3, $2}')
want=$("$R/scripts/usagov-fetch-has.sh" 2>/dev/null)
[ "$(printf '%s\n' "$want" | grep -c .)" -eq 11 ] && ok "classifier reports 11 keys" || bad "classifier key count != 11"
[ "$got" = "$want" ] && ok "doc table equals the classifier output" || bad "doc table differs from the classifier output"
grep -q 'chrome_ver=145.0.7632.116' "$MD" && ok "chrome_ver stated" || bad "chrome_ver missing"
grep -q "$(sha256sum "$R/docs/bench/engine-fetch/www.usa.gov.engine-fetch.html" | cut -c1-64)" "$MD" && ok "stored fetch sha stated" || bad "stored fetch sha not in doc"
if [ -z "${TDD_CHILD:-}" ]; then export TDD_CHILD=1
  # mutant 1: the key list loses one key -> the classifier no longer matches the 11-row doc table
  "$R/scripts/usagov-paths-diff.sh" 2>/dev/null | awk -F'`' '/^\| `/{print $2}' | sed '$d' > "$T/keys-minus1.txt"
  m1=$("$R/scripts/usagov-fetch-has.sh" "" "$T/keys-minus1.txt" 2>/dev/null)
  [ "$m1" = "$want" ] && bad "mutant dropped key still matched" || ok "mutant dropped key refused"
  # mutant 2: a fixture whose element a script writes -> must be JS_ONLY (red if the classifier reads the JS-enabled DOM)
  cat > "$T/fix.html" <<'HTML'
<!DOCTYPE html><html><head><title>f</title></head><body><div><p>static</p></div>
<script>var d=document.createElement('div');d.id='x';document.body.appendChild(d);</script>
</body></html>
HTML
  printf 'body:nth-of-type(1)/div:nth-of-type(1)\nbody:nth-of-type(1)/div:nth-of-type(2)\n' > "$T/keys-fix.txt"
  f=$("$R/scripts/usagov-fetch-has.sh" "$T/fix.html" "$T/keys-fix.txt" 2>/dev/null)
  printf '%s\n' "$f" | grep -qP '^RAW\tbody:nth-of-type\(1\)/div:nth-of-type\(1\)$' && ok "static element RAW" || bad "static element not RAW: $f"
  printf '%s\n' "$f" | grep -qP '^JS_ONLY\tbody:nth-of-type\(1\)/div:nth-of-type\(2\)$' && ok "script-written element JS_ONLY" || bad "script-written element not JS_ONLY: $f"
fi
[ $fail -eq 0 ] && echo "usagov-fetch-has-test: PASS" || { echo "usagov-fetch-has-test: FAIL"; exit 1; }
