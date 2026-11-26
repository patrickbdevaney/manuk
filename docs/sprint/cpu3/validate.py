#!/usr/bin/env python3
"""CPU.3.a gate: validate a quiet-host remeasure set and decide.
usage: validate.py <dir> [--selftest]
Gate (pre-registered in the node row): every row has host load1 < 2, every run ok, chrome_ver == $MANUK_CHROME_PIN,
5 rounds per variant on the 10 pages. Decision: opt-level 2 reduces summed 10-page cpu-s by >= 10% -> OWNER MEMO;
otherwise -> CLOSE. Exit 0 = valid set (decision printed), 3 = invalid (load/ok/pin/coverage violated), 4 = selftest failed.
"""
import sys, os, csv, statistics as S, collections, copy
PIN = os.environ.get("MANUK_CHROME_PIN", "145.0.7632.116")
PAGES = ["hackernews","rustlang","mdn-css","github","lobsters","python-docs","sqlite","pgdocs","rfc-editor","stripe-docs"]
VARS = ["s","2","3","hot3"]; ROUNDS = 5; MAX_LOAD = 2.0; MIN_GAIN = 0.10

def evaluate(pages, conc):
    """pages: rows [round,page,variant,nav,obs,cpu,pss,ok,load,chrome]; conc: rows (tab-split, full header-less list).
    Returns (valid, problems, summary)."""
    probs = []
    for r in pages:
        if r[7] != "True": probs.append(f"page run failed: {r[:3]}")
        if float(r[8]) >= MAX_LOAD: probs.append(f"load {r[8]} >= 2 on page row {r[:3]}")
        if r[9] != PIN: probs.append(f"chrome_ver {r[9]} != pin on page row {r[:3]}")
    cover = collections.Counter((r[0], r[1], r[2]) for r in pages)
    for v in VARS:
        for k in range(ROUNDS):
            for p in PAGES:
                if cover[(str(k), p, v)] != 1: probs.append(f"coverage {v} round {k} {p}: {cover[(str(k), p, v)]}")
    for r in conc:
        if float(r[5]) >= MAX_LOAD: probs.append(f"load {r[5]} >= 2 on conc row {r[2]}/{r[3]}")
        if r[0] != PIN: probs.append(f"chrome_ver {r[0]} != pin on conc row")
        if r[9] != "1.0": probs.append(f"conc success {r[9]} on {r[2]}")
    # sums
    per = {v: collections.defaultdict(list) for v in VARS}
    for r in pages: per[r[2]][r[1]].append(float(r[5]))
    tot = {v: sum(S.mean(per[v][p]) for p in PAGES) for v in VARS}
    gain2 = 1 - tot["2"] / tot["s"]
    rounds_better = sum(1 for k in range(ROUNDS) if sum(float(r[5]) for r in pages if r[2]=="2" and r[0]==str(k)) <
                        sum(float(r[5]) for r in pages if r[2]=="s" and r[0]==str(k)))
    valid = not probs
    dec = ("OWNER MEMO (gain >= 10%)" if gain2 >= MIN_GAIN else "CLOSE (gain < 10%)") if valid else "NONE (invalid set)"
    summary = {"tot": tot, "gain2": gain2, "rounds_2_lt_s": rounds_better, "decision": dec}
    return valid, probs, summary

def conc_summary(d):
    out = {}
    for v in VARS:
        rows = [l.rstrip("\n").split("\t") for l in open(os.path.join(d, f"conc-{v}.tsv")) if not l.startswith("chrome_ver")]
        out[v] = [float(r[19]) for r in rows]
    return out

def selftest():
    base_pages = [[str(k),p,v,"0","0","1.0","50","True","1.0",PIN] for k in range(ROUNDS) for p in PAGES for v in VARS]
    # 1. a green set with a 15% gain must be valid and decide OWNER MEMO
    p = copy.deepcopy(base_pages)
    for r in p:
        if r[2]=="2": r[5]="0.85"
    ok1, pr1, s1 = evaluate(p, [])
    # 2. the same set with one row at load 15 must be invalid (red)
    p2 = copy.deepcopy(p); p2[3][8] = "15.0"
    ok2, pr2, _ = evaluate(p2, [])
    # 3. a 9% gain must decide CLOSE
    p3 = copy.deepcopy(base_pages)
    for r in p3:
        if r[2]=="2": r[5]="0.91"
    ok3, pr3, s3 = evaluate(p3, [])
    checks = {"green_valid": ok1 and s1["decision"].startswith("OWNER"),
              "load15_red": (not ok2) and any("load 15.0" in x for x in pr2),
              "gain9_close": ok3 and s3["decision"].startswith("CLOSE")}
    print("selftest", checks)
    return all(checks.values())

if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(0 if selftest() else 4)
    d = sys.argv[1]
    pages = [l.rstrip("\n").split("\t") for l in open(os.path.join(d,"pages.tsv")) if l.strip()]
    conc = []
    for v in VARS:
        for l in open(os.path.join(d, f"conc-{v}.tsv")):
            if l.startswith("chrome_ver") or not l.strip(): continue
            conc.append(l.rstrip("\n").split("\t"))
    valid, probs, s = evaluate(pages, conc)
    for p in probs[:20]: print("PROBLEM:", p)
    print("pages sum cpu-s:", {v: round(s["tot"][v],2) for v in VARS}, "gain opt2 vs s: %.1f%%" % (s["gain2"]*100))
    print("rounds where 2 < s:", s["rounds_2_lt_s"], "of", ROUNDS)
    cs = conc_summary(d)
    print("CONC cpu_s mean:", {v: round(S.mean(cs[v]),2) for v in VARS}, "n:", {v: len(cs[v]) for v in VARS})
    print("valid:", valid, "decision:", s["decision"])
    sys.exit(0 if valid else 3)
