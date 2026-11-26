#!/usr/bin/env python3
"""CPU.4.b gate: gate.py [--mutant]. Reads ab.tsv (old vs new manuk-mcp on w3c, interleaved). Passes when
(1) new median wait4 total <= 0.70 x the CPU.4 baseline median for w3c manuk-mcp (cpu4/totals.tsv), (2) every run, both arms,
fetched the same 9-image set (names+decoded sizes) and produced the same a11y/observe hash (port masked), (3) README names the cause.
--mutant: swaps old/new rows, so (1) must go red."""
import sys,os,statistics as st
d=os.path.dirname(os.path.abspath(__file__));mut="--mutant" in sys.argv
R=[l.rstrip("\n").split("\t") for l in open(d+"/ab.tsv") if l.strip()]
base=st.median(float(l.split("\t")[3]) for l in open(d+"/../cpu4/totals.tsv") if l.split("\t")[1:3]==["w3c","manuk-mcp"])
new=[float(r[2]) for r in R if r[1]==("old" if mut else "new")];old=[float(r[2]) for r in R if r[1]==("new" if mut else "old")]
bad=[]
if st.median(new)>0.70*base:bad.append(f"new median {st.median(new):.3f} > 0.70 x baseline {base:.3f}")
if len({r[5] for r in R})!=1:bad.append("image set differs between runs")
if len({r[6] for r in R})!=1:bad.append("a11y/observe hash differs between runs")
if "CAUSE:" not in open(d+"/README.md").read():bad.append("README has no CAUSE: line")
print("\n".join(bad) if bad else f"CPU.4.b gate: ok (old median {st.median(old):.3f}, new median {st.median(new):.3f}, baseline {base:.3f})");sys.exit(1 if bad else 0)
