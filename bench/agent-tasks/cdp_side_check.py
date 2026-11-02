#!/usr/bin/env python3 -I
"""W4.P0.f check: docs/sprint/w4p0/CDP-SIDE.tsv covers the 6 cdp-side rows (opendining-3/4/5/8/10, X03).
class TRACED needs a probe pointer that exists and hashes to probe_sha256 (a file under the repo) or names a stored
scoreboard row (`scoreboard|<file>|<ts>|<arm>|<task>|<run>`) that exists. class UNCLASSIFIED needs a non-empty settling_probe.
A label with neither is rejected. Usage: cdp_side_check.py [--selftest]"""
import csv, hashlib, os, sys
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
TAB = os.path.join(ROOT, "docs/sprint/w4p0/CDP-SIDE.tsv")
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
        base = check(rows)
        if base:
            print("SELFTEST FAIL: base table red:", base); return 1
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
        print("SELFTEST OK: probe-less label, edited probe hash, UNCLASSIFIED without settling probe, and a missing row are all rejected")
        return 0
    errs = check(rows)
    for e in errs: print("FAIL", e)
    print("cdp-side check:", "RED" if errs else f"OK {len(rows)} rows")
    return 1 if errs else 0
sys.exit(main(sys.argv[1:]))
