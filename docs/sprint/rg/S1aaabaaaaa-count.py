"""RG.S1.a.a.a.b.a.a.a.a.a gate: SLOW RESTYLE+LAYOUT lines inside the 'click:Beds & Baths' step of a
S1aaab-drain.py stderr log, per run. Usage: S1aaabaaaaa-count.py STDERR_LOG MAX   (exit 1 if any run > MAX)"""
import sys,re
runs={};cur=None;buf=[]
for l in open(sys.argv[1],errors='replace'):
    m=re.match(r'### run (\d+) step click:(.*?) \d+ms',l)
    if m:
        if m.group(2)=='Beds & Baths': runs[m.group(1)]=sum('SLOW RESTYLE+LAYOUT' in b for b in buf)
        buf=[];continue
    if l.startswith('### run'): buf=[];continue
    buf.append(l)
print('SLOW per run:',runs)
sys.exit(1 if not runs or any(v>int(sys.argv[2]) for v in runs.values()) else 0)
