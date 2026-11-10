#!/usr/bin/env python3
"""RG.2.0: REAL gap ledger round 2. GAP tasks come from rgb_tally.py (RG.B rows); for each, the first failure signal
of the failed manuk runs (RG.B transcripts) is classified with the RG.0 rules (rg0_ledger.py) and set beside the
Playwright W5.B1 transcript. Gate: every GAP task has a mechanism, first divergent step and excerpt, and each
mechanism label must agree with its signal (a label swap is caught).

  rg2_ledger.py [--mutate swap-labels|drop-task]   exit 0 = GATE PASS, 1 = GATE FAIL
"""
import argparse, collections, csv, io, os, sys, contextlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg0_ledger as r0
import rgb_tally as rt

ROOT = r0.ROOT
RG = os.path.join(ROOT, "docs/sprint/rg")
SIG_MECH = {n: m for m, n, _ in r0.SIGNALS}
SIG_MECH.update({n: m for m, n, _ in r0.ANSWER_SIGNALS})
SIG_MECH["same-page-state-different-decision"] = "agent"


def gap_list():
    with contextlib.redirect_stdout(io.StringIO()):
        rep = rt.report(quiet=True)
    return sorted(rep["gap_tasks"]) if isinstance(rep, dict) else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tr", default=os.path.expanduser("~/.cache/manuk-rgb/transcripts"))
    ap.add_argument("--pwtr", default=os.path.expanduser("~/.cache/manuk-b1/transcripts"))
    ap.add_argument("--out", default=RG)
    ap.add_argument("--mutate")
    a = ap.parse_args()
    gap = gap_list()
    mrows = list(csv.DictReader(open(os.path.join(RG, "B-real-manuk.tsv")), delimiter="\t"))
    cv = mrows[0]["chrome_ver"]
    fails = collections.defaultdict(list)
    for r in mrows:
        if r["pass"] == "0":
            fails[r["task"]].append(r["run"])
    pwpass = collections.defaultdict(list)
    for r in r0.rows("playwright"):
        if r["pass"] == "1":
            pwpass[r["task"]].append(r["run"])
    out = []
    for t in gap:
        sigs = []
        for run in fails.get(t, []):
            f = os.path.join(a.tr, f"manuk_{t}_r{run}.jsonl")
            if os.path.exists(f):
                sigs.append((run,) + r0.first_signal(f))
        if not sigs:
            out.append(dict(task=t)); continue
        mech = collections.Counter(s[1] for s in sigs).most_common(1)[0][0]
        run, m, name, n, tool, text = next(s for s in sigs if s[1] == mech)
        pr = pwpass[t][0]
        pst, _ = r0.steps(os.path.join(a.pwtr, f"playwright_{t}_r{pr}.jsonl"))
        mst, _ = r0.steps(os.path.join(a.tr, f"manuk_{t}_r{run}.jsonl"))
        # the Playwright step at the same index (or last), for the side-by-side
        pw_at = next((s for s in pst if s[0] == min(n, len(pst))), (0, "-", "", ""))
        out.append(dict(task=t, mechanism=mech, signal=name, manuk_run=run, step=n, tool=tool, manuk_steps=len(mst),
                        pw_run=pr, pw_steps=len(pst), runs_failed=len(sigs), mech_votes="/".join(s[1][:3] for s in sigs),
                        evidence=f"manuk r{run} step {n} ({tool}): {r0.one_line(text)}",
                        pw_excerpt=f"playwright r{pr} step {pw_at[0]} ({pw_at[1]}): {r0.one_line(pw_at[3] or pw_at[2])}",
                        chrome_ver=cv))
    if a.mutate == "swap-labels":
        d = [o for o in out if o.get("mechanism")]
        x = next((o for o in d for p in d if p["mechanism"] != o["mechanism"]), None)
        y = next((p for p in d if x and p["mechanism"] != x["mechanism"]), None)
        if x and y:
            x["mechanism"], y["mechanism"] = y["mechanism"], x["mechanism"]
    if a.mutate == "drop-task":
        out = out[:-1]
    cols = ["task", "mechanism", "signal", "manuk_run", "step", "tool", "manuk_steps", "pw_run", "pw_steps", "runs_failed",
            "mech_votes", "evidence", "pw_excerpt", "chrome_ver"]
    good = [o for o in out if o.get("evidence")]
    bad = [o["task"] for o in out if not o.get("evidence")]
    mislabel = [o["task"] for o in good if SIG_MECH.get(o["signal"]) != o["mechanism"]]
    ok = len(out) == 11 and not bad and not mislabel
    print(f"gap={len(out)} classified={len(good)} unclassified={bad} mislabelled={mislabel}")
    if a.mutate is None:
        os.makedirs(a.out, exist_ok=True)
        with open(os.path.join(a.out, "LEDGER-2.tsv"), "w") as fh:
            w = csv.DictWriter(fh, cols, delimiter="\t", extrasaction="ignore", restval=""); w.writeheader(); w.writerows(out)
        rank = collections.Counter(o["mechanism"] for o in good)
        md = ["# RG.2.0 REAL gap ledger, round 2", "",
              f"chrome_ver={cv}. GAP = the {len(out)} tasks in `docs/sprint/rg/B.md` (Playwright passes >=2 of 3 in W5.B1, manuk does not in RG.B).",
              "Manuk side: RG.B transcripts. Mechanism = class of the FIRST failure signal in a failed manuk run (majority over failed runs); "
              "rules in `bench/agent-tasks/rg0_ledger.py`, driver `bench/agent-tasks/rg2_ledger.py`.", "",
              "## Mechanisms ranked by tasks explained", "", "| mechanism | tasks | which |", "|---|---|---|"]
        for m, c in rank.most_common():
            md.append(f"| {m} | {c} | {', '.join(o['task'] for o in good if o['mechanism']==m)} |")
        md += ["", "## Per task: first divergent step", "",
               "| task | mechanism | signal | manuk first failing step | Playwright, same step index |", "|---|---|---|---|---|"]
        md += [f"| {o['task']} | {o['mechanism']} | {o['signal']} | {o['evidence'].replace('|', '/')} ({o['step']} of {o['manuk_steps']} steps; pw passes in {o['pw_steps']}) | {o['pw_excerpt'].replace('|', '/')} |" for o in good]
        md += ["", "## Caveats",
               "- First-failure-signal heuristic over transcripts, not a root-cause proof; `answer-reports-overlay` rests on the agent's own answer; `agent` = no tool-level signal fired.",
               "- Gate: `python3 -I bench/agent-tasks/rg2_ledger.py` exits 0; `--mutate swap-labels` exits 1 (label disagrees with its signal); `--mutate drop-task` exits 1."]
        open(os.path.join(a.out, "LEDGER-2.md"), "w").write("\n".join(md) + "\n")
    print("ranking=%s" % dict(collections.Counter(o["mechanism"] for o in good)))
    print("GATE PASS" if ok else "GATE FAIL")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
