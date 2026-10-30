#!/usr/bin/env python3
"""W3.18.a.a: REAL (agisdk 0.3.5 v1) DETERMINISTIC-subset slice, one harness for every arm.

  real_run.py run   --arm manuk|manuk+rollback.v2|cdp-layer|playwright [--runs 3] [--tasks ID,..] [--jobs 1] [--out TSV]
  real_run.py list                       (the scored task list: every eval is jmespath, omnizon excluded)
  real_run.py grade FILE.json TASK       (grade a /finish JSON offline, REAL's own jmespath + exact_match rule)
  real_run.py proxy ... -- CMD ...       (internal: stdio MCP shim, see below)

Scoring is REAL's own: env_state = the clone's /finish JSON (localStorage s0->sT diff), each jmespath eval passes iff
jmespath.search(query, env_state) == expected_value (evaluate.py exact_match), a task passes iff all evals pass.
The 64 llm_boolean evals need REAL's OpenAI grader (no key was provided): any task holding one is NOT SCORED, for every
arm alike; omnizon (HTTP 451 DMCA) is excluded for every arm. Nothing is stubbed.

The shim sits between `claude -p` and the arm's MCP server. After `initialized` it replays REAL's setup through the
arm's own tools (/config?run_id=0&task_id=..., then /finish for s0, then the start URL is left to the agent) and when
claude closes stdin / sends SIGTERM it reads /finish once more and writes the JSON to --state. The same two calls are
made on every arm, so the arms differ only in the browser. Run with `python3 -I`. Requires MANUK_CHROME_PIN.
"""
import argparse, glob, json, os, queue, signal, subprocess, sys, threading, time, concurrent.futures as cf

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
REAL = os.path.join(HERE, "real")
sys.path.insert(0, os.path.join(REAL, "vendor", "jmespath-1.0.1-py3-none-any.whl"))
EXCLUDED_SITES = {"omnizon"}
TOOLS = {  # (navigate tool, read-the-<pre> call) per tool surface
    "manuk": ("navigate", lambda u: {"url": u}, "extract", {}),
    "playwright": ("browser_navigate", lambda u: {"url": u}, "browser_evaluate",
                   {"function": "() => (document.querySelector('pre')||{}).innerText || ''"}),
}


def surface(arm):
    return "playwright" if arm == "playwright" else "manuk"


def load_tasks():
    return [json.load(open(f)) for f in sorted(glob.glob(os.path.join(REAL, "tasks", "*.json")))]


def scored(t):
    return t["website"]["id"] not in EXCLUDED_SITES and all(e["type"] == "jmespath" for e in t["evals"])


def grade(task, env):
    import jmespath
    res = []
    for e in task["evals"]:
        try:
            got = jmespath.search(e["query"], env)
        except Exception as ex:  # REAL: a query error is a failed eval
            got = f"Error: {ex}"
        res.append(got == e["expected_value"])
    return bool(res) and all(res), sum(res), len(res)


# ---------------------------------------------------------------- shim
def parse_state(text):
    if "### Result\n" in text:  # playwright: the evaluate result is a JSON string literal under '### Result'
        seg = text.split("### Result\n", 1)[1].split("\n### ", 1)[0].strip()
        try:
            v = json.loads(seg)
            text = v if isinstance(v, str) else seg
        except ValueError:
            text = seg
    a, b = text.find("{"), text.rfind("}")
    if a < 0 or b < a:
        return None
    try:
        v = json.loads(text[a:b + 1])
    except ValueError:
        return None
    return v if isinstance(v, dict) and v else None


def config_task(cfg):
    """The task_id REAL's /config stored: top-level, or one level down (opendining: config.open_dining.task_id)."""
    if not isinstance(cfg, dict):
        return None
    if "task_id" in cfg:
        return cfg["task_id"]
    return next((v["task_id"] for v in cfg.values() if isinstance(v, dict) and "task_id" in v), None)


