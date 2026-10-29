#!/usr/bin/env python3
"""TOKENS.1.a.a: manuk (r216 baseline transcripts) vs cdp-layer (arm3 full transcripts), same own tasks.
Per tool: calls and tool-result bytes per run; per task: assistant turns and result bytes. Writes bytes.tsv, tasks.tsv.
Exits 1 if either arm has no transcripts or the arms share no task."""
import collections, glob, json, os, sys
R = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))
DIRS = {'manuk': 'docs/sprint/r216/transcripts/manuk_*_r*.jsonl',
        'cdp-layer': 'docs/sprint/arm3/transcripts-full/cdp-layer_*_r*.jsonl'}
CHROME = os.environ.get('MANUK_CHROME_PIN', '145.0.7632.116')

def scan(path):
    names, tools, turns = {}, collections.defaultdict(lambda: [0, 0]), 0
    for line in open(path):
        d = json.loads(line)
        m = d.get('message')
        if not isinstance(m, dict) or not isinstance(m.get('content'), list):
            continue
        if d.get('type') == 'assistant':
            turns += 1
        for c in m['content']:
            if c.get('type') == 'tool_use':
                n = c['name'].replace('mcp__browser__', '').replace('mcp__manuk__', '')
                names[c['id']] = n; tools[n][0] += 1
            elif c.get('type') == 'tool_result':
                b = c.get('content')
                s = len(b) if isinstance(b, str) else len(json.dumps(b))
                tools[names.get(c.get('tool_use_id'), 'other')][1] += s
    return tools, turns

data = {}
for arm, pat in DIRS.items():
    for p in sorted(glob.glob(os.path.join(R, pat))):
        task = os.path.basename(p).split('_')[1]
        data.setdefault(arm, {}).setdefault(task, []).append(scan(p))
if len(data) < 2:
    print('missing arm transcripts', file=sys.stderr); sys.exit(1)
common = sorted(set(data['manuk']) & set(data['cdp-layer']))
common = [t for t in common if not t.startswith(('I', 'V'))]
if not common:
    print('no common tasks', file=sys.stderr); sys.exit(1)

def agg(arm):
    tot = collections.defaultdict(lambda: [0, 0]); runs = turns = 0
    for t in common:
        for tools, tu in data[arm][t]:
            runs += 1; turns += tu
            for n, (c, b) in tools.items():
                tot[n][0] += c; tot[n][1] += b
    return tot, runs, turns
A, B = agg('manuk'), agg('cdp-layer')
with open(os.path.join(R, 'docs/sprint/tokens1aa/bytes.tsv'), 'w') as f:
    f.write('tool\tmanuk_calls_per_run\tcdp_calls_per_run\tmanuk_result_bytes_per_run\tcdp_result_bytes_per_run\tdelta_bytes_per_run\tchrome_ver\n')
    for n in sorted(set(A[0]) | set(B[0]), key=lambda n: -(A[0][n][1] / A[1] - B[0][n][1] / B[1])):
        a, b = A[0][n], B[0][n]
        f.write(f'{n}\t{a[0]/A[1]:.2f}\t{b[0]/B[1]:.2f}\t{a[1]/A[1]:.0f}\t{b[1]/B[1]:.0f}\t{a[1]/A[1]-b[1]/B[1]:+.0f}\t{CHROME}\n')
with open(os.path.join(R, 'docs/sprint/tokens1aa/tasks.tsv'), 'w') as f:
    f.write('task\tmanuk_turns\tcdp_turns\tmanuk_result_bytes\tcdp_result_bytes\tdelta_bytes\tchrome_ver\n')
    rows = []
    for t in common:
        def g(arm):
            r = data[arm][t]
            return (sum(x[1] for x in r) / len(r), sum(sum(b for _, b in x[0].values()) for x in r) / len(r))
        (mt, mb), (ct, cb) = g('manuk'), g('cdp-layer')
        rows.append((mb - cb, t, mt, ct, mb, cb))
    for d, t, mt, ct, mb, cb in sorted(rows, reverse=True):
        f.write(f'{t}\t{mt:.1f}\t{ct:.1f}\t{mb:.0f}\t{cb:.0f}\t{d:+.0f}\t{CHROME}\n')
print(f'runs manuk={A[1]} cdp={B[1]} turns/run manuk={A[2]/A[1]:.2f} cdp={B[2]/B[1]:.2f}')
