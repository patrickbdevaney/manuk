#!/usr/bin/env python3
"""W6.H1.b: the TaskRun-vs-Hub A/B on suite-v1 (40 tasks), engine backend, 1 run each, one judge.

  python3 -I bench/agent-tasks/h1_ab.py run --out bench-queue/out/w6h1b-ab.tsv     # 80 runs (40 tasks x 2 drivers)
  python3 -I bench/agent-tasks/h1_ab.py tally bench-queue/out/w6h1b-ab.tsv          # per-driver pass + act-sequence diff
  python3 -I bench/agent-tasks/h1_ab.py self-test                                  # the act normaliser and the
                                                                                   # tally's diff check can go red

Drivers: `manuk agent-task <url> <goal> --driver taskrun|hub` (shell/src/headless.rs). Both run the same engine page
loader and the same model backend (Groq, GROQ_MODEL); only the agent state machine differs. The judge is the suite's
own deterministic verifier (verify.verify) applied to the ANSWER line of both drivers, so it is identical across arms.

Infra failures (HTTP 429/5xx from the model, timeouts, connection errors) are retried once with the next key and
recorded as attempts; a run still infra-failed after the retry is unscored (pass empty), never counted as a fail.
Keys come from .env (GROQ_API_KEY_n), read in-process and passed through the environment only; never written out.
Every row carries chrome_ver=$MANUK_CHROME_PIN (the engine arms never touch Chrome; the column is kept for the join).
"""
import csv, json, os, re, subprocess, sys, time, collections

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "../.."))
sys.path.insert(0, HERE)
import verify as V  # noqa: E402  (the suite's own verifier: the one judge for both arms)
import fixtures  # noqa: E402

BIN = os.path.join(ROOT, "target/release/manuk")
MODEL = os.environ.get("GROQ_MODEL", "qwen/qwen3.8-27b")  # the crate default qwen/qwen3.6-27b is no longer served
BUDGET_S, MAX_STEPS = 240, 40
MAX_ATTEMPTS, RETRY_WAIT_S = 4, 65  # a 429 says "try again in ~3s" per request; the ITPM window is one minute
DRIVERS = ["taskrun", "hub"]
# infra = the model endpoint refused or timed out (a failed/Error line), never page text: Groq's per-key tokens-per-minute
# cap (7000 ITPM on qwen3.8-27b) is the expected case
INFRA = re.compile(r"^(failed|Error).*(groq HTTP (429|5\d\d)|timed out|dns error|Connection refused)", re.M | re.I)
COLS = ["task", "run", "pass", "judge_model", "arm", "driver", "attempt", "rc", "infra", "steps", "answer",
        "reasons", "act_seq", "observe_n", "wall_s", "chrome_ver"]


def groq_keys():
    keys = []
    with open(os.path.join(ROOT, ".env")) as f:
        for line in f:
            m = re.match(r"GROQ_API_KEY_(\d+)=(.*)$", line.strip())
            if m and m.group(2).strip().strip("'\""):
                keys.append((int(m.group(1)), m.group(2).strip().strip("'\"")))
    return [k for _, k in sorted(keys)]


def normalise_act(line):
    """The action name a transcript line records, or None for a non-action line.

    Hub lines: `<tool> <args>`; TaskRun lines: `step N: <Action> {...}`. `observe` is a read (counted apart), and
    `finish`/`done` is the terminal step (the Hub prints it as `done:`, TaskRun as `Finish`): both are dropped so the
    sequences compare the acts taken before the answer. Lower-cased on both sides."""
    m = re.match(r"^step \d+: (\w+)", line)
    if m:
        name = m.group(1).lower()
    else:
        m = re.match(r"^(\w+)(?::| |$)", line)
        if not m:
            return None
        name = m.group(1).lower()
    if name in ("task", "driver", "done", "failed", "finish", "observe"):
        return None
    return name


