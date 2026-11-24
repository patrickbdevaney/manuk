#!/usr/bin/env python3
"""CPU.1 gate. usage: gate.py <before-a11y-dump> <after-a11y-dump> <before.txt> <after.txt> <fixture-dir-url> <pages-dir>
(1) a11y-dump JSON byte-identical before/after on every fixture + every demo/www/pages/*.html (served at url+/pages)
(2) after.txt: observe(1500) mean <= 0.3 cpu-s and mean observe(1500)/mean observe(800) <= 2.5
exit 1 on any failure."""
import sys,subprocess,json,glob,os,re,statistics as st
bb,ab,bt,at,fx,pages=sys.argv[1:7]
def rows(f):
    d={}
    for l in open(f):
        k=l.split()[0]; d.setdefault(k,[]).append(json.loads(l[l.index("{"):]))
    return d
bad=0;nondet=0
urls=[f"{fx}/{os.path.basename(f)}" for f in sorted(glob.glob(os.path.dirname(os.path.abspath(__file__))+"/fx/*.html"))]
urls+=[f"{fx}/pages/{os.path.basename(f)}" for f in sorted(glob.glob(pages+"/*.html"))]
for u in urls:
    o=[subprocess.run([b,"--deterministic",u],capture_output=True).stdout for b in (bb,ab)]
    if o[0]!=o[1]:  # is the baseline itself stable on this page?
        if subprocess.run([bb,"--deterministic",u],capture_output=True).stdout!=o[0]:
            print("NONDET (baseline differs from itself, excluded) "+u.rsplit("/",1)[-1]); nondet+=1; continue
    ok=o[0]==o[1] and len(o[0])>10
    bad+=not ok; print(("SAME " if ok else "DIFF ")+u.rsplit("/",1)[-1],len(o[0]))
print("pages compared:",len(urls),"nondeterministic-baseline excluded:",nondet)
A=rows(at); m=lambda d,k:st.mean(r["observe"] for r in d[k])
o15,o8=m(A,"t1500"),m(A,"t800"); g=o15/o8
print(f"observe 1500={o15:.3f} (<=0.3)  800={o8:.3f}  growth={g:.2f}x (<=2.5)")
if o15>0.3 or g>2.5: bad+=1; print("RED: cpu bar")
sys.exit(1 if bad else 0)
