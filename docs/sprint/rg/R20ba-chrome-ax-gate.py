#!/usr/bin/env python3
# RG.2.0.b.a.b gate: the Chrome AX measurement must be pinned, complete (3 runs x 3 states) and self-consistent
# with the markdown table. Exit 1 on any mismatch. Mutation: edit chrome_ver in R20ba-chrome-ax.json -> red.
import json, os, sys
d = os.path.dirname(os.path.abspath(__file__))
pin = os.environ["MANUK_CHROME_PIN"]
j = json.load(open(os.path.join(d, "R20ba-chrome-ax.json")))
bad = []
if j["chrome_ver"] != pin or j["pin"] != pin: bad.append(f"chrome_ver {j['chrome_ver']} != pin {pin}")
if len(j["runs"]) != 3: bad.append(f"runs {len(j['runs'])} != 3")
for r in j["runs"]:
    if len(r["states"]) != 3: bad.append(f"run {r['run']} has {len(r['states'])} states")
    for s in r["states"]:
        if s["present_after"] + len(s["absent_after"]) != s["baseline_count"]: bad.append(f"run {r['run']} {s['state']}: counts do not add up")
md = open(os.path.join(d, "R20ba-chrome-ax.md")).read()
if f"chrome_ver={pin}" not in md: bad.append("md lacks chrome_ver line")
if bad: print("RED:", *bad, sep="\n  "); sys.exit(1)
print("GREEN: pin", pin, "3 runs x 3 states, counts consistent")
