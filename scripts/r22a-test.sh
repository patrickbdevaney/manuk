#!/usr/bin/env bash
# R2.2.a gate: DOCS-PYTHON.md names >=1 element path, and every path it names is in the stored diff.
D=${1:-/home/patrickd/manuk/docs/sprint/r22}
paths=$(grep -oE 'body:nth-of-type\(1\)/[A-Za-z0-9:()/-]+' "$D/DOCS-PYTHON.md" | sort -u)
[ -n "$paths" ] || { echo "FAIL: no path named"; exit 1; }
for p in $paths; do grep -qF "$p" "$D/APPLE-manuk-paths.diff" || { echo "FAIL: $p not in diff"; exit 1; }; done
echo "PASS: $(echo "$paths" | wc -l) paths named, all in diff"
