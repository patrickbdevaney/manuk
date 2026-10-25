#!/usr/bin/env bash
# GATES.ALL — build and EXECUTE every manuk-page integration-test gate, in batches, one verdict per gate.
# Peak disk stays bounded: each batch's test binaries are deleted before the next batch compiles.
#   usage: scripts/gates-all.sh [--batch N] [--only REGEX] [--out FILE]
# Output: docs/sprint/GATES-ALL.tsv  (gate, verdict, secs, chrome_ver, detail)
set -uo pipefail
cd "$(dirname "$0")/.."; ROOT=$PWD
BATCH=40; ONLY=.; OUT=docs/sprint/GATES-ALL.tsv
while [ $# -gt 0 ]; do case "$1" in
  --batch) BATCH=$2; shift 2;; --only) ONLY=$2; shift 2;; --out) OUT=$2; shift 2;; *) echo "bad arg $1"; exit 2;; esac; done
FEAT="--features stylo,spidermonkey"   # same set as the wall's warm build: no fingerprint thrash
PIN=${MANUK_CHROME_PIN:-$(cat docs/sprint/CHROME_PIN 2>/dev/null)}
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}
mapfile -t GATES < <(ls engine/page/tests/*.rs | xargs -n1 basename | sed 's/\.rs$//' | grep -E "$ONLY" | sort)
printf 'gate\tverdict\tsecs\tchrome_ver\tdetail\n' > "$OUT"
DUMAX=0
for ((i=0; i<${#GATES[@]}; i+=BATCH)); do
  touch .git/sprint-working
  chunk=("${GATES[@]:i:BATCH}"); args=(); for g in "${chunk[@]}"; do args+=(--test "$g"); done
  J=$(mktemp); 
  cargo test -p manuk-page $FEAT --no-run --message-format=json "${args[@]}" 2>/tmp/gates-all-build.err >"$J"
  declare -A EXE=()
  while IFS=$'\t' read -r n p; do EXE[$n]=$p; done < <(jq -r 'select(.reason=="compiler-artifact" and .profile.test==true and .executable!=null and (.target.kind|index("test"))) | [.target.name,.executable]|@tsv' "$J")
  for g in "${chunk[@]}"; do
    touch .git/sprint-working
    exe=${EXE[$g]:-}
    if [ -z "$exe" ]; then printf '%s\tBUILD_FAIL\t0\t%s\t%s\n' "$g" "$PIN" "$(grep -m1 -A0 "error" /tmp/gates-all-build.err | tr '\t\n' '  ' | cut -c1-120)" >> "$OUT"; continue; fi
    s=$SECONDS
    log=$(cd engine/page && timeout 600 "$exe" --test-threads=1 2>&1); rc=$?
    secs=$((SECONDS-s))
    res=$(echo "$log" | grep -E '^test result:' | tail -1)
    if [ $rc -eq 0 ]; then v=GREEN; d=$(echo "$res" | grep -oE '[0-9]+ passed; [0-9]+ failed; [0-9]+ ignored' )
    else v=RED; d="rc=$rc $(echo "$log" | grep -E 'FAILED|panicked at' | head -2 | tr '\t\n' '  ' | cut -c1-160)"; fi
    printf '%s\t%s\t%s\t%s\t%s\n' "$g" "$v" "$secs" "$PIN" "$d" >> "$OUT"
  done
  for p in "${EXE[@]}"; do rm -f "$p"; done
  unset EXE; rm -f "$J"
  du=$(du -sBG target 2>/dev/null | cut -f1 | tr -d G); [ "${du:-0}" -gt "$DUMAX" ] && DUMAX=$du
  echo "batch $((i/BATCH+1)) done; target/=${du}G (peak ${DUMAX}G)" >&2
done
echo "rows=$(($(wc -l <"$OUT")-1)) green=$(grep -c $'\tGREEN\t' "$OUT") red=$(grep -c $'\tRED\t' "$OUT") build_fail=$(grep -c $'\tBUILD_FAIL\t' "$OUT") peak_target=${DUMAX}G" >&2
