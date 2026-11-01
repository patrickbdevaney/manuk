#!/usr/bin/env python3
"""W4.B4.q turns report: did W4.B3 (navigate(url, query) + extract(query)) cut turns per task on the agent bench?

  b4q_turns_report.py [--dir docs/sprint/w4/B4q] [--scoreboard docs/sprint/SCOREBOARD.tsv]

Arms compared (run_arm.py output, full suite = own 40 + external 14 incl. both lookup fixtures, 3 runs each):
  manuk          = HEAD binary, query honoured (the treatment)
  manuk-noquery  = the same binary with MANUK_QUERY_MUTATE=ignore (the mutant: query ignored, the pre-W4.B3 behaviour)
  baseline       = W4.B1 before-the-token-work manuk arm: SCOREBOARD rows, arm manuk, own suite 69903c7d, 2026-10-31 09-10
                   (own slice only; the external slice was never run on that arm)
Per arm it prints: turns per task (mean over the 3 runs, with the min..max band of the per-run means), pass per run
(the band), tasks passing in >=2 of 3 runs, and for the L-tasks (own lookup tasks, L01..L25) the median of each task's
3 turn counts, plus how many of those task medians are <= 2.5 (the node's L-task gate).
Every row is checked for chrome_ver == $MANUK_CHROME_PIN; other pins are refused, never mixed.
Run with `python3 -I`.
"""
import argparse, csv, os, statistics as st, sys, collections

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
PIN = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
BASE_SUITE = "69903c7d"  # own suite v1 hash prefix (W4.B1 measurement)


def load(path):
    rows = list(csv.DictReader(open(path), delimiter="\t"))
    bad = [r["chrome_ver"] for r in rows if r["chrome_ver"] != PIN]
    if bad:
        sys.exit(f"refusing: {path} has {len(bad)} rows with chrome_ver != {PIN}")
    return rows


def baseline(scoreboard):
    return [r for r in load(scoreboard)
            if r["arm"] == "manuk" and r["suite_hash"].startswith(BASE_SUITE)
            and "2026-10-31T09" <= r["ts"][:13] < "2026-10-31T11"]


def summarise(name, rows):
    if not rows:
        return f"{name}: no rows"
    runs = sorted(set(r["run"] for r in rows))
    own = [r for r in rows if r["slice"] == "own"]
    ext = [r for r in rows if r["slice"] != "own"]
    # headline turns are OWN-slice only, so the baseline (own only) and both arms compare like with like
    per_run_turns = [st.mean(int(r["turns"]) for r in own if r["run"] == k) for k in runs]
    per_run_pass = [sum(int(r["pass"]) for r in rows if r["run"] == k) for k in runs]
    tasks = sorted(set(r["task"] for r in rows))
    ok2 = sum(1 for t in tasks if sum(int(r["pass"]) for r in rows if r["task"] == t) >= 2)
    L = sorted(set(r["task"] for r in own if r["task"].startswith("L")))
    lmed = {t: st.median(int(r["turns"]) for r in own if r["task"] == t) for t in L}
    lowish = sum(1 for v in lmed.values() if v <= 2.5)
    out = [f"{name}: {len(tasks)} tasks x {len(runs)} runs ({len(own)} own rows, {len(ext)} external rows)",
           f"  own turns/task {st.mean(per_run_turns):.2f}  band [{min(per_run_turns):.2f}, {max(per_run_turns):.2f}]",
           f"  pass per run {per_run_pass} of {len(tasks)}; tasks passing >=2/3: {ok2}/{len(tasks)}",
           f"  L-tasks ({len(L)}): median of task medians {st.median(lmed.values()):.1f}; "
           f"task medians <=2.5: {lowish}/{len(L)}"]
    if ext:
        ext_turns = [st.mean(int(r["turns"]) for r in ext if r["run"] == k) for k in runs]
        out.append(f"  external turns/task {st.mean(ext_turns):.2f} (no pre-W4.B3 external arm exists; compare with noquery)")
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=os.path.join(ROOT, "docs/sprint/w4/B4q"))
    ap.add_argument("--scoreboard", default=os.path.join(ROOT, "docs/sprint/SCOREBOARD.tsv"))
    a = ap.parse_args()
    base = baseline(a.scoreboard)
    arms = [("baseline (W4.B1, own only)", base),
            ("manuk (HEAD, query honoured)", load(os.path.join(a.dir, "manuk.tsv"))),
            ("manuk-noquery (mutant)", load(os.path.join(a.dir, "manuk-noquery.tsv")))]
    for name, rows in arms:
        print(summarise(name, rows)); print()
    # the gate's direction checks, printed not asserted, so a red result still shows its numbers
    mt = [int(r["turns"]) for r in arms[1][1] if r["slice"] == "own"]
    mq = [int(r["turns"]) for r in arms[2][1] if r["slice"] == "own"]
    if mt and mq:
        print(f"own turns: manuk {st.mean(mt):.2f} vs noquery {st.mean(mq):.2f} (mutant should be higher)")
    print(f"baseline own turns {st.mean(int(r['turns']) for r in base):.2f} (the W4.B1 journal says 4.97)" if base else "")


if __name__ == "__main__":
    main()
