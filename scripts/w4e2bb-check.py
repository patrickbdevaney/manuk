#!/usr/bin/env python3
"""W4.E2.b.b gate: v2 runs.tsv -> every URL loads OK in >=2 of 3 runs in both arms (each mode), and >=5 URLs show >=3 post-JS images.
exit 0 GREEN, 1 RED. --mutant marks the runs of one URL's lean arm as TIMEOUT; must exit 1.
Run: python3 -I scripts/w4e2bb-check.py [--mutant] [runs.tsv]"""
import csv, collections, sys
args = [a for a in sys.argv[1:] if not a.startswith("--")]
path = args[0] if args else "docs/sprint/w4e2b-v2/runs.tsv"
rows = list(csv.DictReader(open(path), delimiter="\t"))
if "--mutant" in sys.argv:
    t = rows[0]["url"]
    for r in rows:
        if r["url"] == t and r["arm"] == "lean": r["status"] = "TIMEOUT"
c = collections.defaultdict(int)
for r in rows:
    if r["status"] == "OK": c[(r["url"], r["mode"], r["arm"])] += 1
cells = {(r["url"], r["mode"], r["arm"]) for r in rows}
bad = [k for k in cells if c[k] < 2]
imgs = {r["url"]: r["img_post_js"] for r in rows}
heavy = sum(1 for v in imgs.values() if v.isdigit() and int(v) >= 3)
print(f"urls={len(imgs)} cells_lt2_ok={len(bad)} urls_ge3_img={heavy}")
ok = not bad and heavy >= 5 and len(imgs) == 10
print("GREEN" if ok else "RED")
sys.exit(0 if ok else 1)
