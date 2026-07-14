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

# ⚠ **NEVER name this `TMP`.** On Windows, `TMP` is the environment variable the OS uses for the temp
# DIRECTORY, and it is already exported — so `TMP=$(mktemp)` silently overwrites it with the path of a
# FILE. `link.exe` then inherits it, tries to create `lnk{GUID}.tmp` *inside a file*, and dies with
# `LNK1104: cannot open file`. **My own instrument broke the build it was measuring**, and it looked
# exactly like a real Windows linker bug (which is what I spent a tick chasing).
_CI_LOG="$(mktemp)"
"$@" 2>&1 | tee "$_CI_LOG"
STATUS=${PIPESTATUS[0]}

if [ "$STATUS" -ne 0 ]; then
  echo "::group::${LABEL} FAILED (exit ${STATUS}) — the real error"
  # Prefer the compiler's own error lines; fall back to the tail if there are none (a build-script panic,
  # a linker kill, an OOM). `signal: 9` is the OOM killer and MUST be named as such, because it looks
  # exactly like a compile error and gets read as "the code is broken" (see scripts/mem-guard.sh).
  # A cargo TEST failure looks nothing like a compile error: there is no `error:` line at all. The signal
  # is `test ... FAILED`, the `failures:` block, and the panic/assert text. The first version of this
  # script missed all of that, fell through to `tail`, and annotated 25 lines of PASSING tests — which is
  # a instrument reporting the opposite of the truth.
  # `LNK[0-9]+` / `cannot open file` matter because MSVC's real diagnosis lives on a NOTE line, not an
  # `error:` line — `error: linking with link.exe failed: exit code 1104` names no file at all, and the
  # file it cannot open is the entire diagnosis.
  ERRS="$(grep -E '^(error|error\[|thread .* panicked|.*signal: 9)|FAILED|^assertion|^ *left:|^ *right:|panicked at|^test result: FAILED|LNK[0-9]+|cannot open file|No such file' "$_CI_LOG" | head -40)"
  [ -z "$ERRS" ] && ERRS="$(grep -vE "^(warning|note|help|  *=|  *\||  *[0-9]+ \|)" "$_CI_LOG" | tail -30)"
  while IFS= read -r line; do
    printf '::error title=%s::%s\n' "$LABEL" "${line//$'\r'/}"
  done <<< "$ERRS"
  echo "::endgroup::"
fi
rm -f "$_CI_LOG"
exit "$STATUS"
