#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a.a.a.a.b.a.a.a.a.a: one salesforce.com drive-probe run with the
# navigation ledger on, named phases summed against wall_s.
#   usage: sf-phase-sum.sh <out.tsv> [binary=debug] [log-to-parse]   (a 3rd arg re-parses a kept log, no run)
# Phases come from three ledgers (all `RUST_LOG=manuk_page=info,manuk_agent=info`):
#   agent:   "agent phase done: fetch html + prewarm js", "agent phase done: service websockets"
#   load_async: `load phase done` lines up to and incl. "load event" (reconciled: total_ms)
#   finish_loading: "finish_loading cancelled in phase" (budget-cancelled) or its own `load phase done` lines
# Red (exit 1) when the sum of named phases < 90% of wall_s, or the largest phase is unknown.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
touch .git/sprint-working
OUT="$1"; BIN="${2:-debug}"; LOG="${3:-}"
PIN="${MANUK_CHROME_PIN:?MANUK_CHROME_PIN not set}"
URL="https://salesforce.com/"
ERR="${OUT%.tsv}.err"
if [ -z "$LOG" ]; then
  t0=$(date +%s.%N)
  RUST_LOG=manuk_page=info,manuk_agent=info MANUK_PHASE_TIMING=1 nice -n 15 timeout 900 "target/$BIN/drive-probe" "$URL" >"${OUT%.tsv}.out" 2>"$ERR"
  rc=$?
  t1=$(date +%s.%N)
  wall=$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.1f", b-a}')
else
  ERR="$LOG"; rc=NA
  wall=$(sed -n 's/.*PHASE navigate-end t=\([0-9.]*\)s.*/\1/p' "$ERR" | tail -1)
fi
sed 's/\x1b\[[0-9;]*m//g' "$ERR" | awk -v url="$URL" -v bin="$BIN" -v rc="$rc" -v wall="$wall" -v pin="$PIN" '
function num(re,   m){ if (match($0, re)) { m=substr($0,RSTART,RLENGTH); sub(/^[^=]*=/,"",m); return m+0 } return -1 }
/agent phase done: fetch html/ { add("fetch html + prewarm js", num("ms=[0-9]+")) }
/agent phase done: service websockets/ { add("service websockets", num("ms=[0-9]+")) }
/load phase done/ { n=$0; sub(/.*phase="/,"",n); sub(/".*/,"",n); ms=num("ms=[0-9]+"); if (seen_load) add("finish_loading: " n, ms); else add("load_async: " n, ms); if (n=="load event") seen_load=1 }
/finish_loading cancelled in phase/ { n=$0; sub(/.*phase="/,"",n); sub(/".*/,"",n); add("finish_loading CANCELLED in: " n, num(" ms=[0-9]+")) }
function add(k,v){ names[++c]=k; vals[c]=v; sum+=v }
END{
  big=0; for(i=1;i<=c;i++){ if(vals[i]>vals[big]||big==0) big=i; printf "phase\t%s\t%.1f\n", names[i], vals[i]/1000 > "/dev/stderr" }
  pct = (wall>0)? 100*(sum/1000)/wall : 0
  unk = (c==0 || names[big] ~ /unknown/)
  printf "url\tbinary\trc\twall_s\tnamed_sum_s\tpct_named\tlargest_phase\tlargest_s\tchrome_ver\n"
  printf "%s\t%s\t%s\t%s\t%.1f\t%.1f\t%s\t%.1f\t%s\n", url, bin, rc, wall, sum/1000, pct, names[big], vals[big]/1000, pin
  exit (pct<90 || unk) ? 1 : 0
}' >"$OUT" 2>"${OUT%.tsv}.phases.tsv"
st=$?
cat "${OUT%.tsv}.phases.tsv" "$OUT"
[ $st -eq 0 ] && echo GREEN || echo RED
exit $st
