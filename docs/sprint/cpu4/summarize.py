#!/usr/bin/env python3
"""summarize.py: median over rounds of phases-mcp.tsv / phases-dump.tsv and totals.tsv -> markdown tables on stdout."""
import sys,statistics as st,collections,os
here=os.path.dirname(os.path.abspath(__file__))
def load(f):
    d=collections.defaultdict(list)
    for l in open(os.path.join(here,f)):
        r,pg,k,v=l.rstrip("\n").split("\t")[:4];d[(pg,k)].append(float(v))
    return {k:st.median(v) for k,v in d.items()}
pages=["mdn-css","w3c","rfc-editor"]
for arm in("mcp","dump"):
    D=load(f"phases-{arm}.tsv")
    keys=[]
    for (pg,k) in D:
        if not k.startswith("  ") and k not in("TOTAL_rusage","SUM_attributed") and k not in keys:keys.append(k)
    print(f"\n### {arm}: median cpu-s of 3 rounds\n\n| phase | "+" | ".join(pages)+" |\n|---|"+"---|"*len(pages))
    for k in keys:print(f"| {k} | "+" | ".join(f"{D.get((p,k),0):.3f}" for p in pages)+" |")
    print("| **sum of the rows above** | "+" | ".join(f"{D[(p,'SUM_attributed')]:.3f}" for p in pages)+" |")
    print("| **total (wait4 rusage)** | "+" | ".join(f"{D[(p,'TOTAL_rusage')]:.3f}" for p in pages)+" |")
    print("| sum/total | "+" | ".join(f"{D[(p,'SUM_attributed')]/D[(p,'TOTAL_rusage')]*100:.1f}%" for p in pages)+" |")
    print("\npasses (median n / cpu-s per call site):\n")
    for p in pages:
        for (pg,k),v in sorted(D.items()):
            if pg==p and k.startswith("  pass[") and k.endswith(" cpu"):
                print(f"- {p} {k[2:-4]}: n={D.get((p,k[:-4]+' n'),0):.0f}, {v:.3f} cpu-s")
T=collections.defaultdict(list)
for l in open(os.path.join(here,"totals.tsv")):
    r,pg,a,c,ok=l.split("\t")[:5];T[(pg,a)].append((float(c),ok))
print("\n### totals (median of 3, all runs ok)\n\n| arm | "+" | ".join(pages)+" |\n|---|"+"---|"*len(pages))
for a in("manuk-mcp","a11y-dump","chrome"):
    print(f"| {a} | "+" | ".join(f"{st.median(c for c,_ in T[(p,a)]):.3f} (runs {', '.join(f'{c:.2f}' for c,_ in T[(p,a)])}; ok={all(o=='True' for _,o in T[(p,a)])})" for p in pages)+" |")
