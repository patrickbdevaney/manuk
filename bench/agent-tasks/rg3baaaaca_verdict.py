#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.c.a: add a verdict column (real|coincidental|not-judged) to docs/sprint/rg/RG3baaaac-renamed.tsv.
The 6 short-token rows (the 5 distinct controls of the node: opendining-4 '2', udriver-9 '18' and '2024', zilloft-2
'Use exact match' x3 steps) carry a verdict and the observation line it rests on; the line must equal the row's obs_name
(copied from the replay's observation). Red when a judged row has no quoted line, the quote differs from obs_name, or
a row is missing from the verdict table.
  rg3baaaaca_verdict.py [--mutate drop-quote|drop-row]   exit 0 = GATE PASS, 1 = GATE FAIL. Run with `python3 -I`."""
import argparse, csv, os, sys
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
TSV = os.path.join(ROOT, "docs/sprint/rg/RG3baaaac-renamed.tsv")
Z = "Use exact match Use exact match"
U = "Choose date, selected date is Jul 18, 2024"
# (task, step) -> (verdict, why). The quoted observation line is the row's own obs_name.
V = {
    ("opendining-4", "6"): ("coincidental", "transcript clicks day-cell '2' of the calendar after two 'Next month' clicks; the observed node is the time slot '2:00 PM', a different control"),
    ("udriver-9", "14"): ("coincidental", "transcript clicks the year radio '2024' in the year view; the observed node is the pickup-date textbox whose name merely contains '2024'; the year view is not open"),
    ("udriver-9", "15"): ("coincidental", "transcript clicks day-cell '18' in the calendar; the observed node is the pickup-date textbox whose name merely contains '18'; no day cells are present"),
    ("zilloft-2", "8"): ("real", "same label with the role changed radio -> button and the label repeated; one control, the 'Use exact match' option"),
    ("zilloft-2", "11"): ("real", "same control as step 8 (same label, button role, label repeated)"),
    ("zilloft-2", "12"): ("real", "same control as step 8 (same label, button role, label repeated)"),
}
QUOTE = {("opendining-4", "6"): "2:00 PM", ("udriver-9", "14"): U, ("udriver-9", "15"): U,
         ("zilloft-2", "8"): Z, ("zilloft-2", "11"): Z, ("zilloft-2", "12"): Z}


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--mutate", choices=["drop-quote", "drop-row"])
    a = ap.parse_args()
    rows = [{k: v for k, v in r.items() if k not in ("verdict", "observation", "why")}
            for r in csv.DictReader(open(TSV), delimiter="\t")]  # idempotent: drop columns from a prior run
    bad, out = [], []
    verdicts, quotes = dict(V), dict(QUOTE)
    if a.mutate == "drop-quote":
        quotes[("udriver-9", "15")] = ""
    if a.mutate == "drop-row":
        verdicts.pop(("zilloft-2", "11"))
    for r in rows:
        k = (r["task"], r["step"])
        if k in verdicts:
            v, why = verdicts[k]; q = quotes.get(k, "")
            if not q or q != r["obs_name"]:
                bad.append(f"{k}: quote {q!r} != obs_name {r['obs_name']!r}")
            out.append(dict(r, verdict=v, observation=q, why=why))
        else:
            out.append(dict(r, verdict="not-judged", observation="", why="outside the short-token set"))
    judged = [r for r in out if r["verdict"] != "not-judged"]
    if len(judged) != len(QUOTE):
        bad.append(f"judged {len(judged)} of {len(QUOTE)} rows")
    for b in bad: print("FAIL", b)
    print("judged:", [(r["task"], r["step"], r["verdict"]) for r in judged])
    if not bad and a.mutate is None:
        cols = list(rows[0].keys()) + ["verdict", "observation", "why"]
        with open(TSV, "w", newline="") as f:
            w = csv.DictWriter(f, cols, delimiter="\t", lineterminator="\n"); w.writeheader(); w.writerows(out)
    print("GATE FAIL" if bad else "GATE PASS"); sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
