#!/usr/bin/env bash
# ── TRI-SWEEP — measure all three exit legs per real site in one pass, honestly.
#   M1  render fidelity   : manuk-wpt fidelity   (structural coverage %, SHAPE %, visual %)
#   M2  drive/addressing  : drive-probe          (rate %, ceiling %)  [no Chrome, per-site parallel]
#   a11y accessibility    : a11y-score           (F1 %, precision, recall)  [own Chrome, per-site serial]
#
# FIXES over old fidelity-sweep.sh: (1) parser matches REBUILT output 'structural: X% (oracle N paths...)' +
# 'SHAPE: X% within Npx' (old grepped dead 'N ids'/'PLACEMENT:'); (2) PER-SITE ISOLATED --out (old shared one
# dir across jobs=3 → concurrent Chrome clobbered → example.com falsely NO_DATA); (3+4) M2 AND a11y run
# PER SITE under a per-site timeout — a hang scores a TAGGED ZERO instead of vanishing (old code ran ~20/10
# sites under ONE chunk timeout, so the first hang wiped the rest: only 11/76 M2 & 10/76 a11y survived, biased
# up). a11y stays SERIAL (a11y-score binds Chrome on 9500+idx; parallel procs collide). Headline prints FULL
# (hang=0) vs MEASURED-only so the sampling bias is visible, not hidden.
#
# usage: tri-sweep.sh [--corpus F] [--limit N] [--jobs J] [--m1-timeout S] [--m2-timeout S] [--a11y-timeout S]
#                     [--m1-from FILE] [--out DIR] [--no-a11y] [--no-m1]
set -uo pipefail
R=/home/patrickd/manuk
CORPUS=$R/docs/bench/oracle-corpus.txt
# M2TMO/AXTMO are GENEROUS on purpose: per-site runs relaunch the fetch (and a11y its own Chrome),
# so a tight limit turns a slow FETCH into a false TIMEOUT=0 (obs. 2026-09-11: 50s starved blog.rust-lang,
# MDN, docs.python — all normally high). Keep them generous; lower --jobs if the box is contended.
LIMIT=0; JOBS=4; M1TMO=30; M2TMO=90; AXTMO=90; A11Y=1; M1=1; M1FROM=""
OUT=/tmp/claude-1000/-home-patrickd-manuk/3538dee1-05ec-426b-a0b7-1512fbafcc55/scratchpad/trisweep
while [ $# -gt 0 ]; do case "$1" in
  --corpus) CORPUS="$2"; shift 2;; --limit) LIMIT="$2"; shift 2;; --jobs) JOBS="$2"; shift 2;;
  --m1-timeout) M1TMO="$2"; shift 2;; --m2-timeout) M2TMO="$2"; shift 2;; --a11y-timeout) AXTMO="$2"; shift 2;;
  --m1-from) M1FROM="$2"; M1=0; shift 2;; --out) OUT="$2"; shift 2;;
  --no-a11y) A11Y=0; shift;; --no-m1) M1=0; shift;;
  *) echo "unknown flag: $1"; exit 2;; esac; done

MW=$R/target/release/manuk-wpt; DP=$R/target/debug/drive-probe; AX=$R/target/debug/a11y-score
for b in "$MW" "$DP" "$AX"; do [ -x "$b" ] || { echo "✗ missing: $b"; exit 1; }; done
command -v google-chrome >/dev/null || { echo "✗ google-chrome required"; exit 1; }
mkdir -p "$OUT/iso"; M2MAP="$OUT/m2.tsv"; M1M2="$OUT/m1m2.tsv"; AXF="$OUT/a11y.tsv"; FINAL="$OUT/results.tsv"
: > "$M2MAP"; : > "$M1M2"; : > "$AXF"

sel() { grep -vE '^#|^$' "$CORPUS" | awk 'NF>=2{n[$1]++;print n[$1]"\t"$1"\t"$2}' | sort -k1,1n -k2,2 | cut -f2-; }
mapfile -t ROWS < <(sel); [ "$LIMIT" -gt 0 ] && ROWS=("${ROWS[@]:0:LIMIT}")
echo "▶ TRI-SWEEP: ${#ROWS[@]} sites  jobs=$JOBS  m1-timeout=${M1TMO}s  corpus=$CORPUS"

