import re,sys,os
# usage: check-page.py <probe-out> <page-dir>
# Counts WebMCP.toolsAdded events in a probe run, writes the distinct tool names (probe self-test 'zz' excluded)
# to <page-dir>/names.txt, and exits 0 only if >=1 event arrived. Zero events = no-panel: exit 1, not counted.
out, pagedir = sys.argv[1], sys.argv[2]
n = 0
got = set()
for line in open(out, encoding='utf8'):
    if '"method":"WebMCP.toolsAdded"' in line:
        n += 1
        got.update(re.findall(r'\{"name":"([^"]+)","description"', line))
names = sorted(got - {"zz"})
if n:
    os.makedirs(pagedir, exist_ok=True)
    with open(os.path.join(pagedir, 'names.txt'), 'w', encoding='utf8') as f:
        f.write('\n'.join(names) + '\n')
print(f"file={out} toolsAdded_events={n} distinct_names={len(names)} names={names} verdict={'PANEL' if n else 'NO-PANEL'}")
sys.exit(0 if n >= 1 else 1)
