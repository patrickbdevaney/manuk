#!/usr/bin/env python3 -I
"""W4.C3.a: best-of-3 with real model-sampled candidates on REAL-47, per arm.
Candidates = the 3 independent Haiku agent runs per task that real_run.py produced (run 1..3, temperature-sampled by the
model, each a full trajectory). single = run 1 alone (pass@1). BoN-3 = a Haiku judge, given ONLY the task goal and each
candidate's final answer + the clone's state diff (never the grader's expected value), picks one; the pick is graded by
REAL's own jmespath rule. oracle = any of the 3 passes (the ceiling for any judge).
This is episode-level BoN (3 whole trajectories, judge at the end), not an act-level fork: the fork is replay-based.
Usage: python3 -I c3a_bon_real.py RUNS.tsv [--out SUMMARY.tsv] [--judged JUDGED.tsv]
MANUK_C3A_MUTATE=1: the judge always picks candidate 1 (BoN must collapse to single run 1).
Needs MANUK_CHROME_PIN (every row must carry it).
"""
import argparse, csv, json, os, random, re, shutil, subprocess, sys, statistics

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
CLAUDE = shutil.which("claude") or os.path.expanduser("~/.local/bin/claude")
sys.path.insert(0, HERE)


def load_tasks():
    import real_run
    return {t["id"]: t for t in real_run.load_tasks()}, real_run


def candidate(arm, task, run):
    d = os.path.join(ROOT, "target/real-runs", f"{arm}_{task}_r{run}")
    ans, diff = "", "(no state)"
    try:
        for l in open(os.path.join(d, "transcript.jsonl")):
            if l.startswith("{"):
                e = json.loads(l)
                if e.get("type") == "result":
                    ans = (e.get("result") or "")[:700]
    except OSError:
        pass
    try:
        st = json.load(open(os.path.join(d, "state.json")))
        if st.get("ok"):
            diff = json.dumps((st["state"] or {}).get("differences", st["state"]))[:1500]
    except (OSError, ValueError):
        pass
    return ans, diff


def judge(goal, cands):
    """-> (index, usd, seconds). Haiku sees the goal and the 3 candidates, nothing about grading."""
    mode = os.environ.get("MANUK_C3A_MUTATE")
    if mode == "random":  # W4.C3.a.a.a red-proof: an unseeded coin-flip judge, must show a spread >= 4 pt somewhere
        return random.randrange(3), 0.0, 0.0
    if mode:
        return 0, 0.0, 0.0
    body = "\n\n".join(f"Candidate {i+1}\nAgent's final message: {a or '(none)'}\nChange the site recorded (state diff): {d}"
                       for i, (a, d) in enumerate(cands))
    prompt = (f"Task given to a browser agent: {goal}\n\nThree independent attempts were made. {body}\n\n"
              "Which attempt most plausibly completed the task exactly as asked? Answer with only the number 1, 2 or 3.")
    import time
    t0 = time.time()
    r = subprocess.run([CLAUDE, "-p", "--model", "haiku", "--output-format", "json", "--no-session-persistence", "--tools", ""],
                       input=prompt, capture_output=True, text=True, timeout=180, cwd="/tmp")
    try:
        j = json.loads(r.stdout); out, usd = j.get("result", ""), j.get("total_cost_usd", 0.0)
    except ValueError:
        out, usd = r.stdout, 0.0
    m = re.search(r"[123]", out)
    return (int(m.group()) - 1 if m else 0), usd, time.time() - t0


