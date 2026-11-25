#!/usr/bin/env python3
"""AXES.1: generate docs/sprint/AXES.md from published TSVs; check that every losing axis has a node or a reason.

  axes-register.py gen     write docs/sprint/AXES.md
  axes-register.py check   AXES.md is current AND every losing/unmeasured axis has a live node id or a written reason
Env: AXES_GRID / AXES_NODES / AXES_GRAPH / AXES_OUT / AXES_PW / AXES_MK override paths (used by the mutation test).
"""
import csv, json, os, re, sys, statistics as st
R = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
P = lambda k, d: os.environ.get(k) or os.path.join(R, d)
GRID = P('AXES_GRID', 'docs/sprint/conc2/layer1-grid.tsv')
NODES = P('AXES_NODES', 'docs/sprint/AXES-NODES.tsv')
GRAPH = P('AXES_GRAPH', 'docs/sprint/GRAPH.tsv')
OUT = P('AXES_OUT', 'docs/sprint/AXES.md')
PW = P('AXES_PW', 'bench-queue/out/w6f1aa-playwright.tsv')
MK = P('AXES_MK', 'bench-queue/out/w6f1ab-manuk.tsv')
TOL = 0.05  # manuk is "behind" only when worse than the best Chrome arm by more than 5%
MANUK = ['engine-proc-full', 'engine-proc-lean', 'engine-fork']
CHROME = ['chrome-mcp', 'pw-proc', 'pw-shared']
REAL = ('rfc', 'w3c', 'mdn')
rel = lambda p: os.path.relpath(p, R)

def load_grid():
    rows = []
    with open(GRID) as f:
        for i, r in enumerate(csv.DictReader(f, delimiter='\t'), start=2):
            r['_line'] = i
            rows.append(r)
    return rows

def cell(rows, arm, n):
    c = [r for r in rows if r['arm'] == arm and r['n'] == str(n)]
    return c if len(c) >= 3 else []

def fnum(x):
    try: return float(x)
    except Exception: return None

def real_success(c):
    a = b = 0
    for r in c:
        for k, v in json.loads(r['per_trace']).items():
            if k in REAL:
                x, y = v.split('/'); a += int(x); b += int(y)
    return a / b if b else None

def metric(c, name):
    if name == 'pss_per_ok': vals = [fnum(r['pss_per_ok_mb']) for r in c]
    elif name == 'cpu_per_ok':
        vals = [fnum(r['cpu_s']) / max(int(r['traces_pass']), 1) for r in c]
    else: vals = [fnum(r[name]) for r in c]
    vals = [v for v in vals if v is not None]
    return (st.mean(vals), min(vals), max(vals), len(vals)) if len(vals) == len(c) and c else None

def src(c):
    return 'layer1-grid.tsv L' + ','.join(str(r['_line']) for r in c)

# (axis id, label, metric, N, lower_is_better, manuk arms considered)
AXES = [
 ('mem-n10', 'memory: peak PSS per ok trace, N=10 (MB)', 'pss_per_ok', 10, True, MANUK),
 ('mem-n25', 'memory: peak PSS per ok trace, N=25 (MB)', 'pss_per_ok', 25, True, MANUK),
 ('cpu-n1', 'cpu per task: CPU-s per passed trace, N=1', 'cpu_per_ok', 1, True, MANUK),
 ('cpu-n10', 'cpu per task: CPU-s per passed trace, N=10', 'cpu_per_ok', 10, True, MANUK),
 ('p50-n10', 'latency p50 per tool call, N=10 (ms)', 'p50_ms', 10, True, MANUK),
 ('p95-n1', 'latency p95 per tool call, N=1 (ms)', 'p95_ms', 1, True, MANUK),
 ('p95-n10', 'latency p95 per tool call, N=10 (ms)', 'p95_ms', 10, True, MANUK),
 ('p95-n25', 'latency p95 per tool call, N=25 (ms)', 'p95_ms', 25, True, MANUK),
 ('p95-lean-n5', 'latency p95, lean profile only, N=5 (ms)', 'p95_ms', 5, True, ['engine-proc-lean']),
 ('cold-n1', 'cold start: first-call p50, N=1 (ms)', 'cold_p50_ms', 1, True, MANUK),
 ('succ-n25', 'success at N=25 (fraction of traces passed)', 'success', 25, False, MANUK),
 ('succ-n50', 'success at N=50 (fraction; an arm OOM-killed in the cage counts 0)', 'success', 50, False, MANUK),
 ('succ-n100', 'success at N=100 (fraction; an arm OOM-killed in the cage counts 0)', 'success', 100, False, MANUK),
]
NS = [1, 5, 10, 25, 50, 100]

def fmt(m, d=2):
    return 'n/a (no surviving cell)' if m is None else f'{m[0]:.{d}f} [{m[1]:.{d}f}-{m[2]:.{d}f}] n={m[3]}'

