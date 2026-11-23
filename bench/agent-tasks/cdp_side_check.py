#!/usr/bin/env python3 -I
"""W4.P0.f check: docs/sprint/w4p0/CDP-SIDE.tsv covers the 6 cdp-side rows (opendining-3/4/5/8/10, X03).
class TRACED needs a probe pointer that exists and hashes to probe_sha256 (a file under the repo) or names a stored
scoreboard row (`scoreboard|<file>|<ts>|<arm>|<task>|<run>`) that exists. class UNCLASSIFIED needs a non-empty settling_probe.
A label with neither is rejected. Usage: cdp_side_check.py [--selftest]"""
import csv, hashlib, os, shutil, sys, tempfile
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
TAB = os.path.join(ROOT, "docs/sprint/w4p0/CDP-SIDE.tsv")
RUNS = "docs/sprint/w4p0/cdp-side/runs"  # W4.P0.f.a: real_run.py --capture output (raw /finish + tool list per run)
RUN_TASKS = ["opendining-3", "opendining-5", "opendining-8", "opendining-10"]
SRC = {"A6-real.tsv": "docs/sprint/w4/A6-real.tsv", "SCOREBOARD.tsv": "docs/sprint/SCOREBOARD.tsv"}
WANT = {"opendining-3", "opendining-4", "opendining-5", "opendining-8", "opendining-10", "X03"}
PIN = os.environ.get("MANUK_CHROME_PIN", "145.0.7632.116")

def row_exists(ptr):
    _, f, ts, arm, task, run = ptr.split("|")
    p = os.path.join(ROOT, SRC.get(f, "/nonexistent"))
    if not os.path.exists(p):
        return False
    for r in csv.reader(open(p), delimiter="\t"):
        if r and r[0] == ts and r[1] == arm and (task in r) :
            return True
    return False

def check_runs(base):
    """W4.P0.f.a: every cdp-layer run of the four opendining tasks has its raw /finish read and tool list stored, hashed
    in MANIFEST.tsv, at the pinned chrome_ver; each task has 3 runs (the last row per key wins). Deleting or editing a
    stored file turns this red."""
    mp = os.path.join(base, RUNS, "MANIFEST.tsv")
    if not os.path.exists(mp):
        return [f"runs manifest missing {RUNS}/MANIFEST.tsv"]
    last = {}
    for r in csv.DictReader(open(mp), delimiter="\t"):
        last[(r["arm"], r["task"], r["run"])] = r
    errs, per = [], {t: 0 for t in RUN_TASKS}
    for (arm, task, run), r in last.items():
        if arm != "cdp-layer" or task not in per:
            continue
        per[task] += 1
        tag = f"run {task} r{run}"
        if r["chrome_ver"] != PIN:
            errs.append(f"{tag}: chrome_ver {r['chrome_ver']} != pin")
        for kind, f, sha in (("finish", r["finish_file"], r["finish_sha256"]), ("tools", r["tools_file"], r["tools_sha256"])):
            p = os.path.join(base, RUNS, f)
            if not os.path.exists(p):
                errs.append(f"{tag}: stored {kind} file missing {f}")
            elif hashlib.sha256(open(p, "rb").read()).hexdigest() != sha:
                errs.append(f"{tag}: stored {kind} file changed {f}")
    for t, n in per.items():
        if n < 3:
            errs.append(f"task {t} has {n} cdp-layer runs stored, need 3")
    return errs


def check(rows):
    errs, seen = [], set()
    for i, r in enumerate(rows, 2):
        tag = f"row {i} {r.get('task')}"
        seen.add(r["task"])
        if r["chrome_ver"] != PIN:
            errs.append(f"{tag}: chrome_ver {r['chrome_ver']} != pin")
        if not r["mechanism"].strip():
            errs.append(f"{tag}: empty mechanism")
        if r["class"] == "TRACED":
            ptr = r["probe"].strip()
            if not ptr:
                errs.append(f"{tag}: TRACED label without a probe")
            elif ptr.startswith("scoreboard|"):
                try:
                    if not row_exists(ptr):
                        errs.append(f"{tag}: scoreboard row not found: {ptr}")
                except ValueError:
                    errs.append(f"{tag}: bad scoreboard pointer")
            else:
                p = os.path.join(ROOT, ptr)
                if not os.path.exists(p):
                    errs.append(f"{tag}: probe file missing {ptr}")
                elif hashlib.sha256(open(p, "rb").read()).hexdigest() != r["probe_sha256"]:
                    errs.append(f"{tag}: probe file hash changed {ptr}")
        elif r["class"] == "UNCLASSIFIED":
            if not r["settling_probe"].strip():
                errs.append(f"{tag}: UNCLASSIFIED without the probe that would settle it")
        else:
            errs.append(f"{tag}: bad class {r['class']!r}")
    for t in WANT - seen:
        errs.append(f"missing row {t}")
    return errs

def load():
    return list(csv.DictReader(open(TAB), delimiter="\t"))

def main(a):
    rows = load()
    if "--selftest" in a:
        base = check(rows) + check_runs(ROOT)
        if base:
            print("SELFTEST FAIL: base table red:", base); return 1
        cap = tempfile.mkdtemp()
        try:  # W4.P0.f.a mutation: delete one stored /finish file from a copy of the runs dir
            shutil.copytree(os.path.join(ROOT, RUNS), os.path.join(cap, RUNS))
            fins = sorted(f for f in os.listdir(os.path.join(cap, RUNS)) if f.endswith(".finish.json"))
            os.unlink(os.path.join(cap, RUNS, fins[0]))
            e5 = check_runs(cap)
        finally:
            shutil.rmtree(cap, ignore_errors=True)
        if not e5:
            print("SELFTEST FAIL: deleting a stored /finish file stayed green"); return 1
        traced = next(i for i, r in enumerate(rows) if r["class"] == "TRACED")
        m = [dict(r) for r in rows]; m[traced]["probe"] = ""
        e1 = check(m)
        m = [dict(r) for r in rows]; m[traced]["probe_sha256"] = "0" * 64
        e2 = check(m) if not m[traced]["probe"].startswith("scoreboard|") else ["skip"]
        unc = next(i for i, r in enumerate(rows) if r["class"] == "UNCLASSIFIED")
        m = [dict(r) for r in rows]; m[unc]["settling_probe"] = ""
        e3 = check(m)
        m = [dict(r) for r in rows if r["task"] != "X03"]
        e4 = check(m)
        if not (e1 and e2 and e3 and e4):
            print("SELFTEST FAIL: a mutation stayed green", bool(e1), bool(e2), bool(e3), bool(e4)); return 1
        print("SELFTEST OK: probe-less label, edited probe hash, UNCLASSIFIED without settling probe, a missing row, "
              "and a deleted stored /finish file are all rejected")
        return 0
    errs = check(rows) + check_runs(ROOT)
    for e in errs: print("FAIL", e)
    print("cdp-side check:", "RED" if errs else f"OK {len(rows)} rows")
    return 1 if errs else 0
sys.exit(main(sys.argv[1:]))