# ── Phase A: M1 fidelity per-site, ISOLATED --out (this is also the REACHABILITY signal). ──
# Bot-wall sites fail here in <=M1TMO and are then SKIPPED by M2/a11y (which otherwise hang their own fetch).
run_m1() {
  local cat="$1" url="$2" iso o cov shape vis paths st
  iso="$OUT/iso/$(printf '%s' "$url"|md5sum|cut -c1-10)"; mkdir -p "$iso"
  o=$(timeout "$M1TMO" nice -n 15 "$MW" fidelity --urls "$url" --out "$iso" 2>&1)
  cov=$(printf '%s' "$o"|grep -oE 'structural: [0-9.]+%'|grep -oE '[0-9.]+'|head -1)
  paths=$(printf '%s' "$o"|grep -oE 'oracle [0-9]+ paths'|grep -oE '[0-9]+'|head -1)
  shape=$(printf '%s' "$o"|grep -oE 'SHAPE: [0-9.]+%'|grep -oE '[0-9.]+'|head -1)
  vis=$(printf '%s' "$o"|grep -oE 'MEAN VISUAL: +[0-9.]+'|grep -oE '[0-9.]+'|head -1)
  if   printf '%s' "$o"|grep -qiE 'segmentation fault|SIGSEGV|panicked'; then st=CRASH
  elif [ "${paths:-0}" -ge 10 ] 2>/dev/null; then st=OK
  elif [ "${paths:-0}" -gt 0 ] 2>/dev/null; then st=LOW_SAMPLE
  elif [ -n "${vis:-}" ]; then st=NO_STRUCT
  else st=UNREACHABLE; fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$cat" "$url" "$st" "${cov:-}" "${shape:-}" "${vis:-}" "${paths:-0}" >> "$M1M2"
  printf '  %-11s %-38s %-11s cov=%-6s shape=%-6s\n' "$cat" "${url:0:38}" "$st" "${cov:-–}" "${shape:-–}"
}
if [ -n "$M1FROM" ]; then
  cp "$M1FROM" "$M1M2"
  echo "▶ phase A SKIPPED — imported M1 reachability from $M1FROM ($(wc -l < "$M1M2") rows)"
else
  echo "▶ phase A (M1 fidelity + reachability)…"
  for row in "${ROWS[@]}"; do
    cat=$(printf '%s' "$row"|cut -f1); url=$(printf '%s' "$row"|cut -f2)
    while [ "$(jobs -rp|wc -l)" -ge "$JOBS" ]; do wait -n 2>/dev/null||sleep 0.2; done
    run_m1 "$cat" "$url" &
  done; wait
fi
echo "  M1 scored: $(wc -l < "$M1M2")  reachable: $(awk -F'\t' '$3=="OK"||$3=="LOW_SAMPLE"||$3=="NO_STRUCT"' "$M1M2"|wc -l)"

# ── reachable set drives the expensive phases (skip bot-walls entirely) ──
mapfile -t RURLS < <(awk -F'\t' '$3=="OK"||$3=="LOW_SAMPLE"||$3=="NO_STRUCT"{print $2}' "$M1M2")

# ── Phase B: M2 drive-probe PER SITE, parallel (no Chrome). ──
# A site that HANGS the driver is undriveable → it counts as rate 0 (tag TIMEOUT), it does NOT vanish.
# Per-site timeout is the fix: the old code ran 20 sites under ONE 300s timeout, so one hang wiped the batch.
run_m2() {
  local url="$1" o rate ceil tag
  o=$(timeout "$M2TMO" nice -n 15 "$DP" "$url" 2>/dev/null)
  read -r rate ceil < <(printf '%s\n' "$o" | awk '/^https?:/{r=$6;c=$9;gsub(/%/,"",r);gsub(/%/,"",c);print r" "c; exit}')
  if [ -z "${rate:-}" ]; then rate=0; ceil=0; tag=TIMEOUT; else tag=OK; fi
  printf '%s\t%s\t%s\t%s\n' "$url" "$rate" "$ceil" "$tag" >> "$M2MAP"
  printf '  M2 %-38s rate=%-6s %s\n' "${url:0:38}" "$rate" "$tag"
}
echo "▶ phase B (M2 drive-probe on ${#RURLS[@]} reachable, per-site tmo=${M2TMO}s jobs=$JOBS)…"
for url in "${RURLS[@]}"; do
  while [ "$(jobs -rp|wc -l)" -ge "$JOBS" ]; do wait -n 2>/dev/null||sleep 0.2; done
  run_m2 "$url" &
