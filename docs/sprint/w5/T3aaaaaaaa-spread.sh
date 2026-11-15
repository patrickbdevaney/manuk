#!/usr/bin/env bash
# W5.T3.a.a.a.a.a.a.a.a: rerun the navigate-quiet task on the after binary (b9742038db32f0fe), n=20 as four batches of 5,
# batch k starting at T0 + (k-1)*40 min, so the first and last runs are at least 120 minutes apart.
# Each batch is recorded with T3aaaaaa-capture.py (query and tool_result of every ToolSearch call); the cwd of each
# batch is recorded so T3aaaaaaa-reqdiff.py can group its sessions by window afterwards.
#
# usage: T3aaaaaaaa-spread.sh init                 (writes T0 = now to T3aaaaaaaa-T0)
#        T3aaaaaaaa-spread.sh batch K              (waits for batch K's start time, then runs 5 navigate-quiet runs)
#        T3aaaaaaaa-spread.sh diff                 (reqdiff over the four batch groups, writes T3aaaaaaaa-reqdiff.tsv)
set -euo pipefail
cd /home/patrickd/manuk
D=docs/sprint/w5
BIN=/home/patrickd/.cache/manuk-t3aaaaa/bin/manuk-mcp-3c0d499d
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"

case "$1" in
init)
  date +%s > "$D/T3aaaaaaaa-T0"
  cat "$D/T3aaaaaaaa-T0"
  ;;
batch)
  K=$2
  T0=$(cat "$D/T3aaaaaaaa-T0")
  START=$((T0 + (K - 1) * 2400))
  now=$(date +%s)
  if [ "$now" -lt "$START" ]; then sleep $((START - now)); fi
  BEGIN=$(date +%H:%M:%S)
  python3 -I "$D/T3aaaaaa-capture.py" --bin "$BIN" --out "$D/T3aaaaaaaa-batch$K.tsv" --runs 5
  END=$(date +%H:%M:%S)
  # the newest cwd of this batch: capture makes one fresh /tmp/w5t3aaaaaa-cwd-* per invocation
  DIR=$(ls -td "$HOME"/.claude/projects/-tmp-w5t3aaaaaa-cwd-* | head -1 | xargs basename)
  echo "$DIR@$BEGIN-$END" > "$D/T3aaaaaaaa-batch$K.group"
  echo "batch $K started $BEGIN ended $END group $DIR@$BEGIN-$END"
  ;;
diff)
  args=()
  for K in 1 2 3 4; do args+=("--group=$(cat "$D/T3aaaaaaaa-batch$K.group")"); done
  python3 -I "$D/T3aaaaaaa-reqdiff.py" "${args[@]}" --out "$D/T3aaaaaaaa-reqdiff.tsv"
  ;;
*)
  echo "usage: $0 init|batch K|diff" >&2
  exit 2
  ;;
esac
