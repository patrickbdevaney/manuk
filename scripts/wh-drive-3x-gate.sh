#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a.a.b.b.a.a.aa.a.b: three release drive-probe runs of whitehouse.gov
# must all return rc=0 and each carry a parsed drive rate, all under the pinned chrome_ver.
# usage: wh-drive-3x-gate.sh [TSV]      (default docs/sprint/WH-DRIVE-3x-20261103.tsv)
# mutation: a row with rc=124 or an empty rate makes it red.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
F="${1:-docs/sprint/WH-DRIVE-3x-20261103.tsv}"
[ -f "$F" ] || { echo "red: missing $F" >&2; exit 1; }
rows=$(awk -F'\t' 'NR>1' "$F" | wc -l)
[ "$rows" -eq 3 ] || { echo "red: $rows rows, want 3" >&2; exit 1; }
bad=$(awk -F'\t' 'NR>1 && ($4!="0" || $8 !~ /[0-9.]+%/ || $7!=ENVIRON["MANUK_CHROME_PIN"]) {n++} END{print n+0}' "$F")
[ "$bad" -eq 0 ] || { echo "red: $bad row(s) with rc!=0, no rate, or chrome_ver != ${MANUK_CHROME_PIN:-unset}" >&2; exit 1; }
echo "green: 3/3 rc=0 with rates, chrome_ver=${MANUK_CHROME_PIN:-unset}" >&2
