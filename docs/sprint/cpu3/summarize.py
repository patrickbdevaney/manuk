#!/usr/bin/env python3
import csv,statistics as S,collections,sys,json
V=["s","2","3","hot3"]
rows=[l.rstrip("\n").split("\t") for l in open("pages.tsv")]
assert all(r[7]=="True" for r in rows),"a page run failed"
cpu=collections.defaultdict(list);pss=collections.defaultdict(list);pg=collections.defaultdict(lambda:collections.defaultdict(list))
for r in rows:
    cpu[r[2]].append(float(r[5]));pss[r[2]].append(float(r[6]));pg[r[1]][r[2]].append(float(r[5]))
tot={v:[sum(S.mean(pg[p][v]) for p in pg)] for v in V}
print("pages: per-page mean cpu-s (navigate+observe) over 3 rounds")
print("| page | "+" | ".join(V)+" |");print("|---|"+"---|"*4)
for p in pg: print(f"| {p} | "+" | ".join(f"{S.mean(pg[p][v]):.2f}" for v in V)+" |")
b=tot["s"][0]
print("| **sum of 10** | "+" | ".join(f"{tot[v][0]:.2f} ({(tot[v][0]/b-1)*100:+.1f}%)" for v in V)+" |")
print("| mean peak PSS MB | "+" | ".join(f"{S.mean(pss[v]):.1f}" for v in V)+" |")
# per-round sums for spread
for v in V:
    rs=[sum(float(r[5]) for r in rows if r[2]==v and r[0]==str(k)) for k in range(3)]
    print(v,"per-round sum",[round(x,2) for x in rs])
print("\nCONC")
for v in V:
    rr=[l.rstrip("\n").split("\t") for l in open(f"conc-{v}.tsv")]
    h=rr[0] if rr[0][0]=="chrome_ver" or "cpu_s" in rr[0] else None
    print(v,len(rr),"rows")
    if v=="s":print(rr[0])