def parse_transcript(out):
    lines = out.splitlines()
    tr, answer, steps = [], None, None
    for l in lines:
        if l.startswith("ANSWER: "):
            answer = l[len("ANSWER: "):]
        elif l.startswith("driver:"):
            m = re.search(r"steps: (\d+)", l)
            steps = int(m.group(1)) if m else None
        else:
            tr.append(l)
    acts = [a for a in (normalise_act(l) for l in tr) if a]
    observe_n = sum(1 for l in tr if l.startswith("observe:") or re.match(r"^step \d+: Observe", l))
    return answer, steps, acts, observe_n, tr


def run_one(url, goal, driver, key):
    env = dict(os.environ, GROQ_API_KEY=key, GROQ_MODEL=MODEL)
    env.pop("MANUK_LLAMA_PORT", None)
    t0 = time.time()
    try:
        p = subprocess.run([BIN, "agent-task", url, goal, "--driver", driver, "--timeout-s", str(BUDGET_S),
                            "--max-steps", str(MAX_STEPS)], capture_output=True, text=True, env=env,
                           timeout=BUDGET_S + 90, cwd=ROOT)
        rc, out, err = p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        rc, out, err = 124, "", "timed out"
    answer, steps, acts, observe_n, tr = parse_transcript(out)
    infra = int(rc != 0 and bool(INFRA.search(out + "\n" + err)))
    if rc == 124:
        infra = 1
    return dict(rc=rc, answer=answer, steps=steps, acts=acts, observe_n=observe_n, wall=time.time() - t0,
                infra=infra, err=(err.strip().splitlines() or [""])[-1][:200])


def run(out_path, only=None):
    from fixtures import serve  # noqa: E402
    keys = groq_keys()
    if not keys:
        sys.exit("h1_ab: no GROQ_API_KEY_n in .env")
    tasks = json.load(open(os.path.join(HERE, "suite-v1.json")))["tasks"]
    if only:
        tasks = [t for t in tasks if t["id"] in only]
    chrome = os.environ.get("MANUK_CHROME_PIN", "")
    srv, base = serve(0)
    rows = []
    try:
        for i, t in enumerate(tasks):
            url = t["start_url"].replace("{BASE}", base)
            for d in DRIVERS:
                attempts = []
                for a in range(MAX_ATTEMPTS):  # infra retries rotate the key and wait out the per-minute cap
                    if a:
                        time.sleep(RETRY_WAIT_S)
                    fixtures.reset()
                    r = run_one(url, t["goal"], d, keys[(i + a) % len(keys)])
                    attempts.append(r)
                    if not r["infra"]:
                        break
                last = attempts[-1]
                if last["infra"]:
                    reasons, passed = "infra: unscored after %d attempts" % len(attempts), ""
                else:
                    state = None
                    try:
                        state = json.load(urllib_open(base + "/__state")) if "{BASE}" in t["start_url"] else None
                    except Exception:
                        state = None
                    v = V.verify(t, last["answer"] or "", state)
                    reasons, passed = "; ".join(v["reasons"]), int(v["pass"])
                for n, r in enumerate(attempts, 1):
                    rows.append({"pass": (passed if n == len(attempts) else ""), "task": t["id"], "run": "1",
                                 "judge_model": "verify.py", "arm": d, "driver": d, "attempt": n, "rc": r["rc"],
                                 "infra": r["infra"], "steps": r["steps"] if r["steps"] is not None else "",
                                 "answer": (r["answer"] or "").replace("\t", " ").replace("\n", " ")[:200],
                                 "reasons": (reasons if n == len(attempts) else "") or "",
                                 "act_seq": ">".join(r["acts"]), "observe_n": r["observe_n"],
                                 "wall_s": round(r["wall"]), "chrome_ver": chrome})
                print(f"{t['id']} {d}: pass={passed!s:>1} rc={last['rc']} infra={last['infra']} "
                      f"acts={'>'.join(last['acts'])[:120]}", flush=True)
                # write as we go so an interrupted run leaves the rows it has
                os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
                with open(out_path, "w", newline="") as f:
                    w = csv.DictWriter(f, fieldnames=COLS, delimiter="\t")
                    w.writeheader()
                    w.writerows(rows)
    finally:
        srv.shutdown()
    print(f"wrote {len(rows)} rows -> {out_path}")


