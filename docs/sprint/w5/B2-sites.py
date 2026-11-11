#!/usr/bin/env python3
"""W5.B2.a: recompute WebArena-Verified hard-subset site footprint vs the disk wall.
Run: python3 -I docs/sprint/w5/B2-sites.py [--add SITE ...] [--fs /home|/]. Exit 1 if chosen subset exceeds the wall.
Sizes are docs/environments sizes (on-disk, optimized images) + external data dirs; registry compressed sizes are in B2-sites.md."""
import json, os, shutil, sys
D = os.path.dirname(os.path.abspath(__file__)) + "/data/"
IMG = {"shopping": 13.3, "shopping_admin": 2.9, "reddit": 8.41, "gitlab": 31.6, "wikipedia": 0.115, "map": 3.28}
DATA = {"wikipedia": 100.0, "map": 60.0}  # external downloads required before start
RESERVE = 25.0  # GB, disk-hygiene reserve
CHOSEN = ["shopping", "shopping_admin", "reddit", "gitlab"]
FS_DEFAULT = "/"  # docker data-root /var/lib/docker lives on /
args = sys.argv[1:]
chosen = list(CHOSEN)
fs = FS_DEFAULT
while args:
    a = args.pop(0)
    if a == "--add": chosen.append(args.pop(0))
    elif a == "--fs": fs = args.pop(0)
sites = json.load(open(D + "B2-hard-sites.json"))
assert len(sites) == 258, len(sites)
cov = sum(1 for s in sites.values() if set(s) <= set(chosen))
need = sum(IMG[s] + DATA.get(s, 0) for s in chosen)
free = shutil.disk_usage(fs).free / 1e9
budget = free - RESERVE
for s in IMG:
    n = sum(1 for v in sites.values() if s in v)
    print(f"{s:15s} image {IMG[s]:7.3f} GB data {DATA.get(s,0):6.1f} GB tasks_touching {n}")
print(f"chosen={chosen} covers {cov}/258 tasks; need {need:.1f} GB; {fs} free {free:.1f} GB - reserve {RESERVE} = budget {budget:.1f} GB")
if need > budget:
    print("FAIL: exceeds the wall"); sys.exit(1)
print("OK")
