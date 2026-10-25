#!/usr/bin/env bash
# R0.4.a.a.a.b gate: the plain-curl evidence for amazon.com must exist and carry an HTTP status line.
# Usage: scripts/m1-amazon-curl-check.sh [path]   (default docs/sprint/m1-empty/amazon.curl.txt)
set -u
f="${1:-docs/sprint/m1-empty/amazon.curl.txt}"
if [ ! -f "$f" ]; then
  echo "FAIL: $f missing"; exit 1
fi
if ! grep -aEq '^HTTP/[0-9.]+ [0-9]{3} ' "$f"; then
  echo "FAIL: $f has no HTTP status line"; exit 1
fi
echo "OK: $f has status line: $(grep -aEo '^HTTP/[0-9.]+ [0-9]{3}' "$f" | head -1 | tr -d '\r')"
