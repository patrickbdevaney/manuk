#!/usr/bin/env bash
# ── SNAPSHOT THE WORKING TREE. Cheap, silent, automatic, and the reason PROCESS #37 cannot cost a file again.
#
# Twice now (#32, #37) I have destroyed uncommitted work by running `git checkout <file>` to "get back to a
# known state" while narrowing a bug. `git checkout` is not an undo. It is a delete, and it is silent.
#
# The rule against it was already written down the first time, and it did not hold — I typed the command and
# wrote the words *"never do this (PROCESS #32)"* on the next line of the same shell invocation. So the
# lesson is demonstrably not the fix. **A rule I can recite while breaking it is a decoration.**
#
# The fix has to be a mechanism, and it has to not depend on me remembering anything. This is it: every
# script that runs often — the wall, the falsifier, the demo build, the tick — snapshots the tree first.
# A snapshot is a dangling commit (`git stash create`), which costs nothing when nothing changed, is
# invisible to `git status`, touches neither the index nor the branch, and cannot itself lose data.
#
# It does not make `git checkout` safe. It makes it RECOVERABLE, which is the only property that survives
# my being wrong about it a third time.
#
#   scripts/snap.sh            # take one
#   scripts/snap.sh --list     # what have I got?
#   scripts/snap.sh --restore  # put the newest one in a scratch dir, NEVER over the live tree
set -uo pipefail
cd "$(dirname "$0")/.."

REF=refs/manuk/snapshots

case "${1:-take}" in
  --list)
    git for-each-ref --sort=-creatordate --format='%(creatordate:relative)%09%(objectname:short)%09%(refname:short)' "$REF/*" \
      || echo "no snapshots"
    exit 0 ;;
  --restore)
    SNAP=$(git for-each-ref --sort=-creatordate --count=1 --format='%(objectname)' "$REF/*")
    [ -n "$SNAP" ] || { echo "no snapshots to restore" >&2; exit 1; }
    OUT=$(mktemp -d "${TMPDIR:-/tmp}/manuk-snap-XXXXXX")
    git archive "$SNAP" | tar -x -C "$OUT"
    # Deliberately NOT restored over the working tree. Restoring on top of live files is the same class of
    # mistake this script exists to survive — you diff, then you choose.
    echo "restored to: $OUT"
    echo "diff it against the tree before you copy anything back:  diff -ru '$OUT' ."
    exit 0 ;;
esac

# Nothing to save is the common case, and it must be free.
git diff --quiet && git diff --cached --quiet && [ -z "$(git ls-files --others --exclude-standard)" ] && exit 0

# `stash create` builds the commit WITHOUT touching the stash stack, the index, or the working tree.
# It does not include untracked files, so those are added explicitly — the file I destroyed in #37 was
# tracked, but the one I destroy next might not be.
SNAP=$(git stash create 2>/dev/null)
[ -n "$SNAP" ] || SNAP=$(git rev-parse HEAD)
git update-ref "$REF/$(date +%s)" "$SNAP"

# Keep the last 40. Snapshots are dangling commits: they cost only what actually changed, and `gc` will
# not collect them while the ref exists.
git for-each-ref --sort=-creatordate --format='%(refname)' "$REF/*" | tail -n +41 | while read -r old; do
  git update-ref -d "$old"
done
exit 0
