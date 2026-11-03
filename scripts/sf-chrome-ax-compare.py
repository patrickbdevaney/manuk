#!/usr/bin/env python3
# R0.4.a.c.a.b.a.a.a.a.a.a.a.a.b: classify manuk's no-layout-box link/button nodes against Chrome's AX tree.
# usage: sf-chrome-ax-compare.py <manuk-release.json> <chrome-ax.json> <out.tsv>
# Per manuk nobox node (role,name): chrome-ignored | chrome-absent | chrome-exposed-nobox | chrome-exposed-with-box.
# Classified = first two (Chrome does not expose it as a target) + third (Chrome exposes it equally without a box).
# Exit 0 only if chrome tree captured (>0 link/button nodes), chrome_ver==pin, and >=90% classified.
import json, sys, os, collections
rel, ch, out = sys.argv[1:4]; PIN = os.environ["MANUK_CHROME_PIN"]
r = json.load(open(rel)); c = json.load(open(ch))
if c.get("chrome_ver") != PIN or not c["nodes"]: print("chrome tree missing or ver mismatch"); sys.exit(2)
nobox = lambda w, h: not w or not h or w <= 0 or h <= 0
cm = collections.defaultdict(set)
for n in c["nodes"]:
    st = "ignored" if n["ignored"] else ("nobox" if n["box"] is None or nobox(*n["box"]) else "box")
    cm[(n["role"], n["name"])].add(st)
cnt = collections.Counter()
for n in r:
    if n["role"] in ("link", "button") and nobox(n["w"], n["h"]):
        s = cm.get((n["role"], n["name"].strip()))
        k = "chrome-absent" if not s else ("chrome-exposed-with-box" if "box" in s else ("chrome-exposed-nobox" if "nobox" in s else "chrome-ignored"))
        cnt[k] += 1
tot = sum(cnt.values()); ok = tot - cnt["chrome-exposed-with-box"]
ex = sum(1 for n in c["nodes"] if not n["ignored"] and n["box"] and not nobox(*n["box"]))
with open(out, "w") as f:
    f.write("class\tcount\tchrome_ver\n")
    for k, v in cnt.most_common(): f.write(f"{k}\t{v}\t{PIN}\n")
    f.write(f"TOTAL_manuk_nobox_targets\t{tot}\t{PIN}\nCLASSIFIED\t{ok}\t{PIN}\nCHROME_exposed_boxed_link_button\t{ex}\t{PIN}\nCHROME_total_link_button_nodes\t{len(c['nodes'])}\t{PIN}\n")
pct = 100.0 * ok / max(1, tot); print(dict(cnt), f"classified {ok}/{tot}={pct:.1f}%")
sys.exit(0 if tot > 0 and pct >= 90 else 1)
