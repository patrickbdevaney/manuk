#!/usr/bin/env python3
"""R0.4.a.c.a.b.a.a.a.a.a.a.a.a: classify release-binary Ungrounded salesforce.com targets by cause.
usage: sf-ungrounded-classify.py <debug.json> <release.json> <release-list.txt> <debug-list.txt> <out.tsv>
Exit 0 = green (>=90% of the drive-probe --list Ungrounded rows classified, debug list has 0 Ungrounded).
"""
import json, re, sys, collections, os
dbg, rel, rlist, dlist, out = sys.argv[1:6]
PIN = os.environ["MANUK_CHROME_PIN"]
A = ("link", "button")
d = json.load(open(dbg)); r = json.load(open(rel))
nobox = lambda n: not n["w"] or not n["h"] or n["w"] <= 0 or n["h"] <= 0
LAND = ("navigation", "banner", "contentinfo", "main", "dialog", "region", "complementary")
def land(t, i):
    while i >= 0:
        if t[i]["role"] in LAND:
            return t[i]["role"] + ":" + t[i]["name"][:30]
        i -= 1
    return "none"
dbg_names = {(n["role"], n["name"].strip()) for n in d if n["role"] in A}
dbg_nav = sum(1 for n in d if n["role"] == "navigation")
rows = collections.Counter(); byname = {}
for i, n in enumerate(r):
    if n["role"] in A and nobox(n):
        box = "no-layout-box" if n["w"] is None else "zero-area-box"
        cause = f"{box} in {land(r, i)}"
        k = (cause, "also-in-debug-tree" if (n["role"], n["name"].strip()) in dbg_names else "absent-from-debug-tree")
        rows[k] += 1
        byname.setdefault((n["role"], n["name"].strip()), cause)
rl = [re.match(r"\s+(\w+)\s+(\w+)\s+\"(.*)\"$", l) for l in open(rlist)]
rl = [m.groups() for m in rl if m]
ung = [(ro, nm) for v, ro, nm in rl if v == "Ungrounded"]
dung = sum(1 for l in open(dlist) if re.match(r"\s+Ungrounded\s", l))
# names in --list are truncated to 60 chars: match by prefix
full = list(byname.items())
cls = sum(1 for ro, nm in ung if any(k[0] == ro and k[1].startswith(nm) for k, _ in full))
pct = 100.0 * cls / max(1, len(ung))
with open(out, "w") as f:
    f.write("cause\tdebug_presence\tcount\tchrome_ver\n")
    for (c, p), n in sorted(rows.items(), key=lambda x: -x[1]):
        f.write(f"{c}\t{p}\t{n}\t{PIN}\n")
    f.write(f"TOTAL_a11y_dump_nobox_targets\t-\t{sum(rows.values())}\t{PIN}\n")
    f.write(f"LIST_ungrounded\t-\t{len(ung)}\t{PIN}\n")
    f.write(f"LIST_ungrounded_classified\t-\t{cls}\t{PIN}\n")
    f.write(f"DEBUG_navigation_landmarks\t-\t{dbg_nav}\t{PIN}\n")
    f.write(f"DEBUG_list_ungrounded\t-\t{dung}\t{PIN}\n")
print(f"classified {cls}/{len(ung)} = {pct:.1f}%  debug_ungrounded={dung}")
sys.exit(0 if (pct >= 90 and dung == 0 and len(ung) > 0) else 1)