def urllib_open(u):
    import urllib.request
    return urllib.request.urlopen(u, timeout=10)


def tally(path):
    rows = list(csv.DictReader(open(path), delimiter="\t"))
    by = collections.defaultdict(dict)
    for r in rows:  # the final attempt of each (task, driver) is the scored one
        cur = by[r["task"]].get(r["driver"])
        if cur is None or int(r["attempt"]) > int(cur["attempt"]):
            by[r["task"]][r["driver"]] = r
    ids = sorted(by)
    passes = {d: 0 for d in DRIVERS}; scored = {d: 0 for d in DRIVERS}; infra = {d: 0 for d in DRIVERS}
    diffs_pass, diffs_act = [], []
    lines = []
    for tid in ids:
        a, b = by[tid].get("taskrun"), by[tid].get("hub")
        if not a or not b:
            continue
        for d, r in (("taskrun", a), ("hub", b)):
            if r["pass"] in ("0", "1"):
                scored[d] += 1; passes[d] += int(r["pass"])
            else:
                infra[d] += 1
        if a["pass"] != b["pass"]:
            diffs_pass.append(tid)
        if a["act_seq"] != b["act_seq"]:
            diffs_act.append(tid)
        lines.append(f"| {tid} | {a['pass'] or 'infra'} | {b['pass'] or 'infra'} | {a['act_seq'] or '-'} | {b['act_seq'] or '-'} | "
                     f"{'DIFF' if a['act_seq'] != b['act_seq'] else 'same'} |")
    out = dict(tasks=len(ids), passes=passes, scored=scored, infra=infra, pass_diff_tasks=diffs_pass,
               act_diff_tasks=diffs_act, table=lines)
    return out


def self_test():
    fails = []

    def check(name, cond):
        if not cond:
            fails.append(name)
    check("taskrun step line normalises", normalise_act("step 2: Click { target: 4 }") == "click")
    check("hub action line normalises", normalise_act("scroll 120") == "scroll")
    check("done/observe/finish are not acts", all(normalise_act(x) is None for x in ["done: x", "observe: y", "step 1: Finish { answer: \"a\" }"]))
    a = parse_transcript("task: t\nstep 1: Navigate { url: \"u\" }\nstep 2: Click { target: 1 }\nstep 3: Finish {}\ndone: A\ndriver: TaskRun  steps: 3\nANSWER: A\n")
    b = parse_transcript("task: t\nobserve: x\nclick 1\ndone: A\ndriver: Hub  steps: 1\nANSWER: A\n")
    check("same answer parsed", a[0] == b[0] == "A")
    check("taskrun acts = navigate>click", a[2] == ["navigate", "click"])
    check("hub acts = click (observe not counted)", b[2] == ["click"])
    check("a click-vs-none difference is visible to the tally", a[2] != b[2])
    check("a 429 from the model is infra", bool(INFRA.search("failed: model: groq HTTP 429: {}")))
    check("page text saying 'connection' is not infra", not INFRA.search("observe: connection refused by page"))
    check("a 404 model error is not infra (a real failure)", not INFRA.search("failed: model: groq HTTP 404: {}"))
    if fails:
        print("self-test RED:", fails)
        sys.exit(1)
    print("self-test green (normaliser, sequence-difference and infra-classifier checks)")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    cmd = sys.argv[1]
    if cmd == "run":
        out = sys.argv[sys.argv.index("--out") + 1]
        only = sys.argv[sys.argv.index("--only") + 1].split(",") if "--only" in sys.argv else None
        run(out, only)
    elif cmd == "tally":
        print(json.dumps({k: v for k, v in tally(sys.argv[2]).items() if k != "table"}, indent=1))
        for l in tally(sys.argv[2])["table"]:
            print(l)
    elif cmd == "self-test":
        self_test()
    else:
        sys.exit(__doc__)
