#!/usr/bin/env python3
"""NET.REACH.1.a: repeat one URL N times through target/release/a11y-dump and record, per run,
the wall time, the exit code, and where the time went: the largest gap between consecutive
MANUK_NET_TRACE lines and the event that opened it (a stall in the net read/stream phase shows as
a long gap right after a `req`/`body` line; a slow subresource shows as one long `req`->`body`).

Usage (run with -I): python3 -I scripts/reach-linode-repeat.py URL --runs 10 --limit 60 --out DIR
Writes DIR/runs.tsv (one row per run) and prints a summary. Exit 0 always; the verdict is the caller's.
"""
import argparse, os, subprocess, time

ap = argparse.ArgumentParser()
ap.add_argument("url")
ap.add_argument("--runs", type=int, default=10)
ap.add_argument("--limit", type=float, default=60.0, help="kill a run after this many seconds")
ap.add_argument("--bar", type=float, default=30.0, help="a run over this many seconds counts as slow")
ap.add_argument("--binary", default="target/release/a11y-dump")
ap.add_argument("--out", required=True)
a = ap.parse_args()
os.makedirs(a.out, exist_ok=True)
ver = os.environ.get("MANUK_CHROME_PIN")
if not ver:
    raise SystemExit("MANUK_CHROME_PIN unset")

rows = []
for i in range(1, a.runs + 1):
    trace = os.path.join(a.out, f"trace-run{i}.log")
    env = dict(os.environ, MANUK_NET_TRACE=trace)
    t0 = time.monotonic()
    try:
        rc = subprocess.run([a.binary, a.url], env=env, stdout=subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL, timeout=a.limit).returncode
        killed = False
    except subprocess.TimeoutExpired:
        rc, killed = -9, True
    wall = time.monotonic() - t0
    # largest gap between consecutive trace events (timestamps are microseconds since process start)
    ev = []
    if os.path.exists(trace):
        for line in open(trace, errors="replace"):
            parts = line.rstrip("\n").split("\t")
            if len(parts) >= 2 and parts[0].isdigit():
                ev.append((int(parts[0]), " ".join(parts[1:3])))
    gap, gap_at, last = 0, "-", ev[-1][1] if ev else "-"
    for (t1, _), (t2, lab2) in zip(ev, ev[1:]):
        if t2 - t1 > gap:
            gap, gap_at = t2 - t1, lab2
    status = "TIMEOUT" if killed else ("OK" if rc == 0 else "FAIL")
    slow = "SLOW" if wall > a.bar or killed else "fast"
    rows.append((i, a.url, round(wall, 2), rc, status, slow, round(gap / 1e6, 2), gap_at[:80], last[:80], ver))
    print(f"run{i} {status:7} {wall:6.2f}s rc={rc} longest_gap={gap/1e6:.2f}s before[{gap_at[:60]}] {slow}", flush=True)

with open(os.path.join(a.out, "runs.tsv"), "w") as f:
    f.write("run\turl\twall_s\trc\tstatus\tbar\tlongest_gap_s\tgap_opened_by\tlast_event\tchrome_ver\n")
    for r in rows:
        f.write("\t".join(str(x) for x in r) + "\n")
slow = sum(1 for r in rows if r[5] == "SLOW")
print(f"{len(rows)} runs, {slow} over {a.bar:.0f}s, {sum(1 for r in rows if r[4] != 'OK')} not OK, chrome_ver={ver}")
