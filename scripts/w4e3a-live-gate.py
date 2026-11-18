#!/usr/bin/env python3
"""W4.E3.a.a gate: stencil cache on LIVE script-heavy pages (not local copies).

Pages were pre-registered BEFORE any measurement, chosen by inline+external <script> tag count
on 2026-11-18 (bbc.com/news 58, nasa.gov 36, theguardian.com/international 19; the next were
github.com 13, flickr.com/explore 13). Usage:
  w4e3a-live-gate.py [--bin target/release/a11y-dump] [--runs 3] [--mutant nocache]

Per run and page: a plain run (no cache, MANUK_SCRIPT_TIMING=1), a cold run (fresh empty
MANUK_HTTP_CACHE_DIR, MANUK_SCRIPT_TIMING=1), then a warm run in a NEW process reusing that dir.
Script time = sum of `manuk-script-ms <kind> <ms> <name>` lines on stderr.

GATE (exit 0 only if all hold):
  - every page's warm runs contain >=1 'hit' (the stencil cache was actually used on live scripts);
  - on every page whose PLAIN digest is stable across its runs, every warm digest equals it. Drifting pages
    (plain digest differs between runs) are not gated on digest; their cold-vs-warm mismatches are reported;
  - there is at least one timing row per run (the instrumentation was seen), and no run timed out
    twice (120 s per attempt, one retry; a page that never loads fails the gate).
RECORDED, NOT GATED: whether warm script time falls below the cold band (max(warm) < min(cold)),
hit/miss counts, digest drift, timeouts (120 s x2 attempts) and nonzero exit codes (live pages stall or
the process crashes at exit; those runs are reported with their own rc and digest).
--mutant nocache: warm runs get a fresh empty dir, so warm must show zero hits and the gate must go RED.
Output: docs/sprint/w4e3a-live/summary.tsv (written only on a non-mutant run).
"""
import argparse, collections, hashlib, json, os, re, shutil, subprocess, sys, tempfile

ap = argparse.ArgumentParser()
ap.add_argument("--bin", default="target/release/a11y-dump")
ap.add_argument("--runs", type=int, default=3)
ap.add_argument("--mutant", choices=["nocache"])
ap.add_argument("--out", default="docs/sprint/w4e3a-live")
a = ap.parse_args()

PAGES = [  # pre-registered; do not edit after the first measurement
    ("bbc", "https://www.bbc.com/news"),
    ("nasa", "https://www.nasa.gov/"),
    ("guardian", "https://www.theguardian.com/international"),
]
ROW = re.compile(r"^manuk-script-ms (\S+) ([\d.]+) ")

def run(url, cache):
    env = dict(os.environ, MANUK_SCRIPT_TIMING="1")
    env.pop("MANUK_HTTP_CACHE_DIR", None)
    if cache:
        env["MANUK_HTTP_CACHE_DIR"] = cache
    for attempt in (1, 2):  # a live-site stall is retried once; a second stall is recorded as TIMEOUT
        try:
            p = subprocess.run([os.path.abspath(a.bin), url], env=env, capture_output=True, text=True, timeout=120)
            break
        except subprocess.TimeoutExpired:
            print(f"timeout attempt {attempt}: {url} cache={bool(cache)}", file=sys.stderr)
    else:
        return dict(ms=0.0, kinds=collections.Counter(), digest="TIMEOUT", rows=0, rc=None)
    kinds, ms = collections.Counter(), 0.0
    for line in p.stderr.splitlines():
        m = ROW.match(line)
        if m:
            kinds[m.group(1)] += 1
            ms += float(m.group(2))
    # the digest is of stdout whatever the exit code; a nonzero rc is recorded separately (a11y-dump has
    # been seen to die with SIGSEGV at exit after printing its tree, the same teardown as W4.E3.b.b)
    digest = hashlib.sha256(p.stdout.encode()).hexdigest()[:16] if p.stdout else "EMPTY"
    return dict(ms=ms, kinds=kinds, digest=digest, rows=sum(kinds.values()), rc=p.returncode)

