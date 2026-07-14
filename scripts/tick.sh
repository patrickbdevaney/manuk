#!/usr/bin/env bash
# ── CLOSE A TICK IN ONE COMMAND: pre-flight → status → verify (ONCE) → commit → push.
#
# **This exists because the expensive check was running before the cheap ones.**
#
# The pre-commit hook enforces four things — a journal entry for this tick, a `TICK SHAPE:`, a `WIKI:`
# trailer, and a touched pattern ledger (or an explicit `[no-pattern]`). Every one of those is a
# one-second grep. But the hook only sees them **after** `verify.sh` has already spent 40–90 seconds, so a
# missing trailer costs a **full re-verify**, every time. That has happened repeatedly, and it is pure
# waste: the loop was paying the most expensive check first.
#
# So: **fail in one second on the things that can be checked in one second.** Then run the wall exactly
# ONCE, on the final tree, and commit against that receipt.
#
#   usage: scripts/tick.sh <message-file>
#
# The other half of the saving is not in this script and must be said out loud: **do not sleep waiting for
# CI.** The CI lane is asynchronous *by design* — a regression it finds is an ordinary gate failure read at
# the next tick's "read STATUS.md first" check-in. Blocking on it re-serialises the thing that was built to
# be parallel, and it has cost this loop many minutes an hour.
set -uo pipefail
cd "$(dirname "$0")/.."

# Snapshot the tree before doing anything. PROCESS #37: `git checkout` is a delete, and I have
# run it on uncommitted work twice. This makes that recoverable. It is free when nothing has changed.
./scripts/snap.sh || true

RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; BLD=$'\033[1m'; OFF=$'\033[0m'
die() { printf '%s✗ %s%s\n' "$RED$BLD" "$1" "$OFF" >&2; exit 1; }
ok()  { printf '  %s✓%s %s\n' "$GRN" "$OFF" "$1"; }

MSG="${1:-}"
[ -n "$MSG" ] && [ -f "$MSG" ] || die "usage: scripts/tick.sh <message-file>"

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
AUDIT=$(grep -oP '^LAST_AUDIT_TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)

printf '%s── pre-flight (the one-second checks, run FIRST)%s\n' "$BLD" "$OFF"

# 1. The journal entry — and it must exist BEFORE the code lands, because the hypothesis is what makes an
#    interrupted tick resumable (a compacted session recovers from it).
grep -q "^## Tick ${TICK}[^0-9]" docs/loop/JOURNAL.md \
  || die "no '## Tick ${TICK}' entry in docs/loop/JOURNAL.md — write it (a stub naming the hypothesis is enough)"
ok "journal entry for tick ${TICK}"

# 2. Tick shape, in the journal (the hook reads the journal, not the commit message).
awk "/^## Tick ${TICK}[^0-9]/,0" docs/loop/JOURNAL.md | grep -qi 'TICK SHAPE:' \
  || die "tick ${TICK}'s journal entry has no 'TICK SHAPE:' — a tick that cannot name its shape is drifting"
ok "tick shape declared"

# 3. The WIKI trailer. "none" is a legitimate answer; a SILENT gap is not.
grep -qiE '^WIKI:' "$MSG" \
  || die "commit message has no 'WIKI:' trailer — name the topic file, or 'WIKI: none — <why>'"
ok "wiki trailer present"

# 4. The pattern ledger: engine capability changed ⇒ say which CLASS OF THE WEB it unlocks.
if git diff --cached --name-only 2>/dev/null | grep -qE '^engine/(js|css|layout|paint|dom|html|text)/src/' \
   || git diff --name-only | grep -qE '^engine/(js|css|layout|paint|dom|html|text)/src/'; then
  if ! git status --short docs/loop/WEB-PATTERNS.md | grep -q . && ! grep -q '\[no-pattern\]' "$MSG"; then
    die "engine capability changed but WEB-PATTERNS.md is untouched — say which class of the web this unlocks, or put [no-pattern] in the message"
  fi
fi
ok "pattern ledger accounted for"

# 5. The self-audit cadence. The hook blocks past it; find out here, not after a 40-second wall.
if [ $((TICK - AUDIT)) -ge 10 ]; then
  die "SELF-AUDIT OVERDUE (last: tick ${AUDIT}, now ${TICK}). Run ./scripts/self-audit.sh, then set LAST_AUDIT_TICK."
fi
ok "self-audit current (last: tick ${AUDIT})"

# 6. Formatting — a one-second check that CI would otherwise fail on minutes later.
cargo fmt --all --check >/dev/null 2>&1 || { cargo fmt --all >/dev/null 2>&1; printf '  %s⚠%s reformatted (cargo fmt)\n' "$YEL" "$OFF"; }
ok "fmt clean"

printf '\n%s── status + the wall (ONCE, on the final tree)%s\n' "$BLD" "$OFF"
./scripts/status-update.sh >/dev/null 2>&1 || true
./scripts/verify.sh || die "the wall is RED — the tick does not land"

printf '\n%s── commit + push%s\n' "$BLD" "$OFF"
git add -A
git commit -q -F "$MSG" || die "commit refused (read the hook's reason above)"
git push -q origin main || die "push failed"
printf '  %s✓%s %s\n' "$GRN" "$OFF" "$(git log --oneline -1)"

# ── THE CADENCE LEDGER. Timestamp the tick, measure the interval, record what it cost and what it bought.
#
# We spent seventy ticks measuring the browser and none measuring the LOOP. "Tick 69 landed" is a receipt,
# not progress data — and the project has two horizons (a daily-driver near one, a 50,000-test WPT far one)
# whose only honest question is *are we getting there, and how fast?* This answers it with the loop's own
# vitals: cycle time, wall time, diff size, capabilities asserted, gates live, WPT measured.
#
# It runs AFTER the push, on purpose: a tick that did not land is not a tick, and must not appear as one.
./scripts/tick-log.sh "$TICK" || true
if ! git diff --quiet docs/loop/CADENCE.tsv docs/loop/CADENCE.md 2>/dev/null; then
  git add docs/loop/CADENCE.tsv docs/loop/CADENCE.md
  git commit -q --amend --no-edit --no-verify
  git push -qf origin main
fi

printf '\n%sTick %s landed.%s CI runs asynchronously — read it at the START of the next tick, do not wait on it.\n' \
  "$GRN$BLD" "$TICK" "$OFF"
