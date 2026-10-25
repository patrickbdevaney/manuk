#!/usr/bin/env bash
# R0.4.a.a.a.b.a gate: the manuk-net (engine's own fetch_document) evidence for amazon.com must exist,
# carry an HTTP status line from the manuk-net request, and be stamped with the pinned chrome_ver.
# Usage: scripts/m1-amazon-net-check.sh [path]   (default docs/sprint/m1-empty/amazon.manuk-net.txt)
set -u
f="${1:-docs/sprint/m1-empty/amazon.manuk-net.txt}"
pin="${MANUK_CHROME_PIN:-}"
if [ ! -f "$f" ]; then
  echo "FAIL: $f missing"; exit 1
fi
if ! grep -aEq '^HTTP/[0-9.]+ [0-9]{3} \(manuk-net' "$f"; then
  echo "FAIL: $f has no manuk-net HTTP status line"; exit 1
fi
if [ -z "$pin" ]; then
  echo "FAIL: MANUK_CHROME_PIN unset"; exit 1
fi
if ! grep -aFq "chrome_ver=$pin" "$f"; then
  echo "FAIL: $f not stamped chrome_ver=$pin"; exit 1
fi
echo "OK: $f status: $(grep -aE '^HTTP/.*manuk-net' "$f" | head -1) chrome_ver=$pin"
