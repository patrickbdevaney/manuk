#!/usr/bin/env python3
"""W6.F1.a: REAL-fast v2, held-out. Rule fixed BEFORE looking at the test rounds (written before first run):
  SELECTION rounds (only these may inform the subset): W5.B1 manuk/playwright/cdp-layer + RG.B manuk/playwright.
  Per task: pooled pass rate p over all selection (arm,run) cells. Eligible = 0.2 <= p <= 0.8 (mid). Order eligible by |p-0.5|
  ascending, ties by task id. Subset(N) = first N. N is widened 10,12,...,40 (capped at #eligible); first N meeting the gate wins.
  TEST rounds (held-out, never used for selection): RG.2.B manuk/playwright + W6.B0 cdp-layer.
  Gate (all at one N<=40): (a) rank agreement: pairwise concordance of subset total vs full-102 total over test (round,run) units >=0.8;
  (b) sign agreement: for each pair of test rounds the sign of the difference in per-run mean (full vs subset) matches, all pairs;
  (c) 2 >= 2*sd(x1 paired diff), sd = mean over test rounds of pstdev(subset total across runs) * sqrt(2).
  Falsifier: no N<=40 passes -> per-commit REAL stays full-102 x3.
  --self-test: mutant = same N but tasks picked with p==1 or p==0 (non-discriminating) must fail (a) or (b).
"""
import csv, collections, itertools, os, statistics, sys
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
SEL = {'W5.B1 manuk': 'docs/sprint/w5/B1-real-manuk.tsv', 'W5.B1 playwright': 'docs/sprint/w5/B1-real-playwright.tsv',
       'W5.B1 cdp-layer': 'docs/sprint/w5/B1-real-cdp-layer.tsv', 'RG.B manuk': 'docs/sprint/rg/B-real-manuk.tsv',
       'RG.B playwright': 'docs/sprint/rg/B-real-playwright.tsv'}
TEST = {'RG.2.B manuk': 'docs/sprint/rg/B2-real-manuk.tsv', 'RG.2.B playwright': 'docs/sprint/rg/B2-real-playwright.tsv',
        'W6.B0 cdp-layer': 'bench-queue/out/w6b0-cdplayer-real.tsv'}

def load(S):
    D = {}
    for k, p in S.items():
        d = collections.defaultdict(dict)
        for r in csv.DictReader(open(os.path.join(ROOT, p), newline=""), delimiter="\t"):
            if r["pass"] in ("0", "1"): d[r["task"]][int(r["run"])] = int(r["pass"])
        D[k] = d
    return D

def rates(D):
    n = collections.Counter(); s = collections.Counter()
    for d in D.values():
        for t, v in d.items(): n[t] += len(v); s[t] += sum(v.values())
    return {t: s[t] / n[t] for t in n}

def order(p):
    return [t for t in sorted((t for t in p if 0.2 <= p[t] <= 0.8), key=lambda t: (abs(p[t] - .5), t))]

def units(D, sub):
    U = []
    for k, d in D.items():
        for r in sorted({r for t in d for r in d[t]}):
            U.append((k, r, sum(d[t].get(r, 0) for t in d), sum(d[t].get(r, 0) for t in sub)))
    return U

def evaluate(T, sub):
    U = units(T, sub); c = dc = 0
    for a, b in itertools.combinations(U, 2):
        if a[2] != b[2] and a[3] != b[3]:
            if (a[2] - b[2]) * (a[3] - b[3]) > 0: c += 1
            else: dc += 1
    conc = c / (c + dc) if c + dc else 0.0
    m = lambda k, i: statistics.mean(u[i] for u in U if u[0] == k)
    signs = []
    for a, b in itertools.combinations(T, 2):
        f, s = m(a, 2) - m(b, 2), m(a, 3) - m(b, 3)
        signs.append((a, b, f, s, (f > 0) == (s > 0) and (f != 0) == (s != 0)))
    sd = statistics.mean(statistics.pstdev([u[3] for u in U if u[0] == k]) for k in T) * 2 ** .5
    return dict(conc=conc, signs=signs, sign_ok=all(x[4] for x in signs), sd=sd, flip=2 >= 2 * sd, ok=conc >= .8 and all(x[4] for x in signs) and 2 >= 2 * sd)

def main():
    S, T = load(SEL), load(TEST); p = rates(S); o = order(p)
    print(f"eligible mid-rate tasks (0.2<=p<=0.8 on selection rounds): {len(o)}")
    Ns = [n for n in range(10, 41, 2) if n <= len(o)] or [len(o)]
    if Ns[-1] < min(40, len(o)): Ns.append(min(40, len(o)))
    res = {}
    for n in Ns:
        e = res[n] = evaluate(T, o[:n])
        print(f"N={n:2d} conc={e['conc']:.3f} sign_ok={e['sign_ok']} sd_paired_x1={e['sd']:.2f} flip_visible={e['flip']} -> {'PASS' if e['ok'] else 'fail'}")
    if "--self-test" in sys.argv:
        bad = [t for t in sorted(p, key=lambda t: (-abs(p[t] - .5), t))]
        n = Ns[-1]; e = evaluate(T, bad[:n])
        print(f"mutant (N={n} most extreme-rate tasks): conc={e['conc']:.3f} sign_ok={e['sign_ok']} sd={e['sd']:.2f} -> {'PASS (BAD)' if e['ok'] else 'red (good)'}")
        return 0 if not e['ok'] else 1
    win = [n for n in Ns if res[n]['ok']]
    if win:
        n = win[0]; print("verdict: subset USABLE at N=%d" % n); print("tasks:", " ".join(sorted(o[:n]))); return 0
    e = res[Ns[-1]]
    for a, b, f, s, ok in e['signs']: print(f"  sign {a} - {b}: full {f:+.2f} subset {s:+.2f} {'ok' if ok else 'DISAGREE'}")
    print("verdict: FALSIFIER FIRES (no N<=40 passes; per-commit REAL stays full-102 x3)"); return 1
if __name__ == "__main__": sys.exit(main())
