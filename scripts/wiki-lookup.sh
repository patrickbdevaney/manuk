#!/usr/bin/env bash
# ── WIKI LOOKUP: precise, DETERMINISTIC retrieval from docs/wiki — no embeddings, no semantic garbage.
#
# The wiki is where the engine's hard-won knowledge accumulates (SpiderMonkey FFI, Stylo/Taffy, DOM/CSS
# spec reality, the Rust engine's own patterns). It is only worth accumulating if it can be RETRIEVED —
# by a future tick, by the agent-driving horizon, by whoever builds the security or embedded species — and
# retrieved *precisely*: the exact section that names the symbol or mechanism you asked about, not a fuzzy
# neighbourhood of it.
#
# So retrieval is keyword/symbol grep over `## `-delimited sections, ranked by how many of your terms a
# section actually contains. Deterministic, auditable, zero-dependency. A query for `reflector SLOT_DOM`
# returns the sections that mention *both*, top first — the real ones, and nothing adjacent-but-wrong.
#
#   scripts/wiki-lookup.sh reflector arena              # sections about reflector arena resolution
#   scripts/wiki-lookup.sh JSAutoRealm segfault         # the realm-missing SIGSEGV mechanism
#   scripts/wiki-lookup.sh innerText display:none       # the rendered-text rule
#   scripts/wiki-lookup.sh --full <terms...>            # print whole matching sections, not a preview
set -uo pipefail
cd "$(dirname "$0")/.."

FULL=0
if [ "${1:-}" = "--full" ]; then FULL=1; shift; fi
[ $# -eq 0 ] && { echo "usage: wiki-lookup.sh [--full] <term> [term...]" >&2; exit 2; }

python3 - "$FULL" "$@" <<'PY'
import sys, glob, re
full = sys.argv[1] == "1"
terms = [t.lower() for t in sys.argv[2:]]
results = []
for path in sorted(glob.glob('docs/wiki/*.md')):
    text = open(path, encoding='utf-8').read()
    parts = re.split(r'(?m)^(#{1,3} .+)$', text)   # split on any heading, keep it
    i = 1
    while i < len(parts):
        head = parts[i].strip()
        body = parts[i + 1] if i + 1 < len(parts) else ''
        blob = (head + '\n' + body).lower()
        matched = sum(1 for t in terms if t in blob)   # DISTINCT query terms present
        hits = sum(blob.count(t) for t in terms)        # total occurrences (tiebreak)
        if matched:
            results.append((matched, hits, path, head, body.rstrip()))
        i += 2
results.sort(key=lambda r: (-r[0], -r[1]))
if not results:
    print("no wiki section matches:", " ".join(terms))
    sys.exit(0)
shown = results if full else results[:8]
for matched, hits, path, head, body in shown:
    print(f"\n═══ {path}  [{matched}/{len(terms)} terms · {hits} hits]")
    print(head)
    lines = body.splitlines()
    for ln in (lines if full else lines[:14]):
        print("  " + ln)
    if not full and len(lines) > 14:
        print(f"  … (+{len(lines) - 14} more lines — rerun with --full)")
if not full and len(results) > 8:
    print(f"\n… {len(results) - 8} more matching sections (narrow the query or use --full).")
PY