def proxy(a, child):
    sys.stderr = open(a.state + ".err", "a", buffering=1)
    nav, navargs, rd, rdargs = TOOLS[surface(a.arm)]
    p = subprocess.Popen(child, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1, start_new_session=True)
    resp, out_lock, n = queue.Queue(), threading.Lock(), [0]

    def emit(line):
        with out_lock:
            sys.stdout.write(line if line.endswith("\n") else line + "\n"); sys.stdout.flush()

    def pump(stream=None):
        for l in (stream or p.stdout):
            try:
                j = json.loads(l)
            except ValueError:
                j = {}
            if str(j.get("id", "")).startswith("px-"):
                resp.put(j)
            else:
                emit(l)
        resp.put(None)
    threading.Thread(target=pump, daemon=True).start()
    log = lambda m: open(a.state + ".log", "a").write(f"{m} {time.time()}\n")

    def call(name, args):
        n[0] += 1
        i = f"px-{n[0]}"
        p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": "tools/call", "params": {"name": name, "arguments": args, "_meta": dict(meta)}}) + "\n")
        p.stdin.flush()
        while True:  # drop any stale response (a late answer to an earlier call)
            r = resp.get(timeout=90)
            if r is None or r.get("id") == i:
                break
        if not r or "result" not in r:
            return "shim-noresult: " + json.dumps(r)[:300]
        txt = "\n".join(c.get("text", "") for c in r["result"].get("content", []))
        log(f"call {name} -> {txt[:300]!r}")
        return txt

    def go(url):  # navigate, then let the SPA hydrate/persist (REAL waits for networkidle; manuk settles inside navigate)
        call(nav, navargs(url))
        if surface(a.arm) == "playwright":
            call("browser_wait_for", {"time": 3})

    def setup():
        go(f"{a.site}/config?run_id=0&task_id={a.task}&latency=0")
        go(f"{a.site}/finish")  # REAL setup(): /finish once so s0 is recorded

    done, setup_done = [False], [False]
    meta = {}  # SEP-2575 servers (rmcp 3) want per-request _meta: mirror what claude announced in initialize

    def snoop(l):
        try:
            j = json.loads(l)
        except ValueError:
            return
        pa = j.get("params") or {}
        if j.get("method") == "initialize":
            meta.update({"io.modelcontextprotocol/protocolVersion": pa.get("protocolVersion"),
                         "io.modelcontextprotocol/clientInfo": pa.get("clientInfo") or {"name": "claude-code", "version": "0"},
                         "io.modelcontextprotocol/clientCapabilities": pa.get("capabilities") or {}})
        elif isinstance(pa.get("_meta"), dict) and "io.modelcontextprotocol/protocolVersion" in pa["_meta"]:
            meta.update(pa["_meta"])

    def finish():
        if done[0]:
            return
        done[0] = True
        try:
            go(f"{a.site}/finish")
            for _ in range(8):  # REAL: wait_for_selector("pre")
                txt = call(rd, rdargs)
                st = parse_state(txt)
                if st is not None:
                    break
                time.sleep(1.5)
            # topwork/networkin-style clones return {xDiff: ...} with no config block; only check the id when one is present
            if not setup_done[0] or not isinstance(st, dict) or "jsonrpc" in st or ("config" in st and config_task(st["config"]) != a.task):
                txt, st = f"invalid state (setup={setup_done[0]}): {txt}", None
            open(a.state, "w").write(json.dumps({"ok": st is not None, "state": st, "raw": None if st else txt[:500]}))
        except Exception as ex:
            open(a.state, "w").write(json.dumps({"ok": False, "state": None, "raw": f"shim: {ex}"}))
        try:
            p.stdin.close()
        except OSError:
            pass
        try:
            p.wait(timeout=5)
        except subprocess.TimeoutExpired:
            p.kill()

    def detach_and_finish():
        """claude gives MCP servers ~0.5 s between SIGINT and SIGKILL, too little for navigate+read on Chrome: hand the
        /finish read to a detached process that outlives us (it keeps the pipes to the arm's MCP server, which has its own session)."""
        if os.fork():
            os._exit(0)
        os.setsid()
        for sg in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP, signal.SIGPIPE):
            signal.signal(sg, signal.SIG_IGN)
        sys.stdout = open(os.devnull, "w")
        threading.Thread(target=pump, args=(os.fdopen(os.dup(p.stdout.fileno()), "r"),), daemon=True).start()  # fresh file object: the old one's lock died with the pump thread
        log("detached")
        finish()
        os._exit(0)

    def term(*_):
        signal.signal(signal.SIGTERM, signal.SIG_IGN); signal.signal(signal.SIGINT, signal.SIG_IGN)  # claude repeats it
        log("signal")
        detach_and_finish()
    signal.signal(signal.SIGTERM, term); signal.signal(signal.SIGINT, term)
    for l in sys.stdin:
        snoop(l)
        if not setup_done[0] and '"tools/call"' in l:  # right before the agent's first tool call, never before initialize/tools/list (claude drops a server that is slow to list while Chrome boots)
            setup_done[0] = True
            setup()
        p.stdin.write(l); p.stdin.flush()
    log("stdin-eof")
    detach_and_finish()


