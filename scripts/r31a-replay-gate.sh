#!/usr/bin/env bash
# R3.1.a gate: a page served by a LIVE local HTTP server (HTML + CSS + JS + image + a fetch() API) is
# recorded once; the server is then KILLED and the page is loaded from the recording 3x with the
# network gone. All 3 replay digests must equal the live digest, with 0 replay misses; and flipping one
# byte of a recorded body must change the digest (the gate can go red).
set -u
cd "$(dirname "$0")/.."
touch .git/sprint-working
BIN=./target/release/a11y-dump
W=$(mktemp -d); trap 'kill $SRV 2>/dev/null; rm -rf "$W"' EXIT
SITE=tests/fixtures/replay-site
PORT=$((20000 + RANDOM % 20000))
( cd $SITE && exec python3 -m http.server $PORT --bind 127.0.0.1 >/dev/null 2>&1 ) & SRV=$!
for _ in $(seq 50); do curl -s -o /dev/null http://127.0.0.1:$PORT/ && break; sleep 0.1; done
URL=http://127.0.0.1:$PORT/index.html
export MANUK_STATE=$W/state
dig() { "$BIN" --deterministic "$@" 2>"$W/err" | sha256sum | cut -c1-16; }
LIVE=$(dig "$URL")
REC=$(dig --record=$W/rec "$URL")
kill $SRV; wait $SRV 2>/dev/null
echo "live=$LIVE record-run=$REC entries=$(wc -l < $W/rec/index.jsonl)"
fail=0; reps=()
for i in 1 2 3; do reps+=("$(dig --replay=$W/rec "$URL")"); done
echo "replay=[${reps[*]}]"
for r in "${reps[@]}"; do [ "$r" = "$LIVE" ] || fail=1; done
[ "$REC" = "$LIVE" ] || fail=1
# mutation: flip a byte in the recorded CSS/JS/HTML body that shapes the a11y output
T=$(grep -l "Replay fixture heading" $W/rec/bodies/* | head -1)
[ -n "$T" ] || { echo "FAIL: recorded html body not found"; exit 1; }
printf 'X' | dd of="$T" bs=1 seek=$(grep -bo "Replay fixture heading" "$T" | head -1 | cut -d: -f1) conv=notrunc 2>/dev/null
MUT=$(dig --replay=$W/rec "$URL")
echo "mutated-replay=$MUT"
[ "$MUT" != "$LIVE" ] || { echo "FAIL: mutation did not change the digest (gate cannot go red)"; exit 1; }
[ $fail = 0 ] && { echo "PASS: 3/3 replay digests == live digest ($LIVE); one flipped body byte -> $MUT"; exit 0; }
echo "FAIL: replay digests differ from live"; exit 1
