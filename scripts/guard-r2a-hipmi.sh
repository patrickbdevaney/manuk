#!/usr/bin/env bash
# GUARD.R2.a gate: N fresh a11y-score runs on one site, serial, on the pinned chrome_ver.
# Each run also re-runs manuk-wpt fidelity into its own dir so the oracle and engine-fetch sha256 are recorded
# per run (same as tri-sweep.sh's attrib sidecar). manuk_n / chrome_n are the kept AX node counts a11y-score saw.
#
# usage: guard-r2a-hipmi.sh [URL] [RUNS] [OUT_TSV]      run the gate, write one TSV row per run
#        guard-r2a-hipmi.sh --verdict TSV               verdict only (used by the mutation test)
# Green: every run tag OK and max(F1) - min(F1) <= 2.0. Red otherwise (exit 1).
set -uo pipefail
R=/home/patrickd/manuk

verdict() {
  awk -F'\t' 'NR>1{ if($3!="OK") bad++; n++; if(min==""||$9+0<min)min=$9+0; if(max==""||$9+0>max)max=$9+0 }
    END{ if(!n){print "verdict: RED (no rows)"; exit 1}
      ok = (max-min)<=2.0 && bad+0==0
      printf "F1 n=%d min=%.1f max=%.1f spread=%.1f non-OK=%d\n", n, min, max, max-min, bad+0
      print "verdict: " (ok ? "GREEN" : "RED"); exit !ok }' "$1"
}
if [ "${1:-}" = "--verdict" ]; then verdict "${2:?usage: --verdict TSV}"; exit $?; fi

URL=${1:-https://hipmiluwuutara.org/}; RUNS=${2:-5}; OUT=${3:-$R/docs/bench/guard-r2a-hipmi-$(date +%Y%m%d-%H%M%S).tsv}
: "${MANUK_CHROME_PIN:?}"
AX=$R/target/release/a11y-score; MW=$R/target/release/manuk-wpt
TMP=$(mktemp -d /tmp/guard-r2a.XXXXXX)
printf 'run\turl\ttag\tmanuk_n\tchrome_n\tmatch\tprec\trec\tf1\toracle_sha\tengine_sha\tm1_paths\tchrome_ver\n' > "$OUT"
for i in $(seq 1 "$RUNS"); do
  touch "$R/.git/sprint-working"
  o=$(timeout 90 nice -n 15 "$AX" "$URL" 2>"$TMP/ax$i.err"); rc=$?
  tag=OK; mn=-; cn=-; mt=-; prec=-; rec=-; f1=-
  if [ "$rc" -eq 124 ]; then tag=TIMEOUT
  elif [ "$rc" -ne 0 ]; then tag=RC$rc
  else
    read -r mn cn mt prec rec f1 < <(printf '%s\n' "$o" | awk '/^https?:/{p=$5;r=$6;f=$7;gsub(/%/,"",p);gsub(/%/,"",r);gsub(/%/,"",f);print $2" "$3" "$4" "p" "r" "f; exit}')
    [ -n "${f1:-}" ] && [ "$f1" != "-" ] || { tag=PARSE; f1=-; }
  fi
  d="$TMP/m1-$i"; mkdir -p "$d"
  timeout 60 nice -n 15 "$MW" fidelity --urls "$URL" --out "$d" >/dev/null 2>&1
  of=$(ls "$d"/*.oracle-paths.tsv 2>/dev/null | head -1); ef=$(ls "$d"/*.engine-fetch.html 2>/dev/null | head -1)
  osha=-; esha=-; paths=0
  [ -n "$of" ] && { osha=$(sha256sum "$of" | cut -c1-64); paths=$(wc -l < "$of"); }
  [ -n "$ef" ] && esha=$(sha256sum "$ef" | cut -c1-64)
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$i" "$URL" "$tag" "$mn" "$cn" "$mt" "$prec" "$rec" "$f1" "$osha" "$esha" "$paths" "$MANUK_CHROME_PIN" >> "$OUT"
  printf '  run %d/%d tag=%s manuk_n=%s chrome_n=%s F1=%s oracle=%s engine=%s\n' "$i" "$RUNS" "$tag" "$mn" "$cn" "$f1" "${osha:0:12}" "${esha:0:12}"
done
rm -rf "$TMP"
verdict "$OUT"; rc=$?
echo "file: $OUT"
exit $rc
