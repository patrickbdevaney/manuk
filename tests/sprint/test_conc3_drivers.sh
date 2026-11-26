#!/bin/bash
# CONC.3 gate: Layer 2 driver logic (offline, fake driver) + committed smoke rows show every driver finished 3 tasks on both arms.
set -e; cd "$(dirname "$0")/../.."
export MANUK_CHROME_PIN=$(cat docs/sprint/CHROME_PIN); L="python3 -I bench/conc/layer2.py run"; T=$(mktemp -d); O=$T/o.tsv
# 1. missing key: exit 3, no rows (both key-holding providers)
CONC3_ENV_GROQ=/nonexistent $L --driver groq --model m --out $O 2>/dev/null && { echo "RED: groq ran without key"; exit 1; } || [ $? -eq 3 ] || { echo "RED: groq exit != 3"; exit 1; }
CONC3_ENV_OPENROUTER=/nonexistent $L --driver openrouter --model m:free --out $O 2>/dev/null && { echo "RED: openrouter ran without key"; exit 1; } || [ $? -eq 3 ] || { echo "RED: openrouter exit != 3"; exit 1; }
[ ! -s $O ] || { echo "RED: rows written without key"; exit 1; }
# 2. a paid OpenRouter id is refused
$L --driver openrouter --model vendor/paid --out $O 2>/dev/null && { echo "RED: paid id accepted"; exit 1; }
# 3. infra on one arm: the whole group re-runs on every arm, nothing scored as failure
CONC3_FAKE_INFRA=pw-proc $L --driver fake --model f --out $O
[ "$(awk -F'\t' 'NR>1&&$9!="pass"' $O | wc -l)" -eq 0 ] && [ "$(awk -F'\t' 'NR>1&&$8==2' $O | wc -l)" -eq 6 ] || { echo "RED: infra group not re-run on every arm"; exit 1; }
# 3b. mutant: infra scored as a task failure on one arm must be caught by the same check
rm -f $O; CONC3_MUTATE=score-infra CONC3_FAKE_INFRA=pw-proc $L --driver fake --model f --out $O
[ "$(awk -F'\t' 'NR>1&&$9=="fail"' $O | wc -l)" -gt 0 ] || { echo "RED: mutant not producing a failure"; exit 1; }
[ "$(awk -F'\t' 'NR>1&&$9!="pass"' $O | wc -l)" -eq 0 ] && { echo "RED: mutant not detected"; exit 1; }
# 4. committed smoke rows: every driver/model has 3 tasks x 2 arms, status pass|fail (not infra)
S=docs/sprint/conc3/layer2-smoke.tsv
for m in haiku sonnet opus gemini-3.8-flash-medium nvidia/nemotron-3-ultra-550b-a55b:free; do
  [ "$(awk -F'\t' -v m=$m 'NR>1&&$4==m&&($9=="pass"||$9=="fail")' $S | cut -f5,6 | sort -u | wc -l)" -eq 6 ] || { echo "RED: $m lacks 3 tasks x 2 arms"; exit 1; }
done
# Groq (key invalid 2026-11-25) and the second OpenRouter model (429) are tracked as CONC.3.a/.b, not asserted here.
echo GREEN
