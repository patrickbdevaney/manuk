#!/usr/bin/env bash
# Checks that every table-row cause cited in docs/sprint/M1-EMPTY.md appears verbatim in the saved M1 stderr it names.
# usage: m1-empty-check.sh [DOC]   exit 0 = all cites found, 1 = a cite is missing from its log
set -uo pipefail
R=/home/patrickd/manuk
DOC=${1:-$R/docs/sprint/M1-EMPTY.md}
fail=0; n=0
while IFS= read -r line; do
  case "$line" in "|"*) ;; *) continue;; esac
  cite=$(printf '%s' "$line" | grep -oE '`UNMEASURABLE \[[^`]+`' | tr -d '`' | head -1)
  log=$(printf '%s' "$line" | grep -oE 'docs/sprint/m1-empty/[a-z]+\.m1\.log' | head -1)
  [ -n "$cite" ] || continue
  n=$((n+1))
  if [ -z "$log" ] || ! grep -qF -- "$cite" "$R/$log"; then
    echo "✗ cite not found in $log: $cite"; fail=1
  else
    echo "✓ $log: $cite"
  fi
done < "$DOC"
[ "$n" -gt 0 ] || { echo "✗ no cites parsed from $DOC"; exit 1; }
echo "m1-empty-check: $n cites, fail=$fail"
exit $fail