# ---------------------------------------------------------------- driver
COLS = ["ts", "arm", "suite", "chrome_ver", "task", "site", "run", "model", "pass", "evals_ok", "evals_n", "state_ok",
        "checkpoints", "restores", "forks", "input_tokens", "output_tokens", "cost_usd", "seconds", "turns", "is_error", "answer"]
SUITE = "real-v1-det"


def tool_counts(stream):
    c = dict(checkpoint=0, restore=0, fork=0)
    for l in stream.splitlines():
        if not l.startswith("{"):
            continue
        try:
            e = json.loads(l)
        except ValueError:
            continue
        if e.get("type") == "assistant":
            for b in (e.get("message") or {}).get("content", []):
                if b.get("type") == "tool_use":
                    for k in c:
                        c[k] += b.get("name", "").endswith("__" + k)
    return c


PINNED = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")  # the CfT 145 pin (real_boot.py); never PATH's google-chrome


def chrome_wrapper(tdir):
    """The pinned Chrome, sandboxed: drop any --no-sandbox the backend passes (CB.SANDBOX is still open in the Rust
    backend; the userns sandbox works here via /etc/apparmor.d/chrome-cft)."""
    w = os.path.join(tdir, "chrome-pinned-sandboxed.sh")
    open(w, "w").write('#!/bin/sh\nfor a; do shift; [ "$a" = --no-sandbox ] || set -- "$@" "$a"; done\n'
                       f'exec {PINNED} "$@"\n')
    os.chmod(w, 0o755)
    return w


def pin_chrome(arm, srv, tdir):
    if arm == "playwright":
        args = [x for x in srv["args"] if x != "--no-sandbox"]
        args[args.index("--executable-path") + 1] = chrome_wrapper(tdir)
        srv["args"] = args
    elif arm == "cdp-layer":
        srv["env"] = dict(srv.get("env", {}), MANUK_CHROME=chrome_wrapper(tdir))


