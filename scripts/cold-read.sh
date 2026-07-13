#!/usr/bin/env bash
# ── COLD READ — can a session with ZERO memory of any conversation recover the project?
#
# The real constraint on this project has never been tokens. It is that **context compacts**, and a
# session that resumes after compaction has no memory of what it decided an hour ago. Everything this
# loop knows lives in files or it does not exist.
#
# So this script asks the question mechanically, the way a stateless session would:
#
#   > *"I have just woken up. I remember nothing. Reading only the files, do I know where this project
#   >  stands, what was decided, what is in flight, and what to do next?"*
#
# **The test is not "do the files exist."** It is *"does a stateless GREP recover the decision."* A
# decision that was reasoned through beautifully in a conversation and never landed in a file **is not
# done** — it is lost, and the next session will re-derive it, wrongly, and call that progress.
#
# ⚠ **When you settle a new architectural decision, ADD IT TO `CRITICAL_DECISIONS` BELOW.** That is
# what converts "I said it out loud" into "a cold session cannot miss it". If the decision is real, it
# survives this script; if it only lived in the chat, this script is what tells you.

set -uo pipefail
cd "$(dirname "$0")/.."

fail=0
ok()  { printf '  \033[32m✓\033[0m %s\n' "$1"; }
bad() { printf '  \033[31m✗ %s\033[0m\n' "$1"; fail=$((fail+1)); }

echo
echo "── COLD READ: a session with no memory, reading only the files ──────────────────"
echo

# ── 1. The entry point. A cold session must be TOLD where to start, in the file it is given.
echo "Q1. Where do I start?"
if grep -qs "STATUS.md" CLAUDE.MD CLAUDE.md 2>/dev/null; then
  ok "CLAUDE.MD points at STATUS.md as the first session action"
else
  bad "CLAUDE.MD does not tell a cold session to read STATUS.md first"
fi

# ── 2. Where does the project stand? Every one of these is a number a cold session needs and
#       cannot infer. If STATUS.md is missing one, the session guesses — and a guess about the
#       current tier is how a loop starts working on the wrong thing.
echo
echo "Q2. Where does the project stand?"
for field in TICK CURRENT_TIER LAST_WALL_TIME ORACLE_CORPUS ORACLE_HANGS PENDING_GATES; do
  if grep -qE "^${field}:" STATUS.md 2>/dev/null; then
    ok "STATUS.md answers ${field}"
  else
    bad "STATUS.md has no ${field} — a cold session cannot know this without asking"
  fi
done

