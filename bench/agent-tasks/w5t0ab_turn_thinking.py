#!/usr/bin/env python3
"""W5.T0.a.b gate: the per-turn log carries thinking_tokens, and its column sums to the run result's thinking_tokens.

  python3 -I w5t0ab_turn_thinking.py                       # offline: stored L2a transcript, exact sum + placement
  python3 -I w5t0ab_turn_thinking.py RUNDIR                # real smoke: RUNDIR holds <arm>.tsv, <arm>.tsv.turns.tsv
                                                           #   and <arm>_<task>_r<run>.jsonl (run_arm.py --transcripts)
  add --mutant to drop the thinking_tokens column before checking (must go RED)

RED when the column is missing, a turn's thinking appears on a turn with no thinking block, or the per-run sum
differs from the result's output_tokens_details.thinking_tokens.
"""
import csv, glob, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import run_arm  # noqa: E402

MUTANT = "--mutant" in sys.argv
ARGS = [a for a in sys.argv[1:] if a != "--mutant"]


def result_and_turns(path):
    """(result usage dict, [per-turn tuples]) for one stream-json transcript, via the harness's own turn_log."""
    out = open(path).read()
    j = run_arm.parse(out)
    u = (j or {}).get("usage", {})
    return j, run_arm.turn_log(out, u.get("output_tokens"), (u.get("output_tokens_details") or {}).get("thinking_tokens"))


def offline():
    path = os.path.join(HERE, "..", "..", "docs/sprint/w5/L2a-transcripts/manuk_L2a_r1.jsonl")
    j, turns = result_and_turns(path)
    want = j["usage"]["output_tokens_details"]["thinking_tokens"]
    got = sum(t[4] for t in turns)
    problems = []
    if MUTANT:
        turns = [t[:4] + (0,) for t in turns]  # the column is dropped: every turn reads 0
        got = 0
    if got != want:
        problems.append(f"offline sum {got} != result thinking_tokens {want}")
    if any(t[4] and not t[0] for t in turns):
        problems.append("thinking on a turn with no thinking block")
    if len(turns) < 2 or want == 0:
        problems.append(f"degenerate transcript: {len(turns)} turns, thinking {want}")
    print(f"offline {os.path.basename(path)}: {len(turns)} turns, thinking sum {got} vs result {want}")
    return problems


def real(rundir):
    problems, checked = [], 0
    for tpath in sorted(glob.glob(os.path.join(rundir, "*.tsv.turns.tsv"))):
        arm = os.path.basename(tpath)[: -len(".tsv.turns.tsv")]
        with open(tpath) as f:
            rows = list(csv.DictReader(f, delimiter="\t"))
        if not rows:
            problems.append(f"{arm}: empty turn log")
            continue
        if MUTANT:
            rows = [{k: v for k, v in r.items() if k != "thinking_tokens"} for r in rows]
        if "thinking_tokens" not in rows[0]:
            problems.append(f"{arm}: turn log has no thinking_tokens column")
            continue
        sums = {}
        for r in rows:
            key = (r["task"], r["run"])
            sums[key] = sums.get(key, 0) + int(r["thinking_tokens"])
        for (task, run), got in sorted(sums.items()):
            tr = os.path.join(rundir, f"{arm}_{task}_r{run}.jsonl")
            if not os.path.exists(tr):
                problems.append(f"{arm} {task} r{run}: no transcript")
                continue
            j, _ = result_and_turns(tr)
            want = ((j or {}).get("usage", {}).get("output_tokens_details") or {}).get("thinking_tokens", 0)
            checked += 1
            if got != want:
                problems.append(f"{arm} {task} r{run}: turn sum {got} != result thinking_tokens {want}")
            print(f"{arm} {task} r{run}: turn sum {got} vs result {want}")
    if checked == 0:
        problems.append("no runs checked")
    return problems


problems = real(ARGS[0]) if ARGS else offline()
if problems:
    print("RED:", *problems, sep="\n  ")
    sys.exit(1)
print("GREEN")
