#!/usr/bin/env python3
"""W6.H1.b: the TaskRun-vs-Hub A/B on suite-v1 (40 tasks), engine backend, 1 run each, one judge.

  python3 -I bench/agent-tasks/h1_ab.py run --out bench-queue/out/w6h1b-ab.tsv     # 80 runs (40 tasks x 2 drivers)
  python3 -I bench/agent-tasks/h1_ab.py tally bench-queue/out/w6h1b-ab.tsv          # per-driver pass + act-sequence diff
  python3 -I bench/agent-tasks/h1_ab.py self-test                                  # the act normaliser and the
                                                                                   # tally's diff check can go red

Drivers: `manuk agent-task <url> <goal> --driver taskrun|hub --grant all` (shell/src/headless.rs). `--grant all` is
required for a fair A/B: TaskRun enforces its grant and the Hub ignores it, so under the default read-only grant the
first W6.H1.b run (2026-11-25) compared a TaskRun that could not click, type or submit against an unrestricted Hub. Both run the same engine page
loader and the same model backend (Groq, GROQ_MODEL); only the agent state machine differs. The judge is the suite's
own deterministic verifier (verify.verify) applied to the ANSWER line of both drivers, so it is identical across arms.

The model is reached through groq_relay.py (MANUK_LLAMA_PORT -> a localhost relay): Groq's per-key tokens-per-minute
cap becomes a WAIT inside the relay instead of a failed task, keys are used one at a time (next key only on a long
wait or a refusal), and every call is logged to <out-dir>/<out-stem>.traces/<task>_<driver>_a<n>.jsonl together with
the agent's stdout/stderr (.out/.err) and the fixture end state (.state.json). Those traces are what h1_attrib.py
reads to decide whether a failure was the ENGINE's or the MODEL's.

The time budget excludes relay waits: a run whose wall time minus its rate-limit waits exceeds BUDGET_S is a timeout;
waiting in Groq's queue is not engine time. A run the relay still could not serve (waits > its cap, 5xx persisting)
is infra and retried once; one whose single request exceeds the key's whole per-minute budget is CAPACITY. Infra and
capacity runs are unscored (pass empty), never counted as fails. Keys are read in-process from .env (GROQ_API_KEY_n),
handed to the relay only, never to the agent's environment and never written out; traces carry key NAMES.
Every row carries chrome_ver=$MANUK_CHROME_PIN (the engine arms never touch Chrome; the column is kept for the join).
"""
import csv, json, os, re, subprocess, sys, time, collections

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "../.."))
sys.path.insert(0, HERE)
import verify as V  # noqa: E402  (the suite's own verifier: the one judge for both arms)
import fixtures  # noqa: E402
import groq_relay  # noqa: E402

BIN = os.path.join(ROOT, "target/release/manuk")
MODEL = os.environ.get("GROQ_MODEL", "qwen/qwen3.8-27b")  # the crate default qwen/qwen3.6-27b is no longer served
BUDGET_S, MAX_STEPS = 240, 40     # BUDGET_S is ACTIVE time: wall minus the relay's rate-limit waits
WAIT_ALLOWANCE_S = 1200            # extra wall the agent is given so relay waits cannot exhaust its own clock
MAX_ATTEMPTS, RETRY_WAIT_S = 2, 65 # the relay already absorbs 429s; a whole-task retry is for a relay give-up only
DRIVERS = ["taskrun", "hub"]
# infra = the model endpoint refused or timed out (a failed/Error line), never page text: Groq's per-key tokens-per-minute
# cap (7000 ITPM on qwen3.8-27b) is the expected case
INFRA = re.compile(r"^(failed|Error).*((groq|llama\.cpp) HTTP (429|5\d\d|502)|timed out|dns error|Connection refused)",
                   re.M | re.I)
COLS = ["task", "run", "pass", "judge_model", "arm", "driver", "attempt", "rc", "infra", "steps", "answer",
        "reasons", "act_seq", "observe_n", "wall_s", "chrome_ver", "outcome", "active_s", "relay_wait_s", "relay_busy_s", "model_s",
        "model_calls", "rate_limited", "prompt_tokens", "max_prompt_tokens", "completion_tokens", "keys_used", "trace"]


def groq_keys():
    keys = []
    with open(os.path.join(ROOT, ".env")) as f:
        for line in f:
            m = re.match(r"GROQ_API_KEY_(\d+)=(.*)$", line.strip())
            if m and m.group(2).strip().strip("'\""):
                keys.append((int(m.group(1)), m.group(2).strip().strip("'\"")))
    return [("GROQ_API_KEY_%d" % n, k) for n, k in sorted(keys)]


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


