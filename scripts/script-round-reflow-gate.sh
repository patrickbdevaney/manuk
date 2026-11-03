#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a.a.b.b.a.a.aa.a: every manuk_js entry that runs page JavaScript in
# engine/page/src/lib.rs must sit inside a function that installs a ReflowScope before the call (so a
# geometry read of a node the script just built sees real layout), or be on the JUSTIFIED list below.
#
# usage: scripts/script-round-reflow-gate.sh [path/to/lib.rs]
#   default path is engine/page/src/lib.rs. Pass `git show HEAD~N:engine/page/src/lib.rs > /tmp/x.rs`
#   to check an older tree: that must go RED (it predates the four fixes).
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
SRC="${1:-engine/page/src/lib.rs}"

# Entries with no fonts / viewport in scope, or whose only callers are host or test paths. Each one is a
# named follow-up, not a silent exemption: R0.4.a.c.a.b.a.a.a.a.a.a.b.b.a.a.aa.a.a.
JUSTIFIED="NONE_EXEMPT"  # empty list (R0.4.a.c.a.b.a.a.a.a.a.a.b.b.a.a.aa.a.a.a): a fn passes only by installing a scope or taking `&ReflowScope`

uncovered=$(awk '
  /^[ \t]*(pub(\([a-z]+\))? )?(async )?fn / { fname=$0; installed=(fname ~ /: &ReflowScope/) }
  /ReflowScope::install/ { installed=1 }
  /manuk_js::(dispatch_[a-z_]*|eval_in_page|resolve_fetch[a-z_]*|deliver_[a-z_]*|fire_[a-z_]*|run_deferred_scripts)\(/ {
    if ($0 ~ /^[ \t]*\/\//) next
    if (!installed) { gsub(/^[ \t]+/, "", fname); printf "%d\t%s\n", NR, substr(fname, 1, 80) }
  }
' "$SRC")

# Only the function names matter for the justified list; a call inside a justified fn is allowed.
bad=$(printf '%s\n' "$uncovered" | grep -v '^$' | grep -vE "fn ($JUSTIFIED)\(" || true)
total=$(printf '%s\n' "$uncovered" | grep -c . || true)
echo "uncovered JS entry calls: $total (justified: $((total - $(printf '%s\n' "$bad" | grep -c . || true))))" >&2
if [ -n "$bad" ]; then
  echo "red: JS entry without a ReflowScope and not justified:" >&2
  printf '%s\n' "$bad" >&2
  exit 1
fi
echo "green: every unguarded JS entry is on the justified list ($JUSTIFIED)" >&2
