#!/usr/bin/env python3
"""RG.2.0.b.a.a.a.a.a: re-judge the 3 recorded gocalendar-2 manuk answers, full vs with the closing view-state
sentences stripped, to test whether the judge's failure is the closing Day-view sentence or another clause.

  python3 -I docs/sprint/rg/R20baaaaa-rejudge.py            write R20baaaaa-rejudge.tsv and print the table
  python3 -I docs/sprint/rg/R20baaaaa-rejudge.py haiku      same with the Haiku fallback judge (-haiku suffix)
  python3 -I docs/sprint/rg/R20baaaaa-rejudge.py --mutate nostrip   red control: the stripper removes nothing,
                                                                    so the gate must exit 1

Conditions per recorded answer (source: R20baaa-real-manuk.tsv, final state: target/real-runs/*/state.json):
  F   the recorded answer, unchanged
  S   F minus the sentences after the last listed event that mention view / page / switch (closing view state)
  SI  S minus the sentences after the last listed event that say the per-day lists were inferred or unopened
Controls (hand-written, not tuned to any result): POS lists exactly the 15 rubric events with no view claim and no
extras, expected true; NEG drops 3 of the 15, expected false. Every cell is judged 3 times (temperature 0).
The judge sees the rubric, the answer and the state, and nothing else (real_judge.verdict). chrome_ver is taken from
MANUK_CHROME_PIN and every row carries it.
"""
import csv, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
sys.path.insert(0, os.path.join(ROOT, "bench", "agent-tasks"))
import real_judge  # noqa: E402

REPS = 3
ANCHOR = "Client Presentation"  # the last listed event of the rubric's Fri Jul 19 list
VIEW_RE = re.compile(r"view|page|switch", re.I)
INFER_RE = re.compile(r"inferred|did not open|not open|confirm the rest", re.I)
SRC = os.path.join(HERE, "R20baaa-real-manuk.tsv")
TASK = os.path.join(ROOT, "bench", "agent-tasks", "real", "tasks", "gocalendar-2.json")
RUBRIC_EVENTS = ["Team Standup Meeting", "Client Call", "Team Brainstorming Session", "Morning Walk", "Team Workshop",
                 "Lunch with Boss", "Project Sync", "Customer Feedback Session", "Team Collaboration Workshop",
                 "Morning Coffee with sister", "Team Check-In", "Lunch with Marketing Team", "Breakfast Meeting with Client",
                 "Team Progress Check", "Client Presentation"]
ANSWER_POS = ("Monday, July 15: Team Standup Meeting, Client Call, Team Brainstorming Session. Tuesday, July 16: Morning Walk, "
              "Team Workshop, Lunch with Boss. Wednesday, July 17: Project Sync, Customer Feedback Session, Team Collaboration "
              "Workshop. Thursday, July 18: Morning Coffee with sister, Team Check-In, Lunch with Marketing Team. Friday, July 19: "
              "Breakfast Meeting with Client, Team Progress Check, Client Presentation.")
ANSWER_NEG = ANSWER_POS.replace(" Team Progress Check,", "").replace(" Lunch with Marketing Team,", "").replace(
    " Morning Walk,", "")


def split_closing(answer, keep):
    """Everything up to the anchor is kept. After it, the event's own time (the text up to the first double space) is
    kept, then each sentence of the rest is kept only if keep(sentence) is True. Whitespace is otherwise unchanged."""
    i = answer.find(ANCHOR)
    if i < 0:
        raise SystemExit(f"anchor {ANCHOR!r} missing from answer")
    head, tail = answer[:i + len(ANCHOR)], answer[i + len(ANCHOR):].strip()
    parts = re.split(r"\s{2,}", tail)
    first = parts[0] if parts else ""
    rest = [x for p in parts[1:] for x in re.split(r"(?<=[.!?])\s+", p) if x and keep(x)]
    return head + first + ("  " + " ".join(rest) if rest else "")


