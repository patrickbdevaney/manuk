#!/usr/bin/env python3 -I
"""CONC.3 Layer 2: many-model drivers over the same MCP arms (docs/sprint/CONCURRENCY-PLAN.md, "Layer 2").

Drivers: claude (sonnet|opus|haiku via `claude -p`), agy (Gemini), groq, openrouter (:free models only), fake (self-test).
Arms (the two the gate needs; more can be passed): pw-proc, engine-proc-full. Same tasks, prompt and turn budget on every arm.
Blocks = one model; arms are interleaved inside a block per (task, rep) group in randomised order.
An infrastructure failure (rate limit, provider 5xx, timeout, CLI auth/start error) is never a task failure: the WHOLE group
(every arm) is re-run; if it still fails after ATTEMPTS the group is written with status=infra on every arm and is excluded
from scoring. Missing credential => exit 3, no rows. Credentials are read inside this process only, one named variable each.
Usage: layer2.py run --driver D --model M [--arms ..] [--tasks ..] [--reps R] [--out F]
Env: MANUK_CHROME_PIN (required), CONC3_ENV_GROQ / CONC3_ENV_OPENROUTER (override env-file path; self-test), CONC3_MUTATE.
"""
import argparse, json, os, random, re, subprocess, sys, tempfile, threading, time, urllib.request, urllib.error

HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE); sys.path.insert(0, os.path.join(ROOT, "scripts"))
import harness, fixtures

BIN = os.path.expanduser("~/.local/bin")
ARMS = ["pw-proc", "engine-proc-full"]
ATTEMPTS = 3
MAX_TURNS = 14
MUTATE = os.environ.get("CONC3_MUTATE", "")
COLS = ["chrome_ver", "commit", "driver", "model", "arm", "task", "rep", "attempt", "status", "success", "turns", "tokens_in",
        "tokens_out", "cost_usd", "wall_s", "answer", "note"]

# Same three tasks on every arm (the Layer 1 form/late/walk traces, as instructions). Success = the final ANSWER token.
TASKS = {
    "form": ("Open {base}/form . Type Ada into the Name field, press the Submit button, then read the heading that changes below it.", "SUBMITTED-Ada"),
    "late": ("Open {base}/late . Press the Go button and wait until the heading below it stops saying idle (it changes after a short delay).", "LANDED-late"),
    "walk": ("Open {base}/p/1 . Follow the Next link on each page until you reach a page whose body has a marker word in capitals.", "PAGE-THREE-REACHED"),
}
PREFIX = ("You are driving a web browser ONLY through the browser tools you were given (no shell, no files). Finish the task, "
          "then end your reply with one final line of the form `ANSWER: <the exact text you read from the page>`. Task: ")


class Infra(Exception):
    """Provider/rate-limit/start failure: not a task result."""


def pin():
    return os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")


def read_key(envfile, name):
    """One named variable from one file, read here and nowhere else. None if absent."""
    try:
        for line in open(os.path.expanduser(envfile)):
            k, _, v = line.strip().partition("=")
            if k == name and v.strip().strip("'\""): return v.strip().strip("'\"")
    except OSError: pass
    return None


def arm_cmd(arm):
    return harness.Arm(arm, 1)._cmd(), ({"MANUK_CHROME": harness.chrome_exe()} if arm == "chrome-mcp" else {})


def host_busy_reason(driver):
    """Shared-quota guards from the owner note: no Groq while a judged batch runs; no OpenRouter while archipelago works."""
    if driver == "groq":
        r = subprocess.run(["pgrep", "-f", "real_run.py|real_judge.py|w5b1_rejudge.py"], capture_output=True, text=True)
        if r.stdout.strip(): return "a judged batch is running"
    if driver == "openrouter":
        for d in os.listdir("/proc"):
            if d.isdigit():
                try:
                    if os.readlink(f"/proc/{d}/cwd").startswith(os.path.expanduser("~/archipelago")): return "archipelago has a live process"
                except OSError: pass
    return None


# ---- drivers: run(arm, prompt) -> dict(answer, turns, tokens_in, tokens_out, cost, note); raise Infra for infrastructure ----
RATE = re.compile(r"rate.?limit|429|overloaded|529|5\d\d |temporar|quota|timed? ?out|connection (reset|refused)|not (logged|signed) in|auth", re.I)