def one(arm, task, run, pin, tdir, timeout):
    sys.path.insert(0, HERE)
    import run_arm
    tmp = os.path.join(tdir, f"{arm}_{task['id']}_r{run}")
    os.makedirs(tmp, exist_ok=True)
    cfg_path, allowed = run_arm.arm_config(arm, tmp)
    cfg = json.load(open(cfg_path))
    srv = cfg["mcpServers"]["browser"]
    pin_chrome(arm, srv, tdir)
    state = os.path.join(tmp, "state.json")
    if os.path.exists(state):
        os.unlink(state)  # never grade a previous run's state
    site = task["website"]["url"]
    srv["args"] = ["-I", os.path.abspath(__file__), "proxy", "--arm", arm, "--site", site, "--task", task["id"], "--state", state,
                   "--", srv["command"], *srv["args"]]
    srv["command"] = sys.executable
    json.dump(cfg, open(cfg_path, "w"))
    prompt = run_arm.PROMPT.format(url=site, goal=task["goal"])
    out, err, secs, _ = run_arm.run_claude(prompt, cfg_path, allowed, timeout, stream=True, sys_hint=run_arm.HINTS.get(arm))
    open(os.path.join(tmp, "transcript.jsonl"), "w").write(out)
    j = run_arm.parse(out, stream=True) or {}
    u = j.get("usage", {})
    st = None
    for _ in range(240):  # claude may exit before the shim has read /finish
        if os.path.exists(state) and os.path.getsize(state):
            break
        time.sleep(0.5)
    if os.path.exists(state):
        st = json.load(open(state))
    ok, k, n = (False, 0, len(task["evals"]))
    if st and st["ok"]:
        ok, k, n = grade(task, st["state"])
    tc = tool_counts(out)
    return {"ts": time.strftime("%Y-%m-%dT%H:%M:%S"), "arm": arm, "suite": SUITE, "chrome_ver": pin, "task": task["id"],
            "site": task["website"]["id"], "run": run, "model": "haiku", "pass": int(ok and not j.get("is_error")), "evals_ok": k,
            "evals_n": n, "state_ok": int(bool(st and st["ok"])), "checkpoints": tc["checkpoint"], "restores": tc["restore"],
            "forks": tc["fork"], "input_tokens": u.get("input_tokens", 0), "output_tokens": u.get("output_tokens", 0),
            "cost_usd": j.get("total_cost_usd", 0), "seconds": f"{secs:.1f}", "turns": j.get("num_turns", ""),
            "is_error": int(bool(j.get("is_error")) or "result" not in j),
            "answer": (j.get("result") or err or "")[:200].replace("\t", " ").replace("\n", " ")}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["run", "list", "grade", "proxy"])
    ap.add_argument("rest", nargs="*")
    ap.add_argument("--arm"); ap.add_argument("--runs", type=int, default=3); ap.add_argument("--tasks"); ap.add_argument("--jobs", type=int, default=1)
    ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/w318/real-runs.tsv"))
    ap.add_argument("--timeout", type=int, default=420); ap.add_argument("--run-base", type=int, default=0, help="first run number - 1 (to interleave arms per run)")
    ap.add_argument("--site"); ap.add_argument("--task"); ap.add_argument("--state")
    a, extra = ap.parse_known_args()
    if a.cmd == "proxy":
        proxy(a, a.rest + [x for x in extra if x != "--"])
        return
    tasks = load_tasks()
    if a.cmd == "list":
        sc = [t for t in tasks if scored(t)]
        print(f"{len(sc)} scored / {len(tasks)} tasks; llm_boolean-holding: {sum(any(e['type']=='llm_boolean' for e in t['evals']) for t in tasks)}; omnizon: {sum(t['website']['id']=='omnizon' for t in tasks)}")
        for t in sc: print(t["id"])
        return
    if a.cmd == "grade":
        t = next(t for t in tasks if t["id"] == a.rest[1])
        print(grade(t, json.load(open(a.rest[0]))))
        return
    pin = os.environ.get("MANUK_CHROME_PIN")
    if not pin:
        sys.exit("refusing: MANUK_CHROME_PIN unset")
    got = subprocess.run([PINNED, "--version"], capture_output=True, text=True).stdout.split()[-1:]
    if got != [pin]:
        sys.exit(f"refusing: {PINNED} is {got}, not the pin {pin}")
    tasks = [t for t in tasks if scored(t)]
    if a.tasks:
        want = a.tasks.split(","); tasks = [t for t in tasks if t["id"] in want]
    tdir = os.path.join(ROOT, "target/real-runs")
    os.makedirs(tdir, exist_ok=True)
    new = not os.path.exists(a.out)
    lock = threading.Lock()
    with open(a.out, "a") as f:
        if new:
            f.write("\t".join(COLS) + "\n")

        def job(rt):
            run, t = rt
            r = one(a.arm, t, run, pin, tdir, a.timeout)
            with lock:
                f.write("\t".join(str(r.get(c, "")) for c in COLS) + "\n"); f.flush()
                print(f"{a.arm} {t['id']} r{run}: {'PASS' if r['pass'] else 'FAIL'} {r['evals_ok']}/{r['evals_n']} state_ok={r['state_ok']} restores={r['restores']}", flush=True)
                os.utime(os.path.join(ROOT, ".git/sprint-working"), None)
        with cf.ThreadPoolExecutor(a.jobs) as ex:
            list(ex.map(job, [(a.run_base + r, t) for r in range(1, a.runs + 1) for t in tasks]))


if __name__ == "__main__":
    main()
