#!/usr/bin/env python3
"""CPU.4 gate: gate.py [dir]. Per page and round, the attributed phase rows (rows not starting with two spaces, excluding TOTAL/SUM) must
sum to within 15% of the wait4 total, in BOTH phases-mcp.tsv and phases-dump.tsv, and totals.tsv must hold a chrome row for each page.
Also every mechanism >= 15% of a page's median total must be named in README.md's follow-up list (a line 'MECH <page> <phase> -> <node id>')."""
import sys,os,collections,statistics as st
d=sys.argv[1] if len(sys.argv)>1 else os.path.dirname(os.path.abspath(__file__))
bad=[]
for arm in("mcp","dump"):
    R=collections.defaultdict(lambda:collections.defaultdict(float));T={}
    for l in open(f"{d}/phases-{arm}.tsv"):
        r,pg,k,v=l.rstrip("\n").split("\t")[:4];v=float(v)
        if k=="TOTAL_rusage":T[(r,pg)]=v
        elif k=="SUM_attributed" or k.startswith("  "):pass
        else:R[(r,pg)][k]+=v
    if not T:bad.append(f"{arm}: no rows")
    for key,tot in T.items():
        s=sum(R[key].values())
        if abs(s-tot)>0.15*tot:bad.append(f"{arm} round {key[0]} {key[1]}: phases sum {s:.3f} vs total {tot:.3f}")
tot=open(f"{d}/totals.tsv").read().split("\n")
for pg in("mdn-css","w3c","rfc-editor"):
    if not any(l.split("\t")[1:3]==[pg,"chrome"] for l in tot if l):bad.append(f"no chrome row for {pg}")
M=collections.defaultdict(list);T2=collections.defaultdict(list)
for l in open(f"{d}/phases-mcp.tsv"):
    r,pg,k,v=l.rstrip("\n").split("\t")[:4]
    if k=="TOTAL_rusage":T2[pg].append(float(v))
    elif k!="SUM_attributed" and not k.startswith("  "):M[(pg,k)].append(float(v))
readme=open(f"{d}/README.md").read()
for (pg,k),v in M.items():
    if st.median(v)>=0.15*st.median(T2[pg]) and f"MECH {pg} {k} ->" not in readme:bad.append(f"mechanism >=15% not routed: {pg} / {k} ({st.median(v):.3f} of {st.median(T2[pg]):.3f})")
print("\n".join(bad) if bad else "CPU.4 gate: ok");sys.exit(1 if bad else 0)
