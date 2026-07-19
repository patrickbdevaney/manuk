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
# ── ONLY BANK A WALL FROM A GREEN RUN (observer, tick 235). This read the receipt's `seconds:`
# unconditionally, so a verify that DIED EARLY banked its short runtime as the wall — and since the
# ratchet now judges STATUS.md's LAST_WALL_TIME immediately after this runs, that made "fail fast"
# the cheapest way to satisfy the wall gate. A gate you can pass by crashing is not a gate. Real
# case that exposed it: a verify aborted at 3s on a half-written manifest and wrote `seconds: 3`.
# On a non-green receipt, KEEP the previous value rather than inventing a flattering one.
_RES=$(grep -oP '^result:\s*\K\w+' .git/manuk-verify-receipt 2>/dev/null || echo "")
if [ "$_RES" = "green" ]; then
  WALL=$(grep -oP '^seconds:\s*\K[0-9]+' .git/manuk-verify-receipt 2>/dev/null || echo "?")
else
  WALL=$(grep -oP '^LAST_WALL_TIME:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo "?")
fi
SITES=$(grep -cE '^[a-z_]+[[:space:]]+https' docs/bench/oracle-corpus.txt 2>/dev/null || echo 0)
CLUSTERS=$(grep -cE '^C[0-9a-f]{4} ' docs/loop/CLUSTERS.md 2>/dev/null || echo 0)
CRAWLED=$(ls /tmp/manuk-oracle-run/*.jsonl 2>/dev/null | wc -l)
# **A results directory containing TWO runs is not a measurement.**
#
# `oracle-crawl.sh` does `export -f one_site`, so its xargs workers survive a `pkill` of the driver and
# keep writing — with the watchdog and the semantics they were forked with — into whatever directory is
# there. That happened, and it produced a run with 113 records from one experiment and 39 from another.
# It was caught only because the two scripts happened to use different status labels and the totals did
# not add up; had I merely changed the watchdog and not the label, the numbers would have merged
# silently and I would have believed them.
MIXED=$(python3 - <<'PYEOF'
import json, glob
runs = set()
labels = set()
legacy = 0
for f in glob.glob('/tmp/manuk-oracle-run/*.jsonl'):
    for line in open(f):
        try: d = json.loads(line)
        except Exception: continue
        if d.get('kind') != 'meta': continue
        labels.add(d.get('status'))
        if 'run' in d:
            runs.add(d['run'])
        elif d.get('status') in ('HANG', 'TIMEOUT', 'FAIL', 'DISCARDED'):
            legacy += 1
# `HANG` is the OLD script's label for a process timeout; `TIMEOUT` is the current one. Both present in
# one directory means two script versions wrote it — the exact signature of the straggler bug, and the
# only reason it was caught at all.
mixed_labels = 'HANG' in labels and 'TIMEOUT' in labels
print(len(runs) + (1 if legacy else 0) + (1 if mixed_labels else 0))
PYEOF
)
if [ "${MIXED:-0}" -gt 1 ]; then
  echo "REFUSING: /tmp/manuk-oracle-run contains results from $MIXED DIFFERENT crawl runs." >&2
  echo "  That is not a measurement. Kill any stragglers, wipe it, and re-crawl:" >&2
  echo "    pkill -9 -f 'manuk-wpt oracle'; rm -rf /tmp/manuk-oracle-run" >&2
  exit 1
fi

# **Bar 0 is measured on OUR clock — `manuk_ms` — not on the oracle process's.**
#
# It used to be `grep -l '"status":"HANG"'`, i.e. "the oracle process hit its watchdog". That process
# runs Chromium too, and on this corpus Chromium is the slower engine on most news sites (bloomberg
# 60.6s vs our 15.5s; vox 59.7s vs 7.6s; economist 53.9s vs 15.1s). So the metric that has been driving
# the whole schedule — "73 of 265 sites hang" — was substantially measuring *Chromium's* time and
# booking it against us.
#
# A hang is now: OUR engine did not produce a rendered page within MANUK_HANG_MS. That is a claim about
# this browser, which is what Bar 0 is a claim about.
HANG_MS="${MANUK_HANG_MS:-30000}"
HUNG=$(python3 - "$HANG_MS" <<'PYEOF'
import json, glob, sys
limit = int(sys.argv[1])
n = 0
for f in glob.glob('/tmp/manuk-oracle-run/*.jsonl'):
    for line in open(f):
        try: d = json.loads(line)
        except Exception: continue
        if d.get('kind') != 'meta':
            continue
        # Our own clock, when we have it.
        if 'manuk_ms' in d and d['manuk_ms'] > limit:
            n += 1
        # A process timeout is UNATTRIBUTED — it is counted separately, never as ours.
print(n)
PYEOF
)
UNATTRIB=$(grep -l '"status":"TIMEOUT"' /tmp/manuk-oracle-run/*.jsonl 2>/dev/null | wc -l)

# **A PARTIAL crawl is not a measurement, and must never be printed as one.**
#
# This bit exactly once and it is worth the six lines. A crawl was killed mid-flight; the results
# directory kept the ~90 sites that had finished; and this script cheerfully wrote `ORACLE_HANGS: 33`
# into the file whose entire purpose is to contain only facts. A number sitting in STATUS.md is
# indistinguishable from a number somebody stands behind — that is what the file is FOR — so a partial
# run has to announce itself rather than quietly under-report.
#
# Under-reporting is the dangerous direction, too: an interrupted crawl always shows FEWER hangs than
# the real number, because the sites that hang are the ones still running when you kill it.
if [ "$CRAWLED" -lt "$SITES" ]; then
  HUNG="$HUNG?"
  CRAWLED="$CRAWLED (PARTIAL — of $SITES; this run did not finish, so the hang count is a FLOOR, not a number)"
fi

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

# The surface-audit cadence field must SURVIVE this regeneration — surface-audit.sh reads it from
# STATUS.md, and a generated file that drops it resets the cadence to tick 0 every commit (which blocked
# tick 84 until this was fixed). It is not hand-state: derive it from the audit LOG, the max tick any
# audit was recorded under. That cannot drift, because the log is append-only.
SURFACE=$(grep -oP '^## Audit #[0-9]+ — tick \K[0-9]+' docs/loop/SURFACE-AUDIT.md 2>/dev/null | sort -n | tail -1)
[ -z "$SURFACE" ] && SURFACE=0
# LAST_CONSTITUTION_CHECK — same survival requirement: constitution-check.sh reads it from STATUS.md, and
# a generated file that drops it resets the anchor to tick 0. Derive it from the check LOG (append-only).
CONST=$(grep -oP '^## Check #[0-9]+ — tick (?:[0-9]+/)?\K[0-9]+' docs/loop/CONSTITUTION-CHECK.md 2>/dev/null | sort -n | tail -1)
[ -z "$CONST" ] && CONST=0
# The loop budget — the operator's tick dial, read from disk so it survives context compaction.
WALL_AUDIT=$(grep -oP '^## Audit #[0-9]+ — tick \K[0-9]+' docs/loop/WALL-AUDIT.md 2>/dev/null | sort -n | tail -1)
[ -z "$WALL_AUDIT" ] && WALL_AUDIT=0
LOOP_TGT=$(grep -oP '^LOOP_UNTIL_TICK=\K[0-9]+' docs/loop/AUTOLOOP 2>/dev/null || echo 0)
LOOP_REM=$(( LOOP_TGT - TICK )); [ "$LOOP_REM" -lt 0 ] && LOOP_REM=0

python3 - "$TICK" "$LAST_AUDIT" "$WALL" "$SITES" "$CLUSTERS" "$CRAWLED" "$HUNG" "$pending" "$SINGLE" "$UNATTRIB" "$SURFACE" "$CONST" "$LOOP_REM" "$LOOP_TGT" "$WALL_AUDIT" <<'PY'
import sys, re, datetime
tick, last_audit, wall, sites, clusters, crawled, hung, pending, single, unattrib, surface, const, loop_rem, loop_tgt, wall_audit = sys.argv[1:16]
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
LAST_SURFACE_AUDIT: {surface}         (surface audit due every 10 ticks — from docs/loop/SURFACE-AUDIT.md)
LAST_CONSTITUTION_CHECK: {const}     (constitution re-read due every 8 ticks — from docs/loop/CONSTITUTION-CHECK.md; anchors the loop to CONSTITUTION.MD)
LOOP_BUDGET:       {loop_rem} ticks remaining (target tick {loop_tgt}) — from docs/loop/AUTOLOOP; the loop STOPS and reports at 0
LAST_WALL_AUDIT:   {wall_audit}         (wall-time audit due every 20 ticks — scripts/wall-audit.sh; hunts wall bloat without cutting a gate)
CURRENT_TIER:      0                     (Part 21 — one Tier-0 item left: the SPA miner)
LAST_WALL_TIME:    {wall}s
ORACLE_CORPUS:     {sites} sites
ORACLE_CRAWLED:    {crawled} sites, {clusters} clusters  → docs/loop/CLUSTERS.md
ORACLE_HANGS:      {hung}   ← Bar 0, on OUR clock (manuk_ms > 30s). Outranks every visual cluster.
ORACLE_UNATTRIB:   {unattrib}   ← oracle process hit its watchdog. Whose time? UNKNOWN — never ours by default.
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
