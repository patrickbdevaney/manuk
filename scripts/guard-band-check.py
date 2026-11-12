#!/usr/bin/env python3
"""GUARD.BAND: honest regression judgement for the human-view guards (GUARD.R4 and later). Rule text: docs/sprint/GUARD.md.

(a) Band per metric from >=3 same-binary baseline runs: lower edge = min(baseline) - max(spread_baseline, spread_candidate),
    where spread = max-min over that set's own same-binary runs. So the band is never narrower than the observed run-to-run
    spread. One-sided (values above the band are not flagged). A metric below its band is a BREACH.
(b) Paired criterion on sites OK in both sets, per-site values averaged over the set's runs: mean paired delta >= -1.0 point,
    and every site moving >= 10 points (either direction) on a11y F1 (the metric the guards bind) needs a row in
    docs/sprint/guard-classes.tsv (INSTRUMENT/ORIGIN/ENGINE + evidence). An ENGINE mover with a negative delta is a REGRESSION.
    Negative >=10-point movers on the other per-site metrics are printed as WATCH (not RED): file a node for each.
A band BREACH is discharged only when its paired metric passes (b); a paired failure is always RED.
Set-level metric -> per-site column: cov->cov, shape->shape, m2_*->m2_rate, ax_*->a11y_f1.
No SUCCESS.md threshold is touched; this changes how noise is read, not what counts as a regression.

usage: python3 -I scripts/guard-band-check.py BASE_ROWS CAND_ROWS [--guard GUARD.tsv] [--classes F] [--bench DIR]
  BASE_ROWS/CAND_ROWS: comma lists of GUARD.tsv row names. Example: run1,run2,run3 R3run1,R3run2,R3run3
"""
import argparse, csv, os, statistics as S, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MET = {"cov": "cov", "shape": "shape", "m2_full": "m2_rate", "m2_meas": "m2_rate", "ax_full": "a11y_f1", "ax_meas": "a11y_f1"}
MIN_MEAN, MOVE = -1.0, 10.0

ap = argparse.ArgumentParser()
ap.add_argument("base"); ap.add_argument("cand")
ap.add_argument("--guard", default=os.path.join(ROOT, "docs/sprint/GUARD.tsv"))
ap.add_argument("--classes", default=os.path.join(ROOT, "docs/sprint/guard-classes.tsv"))
ap.add_argument("--bench", default=os.path.join(ROOT, "docs/bench"))
a = ap.parse_args()

def runs(names):
    by = {r["row"]: r for r in csv.DictReader(open(a.guard, newline=""), delimiter="\t")}
    return [by[n] for n in names.split(",") if n in by]

def sites(r):
    with open(os.path.join(a.bench, r["source"]), newline="") as f:
        return {x["url"]: x for x in csv.DictReader(f, delimiter="\t")}

base, cand = runs(a.base), runs(a.cand)
if len(base) < 3 or len(cand) < 3:
    print(f"FAIL: need >=3 runs each, got base={len(base)} cand={len(cand)}"); sys.exit(2)
vers = {r["chrome_ver"] for r in base + cand}
if len(vers) != 1:
    print(f"FAIL: mixed chrome_ver {sorted(vers)}"); sys.exit(2)
classes = {}
if os.path.exists(a.classes):
    for r in csv.DictReader(open(a.classes, newline=""), delimiter="\t"):
        classes[r["url"]] = (r["class"], r["evidence"])

red, breach, watch = [], {}, []
print(f"chrome_ver={vers.pop()}")
for m in MET:
    bv, cv = [float(r[m]) for r in base], [float(r[m]) for r in cand]
    sb, sc = max(bv) - min(bv), max(cv) - min(cv)
    lo = min(bv) - max(sb, sc)
    below = [v for v in cv if v < lo]
    old = min(bv)
    print(f"band {m}: base[{min(bv):.2f},{max(bv):.2f}] spread_base={sb:.2f} spread_cand={sc:.2f} -> lower {lo:.2f} (fixed-band lower {old:.2f}); cand {[round(v,2) for v in cv]} below={len(below)}")
    if below:
        breach[m] = len(below)

bs, cs = [sites(r) for r in base], [sites(r) for r in cand]
paired = {}
for col in sorted(set(MET.values())):
    def avg(ss):
        o = {}
        for s in ss:
            for u, x in s.items():
                if x["m1_status"] == "OK" and x.get(col, "") != "":
                    o.setdefault(u, []).append(float(x[col]))
        return {u: S.mean(v) for u, v in o.items()}
    ab, ac = avg(bs), avg(cs)
    d = {u: ac[u] - ab[u] for u in ab if u in ac}
    mean = S.mean(d.values()) if d else 0.0
    ok = mean >= MIN_MEAN
    msgs = []
    if not ok:
        msgs.append(f"{col} paired mean {mean:.2f} < {MIN_MEAN}")
    print(f"paired {col}: n={len(d)} mean_delta={mean:.2f} {'ok' if ok else 'FAIL'}")
    for u, x in sorted(d.items(), key=lambda t: t[1]):
        if abs(x) >= MOVE:
            c = classes.get(u)
            tag = c[0] if c else "UNCLASSIFIED"
            print(f"  mover {col} {x:+.1f} {u} {tag}" + (f" | {c[1][:90]}" if c else ""))
            if col != "a11y_f1":
                if x < 0:
                    watch.append(f"{col} {x:+.1f} {u}")
            elif not c:
                msgs.append(f"unclassified mover {u} ({col} {x:+.1f})")
            elif c[0] == "ENGINE" and x < 0:
                msgs.append(f"ENGINE drop {u} ({col} {x:+.1f})")
    paired[col] = msgs
    red += msgs
for m, n in breach.items():
    st = "discharged by paired criterion" if not paired[MET[m]] else "NOT discharged"
    print(f"breach {m} x{n}: {st}")
for w in watch:
    print("WATCH", w)
print("RED " + "; ".join(red) if red else "GREEN", f"breaches={len(breach)} paired_failures={len(red)}")
sys.exit(1 if red else 0)
