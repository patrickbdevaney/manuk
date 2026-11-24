#!/usr/bin/env python3
"""PERF.1.2.b.a.b: paired cascade comparison (incremental vs MANUK_FULL_RESTYLE=1) on a QUIET host.

  run    : wait until load1 < 4, then run perf1-profile.py paired (same binary, 3 runs x suite URLs),
           sampling load every 30 s. A run whose load1 reaches 8 is aborted and voided.
  report : judge <out>/paired.jsonl + <out>/meta.json. Exit 0 only if the measurement is VALID
           (load1 < 4 before AND after, one binary in both arms, the right env per arm, one chrome_ver,
           >= 20 URLs with 3 complete A/B pairs). A/B against the pre-registered 0.80 is printed
           separately: an invalid run never yields a verdict.

  python3 -I scripts/perf1-quiet-cascade.py run    --out docs/sprint/perf1-pf12bab --wait 7200
  python3 -I scripts/perf1-quiet-cascade.py report --out docs/sprint/perf1-pf12bab
  python3 -I scripts/perf1-quiet-cascade.py self-test
"""
import argparse
import copy
import hashlib
import json
import os
import signal
import statistics
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
QUIET = 4.0
ABORT = 8.0
BAR = 0.80
RUNS = 3
MIN_URLS = 20
ARM_ENV = {"manuk": {}, "manuk-b": {"MANUK_FULL_RESTYLE": "1"}}


def load1():
    return float(open("/proc/loadavg").read().split()[0])


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def run(a):
    pin = os.environ.get("MANUK_CHROME_PIN")
    if not pin:
        sys.exit("MANUK_CHROME_PIN unset: refusing to measure without a chrome_ver")
    os.makedirs(a.out, exist_ok=True)
    log = open(os.path.join(a.out, "load.tsv"), "a")

    def sample(tag):
        v = load1()
        log.write(f"{time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}\t{v}\t{tag}\n")
        log.flush()
        return v

    deadline = time.time() + a.wait
    attempt = 0
    while True:
        before = sample("wait")
        if before < QUIET:
            attempt += 1
            tmp = os.path.join(a.scratch, f"attempt-{os.getpid()}-{attempt}")
            os.makedirs(tmp)
            digest, peak = sha(a.manuk), before
            p = subprocess.Popen(
                [sys.executable, "-I", os.path.join(HERE, "perf1-profile.py"), "--runs", str(RUNS), "--out", tmp,
                 "--manuk", a.manuk, "--manuk-b", a.manuk, "--env-b", "MANUK_FULL_RESTYLE=1"],
                start_new_session=True)
            t0 = time.time()
            while p.poll() is None:
                time.sleep(10)
                peak = max(peak, sample("run"))
                if peak >= ABORT:
                    os.killpg(p.pid, signal.SIGTERM)
                    p.wait()
            after = sample("after")
            ok = p.returncode == 0 and after < QUIET and digest == sha(a.manuk)
            meta = {"chrome_ver": pin, "load1_before": before, "load1_after": after, "load1_peak": peak,
                    "binary_sha256": digest, "seconds": round(time.time() - t0), "rc": p.returncode}
            print("valid-load" if ok else "VOID", meta, flush=True)
            if ok:
                os.replace(os.path.join(tmp, "paired.jsonl"), os.path.join(a.out, "paired.jsonl"))
                json.dump(meta, open(os.path.join(a.out, "meta.json"), "w"), indent=1)
                return 0
        if time.time() > deadline:
            print("no quiet window", flush=True)
            return 2
        time.sleep(30)


def judge(rows, meta):
    """-> (valid, problems, table, dropped urls, sum A, sum B). Sums are medians of `RUNS` per URL."""
    bad = []
    for k in ("load1_before", "load1_after"):
        v = meta.get(k)
        if v is None or v >= QUIET:
            bad.append(f"{k} {v} not < {QUIET}")
    vers = {r.get("chrome_ver") for r in rows} | {meta.get("chrome_ver")}
    if len(vers) != 1 or None in vers:
        bad.append(f"chrome_ver not unique: {vers}")
    if len({r.get("binary") for r in rows}) != 1 or not meta.get("binary_sha256"):
        bad.append("arms did not run one recorded binary")
    for r in rows:
        if r.get("env") != ARM_ENV.get(r["engine"]):
            bad.append(f"{r['engine']} ran with env {r.get('env')}")
            break
    cell = {}
    for r in rows:
        if r.get("status") == "OK":
            key = (r["url"], r["engine"], r["run"])
            if key in cell:
                bad.append(f"duplicate row {key}")
            cell[key] = r["phases_s"]["cascade"]
    table, dropped = [], []
    for url in dict.fromkeys(r["url"] for r in rows):
        av = [cell.get((url, "manuk", n)) for n in range(1, RUNS + 1)]
        bv = [cell.get((url, "manuk-b", n)) for n in range(1, RUNS + 1)]
        if None in av + bv:
            dropped.append(url)
            continue
        table.append((url, av, bv))
    if len(table) < MIN_URLS:
        bad.append(f"{len(table)} URLs with {RUNS} complete pairs, need {MIN_URLS}")
    sa = sum(statistics.median(x) for _, x, _ in table)
    sb = sum(statistics.median(x) for _, _, x in table)
    return not bad, bad, table, dropped, sa, sb