class Claude:
    name = "claude"
    def __init__(self, model): self.model = model
    def run(self, arm, prompt, tmp):
        cmd, env = arm_cmd(arm)
        cfg = os.path.join(tmp, "mcp.json"); json.dump({"mcpServers": {"arm": {"command": cmd[0], "args": cmd[1:], "env": env}}}, open(cfg, "w"))
        argv = [f"{BIN}/claude", "-p", prompt, "--model", self.model, "--mcp-config", cfg, "--strict-mcp-config", "--tools", "",
                "--allowedTools", "mcp__arm", "--permission-mode", "dontAsk", "--output-format", "json", "--max-turns", str(MAX_TURNS),
                "--no-session-persistence", "--disable-slash-commands", "--max-budget-usd", "1"]
        try: p = subprocess.run(argv, capture_output=True, text=True, timeout=420, cwd=tmp)
        except subprocess.TimeoutExpired: raise Infra("timeout")
        try: j = json.loads(p.stdout)
        except ValueError: raise Infra(f"no json rc={p.returncode}: {(p.stderr or p.stdout)[:120]}")
        res = str(j.get("result", "")); u = j.get("usage", {}) or {}
        if j.get("is_error") and RATE.search(res + str(j.get("subtype", ""))): raise Infra(res[:120])
        tin = sum(int(u.get(k, 0) or 0) for k in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"))
        return dict(answer=res, turns=j.get("num_turns"), tokens_in=tin, tokens_out=u.get("output_tokens"), cost=j.get("total_cost_usd"),
                    note=j.get("subtype", ""))


class Agy:
    """Gemini through agy. agy has no per-call MCP flag, so one server is registered for the call and removed after (one at a time).
    Its built-in tools cannot be disabled from here; the prompt forbids them and the note records that limit."""
    name = "agy"
    def __init__(self, model): self.model = model
    def run(self, arm, prompt, tmp):
        cmd, env = arm_cmd(arm); srv = "conc3-arm"
        subprocess.run([f"{BIN}/agy", "mcp", "remove", srv], capture_output=True)
        add = [f"{BIN}/agy", "mcp", "add"] + [x for k, v in env.items() for x in ("--env", f"{k}={v}")] + ["--", srv] + cmd
        if subprocess.run(add, capture_output=True, text=True).returncode: raise Infra("agy mcp add failed")
        try:
            argv = [f"{BIN}/agy", "-p", prompt, "--model", self.model, "--dangerously-skip-permissions", "--output-format", "json"]
            try: p = subprocess.run(argv, capture_output=True, text=True, timeout=420, cwd=tmp)
            except subprocess.TimeoutExpired: raise Infra("timeout")
        finally: subprocess.run([f"{BIN}/agy", "mcp", "remove", srv], capture_output=True)
        out = p.stdout.strip()
        if p.returncode and RATE.search(p.stderr + out): raise Infra((p.stderr or out)[:120])
        try: j = json.loads(out); res = str(j.get("result") or j.get("response") or j.get("text") or out); u = j.get("usage", {}) or {}
        except ValueError: j, res, u = {}, out, {}
        if not res: raise Infra(f"empty rc={p.returncode}: {p.stderr[:100]}")
        return dict(answer=res, turns=j.get("num_turns"), tokens_in=u.get("input_tokens"), tokens_out=u.get("output_tokens"), cost=None,
                    note="agy builtin tools not disabled")


class ChatApi:
    """OpenAI-compatible chat + tool loop over the arm's MCP server (Groq, OpenRouter)."""
    url = key = None
    def __init__(self, model): self.model = model
    def post(self, body):
        req = urllib.request.Request(self.url, json.dumps(body).encode(), {"Content-Type": "application/json", "Authorization": f"Bearer {self.key}", "User-Agent": "conc3-driver/1"})
        try:
            with urllib.request.urlopen(req, timeout=120) as r: return json.load(r)
        except urllib.error.HTTPError as e:
            if e.code in (408, 429) or e.code >= 500: self.on_limit(e.code); raise Infra(f"http {e.code}")
            raise Infra(f"http {e.code}: {e.read()[:100]!r}")  # auth/model/provider refusal: not a task result
        except (urllib.error.URLError, TimeoutError, ConnectionError) as e: raise Infra(f"net: {e}"[:120])
    def on_limit(self, code): pass
    def check(self, resp): pass
    def run(self, arm, prompt, tmp):
        cmd, env = arm_cmd(arm); cli = harness.Stdio(cmd, env); cli.hello()
        try:
            tl = cli.call("tools/list", {}).get("result", {}).get("tools", [])
            tools = [{"type": "function", "function": {"name": t["name"], "description": (t.get("description") or "")[:600], "parameters": t.get("inputSchema") or {"type": "object"}}} for t in tl]
            msgs = [{"role": "user", "content": prompt}]; tin = tout = turns = 0; cost = 0.0; last = ""
            for _ in range(MAX_TURNS):
                r = self.post({"model": self.model, "messages": msgs, "tools": tools, "temperature": 0}); self.check(r)
                u = r.get("usage") or {}; tin += u.get("prompt_tokens", 0); tout += u.get("completion_tokens", 0); cost += float(u.get("cost") or 0); turns += 1
                try: m = r["choices"][0]["message"]
                except (KeyError, IndexError): raise Infra(f"bad reply: {str(r)[:100]}")
                msgs.append({k: v for k, v in m.items() if k in ("role", "content", "tool_calls")}); last = m.get("content") or last
                if not m.get("tool_calls"): break
                for tc in m["tool_calls"]:
                    try: args = json.loads(tc["function"]["arguments"] or "{}"); out, err = cli.tool(tc["function"]["name"], args)
                    except Exception as e: out, err = f"error: {e}", True
                    msgs.append({"role": "tool", "tool_call_id": tc["id"], "content": out[:12000]})
            return dict(answer=last, turns=turns, tokens_in=tin, tokens_out=tout, cost=cost, note="")
        finally: cli.close()


class Groq(ChatApi):
    name = "groq"; url = "https://api.groq.com/openai/v1/chat/completions"
    def __init__(self, model, key): super().__init__(model); self.key = key


class OpenRouter(ChatApi):
    name = "openrouter"; url = "https://openrouter.ai/api/v1/chat/completions"
    def __init__(self, model, key):
        if not model.endswith(":free"): sys.exit("refusing: openrouter model id must end in :free (paid account, no spending limit)")
        super().__init__(model); self.key = key
    def on_limit(self, code):
        if code == 429: raise Stop("openrouter 429: block stopped (shared key)")
    def check(self, r):
        c = (r.get("usage") or {}).get("cost")
        if c not in (None, 0, 0.0): raise Stop(f"openrouter returned nonzero cost {c}: block stopped")


class Stop(Exception):
    """Stop the whole block (no further requests)."""


class Fake:
    """Self-test driver: no network, no browser. CONC3_FAKE_INFRA=<arm> makes that arm raise Infra on attempt 1 of every group."""
    name = "fake"
    def __init__(self, model): self.model = model; self.calls = {}
    def run(self, arm, prompt, tmp):
        k = prompt; self.calls[(arm, k)] = self.calls.get((arm, k), 0) + 1
        if os.environ.get("CONC3_FAKE_INFRA") == arm and self.calls[(arm, k)] == 1: raise Infra("fake 429")
        return dict(answer=f"ANSWER: {self.expect(prompt)}", turns=1, tokens_in=1, tokens_out=1, cost=0.0, note="fake")
    @staticmethod
    def expect(prompt):
        for t, (p, tok) in TASKS.items():
            if p.split("{base}")[1] in prompt: return tok
        return ""


def make_driver(a):
    d = a.driver
    if d == "claude": return Claude(a.model)
    if d == "agy": return Agy(a.model)
    if d == "fake": return Fake(a.model)
    if d == "groq":
        k = read_key(os.environ.get("CONC3_ENV_GROQ", os.path.join(ROOT, ".env")), "GROQ_API_KEY_1")
        if not k: print("refusing: GROQ_API_KEY_1 missing (exit 3, no rows)", file=sys.stderr); sys.exit(3)
        return Groq(a.model, k)
    if d == "openrouter":
        if not a.model.endswith(":free"): sys.exit("refusing: openrouter model id must end in :free (paid account, no spending limit)")
        k = read_key(os.environ.get("CONC3_ENV_OPENROUTER", "~/archipelago/.env"), "OPENROUTER_API_KEY_0")
        if not k: print("refusing: OPENROUTER_API_KEY_0 missing (exit 3, no rows)", file=sys.stderr); sys.exit(3)
        return OpenRouter(a.model, k)
    sys.exit(f"unknown driver {d}")


def score(answer, expect):
    m = re.findall(r"ANSWER:\s*(.+)", answer or "")
    return bool(m) and expect in m[-1]


def commit():
    return subprocess.run(["git", "-C", ROOT, "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()


def run_block(a, out):
    drv = make_driver(a)
    why = host_busy_reason(a.driver) if os.environ.get("CONC3_NO_GUARD") != "1" else None
    if why: print(f"refusing: {why}", file=sys.stderr); return 4
    srv = fixtures.serve(); threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{srv.server_address[1]}"; rows = []; rng = random.Random(a.seed)
    groups = [(t, r) for r in range(1, a.reps + 1) for t in a.tasks]; rng.shuffle(groups)
    for task, rep in groups:
        prompt = PREFIX + TASKS[task][0].format(base=base); res = {}
        for attempt in range(1, ATTEMPTS + 1):
            order = list(a.arms); rng.shuffle(order); res = {}; infra = False
            for arm in order:
                t0 = time.time(); tmp = tempfile.mkdtemp(prefix="conc3-")
                try: r = drv.run(arm, prompt, tmp); st = "ok"
                except Infra as e:
                    r, st = dict(answer="", turns=None, tokens_in=None, tokens_out=None, cost=None, note=str(e)[:100]), "infra"; infra = True
                except Stop as e: print(f"stopped: {e}", file=sys.stderr); write(out, rows); return 5
                r["wall_s"] = round(time.time() - t0, 1); r["status"] = st; res[arm] = r
                if st == "infra" and MUTATE != "score-infra": break  # a block is only valid when every arm ran: abandon, re-run all
                if a.driver == "openrouter": time.sleep(2)
            if not infra or MUTATE == "score-infra": break
            time.sleep(5 if a.driver != "fake" else 0)
        for arm in a.arms:
            r = res.get(arm) or dict(answer="", turns=None, tokens_in=None, tokens_out=None, cost=None, note="not run: sibling arm infra", wall_s=0, status="infra")
            sib = any(x["status"] == "infra" for x in res.values()) and MUTATE != "score-infra"
            st = "infra" if (r["status"] == "infra" or sib) else ("pass" if score(r["answer"], TASKS[task][1]) else "fail")
            if MUTATE == "score-infra" and r["status"] == "infra": st = "fail"  # mutant: infra scored as a task failure
            if st == "infra" and r["status"] != "infra": r["note"] = "sibling arm infra"
            rows.append([pin(), commit(), a.driver, a.model, arm, task, rep, attempt, st, int(st == "pass"), r["turns"], r["tokens_in"], r["tokens_out"],
                         r["cost"], r["wall_s"], re.sub(r"\s+", " ", (r["answer"] or "")[-160:]), r["note"]])
    write(out, rows); return 0


def write(out, rows):
    if not rows: return
    new = not os.path.exists(out) or not os.path.getsize(out)
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    with open(out, "a") as f:
        if new: f.write("\t".join(COLS) + "\n")
        for r in rows: f.write("\t".join("" if x is None else str(x).replace("\t", " ").replace("\n", " ") for x in r) + "\n")


def main():
    ap = argparse.ArgumentParser(); sp = ap.add_subparsers(dest="cmd", required=True); r = sp.add_parser("run")
    r.add_argument("--driver", required=True); r.add_argument("--model", required=True); r.add_argument("--arms", nargs="+", default=ARMS)
    r.add_argument("--tasks", nargs="+", default=list(TASKS)); r.add_argument("--reps", type=int, default=1)
    r.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/conc3/layer2.tsv")); r.add_argument("--seed", type=int, default=0)
    a = ap.parse_args(); pin()
    sys.exit(run_block(a, a.out))


if __name__ == "__main__":
    main()
