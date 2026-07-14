#!/usr/bin/env bash
# ── MAKE A CI FAILURE READABLE WITHOUT ADMIN RIGHTS.
#
# GitHub's job LOGS require `actions:read` — on this repo the API answers **403 "Must have admin rights"**
# even though the repo is public. So a CI failure was, from the outside, a single line: *"Process completed
# with exit code 101."* That is not a diagnosis, and two ticks were spent GUESSING at causes (mozjs? the
# GUI libs? a stale cache?) because the actual compiler error was unreadable.
#
# **Check-run ANNOTATIONS, however, are public.** GitHub turns any line starting with `::error::` into one.
#
# So: run the command, stream it normally, and on failure re-emit its tail as annotations. The next person
# (or the loop) reads the real error straight off the API instead of theorising.
#
#   usage: scripts/ci-run.sh <label> <command...>
set -uo pipefail
LABEL="$1"; shift

TMP="$(mktemp)"
"$@" 2>&1 | tee "$TMP"
STATUS=${PIPESTATUS[0]}

if [ "$STATUS" -ne 0 ]; then
  echo "::group::${LABEL} FAILED (exit ${STATUS}) — the real error"
  # Prefer the compiler's own error lines; fall back to the tail if there are none (a build-script panic,
  # a linker kill, an OOM). `signal: 9` is the OOM killer and MUST be named as such, because it looks
  # exactly like a compile error and gets read as "the code is broken" (see scripts/mem-guard.sh).
  ERRS="$(grep -E '^(error|error\[|thread .* panicked|.*signal: 9)' "$TMP" | head -20)"
  [ -z "$ERRS" ] && ERRS="$(tail -25 "$TMP")"
  while IFS= read -r line; do
    printf '::error title=%s::%s\n' "$LABEL" "${line//$'\r'/}"
  done <<< "$ERRS"
  echo "::endgroup::"
fi
rm -f "$TMP"
exit "$STATUS"