def best(rows, arms, name, n, lower):
    cands = []
    for a in arms:
        c = cell(rows, a, n)
        m = metric(c, name) if c else None
        if m: cands.append((m[0], a, m, c))
    if not cands:
        return None
    return (min if lower else max)(cands, key=lambda t: t[0])

def evaluate():
    rows = load_grid(); out = []
    for aid, label, name, n, lower, marms in AXES:
        bm, bc = best(rows, marms, name, n, lower), best(rows, CHROME, name, n, lower)
        if bm is None: verdict = 'unmeasured'
        elif bc is None:
            # Chrome arms produced no complete cell: for success they count 0, otherwise manuk wins by survival
            verdict = 'win (every Chrome arm OOM-killed in the 8 GB cage)'
        else:
            a, b = bm[0], bc[0]
            if b == 0: ratio_bad = (a > 0) if lower else False
            else: ratio_bad = (a > b * (1 + TOL)) if lower else (a < b * (1 - TOL))
            ratio_win = (a < b * (1 - TOL)) if lower else (a > b * (1 + TOL))
            verdict = 'LOSS' if ratio_bad else ('win' if ratio_win else 'tie (within 5%)')
        out.append(dict(id=aid, label=label, m=bm, c=bc, verdict=verdict,
                        mtxt=f'{bm[1]}: {fmt(bm[2])}' if bm else '-', ctxt=f'{bc[1]}: {fmt(bc[2])}' if bc else 'all Chrome arms died (OOM)',
                        src=(src(bm[3]) if bm else '') + ((' | ' + bc[1] + ' ' + src(bc[3])) if bc else ' | Chrome arms: no rows (OOM-killed cells, see conc2/analysis.md)')))
    # capacity at 8 GB on real pages
    def cap(arm):
        c1 = cell(rows, arm, 1); base = real_success(c1) if c1 else None
        best_n = None
        for n in NS:
            c = cell(rows, arm, n); s = real_success(c) if c else 0.0
            if base is not None and s is not None and s >= base - TOL: best_n = n
            else: break
        return best_n
    caps = {a: cap(a) for a in MANUK + CHROME}
    bmn = max(MANUK, key=lambda a: caps[a] or 0); bcn = max(CHROME, key=lambda a: caps[a] or 0)
    v = 'LOSS' if caps[bmn] < caps[bcn] * (1 - TOL) else ('win' if caps[bmn] > caps[bcn] * (1 + TOL) else 'tie (within 5%)')
    out.append(dict(id='capacity-8gb', label='agent capacity at 8 GB (largest ladder N, real-page traces rfc/w3c/mdn, within 5 pts of own N=1; ladder resolution only, bisection not done)',
                    m=None, c=None, verdict=v, mtxt=f'{bmn}: {caps[bmn]}', ctxt=f'{bcn}: {caps[bcn]}',
                    src='layer1-grid.tsv per_trace column, all rows of each arm (all caps: ' + ', '.join(f'{a}={caps[a]}' for a in MANUK + CHROME) + ')'))
    # saturation: best manuk arm at N=100 against its own N=1 (no Chrome arm survives to compare)
    b1, b100 = best(rows, MANUK, 'success', 1, False), best(rows, MANUK, 'success', 100, False)
    out.append(dict(id='succ-n100-vs-self', label='success at N=100 against the same arm at N=1 (cores saturated; the Chrome arms are dead there, so the comparison is manuk against its own N=1)', m=None, c=None,
                    verdict='LOSS' if b100[0] < b1[0] * (1 - TOL) else 'tie (within 5%)',
                    mtxt=f'{b100[1]}: {fmt(b100[2])}', ctxt=f'{b1[1]} at N=1: {fmt(b1[2])}', src=src(b100[3]) + ' | ' + src(b1[3])))
    # isolation
    def iso(arms):
        r = [x for x in rows if x['arm'] in arms and int(x['n']) <= 25]; return sum(x['isolation_ok'] == 'True' for x in r), len(r)
    (mi, mt), (ci, ct) = iso(MANUK), iso(CHROME)
    out.append(dict(id='isolation', label='isolation: cells with isolation_ok=True (cross-session leak probe, trace "iso"), N<=25 where Chrome arms have cells; the 2 N=100 manuk cells that read False are 50 s tool timeouts under saturation (fail_reasons), counted in the N=100 success row', m=None, c=None,
                    verdict='LOSS' if mi / mt < ci / ct else ('win' if mi / mt > ci / ct else 'tie (equal rate)'),
                    mtxt=f'manuk arms: {mi}/{mt} cells', ctxt=f'chrome arms: {ci}/{ct} cells', src='layer1-grid.tsv isolation_ok column, rows with n<=25'))
    # 102-task real pass rate
    def passes(path):
        by = {}
        for r in csv.DictReader(open(path), delimiter='\t'):
            by.setdefault(r['run'], {})[r['task']] = int(r['pass'] or 0)
        return {k: sum(v.values()) for k, v in sorted(by.items())}, {k: len(v) for k, v in by.items()}
    mp, mn = passes(MK); pp, pn = passes(PW)
    mm, pm = st.mean(mp.values()), st.mean(pp.values())
    tot = max(mn.values())
    out.append(dict(id='pass-102', label=f'success on the real {tot}-task suite (tasks passed per run, 3 runs each, Haiku, judge gpt-oss-120b)', m=None, c=None,
                    verdict='LOSS' if mm < pm - 1e-9 and (pm - mm) / tot > 0.0 else ('win' if mm > pm else 'tie'),
                    mtxt=f'manuk: {"/".join(map(str, mp.values()))} of {tot}, mean {mm:.1f}', ctxt=f'playwright+Chrome: {"/".join(map(str, pp.values()))} of {tot}, mean {pm:.1f} (delta {mm-pm:+.1f}; the two files were run at different times)',
                    src=f'{rel(MK)} (one build, no CPU.1) | {rel(PW)}'))
    out.append(dict(id='disk', label='disk: install footprint (binary + data) manuk vs Chrome', m=None, c=None, verdict='unmeasured',
                    mtxt='-', ctxt='-', src='no published TSV row exists for disk'))
    return out

