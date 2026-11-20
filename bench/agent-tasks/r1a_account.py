#!/usr/bin/env python3
"""W6.R1.a: account for the Chrome-vs-manuk y offset from the two DOM dumps (r1a_above.py / dom-eval). Prints a markdown table.
Alert subtree = elements whose class contains MuiAlert plus descendants (Chrome elements inside the Alert's Paper root box).
Rest = remaining elements, paired in document order by (tag,class,text); share = fraction whose dy equals the header offset +-1px."""
import json, sys, collections
root = sys.argv[1]
MUTANT = len(sys.argv) > 2 and sys.argv[2] == "--mutant-wrong-class"  # explain by the Toolbar instead of the Alert: must go red
bad = []
for s in ["udriver", "staynb"]:
    C = json.load(open(f"{root}/{s}-chrome-dom.json"))["els"]; M = json.load(open(f"{root}/{s}-manuk-dom.json"))["els"]
    alert = next(e for e in C if e["cls"].startswith("MuiPaper-root") and e["h"] > 100 and e["tag"] == "div")  # the Alert root (cls is truncated to 60 chars)
    a0, a1 = alert["y"], alert["y"] + alert["h"]
    inside = [e for e in C if e["y"] >= a0 - .5 and e["y"] + e["h"] <= a1 + .5 and e is not alert and e["x"] >= alert["x"] - .5 and e["w"] <= alert["w"] + .5 and e["y"] < a1 - .5]
    key = lambda e: (e["tag"], e["cls"], e["txt"])
    ids = {id(e) for e in inside} | {id(alert)}
    restC = [e for e in C if id(e) not in ids]
    pool = collections.defaultdict(list)
    for e in M: pool[key(e)].append(e)
    dys, vdys = [], []
    for e in restC:
        l = pool.get(key(e))
        if l:
            d = round(e["y"] - l.pop(0)["y"], 1); dys.append(d)
            if e["y"] < 768: vdys.append(d)
    off = next(e for e in C if e["cls"].startswith("MuiToolbar") or e["cls"].startswith("MuiInputBase-root"))["h"] if MUTANT else alert["h"]
    anchor = next(e for e in restC if e["cls"].startswith("MuiBox-root css-vfqg9a") or e["tag"] == "header")  # first content block below the alert
    mh = next(e for e in M if key(e) == key(anchor)); hdr = anchor
    hoff = hdr["y"] - mh["y"]
    ok = sum(1 for d in dys if abs(d - hoff) <= 1)
    print(f"## {s}\nalert root: {alert['tag']} y={alert['y']:.0f} h={alert['h']:.0f}; elements inside alert: {len(inside)+1} (manuk has 0 MuiAlert elements: {sum(1 for e in M if e['cls'].startswith('MuiAlert') or (e['cls'].startswith('MuiPaper-root') and e['h'] > 100))})")
    print(f"first-content y chrome={hdr['y']:.0f} manuk={mh['y']:.0f} offset={hoff:.0f}; alert height={off:.0f}; alert explains {min(off,hoff)/hoff*100:.1f}% of offset")
    print(f"non-alert elements paired: {len(dys)}; dy within 1px of header offset: {ok} ({ok/max(1,len(dys))*100:.1f}%); chrome DOM total {len(C)} manuk {len(M)}")
    vok = sum(1 for d in vdys if abs(d - hoff) <= 1)
    print(f"first viewport only (chrome y<768): paired {len(vdys)}, dy within 1px of offset: {vok} ({vok/max(1,len(vdys))*100:.1f}%)")
    if min(off, hoff) / hoff < 0.9: bad.append(s)
    print("dy histogram (top 5):", collections.Counter(dys).most_common(5), "\n")
print("GATE", "RED (explained < 90%): " + ",".join(bad) if bad else "GREEN (single class explains >=90% on both sites)")
sys.exit(1 if bad else 0)
