#!/usr/bin/env python3
"""W4.P0.e verifier audit check.

The audit table (docs/sprint/w4p0/VERIFIER-AUDIT.tsv) holds one hand verdict per disagreeing run and per
manuk pass whose answer disclaims success. Every row must carry a pointer to the evidence it was read from:
a transcript file (checked: it exists and its sha256 matches the table) or a stored-answer row
`scoreboard|<file>|<ts>|<arm>|<task>|<run>` (checked: that row exists and its pass column matches).
A verdict with no pointer, a dead pointer, or a changed transcript fails the check.
The frozen inputs (suite-v1.json, suite-v1-external.json, SUCCESS.md) must still hash to FROZEN.sha256.

Usage: verifier_audit.py [--selftest]
"""
import csv, hashlib, os, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
AUDIT = os.path.join(ROOT, "docs/sprint/w4p0/VERIFIER-AUDIT.tsv")
FROZEN = os.path.join(ROOT, "docs/sprint/w4p0/FROZEN.sha256")
SOURCES = {"SCOREBOARD.tsv": "docs/sprint/SCOREBOARD.tsv", "A6-real.tsv": "docs/sprint/w4/A6-real.tsv"}
VERDICTS = {"TRUE_PASS", "TRUE_FAIL", "FALSE_PASS", "FALSE_FAIL", "UNVERIFIABLE"}
COLS = ["chrome_ver", "slice", "task", "arm", "run", "scoreboard_pass", "kind", "verdict", "pointer", "pointer_sha256", "note"]
PIN = os.environ.get("MANUK_CHROME_PIN", "145.0.7632.116")


def sha(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


def check(rows):
    errs = []
    cache = {}
    for i, r in enumerate(rows, 2):
        tag = f"row {i} {r.get('task')} {r.get('arm')} r{r.get('run')}"
        if r["chrome_ver"] != PIN:
            errs.append(f"{tag}: chrome_ver {r['chrome_ver']} != pin")
        if r["verdict"] not in VERDICTS:
            errs.append(f"{tag}: bad verdict {r['verdict']!r}")
        ptr = r["pointer"].strip()
        if not ptr:
            errs.append(f"{tag}: verdict without transcript pointer")
            continue
        if ptr.startswith("scoreboard|"):
            _, src, ts, arm, task, run = ptr.split("|")
            path = os.path.join(ROOT, SOURCES.get(src, "/nonexistent"))
            if path not in cache:
                cache[path] = list(csv.DictReader(open(path, newline=""), delimiter="\t")) if os.path.exists(path) else []
            hit = [x for x in cache[path] if x["ts"] == ts and x["arm"] == arm and x["task"] == task and x["run"] == run]
            if not hit:
                errs.append(f"{tag}: pointer row not found: {ptr}")
            elif hit[0]["pass"] != r["scoreboard_pass"]:
                errs.append(f"{tag}: scoreboard_pass {r['scoreboard_pass']} != stored {hit[0]['pass']}")
        else:
            p = os.path.join(ROOT, ptr)
            if not os.path.isfile(p):
                errs.append(f"{tag}: transcript missing: {ptr}")
            elif sha(p) != r["pointer_sha256"]:
                errs.append(f"{tag}: transcript sha256 changed: {ptr}")
    return errs


def frozen_errs():
    errs = []
    for line in open(FROZEN):
        h, p = line.split()
        if sha(os.path.join(ROOT, p)) != h:
            errs.append(f"frozen file changed: {p}")
    return errs


def load():
    return list(csv.DictReader(open(AUDIT, newline=""), delimiter="\t"))


def main(argv):
    rows = load()
    errs = check(rows) + frozen_errs()
    if "--selftest" in argv:
        if errs:
            print("SELFTEST FAIL: clean table is red:", errs[:3]); return 1
        m = [dict(r) for r in rows]
        m[0]["pointer"] = ""; m[0]["verdict"] = "TRUE_PASS"
        if not check(m):
            print("SELFTEST FAIL: verdict with blank pointer was accepted"); return 1
        m = [dict(r) for r in rows]
        k = next(j for j, r in enumerate(m) if not r["pointer"].startswith("scoreboard|"))
        m[k]["pointer"] = m[k]["pointer"] + ".gone"
        if not check(m):
            print("SELFTEST FAIL: dead pointer accepted"); return 1
        m = [dict(r) for r in rows]
        m[k]["pointer_sha256"] = "0" * 64
        if not check(m):
            print("SELFTEST FAIL: changed transcript accepted"); return 1
        print(f"SELFTEST OK: {len(rows)} rows clean; blank pointer, dead pointer and changed transcript each go red")
        return 0
    for e in errs:
        print("FAIL", e)
    from collections import Counter
    print(f"{len(rows)} rows;", dict(Counter(r["verdict"] for r in rows)), "; frozen hashes OK" if not frozen_errs() else "")
    return 1 if errs else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
