"""RG.S1.a.a.a.b gate: exit 1 if any zilloft panel click drain in TSV exceeds LIMIT_MS (default 10000) or
the TSV is not at the pinned chrome_ver / has a NOHANDLE step. Usage: S1aaab-gate.py TSV [LIMIT_MS]"""
import sys,os
rows=[l.rstrip('\n').split('\t') for l in open(sys.argv[1])][1:]
lim=int(sys.argv[2]) if len(sys.argv)>2 else 10000
pin=os.environ.get('MANUK_CHROME_PIN','')
bad=[r for r in rows if r[0]!=pin]
if bad: print('wrong chrome_ver',bad[0][0]); sys.exit(2)
clicks=[(r[1],r[2],int(r[3])) for r in rows if r[2].startswith('click:')]
if any(ms<0 for _,_,ms in clicks): print('NOHANDLE step'); sys.exit(1)
over=[c for c in clicks if c[2]>lim]
print(f'{len(clicks)} clicks, max {max(c[2] for c in clicks)} ms, limit {lim}, over: {over}')
sys.exit(1 if over else 0)
