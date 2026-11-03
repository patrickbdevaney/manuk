#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a.a.b.b.a.a.a: the whitehouse.gov release stall is the page's own /live.json handler.
# Runs drive-probe twice: MANUK_DELIVER_STUB=<needle> empties the delivered body whose text contains <needle>.
#   stubbed   (needle icuRZ2LXk1E) must finish well before the 30 s trace with no SPINNER_SUMMARY
#   unstubbed must show SPINNER_SUMMARY naming the deliver eval (event_loop_deliver.js)
# usage: wh-live-stub-gate.sh [out.tsv]     NEEDLE=zzz makes the stubbed arm a no-op (mutant: must go red)
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
OUT="${1:-/dev/stdout}"; PIN="${MANUK_CHROME_PIN:?MANUK_CHROME_PIN not set}"
DP="${DP:-target/release/drive-probe}"; NEEDLE="${NEEDLE:-icuRZ2LXk1E}"
run() { local s=$(date +%s); local l; l=$( (ulimit -v 16000000; MANUK_DELIVER_STUB="$1" MANUK_TASK_TRACE=30 timeout 45 "$DP" https://whitehouse.gov/ 2>&1 >/dev/null) | grep -a SPINNER_SUMMARY | head -1); echo "$(( $(date +%s)-s ))	$([ -n "$l" ] && echo spinner || echo clean)	$(sed -n 's/.*stage="\(event_loop_[a-z_.]*\).*/\1/p' <<<"$l")"; }
IFS=$'\t' read -r ts tr _ <<<"$(run "$NEEDLE")"
IFS=$'\t' read -r us ur ustage <<<"$(run "")"
printf 'arm\twall_s\tresult\tstage\tchrome_ver\nstubbed\t%s\t%s\t\t%s\nunstubbed\t%s\t%s\t%s\t%s\n' "$ts" "$tr" "$PIN" "$us" "$ur" "$ustage" "$PIN" > "$OUT"
[ "$tr" = clean ] && [ "$ts" -lt 25 ] || { echo "red: stubbed arm ${tr} ${ts}s" >&2; exit 1; }
[ "$ur" = spinner ] || { echo "red: unstubbed arm did not stall" >&2; exit 1; }
echo "green: stubbed ${ts}s clean; unstubbed stalls in $ustage" >&2