def report(a):
    rows = [json.loads(l) for l in open(os.path.join(a.out, "paired.jsonl"))]
    meta = json.load(open(os.path.join(a.out, "meta.json")))
    valid, bad, table, dropped, sa, sb = judge(rows, meta)
    band = lambda x: f"{statistics.median(x):.3f} [{min(x):.3f}-{max(x):.3f}]"
    print(f"chrome_ver={meta.get('chrome_ver')} load1 before {meta.get('load1_before')} after "
          f"{meta.get('load1_after')} peak {meta.get('load1_peak')}\n")
    print("| URL | A incremental, cascade s | B MANUK_FULL_RESTYLE=1 | A/B |\n|---|---|---|---|")
    for url, av, bv in table:
        m = statistics.median(bv)
        print(f"| {url} | {band(av)} | {band(bv)} | {statistics.median(av) / m if m else float('nan'):.3f} |")
    print(f"| **Σ medians, {len(table)} URLs** | {sa:.3f} | {sb:.3f} | {sa / sb if sb else float('nan'):.3f} |")
    ma, mb = (sum(statistics.mean(x[i]) for x in table) for i in (1, 2))
    print(f"| Σ means (the reduction PERF.1.2.b.a.a recorded) | {ma:.3f} | {mb:.3f} | {ma / mb if mb else float('nan'):.3f} |")
    for u in dropped:
        print(f"- no complete triple of pairs (not in the sum): {u}")
    for b in bad:
        print(f"- INVALID: {b}")
    print("MEASUREMENT: " + ("VALID" if valid else "INVALID (no verdict)"))
    if valid:
        print(f"BAR A/B <= {BAR}: " + ("MET" if sa <= BAR * sb else "NOT MET"))
    return 0 if valid else 1


def self_test():
    meta = {"chrome_ver": "1", "load1_before": 1.0, "load1_after": 2.0, "binary_sha256": "x"}
    rows = [{"engine": e, "url": f"u{u}", "run": n, "status": "OK", "chrome_ver": "1", "binary": "b",
             "env": ARM_ENV[e], "phases_s": {"cascade": 0.1 if e == "manuk" else 0.2}}
            for u in range(20) for n in (1, 2, 3) for e in ("manuk", "manuk-b")]
    v = judge(rows, meta)
    assert v[0] and abs(v[4] - 2.0) < 1e-9 and abs(v[5] - 4.0) < 1e-9, v

    def red(name, f):
        r, m = copy.deepcopy(rows), dict(meta)
        r = f(r, m) or r
        assert not judge(r, m)[0], f"mutant not red: {name}"
        print("red:", name)

    red("load before 4.0", lambda r, m: m.update(load1_before=4.0))
    red("load after 33.8", lambda r, m: m.update(load1_after=33.8))
    red("load never recorded", lambda r, m: m.pop("load1_after") and None)
    red("arm B without MANUK_FULL_RESTYLE", lambda r, m: [x.update(env={}) for x in r] and None)
    red("different binaries", lambda r, m: r[0].update(binary="c"))
    red("mixed chrome_ver", lambda r, m: r[0].update(chrome_ver="2"))
    red("one run failed on one URL", lambda r, m: r[0].update(status="FAIL"))
    red("duplicate rows", lambda r, m: r + [r[0]])
    print("self-test OK")
    return 0


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["run", "report", "self-test"])
    ap.add_argument("--out", default="docs/sprint/perf1-pf12bab")
    ap.add_argument("--scratch", default="target/perf-pf12bab")
    ap.add_argument("--manuk", default="target/release/a11y-dump")
    ap.add_argument("--wait", type=float, default=7200)
    a = ap.parse_args()
    sys.exit(self_test() if a.cmd == "self-test" else {"run": run, "report": report}[a.cmd](a))
