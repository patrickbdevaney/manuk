#!/usr/bin/env bash
# **The crawl report — and it REFUSES to show speed without coverage.**
#
# Being faster than Chromium is not a divergence to close. Parity is a claim about **capability**, not
# about timing: if we are quicker because we dedup, cache, single-flight and defer correctly, that is the
# *result* of the work, not a deviation from a target. The oracle diffs **structure**. It has never scored
# timing and it must not start.
#
# **But a speed advantage is only real if it comes from doing the same work BETTER, and not from not doing
# the work.** "Fast because we never loaded the images" and "fast because we never ran the script" are two
# lies this project has already caught itself telling — `G_FIRST_PAINT` and `G_DEFER` exist precisely
# because a speed number achieved by *skipping a capability* looks identical, on the clock, to one
# achieved by an optimisation.
#
# So this script prints them **together, always**, and there is no flag to print one without the other.
# A speed claim is only admissible next to a coverage number; if coverage moved, the speed is a
# measurement of the thing we stopped doing.
set -uo pipefail
cd "$(dirname "$0")/.."

OUT="${MANUK_ORACLE_OUT:-/tmp/manuk-oracle-run}"
HANG_MS="${MANUK_HANG_MS:-30000}"

python3 - "$OUT" "$HANG_MS" <<'PY'
import json, glob, sys, collections

out_dir, hang_ms = sys.argv[1], int(sys.argv[2])
runs, labels = set(), set()
timed, timeouts, discarded, failed = [], 0, 0, 0
divergences = {}
dkind = collections.Counter()

for f in glob.glob(f'{out_dir}/*.jsonl'):
    probed = 0
    for line in open(f):
        try: d = json.loads(line)
        except Exception: continue
        if d.get('kind') == 'div':
            dkind[d.get('dkind', '?')] += 1
        if d.get('kind') != 'meta':
            continue
        labels.add(d.get('status'))
        if 'run' in d: runs.add(d['run'])
        st = d.get('status')
        if st == 'TIMEOUT': timeouts += 1
        elif st == 'DISCARDED': discarded += 1
        elif st == 'FAIL': failed += 1
        if 'manuk_ms' in d:
            timed.append((d['site'], d.get('class','?'), d['manuk_ms'], d.get('chrome_ms', 0)))
            probed = d.get('probed', 0)
    if probed:
        divergences[f] = probed

# ── The guards. A number from a contaminated or partial directory is not a number.
if 'HANG' in labels and 'TIMEOUT' in labels:
    sys.exit("REFUSING: two script versions wrote this directory (both HANG and TIMEOUT labels present).")
if len(runs) > 1:
    sys.exit(f"REFUSING: {len(runs)} different crawl runs wrote this directory. That is not a measurement.")

n = len(timed)
if n == 0:
    sys.exit("no timed sites — nothing to report")

# ── COVERAGE FIRST, ALWAYS. And **the three divergence kinds are not the same thing** — lumping them
#    is how this report, on its first run, announced "coverage 2.8%" for a browser that renders fine.
#
#      missing   the node IS NOT THERE          → Bar 1. The real one. This is "we failed to render it."
#      display   the node is there, `display` differs → Bar 1. Often `none` vs `block`: shown or hidden.
#      geometry  the node is there, SAME SIZE, different position → **Bar 2. DEFERRED by settled decision.**
#
#    A geometry divergence is not a rendering failure. bbc.co.uk's `<h3>` is 208x88 in both engines and
#    sits at a different y — that is pixel precision, which this project has explicitly deferred until
#    breadth is real. Reporting it as "coverage" would have said 2.8% about a page that renders.
tot_probed = sum(divergences.values())
missing = dkind['missing']
display = dkind['display']
geometry = dkind['geometry']

present   = 100.0 * (tot_probed - missing) / max(tot_probed, 1)
displayed = 100.0 * (tot_probed - missing - display) / max(tot_probed, 1)

print("=" * 74)
print("  BAR 1 — is the node THERE? (this is what 'coverage' means)")
print("=" * 74)
print(f"  node PRESENT           : {present:5.1f}%   ({tot_probed - missing:,} of {tot_probed:,} probed)")
print(f"  ...and `display` agrees: {displayed:5.1f}%   (a `display` mismatch is often none-vs-block: shown or hidden)")
print(f"  sites diffed           : {len(divergences)}")
print()
print("  BAR 2 — pixel precision. DEFERRED by settled decision; reported, never scored.")
print(f"  geometry divergences   : {geometry:,}  (the node exists, at the SAME SIZE, in a different place)")
print()
print("  ^ BAR 1 gates the speed number below. A speed advantage is only real if it comes from doing the")
print("    same work BETTER — not from not doing the work. If PRESENCE moved, the speed below is a")
print("    measurement of the thing we stopped rendering.")
print()

hangs  = [r for r in timed if r[2] > hang_ms]
faster = [r for r in timed if r[3] > r[2]]
ms  = sorted(r[2] for r in timed)
cs  = sorted(r[3] for r in timed if r[3])

print("=" * 74)
print("  SPEED — on OUR clock (manuk_ms), never the oracle process's")
print("=" * 74)
print(f"  both engines rendered  : {n} sites")
print(f"  Bar 0 — over {hang_ms//1000}s on our clock : {len(hangs)}  ({100*len(hangs)/n:.1f}%)")
print(f"  we are FASTER than Chromium : {len(faster)}/{n}  ({100*len(faster)//n}%)")
print(f"  median : ours {ms[len(ms)//2]/1000:5.1f}s   Chromium {cs[len(cs)//2]/1000:5.1f}s")
print(f"  p90    : ours {ms[int(n*.9)]/1000:5.1f}s   Chromium {cs[int(len(cs)*.9)]/1000:5.1f}s")
print()
print("  Being faster is NOT a divergence to close. Parity is a claim about CAPABILITY, not timing.")
print("  There is nothing to regress toward.")
if hangs:
    print(f"\n  the {len(hangs)} sites slow on OUR clock:")
    for s_, c_, m_, ch_ in sorted(hangs, key=lambda r: -r[2])[:10]:
        flag = "  ← WE are slower" if m_ > ch_ else ""
        print(f"    {s_:<24} {c_:<12} ours {m_/1000:>6.1f}s   chromium {ch_/1000:>6.1f}s{flag}")

print(f"\n  unattributed TIMEOUTs {timeouts} (whose time? unknown) · discarded {discarded} · failed {failed}")
PY
