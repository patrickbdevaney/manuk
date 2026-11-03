#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a.a.b.b: name the load phase in which release drive-probe on whitehouse.gov exceeds
# the 90 s M2 timeout. Runs target/release/drive-probe with MANUK_PHASE_TIMING=1 and manuk_page=debug, then checks:
#   - it was still running when killed at TMO seconds (rc 124), TMO in [91,200] (the M2 90 s timeout is never raised)
#   - "PHASE navigate-start" seen, "PHASE navigate-end" NOT seen (the hang is inside navigate)
#   - the last "load phase done" is "subframes" and the last page log is "performing page fetches" (page-fetch pump)
#   usage: wh-release-hang-phase.sh [url] [tmo_s] [out.tsv]
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
URL="${1:-https://whitehouse.gov/}"; TMO="${2:-100}"; OUT="${3:-/dev/stdout}"
PIN="${MANUK_CHROME_PIN:?MANUK_CHROME_PIN not set}"
DP="${DP:-target/release/drive-probe}"
[ -x "$DP" ] || { echo "red: missing $DP" >&2; exit 1; }
[ "$TMO" -ge 91 ] && [ "$TMO" -le 200 ] || { echo "red: tmo $TMO outside [91,200]" >&2; exit 1; }
ERR=$(mktemp); trap 'rm -f "$ERR"' EXIT
s=$(date +%s)
MANUK_PHASE_TIMING=1 RUST_LOG=error,manuk_page=debug timeout "$TMO" "$DP" "$URL" >/dev/null 2>"$ERR"; rc=$?
wall=$(( $(date +%s) - s ))
clean=$(sed 's/\x1b\[[0-9;]*m//g' "$ERR")
nstart=$(grep -c 'PHASE navigate-start' <<<"$clean"); nend=$(grep -c 'PHASE navigate-end' <<<"$clean")
lastphase=$(grep -o 'load phase done.*phase="[^"]*"' <<<"$clean" | tail -1 | sed 's/.*phase="//; s/"//')
lastlog=$(grep -E 'manuk_page' <<<"$clean" | tail -1 | grep -oE 'performing page fetches|load phase done|[a-z ]+$' | head -1)
printf 'url\ttmo_s\trc\twall_s\tnavigate_start\tnavigate_end\tlast_load_phase\tlast_page_log\tchrome_ver\n' > "$OUT"
printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$URL" "$TMO" "$rc" "$wall" "$nstart" "$nend" "$lastphase" "$lastlog" "$PIN" >> "$OUT"
ok=1
[ "$rc" = 124 ] || { echo "red: rc=$rc (not a timeout)" >&2; ok=0; }
[ "$wall" -ge 91 ] || { echo "red: wall ${wall}s < 91" >&2; ok=0; }
[ "$nstart" = 1 ] && [ "$nend" = 0 ] || { echo "red: navigate-start=$nstart navigate-end=$nend" >&2; ok=0; }
[ "$lastphase" = subframes ] || { echo "red: last load phase '$lastphase' != subframes" >&2; ok=0; }
[ "$lastlog" = "performing page fetches" ] || { echo "red: last page log '$lastlog'" >&2; ok=0; }
[ "$ok" = 1 ] && echo "green: navigate exceeds ${wall}s inside the page-fetch pump after phase '$lastphase'" >&2
[ "$ok" = 1 ]