done; wait
echo "  M2 measured: $(awk -F'\t' '$4=="OK"' "$M2MAP"|wc -l)/${#RURLS[@]}  (timeout: $(awk -F'\t' '$4=="TIMEOUT"' "$M2MAP"|wc -l))"

# ── Phase C: a11y-score PER SITE, SERIAL (a11y-score binds Chrome on port 9500; parallel procs collide). ──
# Same fix as phase B: a per-site timeout so one hang scores F1 0 (tag TIMEOUT) instead of killing the batch.
if [ "$A11Y" -eq 1 ]; then
  echo "▶ phase C (a11y-score on ${#RURLS[@]} reachable, per-site serial tmo=${AXTMO}s)…"
  n=0
  for url in "${RURLS[@]}"; do
    n=$((n+1))
    o=$(timeout "$AXTMO" nice -n 15 "$AX" "$url" 2>/dev/null)
    read -r prec rec f1 < <(printf '%s\n' "$o" | awk '/^https?:/{p=$5;r=$6;f=$7;gsub(/%/,"",p);gsub(/%/,"",r);gsub(/%/,"",f);print p" "r" "f; exit}')
    if [ -z "${f1:-}" ]; then prec=0; rec=0; f1=0; tag=TIMEOUT; else tag=OK; fi
    printf '%s\t%s\t%s\t%s\t%s\n' "$url" "$prec" "$rec" "$f1" "$tag" >> "$AXF"
    printf '  a11y %3d/%d %-34s F1=%-6s %s\n' "$n" "${#RURLS[@]}" "${url:0:34}" "$f1" "$tag"
  done
  echo "  a11y measured: $(awk -F'\t' '$5=="OK"' "$AXF"|wc -l)/${#RURLS[@]}  (timeout: $(awk -F'\t' '$5=="TIMEOUT"' "$AXF"|wc -l))"
fi

# ── join: M1M2 (cat url status cov shape vis paths) + M2MAP (rate ceil) + AXF (prec rec f1) ──
awk -F'\t' '
  FILENAME==m2{mr[$1]=$2; mc[$1]=$3; next}
  FILENAME==ax{ap[$1]=$2; ar[$1]=$3; af[$1]=$4; next}
  {print $0"\t"mr[$2]"\t"mc[$2]"\t"ap[$2]"\t"ar[$2]"\t"af[$2]}
' m2="$M2MAP" ax="$AXF" "$M2MAP" "$AXF" "$M1M2" > "$FINAL"
echo ""; echo "=== DONE $(date '+%F %T') → $FINAL"
echo "cols: cat url status cov shape vis paths m2rate m2ceil a11yprec a11yrec a11yf1"
# ── honest headline: FULL denominator (a hang scores 0) vs MEASURED-only, so the bias is visible ──
TOT=$(wc -l < "$M1M2")
REACH=$(awk -F'\t' '$3=="OK"||$3=="LOW_SAMPLE"||$3=="NO_STRUCT"{n++}END{print n+0}' "$M1M2")
echo "  sites=$TOT  reachable=$REACH"
awk -F'\t' '($3=="OK"||$3=="LOW_SAMPLE")&&$5!=""{s+=$5;n++}
  END{if(n)printf "  M1  SHAPE (reachable):   %.1f%%  (n=%d)\n",s/n,n}' "$M1M2"
awk -F'\t' '{n++;all+=$2; if($4=="OK"){ok++;oks+=$2}else to++}
  END{if(n)printf "  M2  drive-rate: FULL %.1f%% (n=%d, %d timeout scored 0)  |  measured-only %.1f%% (n=%d)\n",all/n,n,to,(ok?oks/ok:0),ok}' "$M2MAP"
[ "$A11Y" -eq 1 ] && awk -F'\t' '{n++;all+=$4; if($5=="OK"){ok++;oks+=$4}else to++}
  END{if(n)printf "  a11y F1:        FULL %.1f%% (n=%d, %d timeout scored 0)  |  measured-only %.1f%% (n=%d)\n",all/n,n,to,(ok?oks/ok:0),ok}' "$AXF"