def strip_view(answer):
    return split_closing(answer, lambda s: not VIEW_RE.search(s))


def strip_view_infer(answer):
    return split_closing(answer, lambda s: not (VIEW_RE.search(s) or INFER_RE.search(s)))


def main():
    mutate = sys.argv[sys.argv.index("--mutate") + 1] if "--mutate" in sys.argv else ""
    pin = os.environ.get("MANUK_CHROME_PIN", "")
    if not pin:
        raise SystemExit("MANUK_CHROME_PIN is not set")
    rubric = next(e["rubric"] for e in json.load(open(TASK))["evals"] if e["type"] == "llm_boolean")
    # --judge haiku: the harness's documented fallback (real_judge.FALLBACK_MODEL) when the primary is rate-limited
    env = {"backend": "claude", "REAL_JUDGE_MODEL": real_judge.FALLBACK_MODEL} if "haiku" in sys.argv \
        else real_judge.load_env()
    rows = list(csv.DictReader(open(SRC), delimiter="\t"))
    cells = []  # (answer_id, condition, text, state or None)
    for r in rows:
        run = r["run"]
        st_path = os.path.join(ROOT, "target", "real-runs", f"manuk_gocalendar-2_r{run}", "state.json")
        st = json.load(open(st_path))["state"] if os.path.exists(st_path) else None
        F = r["answer"]
        S = F if mutate == "nostrip" else strip_view(F)
        SI = S if mutate == "nostrip" else strip_view_infer(F)
        for cond, text in (("F", F), ("S", S), ("SI", SI)):
            cells.append((f"run{run}", cond, text, st))
    state_all = cells[0][3]
    cells.append(("POS", "control", ANSWER_POS, state_all))
    cells.append(("NEG", "control", ANSWER_NEG, state_all))
    out = []
    for aid, cond, text, st in cells:
        for rep in range(1, REPS + 1):
            try:
                v, reason = real_judge.verdict(env, rubric, text, st)
            except RuntimeError as x:
                v, reason = "error", str(x)[:120]
            out.append({"chrome_ver": pin, "answer": aid, "cond": cond, "rep": rep, "verdict": v,
                        "reason": reason.replace("\t", " ").replace("\n", " "), "answer_chars": len(text),
                        "judge_model": env["REAL_JUDGE_MODEL"]})
            print(f"{aid} {cond} r{rep}: {v} | {reason}", flush=True)
    cols = ["chrome_ver", "answer", "cond", "rep", "verdict", "reason", "answer_chars", "judge_model"]
    tag = "-haiku" if "haiku" in sys.argv else ""
    dst = os.path.join(HERE, f"R20baaaaa-rejudge{tag}.tsv" if not mutate else f"R20baaaaa-rejudge{tag}-mutant-{mutate}.tsv")
    with open(dst, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=cols, delimiter="\t")
        w.writeheader()
        w.writerows(out)
    # the stripped texts themselves, so the edit is auditable
    with open(os.path.join(HERE, f"R20baaaaa-stripped{tag}.txt"), "w") as f:
        for aid, cond, text, _ in cells:
            if cond in ("S", "SI") or aid in ("POS", "NEG"):
                f.write(f"[{aid} {cond}] {text}\n\n")
    # gate: the stripper must change every recorded answer, and the judge must pass POS and fail NEG every time
    changed = all(next(t for a, c, t, _ in cells if a == f"run{r['run']}" and c == "F") !=
                  next(t for a, c, t, _ in cells if a == f"run{r['run']}" and c == "S") for r in rows)
    ctrl_ok = all(x["verdict"] is True for x in out if x["answer"] == "POS") and \
        all(x["verdict"] is False for x in out if x["answer"] == "NEG")
    ok = changed and ctrl_ok
    print(f"GATE {'PASS' if ok else 'FAIL'}: stripper changed all {len(rows)} answers={changed}; controls POS true/NEG false={ctrl_ok}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
