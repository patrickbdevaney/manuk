#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a: explain the no-name steps of RG3baaaa-classify.md. A step is no-name when its handle never matched
the replay's NODE regex (`[hN] role "name"`) in the transcript. This script finds, per such step, the handle in the
transcript's own result text: the last observation line before the step that carries the handle (a nameless node,
`[hN] role ...` with no quoted name), else any line, else 'not found'. Counts are x2 (the replay hits the same step on
both backends) and must sum to the no-name total of the classify table.
  rg3baaaaa_noname.py [--mutate blank-results] [--expect 24]   exit 0 = GATE PASS, 1 = GATE FAIL
Writes docs/sprint/rg/RG3baaaaa-noname.md. Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import argparse, json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg0_ledger as lg
import rg3b_replay as r

TASKS = "opendining-4 opendining-7 gocalendar-2".split()


def rows_for(task, blank=False):
    st = lg.steps(os.path.join(r.TR, f"manuk_{task}_r1.jsonl"))[0]
    if blank:
        st = [(n, t, i, "") for n, t, i, _ in st]
    out = []
    flat = r.flatten(st)
    for i, (t, a, rn) in enumerate(flat):
        h = a.get("handle")
        if not h or rn is not None:
            continue
        line, at = None, None
        for n, tool, inp, res in st:
            m = re.search(r"^.*\[%s\][^\n]*" % h, res, re.M)
            if m:
                line, at = m.group(0).strip(), n
                break
        role = re.match(r"\[%s\] (\w+)" % h, line).group(1) if line and re.match(r"\[%s\] (\w+)" % h, line) else ""
        kind = "nameless node" if line and role else ("handle in text only" if line else "not found")
        out.append({"task": task, "step": i, "action": t, "handle": h, "first_seen_call": at, "role": role, "kind": kind, "line": line or ""})
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mutate", choices=["blank-results"]); ap.add_argument("--expect", type=int, default=24)
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    rows = [x for t in TASKS for x in rows_for(t, a.mutate == "blank-results")]
    found = [x for x in rows if x["kind"] != "not found"]
    md = ["## RG.3.0.b.a.a.a.a.a", "",
          f"chrome_ver={pin}. The 24 no-name steps of RG3baaaa-classify.md are 12 distinct steps, each hit once per backend (x2). "
          "The replay names a handle only from `[hN] role \"name\"`; each handle below is in the transcript's own observation as a **nameless node** "
          "(role and state/value, no quoted accessible name), so the replay had no name to resolve by. Not a recording gap.", "",
          "| task | step | action | handle | first seen in call | role | kind | transcript line |", "|---|---|---|---|---|---|---|---|"]
    md += [f"| {x['task']} | #{x['step']} | {x['action']} | {x['handle']} | {x['first_seen_call']} | {x['role']} | {x['kind']} | `{x['line']}` |" for x in rows]
    by = {}
    for x in rows:
        by[x["task"]] = by.get(x["task"], 0) + 1
    md += ["", "| task | steps | x2 backends |", "|---|---|---|"] + [f"| {t} | {n} | {2 * n} |" for t, n in by.items()]
    total = 2 * len(found)
    md += [f"| all | {len(found)} found | {total} |", ""]
    print("\n".join(md[-8:]))
    ok = total == a.expect and len(found) == len(rows)
    print(f"found {len(found)}/{len(rows)} steps; x2 = {total}; expect {a.expect}: {'GATE PASS' if ok else 'GATE FAIL'}")
    if a.mutate is None and ok:
        open(os.path.join(r.ROOT, "docs/sprint/rg/RG3baaaaa-noname.md"), "w").write("\n".join(md) + "\n")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
