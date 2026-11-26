#!/usr/bin/env python3
"""AXES.1.c: per-task pass counts, manuk vs playwright, from the published 3-run TSVs.

  axes-pass102.py gen     write docs/sprint/axes1c/pass102.tsv and pass102.md
  axes-pass102.py check   outputs current; sum of per-task deltas / 3 == mean(run totals) diff; every manuk-only failure has a class
Env: P102_MK / P102_PW / P102_OUT override paths (mutation test).
"""
import csv, collections, os, re, sys
R = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
P = lambda k, d: os.environ.get(k) or os.path.join(R, d)
MK = P('P102_MK', 'bench-queue/out/w6f1ab-manuk.tsv')
PW = P('P102_PW', 'bench-queue/out/w6f1aa-playwright.tsv')
OUT = P('P102_OUT', 'docs/sprint/axes1c/pass102.tsv')
MD = OUT[:-4] + '.md'
CLASSES = ('declined-no-control', 'stopped-before-payment', 'partial-state-mismatch', 'answered-wrong')

def load(f):
    d = collections.defaultdict(dict)
    for r in csv.DictReader(open(f), delimiter='\t'): d[r['task']][int(r['run'])] = r
    return d

def classify(rows):
    """Reason class from the failing manuk runs' answers and eval fields (rule order is the precedence)."""
    fails = [r for r in rows.values() if r['pass'] == '0']
    cs = []
    for r in fails:
        a = r['answer'].lower(); en, eo = int(r['evals_n']), int(r['evals_ok'])
        if re.search(r'(payment|card details|confirm and pay)', a) and 'did not submit' in a: cs.append('stopped-before-payment')
        elif re.match(r"\W*(i could not|i couldn't|i can't|i did not complete|i was unable)", a): cs.append('declined-no-control')
        elif en and eo < en or (en and eo == en and r['llm_n'] == '0'): cs.append('partial-state-mismatch')
        else: cs.append('answered-wrong')
    return collections.Counter(cs).most_common(1)[0][0]

def build():
    m, p = load(MK), load(PW); out = []
    for t in sorted(set(m) | set(p)):
        mp = sum(int(r['pass']) for r in m.get(t, {}).values()); pp = sum(int(r['pass']) for r in p.get(t, {}).values())
        only = mp <= 1 and pp >= 2
        out.append(dict(task=t, manuk=mp, pw=pp, delta=mp - pp, manuk_only_fail='1' if only else '0',
                        reason=classify(m[t]) if only else '-',
                        evidence=(' || '.join(f"r{k}:{r['answer'][:110]}" for k, r in sorted(m[t].items()) if r['pass'] == '0').replace('\t', ' ').replace('\n', ' ') if only else '-')))
    return out, m, p

def render(rows, m, p):
    tot = lambda d, k: sum(int(d[t][k]['pass']) for t in d for _ in [0] if k in d[t])
    mt = [tot(m, k) for k in (1, 2, 3)]; pt = [tot(p, k) for k in (1, 2, 3)]
    tsv = '\n'.join(['\t'.join(['task', 'manuk_pass_of3', 'pw_pass_of3', 'delta', 'manuk_only_fail', 'reason', 'evidence'])] +
                    ['\t'.join(str(r[k]) for k in ('task', 'manuk', 'pw', 'delta', 'manuk_only_fail', 'reason', 'evidence')) for r in rows]) + '\n'
    d = sum(r['delta'] for r in rows)
    md = [f'# AXES.1.c per-task pass attribution (chrome_ver=145.0.7632.116)', '',
          f'Sources: `{os.path.relpath(MK, R)}`, `{os.path.relpath(PW, R)}` (3 runs x 102 tasks each; the arms ran at different times).', '',
          f'- manuk per-run passes {mt} (mean {sum(mt)/3:.2f}); playwright {pt} (mean {sum(pt)/3:.2f}); mean diff {(sum(mt)-sum(pt))/3:+.2f}.',
          f'- sum of per-task deltas = {d} -> /3 = {d/3:+.2f}.',
          f'- manuk-only failures (manuk passes <=1 of 3, playwright >=2 of 3): {sum(r["manuk_only_fail"]=="1" for r in rows)} tasks, net delta {sum(r["delta"] for r in rows if r["manuk_only_fail"]=="1")} of the {d} total (the other tasks net {d-sum(r["delta"] for r in rows if r["manuk_only_fail"]=="1"):+d}, i.e. manuk wins elsewhere).',
          f'- tasks with any delta: {sum(r["delta"]!=0 for r in rows)}; the rest of the gap is single-run flips (not attributable to one task from 3 runs).', '',
          '| task | manuk/3 | pw/3 | class |', '|---|---|---|---|'] + \
         [f"| {r['task']} | {r['manuk']} | {r['pw']} | {r['reason']} |" for r in rows if r['manuk_only_fail'] == '1'] + ['']
    return tsv, '\n'.join(md), d, mt, pt

def check():
    rows, m, p = build(); tsv, md, d, mt, pt = render(rows, m, p); bad = []
    if d != sum(mt) - sum(pt): bad.append(f'sum of deltas {d} != run-total diff {sum(mt)-sum(pt)}')
    for r in rows:
        if r['manuk_only_fail'] == '1' and r['reason'] not in CLASSES: bad.append(f"{r['task']}: no class")
    for f, s in ((OUT, tsv), (MD, md)):
        if not os.path.exists(f) or open(f).read() != s: bad.append(f'{os.path.relpath(f, R)} stale; run gen')
    for b in bad: print('FAIL:', b)
    if not bad: print(f'ok: sum of deltas {d} (mean {d/3:+.2f}), {sum(r["manuk_only_fail"]=="1" for r in rows)} manuk-only failures classified')
    return 1 if bad else 0

if __name__ == '__main__':
    c = sys.argv[1] if len(sys.argv) > 1 else ''
    if c == 'gen':
        rows, m, p = build(); tsv, md, *_ = render(rows, m, p)
        os.makedirs(os.path.dirname(OUT), exist_ok=True); open(OUT, 'w').write(tsv); open(MD, 'w').write(md); print('wrote', OUT)
    elif c == 'check': sys.exit(check())
    else: print(__doc__); sys.exit(2)
