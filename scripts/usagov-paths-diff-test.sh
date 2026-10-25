#!/usr/bin/env bash
# gate for the 11-path table in docs/sprint/USAGOV-ATTRIB.md: it must equal usagov-paths-diff.sh's recomputation from the
# stored oracle files, count 11, and go red on mutants (dropped row, altered rect, stored file with a key removed).
# usage: usagov-paths-diff-test.sh [ATTRIB.md]
set -uo pipefail
R=/home/patrickd/manuk; MD=${1:-$R/docs/sprint/USAGOV-ATTRIB.md}; D=${PDIR:-$R/docs/bench/oracle-paths}; fail=0
ok(){ echo "  ok   $*"; }; bad(){ echo "  FAIL $*"; fail=1; }
got=$(awk '/paths-diff:begin/{s=1;next} /paths-diff:end/{s=0} s && /^\| `/' "$MD")
want=$("$R/scripts/usagov-paths-diff.sh" "$D" 2>/dev/null)
[ "$(printf '%s\n' "$want"|grep -c .)" -eq 11 ] && ok "recomputed diff has 11 keys" || bad "recomputed key count != 11"
[ "$got" = "$want" ] && ok "doc table equals recomputation" || bad "doc table differs from recomputation"
grep -q 'chrome_ver=145.0.7632.116' "$MD" && ok "chrome_ver stated" || bad "chrome_ver missing"
if [ -z "${TDD_CHILD:-}" ]; then
  T=$(mktemp -d /tmp/usagov-pd.XXXXXX); trap 'rm -rf "$T"' EXIT; export TDD_CHILD=1
  grep -v 'span:nth-of-type(1)/a:nth-of-type(1)` |' "$MD" > "$T/m1.md"; "$0" "$T/m1.md" >/dev/null 2>&1; [ $? -eq 1 ] && ok "mutant dropped row refused" || bad "mutant dropped row passed"
  sed 's/| 66 | 3395 | 250 | 97 |/| 66 | 3395 | 250 | 98 |/' "$MD" > "$T/m2.md"; "$0" "$T/m2.md" >/dev/null 2>&1; [ $? -eq 1 ] && ok "mutant altered rect refused" || bad "mutant altered rect passed"
  mkdir "$T/d"; cp "$D"/* "$T/d/"; for f in $(cd "$D" && sha256sum *|awk '$1~/^2286d97a1714/{print $2}'); do grep -v 'h4:nth-of-type(1)' "$D/$f" > "$T/d/$f"; done
  PDIR="$T/d" "$0" "$MD" >/dev/null 2>&1; [ $? -eq 1 ] && ok "mutant edited stored file refused" || bad "mutant stored file passed"
fi
[ $fail -eq 0 ] && echo "usagov-paths-diff-test: PASS" || { echo "usagov-paths-diff-test: FAIL"; exit 1; }