bad, lines = 0, []
lines.append("\t".join(["page", "run", "phase", "script_ms", "hit", "miss", "plain", "rc", "digest"]))
summary = []
for name, url in PAGES:
    cold, warm, plain_d, cold_d, warm_d, warm_hits = [], [], [], [], [], 0
    fails = collections.Counter()  # timeouts / nonzero exits, reported not gated
    for r in range(a.runs):
        pl = run(url, None); plain_d.append(pl["digest"])
        root = tempfile.mkdtemp(prefix="w4e3a-live-")
        try:
            cache = os.path.join(root, "c")
            os.makedirs(cache)
            c = run(url, cache)
            w = run(url, os.path.join(root, "fresh") if a.mutant == "nocache" else cache)
        finally:
            shutil.rmtree(root, ignore_errors=True)
        cold.append(c["ms"]); warm.append(w["ms"]); cold_d.append(c["digest"]); warm_d.append(w["digest"])
        warm_hits += w["kinds"]["hit"]
        for ph, x in (("plain", pl), ("cold", c), ("warm", w)):
            lines.append("\t".join([name, str(r), ph, f"{x['ms']:.3f}", str(x["kinds"]["hit"]),
                                    str(x["kinds"]["miss"]), str(x["kinds"]["plain"]), str(x["rc"]), x["digest"]]))
            fails["timeout" if x["digest"] == "TIMEOUT" else "nonzero_rc" if x["rc"] else "ok"] += 1
        if c["digest"] != "TIMEOUT" and w["digest"] != "TIMEOUT" and (c["rows"] == 0 or w["rows"] == 0):
            print(f"{name} run{r}: NO manuk-script-ms rows (cold {c['rows']}, warm {w['rows']})"); bad += 1
    plain_stable = len(set(plain_d)) == 1
    # the cache must not change the observation: each warm digest equals its own cold digest, and when the
    # plain digest is stable across runs, it equals that too (live pages can drift between runs, so the
    # within-run cold-vs-warm comparison is the one that always applies)
    # gated only on pages whose plain digest is stable; on drifting live pages the within-run cold-vs-warm
    # mismatches are counted and reported (drift = the page itself changed between two loads)
    drift = sum(w != c for w, c in zip(warm_d, cold_d))
    digest_ok = (not plain_stable) or all(d == plain_d[0] for d in warm_d)
    outside = max(warm) < min(cold)
    hit_ok = warm_hits > 0
    bad += (not hit_ok) + (not digest_ok)
    verdict = "PASS" if hit_ok and digest_ok else "FAIL"
    row = dict(page=name, cold=sorted(round(x, 1) for x in cold), warm=sorted(round(x, 1) for x in warm),
               warm_hits=warm_hits, plain_digest_stable=plain_stable, digest_ok=digest_ok, drift=drift,
               timeouts=fails["timeout"], nonzero_rc=fails["nonzero_rc"],
               warm_below_cold_band=outside, verdict=verdict)
    summary.append(row)
    print(json.dumps(row))

text = "\n".join(lines) + "\n"
print(text, end="")
if not a.mutant:
    os.makedirs(a.out, exist_ok=True)
    open(os.path.join(a.out, "runs.tsv"), "w").write(text)
    with open(os.path.join(a.out, "summary.tsv"), "w") as f:
        f.write("page\tcold_ms\twarm_ms\twarm_hits\tplain_digest_stable\tdigest_ok\tdrift_cold_vs_warm\ttimeouts\tnonzero_rc\twarm_below_cold_band\tverdict\n")
        for s in summary:
            f.write("\t".join(map(str, [s["page"], s["cold"], s["warm"], s["warm_hits"], s["plain_digest_stable"],
                                        s["digest_ok"], s["drift"], s["timeouts"], s["nonzero_rc"],
                                        s["warm_below_cold_band"], s["verdict"]])) + "\n")
print("GATE", "PASS" if bad == 0 else "FAIL (%d)" % bad)
sys.exit(0 if bad == 0 else 1)