# ── 3. What is IN FLIGHT? This is the mid-tick-compaction case, and it is the one most likely to
#       be silently broken: the tick is half done, the context is gone, and the only thing that can
#       say what was being attempted is the journal's HYPOTHESIS — written BEFORE the code, on
#       purpose, for exactly this moment.
echo
echo "Q3. What was I in the middle of? (the mid-compaction resume path)"
TICK=$(grep -oP '^TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
if grep -q "Tick ${TICK}" docs/loop/JOURNAL.md 2>/dev/null; then
  ok "JOURNAL.md has an entry for the current tick (${TICK})"
  # The hypothesis is what makes an interrupted tick resumable. A journal entry that records only
  # the RESULT is a record of finished work — useless to a session that was interrupted BEFORE it.
  #
  # Read from the LAST `## Tick N` heading, not the first: the journal is append-only and a tick
  # number can recur across epochs. The pre-commit hook had this exact bug (PROCESS.md) and refused
  # a good tick over a stale entry — so it is written down, and here it is being made twice.
  #
  # And NOT `\b`: awk's ERE has no word-boundary escape, so `/## Tick 42\b/` silently matches
  # NOTHING. This script's first run reported "tick 42 has no hypothesis" about a journal entry that
  # plainly had one. **The auditor was wrong, not the file** — which is exactly why a verdict from a
  # new instrument gets verified before it is believed.
  last=$(grep -n "^## Tick ${TICK}[^0-9]" docs/loop/JOURNAL.md | tail -1 | cut -d: -f1)
  if [ -n "$last" ] && tail -n "+${last}" docs/loop/JOURNAL.md | grep -qi 'hypothesis'; then
    ok "...and it states a HYPOTHESIS — an interrupted tick is resumable from it"
  else
    bad "tick ${TICK}'s journal entry has no hypothesis: if this tick is interrupted, what it was ATTEMPTING is lost"
  fi
else
  bad "no JOURNAL.md entry for tick ${TICK} — a compacted session cannot resume an unrecorded tick"
fi

# ── 4. What must I NOT relitigate? The expensive failure mode is not forgetting a fact — it is
#       re-opening a settled question, re-deriving it wrongly, and spending a tick on it.
#
#       Each entry: "the decision" | file | a grep pattern that must find it.
echo
echo "Q4. What is already SETTLED? (a cold session must not reopen these)"
CRITICAL_DECISIONS=(
  "process-per-tab is DECIDED|docs/loop/PROCESS-MODEL.md|PROCESS-PER-TAB IS DECIDED|STATUS.md"
  "per-origin Site Isolation is REJECTED|docs/loop/PROCESS-MODEL.md|REJECTED|STATUS.md"
  "EME/DRM is permanently out of scope|STATUS.md|EME/DRM is OUT OF SCOPE|docs/loop/MEDIA.md"
  "a borrowed engine is a means, not a constraint|STATUS.md|BORROWED ENGINE IS A MEANS|-"
  "Chromium is the CEILING on capability, FLOOR on everything else|STATUS.md|NORTH STAR|-"
  "the oracle's scope is the ceiling|STATUS.md|ORACLE'S SCOPE IS THE CEILING|-"
  "media is tick-sized; MSE comes after; never advertise MediaSource early|docs/loop/MEDIA.md|MediaSource|-"
  "cross-platform: warm-fork Linux-only, plain spawn elsewhere|docs/loop/PROCESS-MODEL.md|CROSS-PLATFORM|-"
  "hibernation exemptions: audio/video, WebSocket/SSE|docs/loop/PROCESS-MODEL.md|non-negotiable exemptions|-"
)
for entry in "${CRITICAL_DECISIONS[@]}"; do
  IFS='|' read -r what file pat alt <<< "$entry"
  if grep -qis -- "$pat" "$file" 2>/dev/null; then
    ok "$what  → $file"
  elif [ "$alt" != "-" ] && grep -qis -- "$pat" "$alt" 2>/dev/null; then
    ok "$what  → $alt"
  else
    bad "NOT RECOVERABLE FROM FILES: \"$what\" (looked for /$pat/ in $file)"
  fi
done

# ── 5. What do I do next? A cold session that knows the state but not the next action will invent
#       one — and an invented next action is how a loop drifts.
echo
echo "Q5. What do I do next?"
if grep -qsE "SEVEN META-INSTRUMENTS|PLATFORM MAP|NEXT" STATUS.md; then
  ok "STATUS.md names the ordered next work (meta-instruments / platform map)"
else
  bad "STATUS.md does not say what comes next — a cold session will pick something plausible instead"
fi

# ── 6. Are the mechanisms themselves still on disk? The constellation IS the memory; a missing
#       file here is amnesia, not an inconvenience.
echo
echo "Q6. Are the mechanisms still on disk?"
for f in CLAUDE.MD STATUS.md docs/loop/JOURNAL.md docs/loop/PROCESS.md docs/loop/CAPABILITIES.md \
         docs/loop/PROCESS-MODEL.md docs/loop/MEDIA.md scripts/verify.sh scripts/falsify.sh; do
  [ -f "$f" ] && ok "$f" || bad "MISSING: $f"
done

echo
if [ "$fail" -eq 0 ]; then
  printf '\033[32m\033[1mCOLD READ: the constellation is self-sufficient.\033[0m A session with zero memory recovers the project.\n\n'
  exit 0
fi
printf '\033[31m\033[1mCOLD READ: %d gap(s).\033[0m\n' "$fail"
echo "  Each one is a thing this project KNOWS but a fresh session CANNOT LEARN."
echo "  Fix the FILE, not the memory — the memory is gone at the next compaction either way."
echo
exit 1