def render(ev):
    L = ['# Axis register', '',
         'Generated by `scripts/axes-register.py gen` from the published TSVs. Do not edit by hand; `axes-register.py check` fails if this file is stale.', '',
         f'- chrome_ver: {os.environ.get("MANUK_CHROME_PIN") or open(os.path.join(R, "docs/sprint/CHROME_PIN")).read().split()[0]} (every row below)',
         '- Grid axes: 3 runs per (arm, N) cell, 8 GB cage, mean [min-max], best manuk arm vs best Chrome arm (lower or higher is better per row). "Behind" = worse than the best Chrome arm by more than 5%.',
         '- A loss must carry an open node id or a written reason in `docs/sprint/AXES-NODES.tsv`; a win carries neither. No row here is a claim beyond its sources.', '',
         '| axis | what | best manuk arm | best Chrome arm | verdict | node / reason | sources |', '|---|---|---|---|---|---|---|']
    nodes = load_nodes()
    for e in ev:
        nd = nodes.get(e['id'])
        link = '' if not nd else (nd[0] if nd[0] != '-' else 'reason: ' + nd[1])
        if nd and nd[0] != '-': link += ': ' + nd[1]
        if e['verdict'] == 'LOSS' or e['verdict'] == 'unmeasured':
            link = link or '**MISSING**'
        else: link = link or '-'
        L.append(f"| {e['id']} | {e['label']} | {e['mtxt']} | {e['ctxt']} | {e['verdict']} | {link} | {e['src']} |")
    L += ['', f"Losing or unmeasured: {sum(e['verdict'] in ('LOSS','unmeasured') for e in ev)} of {len(ev)} axes.", '']
    return '\n'.join(L)

def load_nodes():
    d = {}
    if os.path.exists(NODES):
        for r in csv.DictReader(open(NODES), delimiter='\t'):
            d[r['axis']] = (r['node'], r['text'])
    return d

def graph_ids():
    return {l.split('\t')[0] for l in open(GRAPH)}

def check():
    ev = evaluate(); bad = []
    try: cur = open(OUT).read()
    except FileNotFoundError: cur = None
    if cur != render(ev): bad.append(f'{rel(OUT)} is stale or missing; run axes-register.py gen')
    nodes, ids = load_nodes(), graph_ids()
    for e in ev:
        if e['verdict'] not in ('LOSS', 'unmeasured'): continue
        nd = nodes.get(e['id'])
        if not nd: bad.append(f"axis {e['id']} is {e['verdict']} with no node and no reason"); continue
        node, text = nd
        if node != '-' and node not in ids: bad.append(f"axis {e['id']}: node {node} is not in GRAPH.tsv")
        if node == '-' and len(text.strip()) < 40: bad.append(f"axis {e['id']}: reason too short to be a reason")
    for b in bad: print('FAIL:', b)
    if not bad: print(f'ok: {len(ev)} axes, every loss/unmeasured axis has a live node id or a written reason')
    return 1 if bad else 0

if __name__ == '__main__':
    cmd = sys.argv[1] if len(sys.argv) > 1 else ''
    if cmd == 'gen': open(OUT, 'w').write(render(evaluate())); print('wrote', rel(OUT))
    elif cmd == 'check': sys.exit(check())
    else: print(__doc__); sys.exit(2)
