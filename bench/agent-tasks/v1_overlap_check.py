#!/usr/bin/env python3
"""W6.V1 gate: the held-out set (suite-v1-heldout.json) must overlap NO other task set and must match its frozen hash.
Overlap = same normalised start URL (scheme/www/trailing slash/query stripped) OR same goal text, against: own suite v1,
external slice, diffview/lean-probe(2), the REAL tasks (their start sites), BASELINE-URLS, guard corpora URLs.
  v1_overlap_check.py            exit 0 = clean and hash matches
  v1_overlap_check.py --inject   adds a copy of an own-suite task into the held-out list: must exit 1 (gate can go red)
  v1_overlap_check.py --tamper   appends one byte to the loaded set bytes: hash check must exit 1"""
import glob, hashlib, json, os, re, sys
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(os.path.dirname(HERE))
SET = os.path.join(HERE, "suite-v1-heldout.json"); HASH = os.path.join(ROOT, "docs/sprint/w6/V1-set.sha256")
def norm(u):
    u = re.sub(r"^https?://(www\.)?", "", u.strip().lower()); return re.sub(r"[?#].*$", "", u).rstrip("/")
def others():
    urls, goals = {}, {}
    for f in sorted(glob.glob(os.path.join(HERE, "suite-v1*.json"))):
        if f == SET: continue
        for t in json.load(open(f))["tasks"]:
            urls.setdefault(norm(t["start_url"]), os.path.basename(f) + ":" + t["id"]); goals[t["goal"].strip().lower()] = t["id"]
    for f in glob.glob(os.path.join(HERE, "real/tasks/*.json")):
        t = json.load(open(f)); u = t.get("website", {}).get("url") or ""
        if u: urls.setdefault(norm(u), "REAL:" + t["id"])
        goals[t["goal"].strip().lower()] = "REAL:" + t["id"]
    for p in [os.path.join(ROOT, "docs/sprint/BASELINE-URLS.txt")]:
        for l in open(p):
            if l.strip(): urls.setdefault(norm(l), "BASELINE-URLS")
    for p in glob.glob(os.path.join(ROOT, "docs/sprint/guard*.tsv")) + glob.glob(os.path.join(ROOT, "docs/sprint/GUARD*.tsv")):
        for l in open(p):
            for u in re.findall(r"https?://[^\s\t]+", l): urls.setdefault(norm(u), os.path.basename(p))
    return urls, goals
def main():
    raw = open(SET, "rb").read()
    if "--tamper" in sys.argv: raw = raw + b" "
    bad = []
    if hashlib.sha256(raw).hexdigest() != open(HASH).read().split()[0]: bad.append("hash mismatch vs V1-set.sha256")
    tasks = json.loads(raw)["tasks"]
    urls, goals = others()
    if "--inject" in sys.argv:
        own = json.load(open(os.path.join(HERE, "suite-v1.json")))["tasks"][0]; tasks = tasks + [dict(own, id="HX")]
    if len(tasks) != 30: bad.append(f"{len(tasks)} tasks, want 30")
    for t in tasks:
        u = norm(t["start_url"])
        if u in urls: bad.append(f"{t['id']} start_url overlaps {urls[u]}")
        if t["goal"].strip().lower() in goals: bad.append(f"{t['id']} goal overlaps {goals[t['goal'].strip().lower()]}")
    print("\n".join(bad) or f"OK: {len(tasks)} tasks, no overlap with {len(urls)} known URLs / {len(goals)} goals, hash matches")
    sys.exit(1 if bad else 0)
main()
