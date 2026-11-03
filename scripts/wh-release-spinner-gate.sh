#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a.a.b.b.a.a: name the step the JS thread is stuck in at t=30 s of release
# drive-probe on whitehouse.gov. MANUK_TASK_TRACE=30 starts an in-process watchdog (engine/js event_loop.rs
# trace_stage/trace_peek) that prints one SPINNER_SUMMARY line naming the last eval/microtask entered.
# Green: the line exists, has a stage="..." naming an event_loop_* eval, and says how long ago it was entered (>=10 s).
#   usage: wh-release-spinner-gate.sh [out.tsv]
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
OUT="${1:-/dev/stdout}"
PIN="${MANUK_CHROME_PIN:?MANUK_CHROME_PIN not set}"
DP="${DP:-target/release/drive-probe}"
[ -x "$DP" ] || { echo "red: missing $DP" >&2; exit 1; }
TRACE="${TRACE:-30}"
line=$( (ulimit -v 16000000; MANUK_TASK_TRACE="$TRACE" timeout 45 "$DP" https://whitehouse.gov/ 2>&1 >/dev/null) | grep -a SPINNER_SUMMARY | head -1)
stage=$(sed -n 's/.*stage="\([^"]*\) entered \([0-9]*\)s ago.*/\1/p' <<<"$line" | cut -c1-200)
age=$(sed -n 's/.*stage="[^"]* entered \([0-9]*\)s ago.*/\1/p' <<<"$line")
printf 'url\ttrace_s\tstage\tstage_age_s\tchrome_ver\n' > "$OUT"
printf '%s\t%s\t%s\t%s\t%s\n' https://whitehouse.gov/ "$TRACE" "$stage" "$age" "$PIN" >> "$OUT"
[ -n "$line" ] || { echo "red: no SPINNER_SUMMARY line" >&2; exit 1; }
case "$stage" in event_loop_*) ;; *) echo "red: stage '$stage' does not name an event_loop_* step" >&2; exit 1;; esac
[ "${age:-0}" -ge 10 ] || { echo "red: stage age '${age}' < 10 s (not the stuck step)" >&2; exit 1; }
echo "green: stuck in '$stage' for ${age}s at t=${TRACE}s" >&2
