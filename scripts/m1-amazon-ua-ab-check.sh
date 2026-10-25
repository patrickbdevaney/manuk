#!/usr/bin/env bash
# R0.4.a.a.a.b.a.a gate: the UA A/B evidence for amazon.com must carry one labelled raw status line
# for each of the three UAs (manuk, curl, chrome), and be stamped with the pinned chrome_ver.
# Usage: scripts/m1-amazon-ua-ab-check.sh [path]   (default docs/sprint/m1-empty/amazon.ua-ab.txt)
set -u
f="${1:-docs/sprint/m1-empty/amazon.ua-ab.txt}"
pin="${MANUK_CHROME_PIN:-}"
if [ ! -f "$f" ]; then
  echo "FAIL: $f missing"; exit 1
fi
for lab in manuk curl chrome; do
  # each labelled block must be followed (before the next block) by a raw HTTP status line
  if ! awk -v L="== ua=$lab " '
        index($0, L) == 1 { inb = 1; next }
        /^== ua=/ { inb = 0 }
        inb && /^status_line: HTTP\/[0-9.]+ [0-9]{3}/ { ok = 1 }
        END { exit ok ? 0 : 1 }' "$f"; then
    echo "FAIL: $f has no raw status line for ua=$lab"; exit 1
  fi
done
if [ -z "$pin" ]; then
  echo "FAIL: MANUK_CHROME_PIN unset"; exit 1
fi
if ! grep -aFq "chrome_ver=$pin" "$f"; then
  echo "FAIL: $f not stamped chrome_ver=$pin"; exit 1
fi
echo "OK: $f three UAs recorded chrome_ver=$pin"
