#!/usr/bin/env bash
# ── TRI-SWEEP — measure all three exit legs per real site in one pass, honestly.
#   M1  render fidelity   : manuk-wpt fidelity   (structural coverage %, SHAPE %, visual %)
#   M2  drive/addressing  : drive-probe          (rate %, ceiling %)  [no Chrome, batched]
#   a11y accessibility    : a11y-score           (F1 %, precision, recall)  [own Chrome]
#
# FIXES over old fidelity-sweep.sh: (1) parser matches REBUILT output 'structural: X% (oracle N paths...)' +
# 'SHAPE: X% within Npx' (old grepped dead 'N ids'/'PLACEMENT:'); (2) PER-SITE ISOLATED --out (old shared one
# dir across jobs=3 → concurrent Chrome clobbered → example.com falsely NO_DATA); (3) M2 batched (fast, no
# Chrome); (4) a11y batched in chunks (a11y-score uses port 9500+arg-index; separate processes collide on 9500).
#
# usage: tri-sweep.sh [--corpus FILE] [--limit N] [--jobs J] [--m1-timeout S] [--out DIR] [--no-a11y] [--no-m1]
set -uo pipefail
R=/home/patrickd/manuk
CORPUS=$R/docs/bench/oracle-corpus.txt
LIMIT=0; JOBS=4; M1TMO=30; A11Y=1; M1=1
OUT=/tmp/claude-1000/-home-patrickd-manuk/3538dee1-05ec-426b-a0b7-1512fbafcc55/scratchpad/trisweep
while [ $# -gt 0 ]; do case "$1" in
  --corpus) CORPUS="$2"; shift 2;; --limit) LIMIT="$2"; shift 2;; --jobs) JOBS="$2"; shift 2;;
  --m1-timeout) M1TMO="$2"; shift 2;; --out) OUT="$2"; shift 2;;
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
echo "▶ phase A (M1 fidelity + reachability)…"
for row in "${ROWS[@]}"; do
  cat=$(printf '%s' "$row"|cut -f1); url=$(printf '%s' "$row"|cut -f2)
  while [ "$(jobs -rp|wc -l)" -ge "$JOBS" ]; do wait -n 2>/dev/null||sleep 0.2; done
  run_m1 "$cat" "$url" &
done; wait
echo "  M1 scored: $(wc -l < "$M1M2")  reachable: $(awk -F'\t' '$3=="OK"||$3=="LOW_SAMPLE"||$3=="NO_STRUCT"' "$M1M2"|wc -l)"

# ── reachable set drives the expensive phases (skip bot-walls entirely) ──
mapfile -t RURLS < <(awk -F'\t' '$3=="OK"||$3=="LOW_SAMPLE"||$3=="NO_STRUCT"{print $2}' "$M1M2")

# ── Phase B: M2 drive-probe on REACHABLE only, batched (no Chrome). ──
echo "▶ phase B (M2 drive-probe on ${#RURLS[@]} reachable)…"
i=0; while [ "$i" -lt "${#RURLS[@]}" ]; do
  chunk=("${RURLS[@]:i:20}")
  timeout 300 nice -n 15 "$DP" "${chunk[@]}" 2>/dev/null \
    | awk '/^https?:/{r=$6;c=$9;gsub(/%/,"",r);gsub(/%/,"",c);print $1"\t"r"\t"c}' >> "$M2MAP"
  i=$((i+20)); echo "  M2: $(wc -l < "$M2MAP")/${#RURLS[@]}"
done

# ── Phase C: a11y-score on REACHABLE sites, chunked (one process/chunk) ──
if [ "$A11Y" -eq 1 ]; then
  echo "▶ phase C (a11y-score on ${#RURLS[@]} reachable)…"
  i=0; while [ "$i" -lt "${#RURLS[@]}" ]; do
    chunk=("${RURLS[@]:i:10}")
    timeout 600 nice -n 15 "$AX" "${chunk[@]}" 2>/dev/null \
      | awk '/^https?:/{p=$5;r=$6;f=$7;gsub(/%/,"",p);gsub(/%/,"",r);gsub(/%/,"",f);print $1"\t"p"\t"r"\t"f}' >> "$AXF"
    i=$((i+10)); echo "  a11y: $(wc -l < "$AXF")/${#RURLS[@]}"
  done
fi

# ── join: M1M2 (cat url status cov shape vis paths) + M2MAP (rate ceil) + AXF (prec rec f1) ──
awk -F'\t' '
  FILENAME==m2{mr[$1]=$2; mc[$1]=$3; next}
  FILENAME==ax{ap[$1]=$2; ar[$1]=$3; af[$1]=$4; next}
  {print $0"\t"mr[$2]"\t"mc[$2]"\t"ap[$2]"\t"ar[$2]"\t"af[$2]}
' m2="$M2MAP" ax="$AXF" "$M2MAP" "$AXF" "$M1M2" > "$FINAL"
echo ""; echo "=== DONE $(date '+%F %T') → $FINAL"
echo "cols: cat url status cov shape vis paths m2rate m2ceil a11yprec a11yrec a11yf1"
# ── honest headline ──
awk -F'\t' '{t++; st=$3
  if(st=="OK"||st=="LOW_SAMPLE")reach++
  if($5!="")  {sh+=$5; shn++}
  if($8!="")  {m2+=$8; m2n++}
  if($12!=""){f1+=$12; f1n++}}
END{printf "  sites=%d  reachable=%d (%.0f%%)\n",t,reach,100*reach/t
  if(shn)printf "  M1  mean SHAPE (reachable): %.1f%%  (n=%d)\n",sh/shn,shn
  if(m2n)printf "  M2  mean drive-rate:        %.1f%%  (n=%d)\n",m2/m2n,m2n
  if(f1n)printf "  a11y mean F1:               %.1f%%  (n=%d)\n",f1/f1n,f1n}' "$FINAL"
