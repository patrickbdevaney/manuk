#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a.a.a.a.b.a.a.a.a.a.a: per-caller full-restyle accounting for salesforce.com.
#   usage: sf-restyle-callers.sh <out.tsv> [binary=debug] [log-to-parse]   (3rd arg re-parses a kept log, no run)
# Every "SLOW RESTYLE+LAYOUT" line (carries caller=file:line via #[track_caller]) is placed in the load phase whose
# [end-ms, end] window contains its timestamp (phase end = its "load phase done"/"cancelled" line).
# Red when: any restyle in the four phases (deferred scripts, initial images+masks, load event, external CSS) has no caller,
# or restyle (cascade+layout+container-query) seconds in those phases < 80% of the phases' total seconds.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
touch .git/sprint-working
OUT="$1"; BIN="${2:-debug}"; LOG="${3:-}"
PIN="${MANUK_CHROME_PIN:?MANUK_CHROME_PIN not set}"
ERR="${OUT%.tsv}.err"
if [ -z "$LOG" ]; then
  RUST_LOG=manuk_page=info,manuk_agent=info MANUK_PHASE_TIMING=1 nice -n 15 timeout 900 "target/$BIN/drive-probe" https://salesforce.com/ >"${OUT%.tsv}.out" 2>"$ERR"
else ERR="$LOG"; fi
sed 's/\x1b\[[0-9;]*m//g' "$ERR" | awk -v pin="$PIN" -v out="$OUT" '
function ts(l,  t,a){ split(substr(l,1,26),a,/[-T:Z]/); return ((a[3]*24+a[4])*60+a[5])*60+a[6] }  # seconds; day wrap ok within a run
function pm(l,  m){ match(l,/ ms=[0-9]+/); return substr(l,RSTART+4,RLENGTH-4)/1000 }
/SLOW RESTYLE\+LAYOUT/ { r++; rt[r]=ts($0); c=$0; sub(/.*cascade_ms=/,"",c); sub(/ .*/,"",c); l=$0; sub(/.*layout_ms=/,"",l); sub(/ .*/,"",l); q=$0; sub(/.*container_query_ms=/,"",q); sub(/ .*/,"",q); rs[r]=(c+l+q)/1000
  cl=$0; if (match(cl,/caller=[^ ]+/)) rc[r]=substr(cl,RSTART+7,RLENGTH-7); else rc[r]="" }
/load phase done|finish_loading cancelled in phase/ { n=$0; sub(/.*phase="/,"",n); sub(/".*/,"",n); p++; pn[p]=n; pe[p]=ts($0); pd[p]=pm($0) }
END{
  split("deferred scripts|initial images+masks|load event|external CSS",W,"|"); tot=0; att=0; bad=0
  printf "phase\trestyles\trestyle_s\tphase_s\tcallers\tchrome_ver\n" > out
  for(w=1;w<=4;w++){ cnt=0; s=0; ph=0; cl=""
    for(i=1;i<=p;i++) if(pn[i]==W[w]){ ph=pd[i]; for(j=1;j<=r;j++) if(rt[j]<=pe[i]+0.05 && rt[j]>pe[i]-ph+0.05){ cnt++; s+=rs[j]; if(rc[j]=="") bad++; cl=cl rc[j] "; " } }
    printf "%s\t%d\t%.1f\t%.1f\t%s\t%s\n", W[w], cnt, s, ph, cl, pin >> out; tot+=ph; att+=s }
  printf "TOTAL\t%d\t%.1f\t%.1f\tunattributed=%d\t%s\n", r, att, tot, bad, pin >> out
  pct = tot>0 ? 100*att/tot : 0; printf "attributed %.1f%% of %.1fs\n", pct, tot > "/dev/stderr"
  exit (bad>0 || pct<80 || r==0) ? 1 : 0 }'
st=$?; cat "$OUT"; [ $st -eq 0 ] && echo GREEN || echo RED; exit $st
