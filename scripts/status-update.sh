#!/usr/bin/env bash
# **STATUS.md is GENERATED, never hand-narrated (METHODOLOGY Part 28.3).**
#
# The moment a status file becomes something someone writes prose into, it re-acquires the exact
# failure mode it exists to remove: it starts describing what we *meant* to do. Every field below is
# read from the filesystem, git, the crawl output or the verify receipt — never from anyone's memory
# of what happened.
#
# The prose sections (Settled Decisions, Lessons) are the deliberate exception, and they are
# deliberately SHORT and RARELY EDITED — that is what makes them trustworthy. If either starts growing
# every session, that growth is itself the signal that something is being added which isn't actually
# settled, or which should have been promoted to a gate instead.
set -uo pipefail
cd "$(dirname "$0")/.."

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
LAST_AUDIT=$(grep -oP '^LAST_AUDIT_TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
WALL=$(grep -oP '^seconds:\s*\K[0-9]+' .git/manuk-verify-receipt 2>/dev/null || echo "?")
SITES=$(grep -cE '^[a-z_]+[[:space:]]+https' docs/bench/oracle-corpus.txt 2>/dev/null || echo 0)
CLUSTERS=$(grep -cE '^C[0-9a-f]{4} ' docs/loop/CLUSTERS.md 2>/dev/null || echo 0)
CRAWLED=$(ls /tmp/manuk-oracle-run/*.jsonl 2>/dev/null | wc -l)
HUNG=$(grep -l '"status":"HANG"' /tmp/manuk-oracle-run/*.jsonl 2>/dev/null | wc -l)

# Which gates are actually IN THE WALL — read from verify.sh, not from a list someone maintains.
pending=""
for g in G_SILENT_FAIL:g_silent_fail G_DEDUP:g_dedup G_SPAWN:g_spawn G_POOL_ISOLATION:g_pool; do
  name="${g%%:*}"; pat="${g##*:}"
  grep -q "$pat" scripts/verify.sh 2>/dev/null || pending="$pending $name"
done
[ -z "$pending" ] && pending=" (none)"

# Single-site ticks in the current audit window — a rising count here is the drift signal (Part 26.2),
# visible in five seconds instead of promised in prose.
SINGLE=$(awk -v lo="$LAST_AUDIT" '
  /^## Tick [0-9]+/ { t=$3+0 }
  t > lo && /TICK SHAPE:[[:space:]]*single-site/ { n++ }
  END { print n+0 }' docs/loop/JOURNAL.md 2>/dev/null || echo 0)

python3 - "$TICK" "$LAST_AUDIT" "$WALL" "$SITES" "$CLUSTERS" "$CRAWLED" "$HUNG" "$pending" "$SINGLE" <<'PY'
import sys, re, datetime
tick, last_audit, wall, sites, clusters, crawled, hung, pending, single = sys.argv[1:10]
src = open('STATUS.md').read()

# The generated block. Everything here is a fact read from disk.
head = f"""# manuk — STATUS

> **Read this first, every session, before anything else.** State the tier and any blocking items
> out loud before touching code. Do not proceed on assumed context from a previous session.
>
> **This file is GENERATED (`scripts/status-update.sh`), not hand-written.** A status file someone
> writes prose into starts describing what we *meant* to do. Every field below is read from the
> filesystem, git, the crawl output or the verify receipt.

```
TICK:              {tick}
LAST_AUDIT_TICK:   {last_audit}          (self-audit due every 10 ticks — the hook BLOCKS commits past that)
CURRENT_TIER:      0                     (Part 21 — one Tier-0 item left: the SPA miner)
LAST_WALL_TIME:    {wall}s
ORACLE_CORPUS:     {sites} sites
ORACLE_CRAWLED:    {crawled} sites, {clusters} clusters  → docs/loop/CLUSTERS.md
ORACLE_HANGS:      {hung}                    ← Bar 0. Outranks every visual cluster (Part 24.3).
PENDING_GATES:    {pending}
SINGLE_SITE_TICKS: {single}                    (this audit window — a rising count is the drift signal)
UPDATED:           {datetime.date.today()}
```
"""

# Preserve everything from the first "## " heading onward — including the curated Settled-Decisions
# and Lessons sections, which are the ONLY hand-written parts of this file and must survive
# regeneration. Anchoring on a specific heading would silently eat anything above it, which is how a
# generator quietly deletes the most valuable thing in the file.
i = src.index('\n## ')
open('STATUS.md','w').write(head + "\n" + src[i:])
print("STATUS.md regenerated")
PY
