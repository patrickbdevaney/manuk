#!/usr/bin/env bash
# ── OPEN THE APERTURE: add a WPT area to the sparse checkout so it can be measured at all.
#
# An area that is not checked out scores zero and is INVISIBLE — it cannot appear in any ranking, so no
# amount of careful work inside the current aperture will ever find it. `scripts/blindspot.sh` names them;
# this makes one real.
#
#   scripts/wpt-expand.sh html/semantics
set -euo pipefail
: "${WPT_DIR:=$HOME/wpt}"
[ $# -ge 1 ] || { echo "usage: scripts/wpt-expand.sh <wpt-area> [more...]" >&2; exit 2; }
CUR=$(git -C "$WPT_DIR" sparse-checkout list | tr '\n' ' ')
echo "── before: $CUR"
git -C "$WPT_DIR" sparse-checkout add "$@"
echo "── after : $(git -C "$WPT_DIR" sparse-checkout list | tr '\n' ' ')"
N=$(find "$WPT_DIR/$1" -name '*.html' 2>/dev/null | wc -l)
echo "── $1 now materialised: $N test files"
echo
echo "Next: add it to AREAS in scripts/wpt-sweep.sh, then ./scripts/orient.sh — the work list has changed."