def run_one(url, goal, driver, relay, port, trace):
    env = {k: v for k, v in os.environ.items() if not k.startswith("GROQ_API_KEY")}
    env.update(GROQ_MODEL=MODEL, MANUK_LLAMA_PORT=str(port))
    relay.begin(trace + ".jsonl")
    t0 = time.time()
    try:
        p = subprocess.run([BIN, "agent-task", url, goal, "--driver", driver,
                            "--timeout-s", str(BUDGET_S + WAIT_ALLOWANCE_S), "--max-steps", str(MAX_STEPS), "--grant", "all"],
                           capture_output=True, text=True, env=env, timeout=BUDGET_S + WAIT_ALLOWANCE_S + 90, cwd=ROOT)
        rc, out, err = p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired as e:
        rc, out, err = 124, (e.stdout or b"").decode() if isinstance(e.stdout, bytes) else (e.stdout or ""), "timed out"
    wall = time.time() - t0
    st = relay.end()
    with open(trace + ".out", "w") as f:
        f.write(out)
    out, err = groq_relay.redact(out), groq_relay.redact(err)
    with open(trace + ".err", "w") as f:
        f.write(err)
    answer, steps, acts, observe_n, tr = parse_transcript(out)
    active = wall - st["busy_s"]   # everything the agent spent inside the relay is the model side's
    if rc != 0 and st["oversize"]:
        outcome = "capacity"
    elif rc != 0 and (st["infra"] or INFRA.search(out + "\n" + err)):
        outcome = "infra"
    elif active > BUDGET_S or rc == 124:
        outcome = "timeout"
    else:
        outcome = "ran"
    return dict(rc=rc, answer=answer, steps=steps, acts=acts, observe_n=observe_n, wall=wall, active=active,
                infra=int(outcome in ("infra", "capacity")), outcome=outcome, relay=st,
                err=(err.strip().splitlines() or [""])[-1][:200])


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
    relay = groq_relay.Relay(keys, MODEL)
    rsrv, rport = groq_relay.serve(relay)
    tdir = os.path.splitext(os.path.abspath(out_path))[0] + ".traces"
    os.makedirs(tdir, exist_ok=True)
    rows = []
    try:
        for i, t in enumerate(tasks):
            url = t["start_url"].replace("{BASE}", base)
            for d in DRIVERS:
                attempts = []
                for a in range(MAX_ATTEMPTS):  # only a relay give-up (or a capacity-sized request) gets here twice
                    if a:
                        time.sleep(RETRY_WAIT_S)
                    fixtures.reset()
                    trace = os.path.join(tdir, f"{t['id']}_{d}_a{a + 1}")
                    r = run_one(url, t["goal"], d, relay, rport, trace)
                    r["trace"] = os.path.relpath(trace, ROOT)
                    attempts.append(r)
                    if not r["infra"] or r["outcome"] == "capacity":  # an oversize request is deterministic
                        break
                last = attempts[-1]
                if last["infra"]:
                    reasons, passed = "%s: unscored after %d attempts" % (last["outcome"], len(attempts)), ""
                else:
                    state = None
                    try:
                        state = json.load(urllib_open(base + "/__state")) if "{BASE}" in t["start_url"] else None
                    except Exception:
                        state = None
                    with open(os.path.join(ROOT, last["trace"]) + ".state.json", "w") as f:
                        json.dump(state, f)
                    v = V.verify(t, last["answer"] or "", state)
                    reasons, passed = "; ".join(v["reasons"]), int(v["pass"])
                    if last["outcome"] == "timeout":
                        reasons, passed = ("timeout: active %ds > %ds; " % (last["active"], BUDGET_S)) + reasons, 0
                for n, r in enumerate(attempts, 1):
                    rows.append({"pass": (passed if n == len(attempts) else ""), "task": t["id"], "run": "1",
                                 "judge_model": "verify.py", "arm": d, "driver": d, "attempt": n, "rc": r["rc"],
                                 "infra": r["infra"], "steps": r["steps"] if r["steps"] is not None else "",
                                 "answer": (r["answer"] or "").replace("\t", " ").replace("\n", " ")[:200],
                                 "reasons": (reasons if n == len(attempts) else "") or "",
                                 "act_seq": ">".join(r["acts"]), "observe_n": r["observe_n"],
                                 "wall_s": round(r["wall"]), "chrome_ver": chrome, "outcome": r["outcome"],
                                 "active_s": round(r["active"]), "relay_wait_s": r["relay"]["wait_s"], "relay_busy_s": r["relay"]["busy_s"],
                                 "model_s": r["relay"]["model_s"], "model_calls": r["relay"]["calls"],
                                 "rate_limited": r["relay"]["rate_limited"],
                                 "prompt_tokens": r["relay"]["prompt_tokens"],
                                 "max_prompt_tokens": r["relay"]["max_prompt_tokens"],
                                 "completion_tokens": r["relay"]["completion_tokens"],
                                 "keys_used": ",".join(r["relay"]["keys_used"]), "trace": r["trace"]})
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
        rsrv.shutdown()
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
    check("a relay-passed 429 is infra", bool(INFRA.search("failed: model: llama.cpp HTTP 429: {}")))
    check("the relay's own self-test is green", groq_relay.self_test() == 0)
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
