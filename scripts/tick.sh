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
#
# ⚠ **Pure awk, NOT `awk … | grep -q`.** Under `set -o pipefail`, `grep -q` exits on the first match and
# closes the pipe; `awk`, still streaming the rest of the (now 1300+-line) tick block, takes SIGPIPE and
# exits 141, and pipefail reports that as the pipeline's status — so the check `die`d on a journal that
# *did* contain the shape. It failed intermittently (foreground timing sometimes let awk finish first) and
# got steadily worse as the journal grew. Doing the whole match inside one awk process removes the pipe.
awk -v t="$TICK" '
  $0 ~ "^## Tick " t "[^0-9]" {f=1}
  f && /TICK SHAPE:/ {found=1}
  END {exit found?0:1}
' docs/loop/JOURNAL.md \
  || die "tick ${TICK}'s journal entry has no 'TICK SHAPE:' — a tick that cannot name its shape is drifting"
ok "tick shape declared"

# 3. The WIKI trailer. "none" is a legitimate answer for a docs/scripts/mechanism tick; a SILENT gap is not.
grep -qiE '^WIKI:' "$MSG" \
  || die "commit message has no 'WIKI:' trailer — name the topic file, or 'WIKI: none — <why>'"
# **Engine ticks MUST accumulate.** If the tick changed engine SOURCE, "none" is not enough — it must
# revise a docs/wiki/*.md topic (the knowledge the downstream horizons need and cannot get from a diff),
# unless it declares an explicit, auditable `WIKI: none [forced] — <reason>`. Checked here so it fails in
# one second, not after the wall. The authoritative copy of this rule is in scripts/hooks/pre-commit.
ENGINE_CHG=$(git diff --cached --name-only 2>/dev/null | grep -cE '^engine/[a-z0-9]+/src/' || true)
[ "$ENGINE_CHG" -eq 0 ] && ENGINE_CHG=$(git diff --name-only 2>/dev/null | grep -cE '^engine/[a-z0-9]+/src/' || true)
WIKI_CHG=$(git status --short docs/wiki/ 2>/dev/null | grep -cE '\.md$' || true)
if [ "${ENGINE_CHG:-0}" -gt 0 ] && [ "${WIKI_CHG:-0}" -eq 0 ] && ! grep -qiE '^WIKI:[[:space:]]*none[[:space:]]*\[forced\]' "$MSG"; then
  die "engine source changed but no docs/wiki/*.md was revised — capture the mechanism (scripts/wiki-index.sh maps it), or 'WIKI: none [forced] — <why>'"
fi
ok "wiki trailer present"

# 3b. The wiki index must be current, so the map never lies about what is retrievable.
./scripts/wiki-index.sh --check >/dev/null 2>&1 || { ./scripts/wiki-index.sh >/dev/null 2>&1; printf '  %s⚠%s regenerated docs/wiki/INDEX.md\n' "$YEL" "$OFF"; }
ok "wiki index current"

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

# 5b. ── THE SURFACE AUDIT. Every 10 ticks the loop must LEAVE ITS OWN FRAME.
#
# Every other instrument measures the browser against a map. Nothing measured the MAP — and the map was
# drawn from memory, which has been wrong six times. Twice this project made an order-of-magnitude leap and
# BOTH times a human had to point at it, because every instrument the loop owned could only see what was
# already on its map. This is the instrument that checks the map.
./scripts/surface-audit.sh check || die "surface audit overdue — run ./scripts/surface-audit.sh run"

# The constitution check. surface-audit checks the MAP against the world; this checks the loop's DIRECTION
# against the long horizon (CONSTITUTION.MD). It exists because tick 84 climbed the encoding tail — a real
# +721k that was the wrong hill — and no instrument read the document that defines the frontier.
./scripts/constitution-check.sh check || die "constitution check overdue — run ./scripts/constitution-check.sh run"

# 6. Formatting — a one-second check that CI would otherwise fail on minutes later.
cargo fmt --all --check >/dev/null 2>&1 || { cargo fmt --all >/dev/null 2>&1; printf '  %s⚠%s reformatted (cargo fmt)\n' "$YEL" "$OFF"; }
ok "fmt clean"

# 7. ── THE RATCHET. It refuses the tick; it does not advise it.
#
# This is the mechanism the first principle never had. "Never regress capability, performance or stability"
# has been written in CLAUDE.md since tick 1 — and tick 82 landed +9,940 WPT subtests while quietly losing
# 2 in an area it was not looking at, and tick 80 shipped while the wall was red. **A rule I can recite
# while breaking it is a decoration.** So it is a gate now, and it runs BEFORE the wall, because a
# regression is a one-second check and the wall is a four-minute one.
if [ -f docs/loop/WPT-AREAS.tsv ]; then
  ./scripts/ratchet.sh check || die "the RATCHET refuses this tick — something went backwards"
else
  printf '  %s⚠%s no WPT sweep on record — run ./scripts/wpt-sweep.sh (a tick that did not measure cannot claim it did not regress)\n' "$YEL" "$OFF"
fi

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

# The marks only ever RISE. `bank` takes max(mark, current), so a regression cannot be laundered into the
# baseline by re-running it, and a bad day cannot lower the bar for a good one.
./scripts/ratchet.sh bank || true
if ! git diff --quiet docs/loop/CADENCE.tsv docs/loop/CADENCE.md 2>/dev/null; then
  git add docs/loop/CADENCE.tsv docs/loop/CADENCE.md
  git commit -q --amend --no-edit --no-verify
  git push -qf origin main
fi

printf '\n%sTick %s landed.%s CI runs asynchronously — read it at the START of the next tick, do not wait on it.\n' \
  "$GRN$BLD" "$TICK" "$OFF"
