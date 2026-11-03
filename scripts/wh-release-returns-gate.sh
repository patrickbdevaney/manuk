#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a.a.b.b.a.a.aa: whitehouse.gov release drive-probe must return (<30 s, rc 0).
# usage: wh-release-returns-gate.sh      DP=/bin/false (or any stalling binary) makes it red
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
DP="${DP:-target/release/drive-probe}"
s=$(date +%s); (ulimit -v 16000000; timeout 30 "$DP" https://whitehouse.gov/ >/dev/null 2>&1); rc=$?
w=$(( $(date +%s)-s ))
[ "$rc" = 0 ] && [ "$w" -lt 30 ] || { echo "red: rc=$rc ${w}s" >&2; exit 1; }
echo "green: returned rc=0 in ${w}s (chrome_ver=${MANUK_CHROME_PIN:-unset})" >&2
