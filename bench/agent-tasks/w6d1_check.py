#!/usr/bin/env python3
"""W6.D1: recompute the engine-ownership kill rule and the WSJF build order from the W6.B0/X1/A1
source rows, and check that docs/sprint/w6/D1.md states exactly those numbers and that decision.

  python3 -I bench/agent-tasks/w6d1_check.py              # exit 0 iff D1.md matches the data
  python3 -I bench/agent-tasks/w6d1_check.py --mutate -1  # cdp-layer loses 1 task -> must go red
  python3 -I bench/agent-tasks/w6d1_check.py --self-test  # shows the check can go red

Kill rule (SYNTHESIS section 4 / W6.B0 / W6.D1 rows, pre-registered): cdp-layer task_pass_2of3 >= 40/102 at HEAD
-> Chrome (cdp-layer) is the default backend; engine build work only in the security/fork/footprint lanes.
WSJF = flips x (1 + surfaces) x conf / hours; ties broken by fewer hours, then id.
Needs MANUK_CHROME_PIN (the tallies refuse rows from another chrome_ver).
"""
import argparse, collections, csv, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import w6b0_tally as b0  # noqa: E402
import w6x1_tally as x1  # noqa: E402

ROOT = b0.ROOT
DOC = os.path.join(ROOT, "docs/sprint/w6/D1.md")
A1 = os.path.join(ROOT, "docs/sprint/w6/A1-labels.tsv")
PW = os.path.join(ROOT, "bench-queue/out/rg2b-real-playwright.tsv")
KILL = 40

# Playwright-only passes (pw passes, cdp-layer fails 2of3) whose winning RG.2.B playwright transcript
# (~/.cache/manuk-bq/transcripts/playwright_<task>_r1.jsonl, 2026-11-19) called browser_evaluate >= 1 time.
PW_EVAL = ["gocalendar-1", "gocalendar-6", "opendining-10", "opendining-9", "staynb-1"]

# Build nodes priced. flips = predicted REAL-102 task_pass_2of3 flips on the DEFAULT backend (Chrome after the rule).
# surfaces = product surfaces served beyond REAL-102 (held-out V1, GUI, MCP handoff, security). conf 0..1. hours = est.
def prices(c):
    return {
        "W6.C2": dict(flips=len(PW_EVAL), surfaces=1, conf=0.4, hours=10,
                      basis="ceiling = pw-only tasks whose winning playwright run used browser_evaluate"),
        "W6.C1": dict(flips=c["x1_moved"] + 1, surfaces=1, conf=0.2, hours=16,
                      basis="X1 guard-off mutant moved 0/15 dialog tasks; +1 for untested non-guard rungs"),
        "W6.H1": dict(flips=0, surfaces=2, conf=0.5, hours=16,
                      basis="GUI + MCP handoff; no REAL-102 task depends on the GUI path"),
        "W6.S1": dict(flips=0, surfaces=2, conf=0.5, hours=20,
                      basis="security lane (engine-retained); REAL-102 has no cross-origin theft task"),
    }


def census(mutate=0):
    r = b0.report(b0.CDP, mutate=mutate, quiet=True)
    tasks = b0.tasks_expected()
    _, cdp_ok = b0.summarise(b0.table(b0.load(b0.CDP), tasks), tasks)
    pw = {row["task"]: int(row["pass"]) for row in b0.load(PW)}
    pw_only = sorted(t for t in tasks if pw.get(t) and not cdp_ok[t])
    missing = [t for t in PW_EVAL if t not in pw_only]
    if missing:
        raise b0.Refuse(f"PW_EVAL tasks not playwright-only: {missing}")
    _, moved, _ = x1.verdict(x1.table(x1.load(x1.MUT), label="mutant"), x1.table(x1.load(x1.UNM), label="unmutated"))
    with open(A1, newline="") as f:
        a1 = collections.Counter(row["arm"] for row in csv.DictReader(f, delimiter="\t"))
    c = {
        "cdp_task_pass_2of3": r["cdp_task_pass_2of3"],
        "manuk_rg2b_task_pass_2of3": r["manuk_rg2b_task_pass_2of3"],
        "x1_moved": len(moved),
        "a1_failed": f"manuk:{a1['manuk']},cdp-layer:{a1['cdp-layer']},playwright:{a1['playwright']}",
        "pw_only": len(pw_only),
    }
    c["decision"] = "chrome" if c["cdp_task_pass_2of3"] >= KILL else "engine"
    p = prices(c)
    for v in p.values():
        v["wsjf"] = round(v["flips"] * (1 + v["surfaces"]) * v["conf"] / v["hours"], 3)
    c["order"] = ",".join(sorted(p, key=lambda k: (-p[k]["wsjf"], p[k]["hours"], k)))
    c["wsjf"] = ",".join(f"{k}:{p[k]['wsjf']}" for k in sorted(p))
    return c, p


KEYS = ["cdp_task_pass_2of3", "manuk_rg2b_task_pass_2of3", "x1_moved", "a1_failed", "pw_only", "decision",
        "order", "wsjf"]


def stated(text):
    m = re.search(r"```d1-facts\n(.*?)```", text, re.S)
    if not m:
        return {}
    return dict(l.split("=", 1) for l in m.group(1).strip().splitlines() if "=" in l)


def check(text, c):
    s = stated(text)
    bad = [f"{k}: doc={s.get(k)!r} data={c[k]!r}" for k in KEYS if s.get(k) != str(c[k])]
    return bad


def self_test():
    text = open(DOC).read()
    ok = True

    def expect(name, cond):
        nonlocal ok
        print(f"{'ok ' if cond else 'FAIL'} {name}")
        ok &= cond
    c0, _ = census()
    expect("doc matches data", check(text, c0) == [])
    cm, _ = census(mutate=-1)
    expect("cdp-layer -1 task flips decision -> red", cm["decision"] == "engine" and check(text, cm) != [])
    swapped = text.replace("order=W6.C2,W6.C1", "order=W6.C1,W6.C2")
    expect("swapped order in doc -> red", check(swapped, c0) != [])
    expect("doc without facts block -> red", check("no facts", c0) != [])
    return ok


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mutate", type=int, default=0)
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    try:
        if a.self_test:
            sys.exit(0 if self_test() else 1)
        c, p = census(a.mutate)
    except (b0.Refuse, x1.Refuse) as e:
        print(f"REFUSE: {e}")
        sys.exit(2)
    print("```d1-facts")
    for k in KEYS:
        print(f"{k}={c[k]}")
    print("```")
    for k, v in p.items():
        print(f"{k}\tflips={v['flips']}\tsurfaces={v['surfaces']}\tconf={v['conf']}\thours={v['hours']}\twsjf={v['wsjf']}")
    bad = check(open(DOC).read() if os.path.exists(DOC) else "", c)
    for l in bad:
        print("MISMATCH", l)
    print("D1.md", "MATCHES the data" if not bad else "DOES NOT MATCH the data")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