def boot_ci(per_task, n=2000, seed=7):
    r = random.Random(seed); k = len(per_task)
    xs = sorted(sum(per_task[r.randrange(k)] for _ in range(k)) for _ in range(n))
    return xs[int(.025 * n)], xs[int(.975 * n)]


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("runs", nargs="+"); ap.add_argument("--out"); ap.add_argument("--judged")
    ap.add_argument("--group", default="1,2,3", help="W4.C3.a.a: the 3 run numbers that form one BoN-3 group; single = the first")
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    group = tuple(int(x) for x in a.group.split(","))
    if len(group) != 3:
        sys.exit("refusing: --group needs exactly 3 run numbers")
    import real_run
    rows = []
    for f in a.runs:  # each file is read by its own header: the W4.C3.a TSV was written under an older COLS (no evals_vec/evals_n),
        lines = open(f).read().splitlines()  # and a header line can land mid-file (parallel arms share it)
        cols = next((l.split("\t") for l in lines if l.startswith("ts\t")), real_run.COLS)
        for l in lines:
            if not l or l.startswith("#"):
                continue
            if l.startswith("ts\t"):
                cols = l.split("\t")
                continue
            rows.append(dict(zip(cols, l.split("\t"))))
    bad = {r["chrome_ver"] for r in rows} - {pin}
    bad and sys.exit(f"refusing: rows under chrome_ver {bad}, pin is {pin}")
    tasks, _ = load_tasks()
    out, jl = [], ["chrome_ver\tarm\ttask\tpick\tpicked_pass\tsingle_pass\toracle\tcand_pass\tjudge_usd\tjudge_s"]
    print("arm\tgroup\tn_tasks\tper-run pass (band)\tsingle(run1)\tBoN-3\tBoN CI95\toracle\tjudge-correct%\ttok/task single\ttok/task BoN\twall_s per success single\twall_s per success BoN"
          "\tpaired BoN-single\tpaired diff CI95\tCI excludes 0")
    for arm in ["manuk", "cdp-layer", "playwright"]:
        R = [r for r in rows if r["arm"] == arm]
        by = {}
        for r in R:
            by.setdefault(r["task"], {})[int(r["run"])] = r
        full = {t: {k: v[k] for k in group} for t, v in by.items() if all(k in v for k in group)}
        if not full:
            continue
        per_run = [sum(int(v[k]["pass"]) for v in full.values()) for k in group]
        sing, bon, orc, jc, jusd, jsec = [], [], [], 0, 0.0, 0.0
        for t, v in sorted(full.items()):
            cs = [candidate(arm, t, k) for k in group]
            ps = [int(v[k]["pass"]) for k in group]
            pick, usd, sec = judge(tasks[t]["goal"], cs)
            jusd += usd; jsec += sec
            sing.append(ps[0]); bon.append(ps[pick]); orc.append(int(any(ps)))
            jc += int(ps[pick] == int(any(ps)))
            jl.append(f"{pin}\t{arm}\t{t}\t{pick+1}\t{ps[pick]}\t{ps[0]}\t{int(any(ps))}\t{''.join(map(str, ps))}\t{usd:.4f}\t{sec:.1f}")
        n = len(full)
        g0 = group[0]
        tok1 = statistics.mean(int(v[g0]["input_tokens"]) + int(v[g0]["output_tokens"]) for v in full.values())
        tok3 = 3 * statistics.mean(int(x["input_tokens"]) + int(x["output_tokens"]) for v in full.values() for x in v.values())
        w1 = sum(float(v[g0]["seconds"]) for v in full.values()); w3 = sum(float(x["seconds"]) for v in full.values() for x in v.values()) + jsec
        lo, hi = boot_ci(bon)
        # paired: per task, BoN minus single (same task, same group); the CI is a task bootstrap of the summed difference
        diff = [b - s for b, s in zip(bon, sing)]
        dlo, dhi = boot_ci(diff)
        ws = lambda w, s: f"{w/max(1,s):.0f}" if s else "n/a"
        excl = "yes" if dlo > 0 or dhi < 0 else "no"
        gname = ",".join(map(str, group))
        line = (f"{arm}\t{gname}\t{n}\t{per_run} ({min(per_run)}-{max(per_run)})\t{sum(sing)}\t{sum(bon)}\t{lo}-{hi}\t{sum(orc)}\t{100*jc/n:.0f}\t{tok1:.0f}\t{tok3:.0f}"
                f"\t{ws(w1, sum(sing))}\t{ws(w3, sum(bon))}\tjudge ${jusd:.2f}\t{sum(diff):+d}\t{dlo:+d}..{dhi:+d}\t{excl}")
        print(line); out.append(line)
    if a.judged:
        open(a.judged, "w").write("\n".join(jl) + "\n")
    if a.out:
        open(a.out, "w").write(f"# chrome_ver={pin}\n" + "\n".join(out) + "\n")


if __name__ == "__main__":
    main()
