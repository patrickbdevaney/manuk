#!/bin/bash
# R0.4.a.c.a.b.b gate: a tri-sweep tsv must have >=MIN rows with a11y_tag=OK, all on the pinned chrome_ver.
f=${1:?usage: tri-a11y-gate.sh TSV}; MIN=${MIN:-12}
ok=$(awk -F'\t' 'NR>1&&$16=="OK"' "$f"|wc -l); bad=$(awk -F'\t' -v p="${MANUK_CHROME_PIN:?}" 'NR>1&&$20!=p' "$f"|wc -l)
echo "a11y OK rows: $ok (min $MIN), rows off chrome pin: $bad"
[ "$ok" -ge "$MIN" ] && [ "$bad" -eq 0 ]
