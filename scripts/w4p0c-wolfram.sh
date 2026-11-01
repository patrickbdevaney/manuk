#!/usr/bin/env bash
# W4.P0.c gate: live Wolfram|Alpha result pages (X06/X07/X08 queries) on manuk; the result text must
# appear in >=2 of 3 runs and the no-JS notice must not. EXPECT_X06/X07/X08 override the patterns
# (used to prove the gate goes red). Needs target/release/examples/lean_probe.
set -u
cd "$(dirname "$0")/.."
probe=target/release/examples/lean_probe
declare -A Q=( [X06]='3%5E71' [X07]='integrate%203e%5E(2x)%20from%200%20to%205' [X08]='days%20between%20February%2012%202024%20and%20August%209%202050' )
declare -A E=( [X06]="${EXPECT_X06:-7\.5094665149}" [X07]="${EXPECT_X07:-33038}" [X08]="${EXPECT_X08:-9675 days}" )
fail=0
for t in X06 X07 X08; do
  ok=0
  for r in 1 2 3; do
    touch .git/sprint-working 2>/dev/null
    LEAN_DUMP=/tmp/w4p0c.$$ timeout 120 "$probe" "https://www.wolframalpha.com/input?i=${Q[$t]}" >/dev/null 2>&1
    if grep -qE "${E[$t]}" /tmp/w4p0c.$$ && ! grep -qi 'without javascript' /tmp/w4p0c.$$; then ok=$((ok+1)); fi
  done
  echo "$t: $ok/3 runs show /${E[$t]}/"
  [ "$ok" -ge 2 ] || fail=1
done
rm -f /tmp/w4p0c.$$
[ $fail = 0 ] && echo PASS || echo "FAIL"
exit $fail
