#!/usr/bin/env python3 -I
"""CONC.1 Layer 1: scripted, no-model driver for 5 browser arms at N concurrent agents (docs/sprint/CONCURRENCY-PLAN.md).

Arms: engine-proc (full|lean profile) | engine-fork (zygote) | chrome-mcp | pw-proc | pw-shared (one Chrome, one context/agent).
Subcommands:
  cell      --arm A --n N [--run R]     one measured cell, run INSIDE the cage (use `run`, which wraps it)
  run       --arms .. --ns .. --runs R --out F    caged cells in randomised blocks, appends TSV rows
  capacity  --arm A --out F             8 GB agent-capacity search (largest N within 5 points of the arm's N=1)
  selftest                              harness self-test (needs engine-proc + a built manuk-mcp)
Env: MANUK_CHROME_PIN (required), MANUK_MCP_BIN, CONC_PW_CLI, CONC1_MUTATE=share-proc|skip-failed (self-test mutants).
"""
import argparse, glob, http.client, json, os, random, re, select, signal, socket, statistics, subprocess, sys, tempfile, threading, time

HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE); sys.path.insert(0, os.path.join(ROOT, "scripts"))
import fixtures
from mcp_readline import readline as _rl

ARMS = ["engine-proc-full", "engine-proc-lean", "engine-fork", "chrome-mcp", "pw-proc", "pw-shared"]
NS = [1, 5, 10, 25, 50, 100]
CAGE = ["systemd-run", "--user", "--scope", "--quiet", "-p", "MemoryMax=8G", "-p", "MemorySwapMax=0"]
MIN_AVAIL_GB = 20
TOOL = {  # abstract op -> tool names per surface
    "manuk": dict(nav="navigate", snap="observe", click="act", type="type", find="find"),
    "pw": dict(nav="browser_navigate", snap="browser_snapshot", click="browser_click", type="browser_type"),
}
MUTATE = os.environ.get("CONC1_MUTATE", "")


def pin():
    return os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")

def chrome_exe():
    exe = os.path.expanduser(f"~/.local/chrome-cft/{pin()}/chrome-linux64/chrome")
    os.path.exists(exe) or sys.exit(f"refusing: {exe} missing")
    return exe

def mcp_bin():
    return os.environ.get("MANUK_MCP_BIN") or os.path.join(ROOT, "target/release/manuk-mcp")

def pw_cli():
    c = os.environ.get("CONC_PW_CLI") or next(iter(sorted(glob.glob(os.path.expanduser("~/.npm/_npx/*/node_modules/@playwright/mcp/cli.js")))), "")
    c and os.path.exists(c) or sys.exit("refusing: @playwright/mcp cli.js not found (set CONC_PW_CLI)")
    return c

def _oom_adj():
    try: open("/proc/self/oom_score_adj", "w").write("500")  # agents die before the harness does
    except OSError: pass


# ---- transports: one JSON-RPC line protocol, three carriers ----
class Rpc:
    def __init__(self): self.id = 0
    def call(self, method, params):
        self.id += 1; return self._send(method, params, self.id)
    def hello(self):
        self.call("initialize", {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "conc1", "version": "0"}})
        self._notify()
    def tool(self, name, args):
        r = self.call("tools/call", {"name": name, "arguments": args})
        if "error" in r: raise RuntimeError(f"{name}: {r['error']}")
        res = r.get("result", {})
        return "\n".join(b.get("text", "") for b in res.get("content", [])), bool(res.get("isError"))

class Stdio(Rpc):
    def __init__(self, cmd, env=None):
        super().__init__(); e = dict(os.environ); e.update(env or {})
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=e, preexec_fn=_oom_adj, text=True)
        self.lock = threading.Lock()
    def _w(self, o): self.p.stdin.write(json.dumps(o) + "\n"); self.p.stdin.flush()
    def _notify(self): self._w({"jsonrpc": "2.0", "method": "notifications/initialized"})
    def _send(self, m, params, i):
        with self.lock:
            self._w({"jsonrpc": "2.0", "id": i, "method": m, "params": params})
            while True:
                line = _rl(self.p)
                if not line: raise RuntimeError("server closed")
                v = json.loads(line)
                if v.get("id") == i: return v
    def close(self):
        try: self.p.kill(); self.p.wait(5)
        except Exception: pass

class Sock(Rpc):
    def __init__(self, path):
        super().__init__(); self.s = socket.socket(socket.AF_UNIX); self.s.settimeout(60); self.s.connect(path); self.buf = b""
    def _w(self, o): self.s.sendall((json.dumps(o) + "\n").encode())
    def _notify(self): self._w({"jsonrpc": "2.0", "method": "notifications/initialized"})
    def _line(self):
        while b"\n" not in self.buf:
            c = self.s.recv(65536)
            if not c: raise RuntimeError("server closed")
            self.buf += c
        l, _, self.buf = self.buf.partition(b"\n"); return l
    def _send(self, m, params, i):
        self._w({"jsonrpc": "2.0", "id": i, "method": m, "params": params})
        while True:
            v = json.loads(self._line())
            if v.get("id") == i: return v
    def close(self):
        try: self.s.close()
        except Exception: pass

class Http(Rpc):
    def __init__(self, port):
        super().__init__(); self.port = port; self.sid = None; self.c = http.client.HTTPConnection("127.0.0.1", port, timeout=60)
    def _post(self, o):
        h = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream"}
        if self.sid: h["mcp-session-id"] = self.sid
        self.c.request("POST", "/mcp", json.dumps(o), h); r = self.c.getresponse(); body = r.read().decode()
        self.sid = r.getheader("mcp-session-id") or self.sid
        return body
    def _notify(self): self._post({"jsonrpc": "2.0", "method": "notifications/initialized"})
    def _send(self, m, params, i):
        body = self._post({"jsonrpc": "2.0", "id": i, "method": m, "params": params})
        for chunk in re.findall(r"^data: (.*)$", body, re.M) or [body]:
            try: v = json.loads(chunk)
            except ValueError: continue
            if v.get("id") == i: return v
        raise RuntimeError(f"no reply to {m}: {body[:200]}")
    def close(self):
        try: self.c.request("DELETE", "/mcp", headers={"mcp-session-id": self.sid or ""}); self.c.getresponse().read()
        except Exception: pass
        self.c.close()


# ---- arm lifecycle: what is shared, what is per agent ----
class Arm:
    def __init__(self, name, n):
        self.name, self.n, self.shared_proc, self.tmp = name, n, None, tempfile.mkdtemp(prefix="conc1-")
        self.surface = "pw" if name.startswith("pw") else "manuk"
        self.server_pids = []
        if name == "engine-fork":
            self.sock = os.path.join(self.tmp, "z.sock"); t = time.time()
            self.zy = subprocess.Popen([mcp_bin(), "--zygote", self.sock, "--backend", "engine"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, preexec_fn=_oom_adj)
            while not os.path.exists(self.sock):
                if self.zy.poll() is not None: raise RuntimeError("zygote exited")
                time.sleep(0.02)
            self.zygote_start_ms = (time.time() - t) * 1000
        if name == "pw-shared":
            self.port = _free_port()
            self.zy = subprocess.Popen(["node", pw_cli(), "--headless", "--isolated", "--sandbox", "--executable-path", chrome_exe(), "--port", str(self.port), "--allowed-hosts", "*"],
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, preexec_fn=_oom_adj, cwd=self.tmp)
            for _ in range(200):
                try: socket.create_connection(("127.0.0.1", self.port), 0.2).close(); break
                except OSError: time.sleep(0.1)
            else: raise RuntimeError("pw-shared server did not listen")
        if MUTATE == "share-proc" and name == "engine-proc-full":  # mutant: isolation dropped, all agents use ONE process
            self.shared_proc = Stdio(self._cmd()); self.shared_proc.hello()

    def _cmd(self):
        b = mcp_bin()
        return {"engine-proc-full": [b, "--backend", "engine", "--agent-profile=full"],
                "engine-proc-lean": [b, "--backend", "engine", "--agent-profile=lean"],
                "chrome-mcp": [b, "--backend", "chrome"],
                "pw-proc": ["node", pw_cli(), "--headless", "--isolated", "--sandbox", "--executable-path", chrome_exe()]}[self.name]

    def connect(self):
        """-> initialized Rpc client for ONE agent."""
        if self.shared_proc: return self.shared_proc
        if self.name == "engine-fork": c = Sock(self.sock)
        elif self.name == "pw-shared": c = Http(self.port)
        else: c = Stdio(self._cmd(), {"MANUK_CHROME": chrome_exe()} if self.name == "chrome-mcp" else {})
        c.hello(); return c

    def stop(self):
        for p in (self.shared_proc, getattr(self, "zy", None)):
            if isinstance(p, Rpc): p.close()
            elif p: p.kill()


def _free_port():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p


# ---- one agent: runs traces through the surface ----
class Agent:
    def __init__(self, arm, idx, base, barrier, lat):
        self.arm, self.i, self.base, self.barrier, self.lat = arm, idx, base, barrier, lat
        self.t = TOOL[arm.surface]; self.cold_ms = None; self.cli = None

    def act(self, op, args):
        t0 = time.time()
        try: return self.cli.tool(self.t[op], args)
        finally: self.lat.append((time.time() - t0) * 1000)

    def find(self, snap, role, label):
        for l in snap.splitlines():
            if f'{role} "{label}"' in l:
                m = re.search(r"\[(h\d+)\]" if self.arm.surface == "manuk" else r"\[ref=(\w+)\]", l)
                if m: return m.group(1)
        return None

    def step(self, kind, target, arg):
        """-> True if this step's expectation was met."""
        if kind == "barrier":
            try: self.barrier.wait(600)
            except threading.BrokenBarrierError: return False
            return True
        if kind == "nav":
            out, err = self.act("nav", {"url": self.base + target.format(id=self.i)})
            return (not err) and self.poll(out, arg.format(id=self.i))
        snap, _ = self.act("snap", {})
        ref = self.find(snap, *target)
        if ref is None and self.arm.surface == "manuk":  # the observation is token-budgeted on big pages; manuk's own answer is find(query)
            ref = self.find(self.act("find", {"query": target[1]})[0], *target)
        if ref is None: return False
        if self.arm.surface == "manuk":
            out, err = self.act(kind if kind == "type" else "click", {"handle": ref, **({"text": arg} if kind == "type" else {"action": "click"})})
        else:
            a = {"element": target[1], "target": ref}
            if kind == "type": a["text"] = arg
            out, err = self.act(kind, a)
        return not err

    def poll(self, first, token, secs=6.0):
        if token in first: return True
        end = time.time() + secs
        while time.time() < end:
            time.sleep(0.25)
            out, err = self.act("snap", {})
            if token in out: return True
        return False

    def run_trace(self, steps):
        # a click's expectation is polled on the snapshots that follow it; nav carries its own token
        for kind, target, arg in steps:
            if not self.step(kind, target, arg): return False
            if kind == "click" and not self.poll("", arg): return False
        return True

    def main(self, traces, results, start_gate):
        try:
            t0 = time.time(); self.cli = self.arm.connect(); self.cold_ms = (time.time() - t0) * 1000
        except Exception as e:
            self.cold_ms = None
            for name in traces: results.append((self.i, name, False, f"start: {e}"[:120]))
            for b in (start_gate, self.barrier):  # a dead starter still takes its place, so it cannot hang or fake an isolation fail
                try: b.wait(600)
                except threading.BrokenBarrierError: pass
            return
        try:
            start_gate.wait(600)
            for name, steps in traces.items():
                try: ok = self.run_trace(steps); why = "" if ok else "expectation unmet"
                except Exception as e: ok, why = False, f"{type(e).__name__}: {e}"[:120]
                if MUTATE == "skip-failed" and not ok: continue  # mutant: failed traces vanish from the books
                results.append((self.i, name, ok, why))
        except threading.BrokenBarrierError: pass
        finally:
            if self.cli is not self.arm.shared_proc: self.cli.close()


# ---- measurement: tree PSS, cgroup memory/cpu ----
def children_map():
    m = {}
    for d in os.listdir("/proc"):
        if d.isdigit():
            try:
                st = open(f"/proc/{d}/stat").read(); pp = int(st.rsplit(") ", 1)[1].split()[1]); m.setdefault(pp, []).append(int(d))
            except (OSError, IndexError, ValueError): pass
    return m

def tree_pids(root=None):
    root = root or os.getpid(); cm = children_map(); out, stack = [], list(cm.get(root, []))
    while stack:
        p = stack.pop(); out.append(p); stack.extend(cm.get(p, []))
    return out

def pss_kb(pid):
    try:
        for l in open(f"/proc/{pid}/smaps_rollup"):
            if l.startswith("Pss:"): return int(l.split()[1])
    except OSError: pass
    return 0

def cgroup_dir():
    try:
        rel = open("/proc/self/cgroup").read().strip().split("::", 1)[1]; d = "/sys/fs/cgroup" + rel
        return d if os.path.exists(d + "/memory.events") else None
    except (OSError, IndexError): return None

def cg_read(d, f, key=None):
    try:
        t = open(f"{d}/{f}").read()
        if key is None: return int(t.strip()) if t.strip().isdigit() else t.strip()
        return next(int(l.split()[1]) for l in t.splitlines() if l.startswith(key + " "))
    except (OSError, StopIteration): return None

class Sampler(threading.Thread):
    def __init__(self, every): super().__init__(daemon=True); self.every, self.peak, self.stop_, self.nproc = every, 0, threading.Event(), 0
    def run(self):
        while not self.stop_.is_set():
            pids = tree_pids(); tot = sum(pss_kb(p) for p in pids)
            if tot > self.peak: self.peak, self.nproc = tot, len(pids)
            self.stop_.wait(self.every)


def pct(xs, q):
    if not xs: return None
    xs = sorted(xs); return xs[min(len(xs) - 1, int(round(q * (len(xs) - 1))))]

def commit():
    return subprocess.run(["git", "-C", ROOT, "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()


def run_cell(armname, n, run, traces=None, with_iso=True):
    """One cell in the current process (which `run` put inside the cage). -> dict row."""
    srv = fixtures.serve(); threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{srv.server_address[1]}"
    allt = dict(fixtures.ISO) if with_iso else {}  # iso first: its barrier is early, so a late failure elsewhere cannot hang it
    allt.update(fixtures.TRACES if traces is None else traces)
    cg = cgroup_dir(); cpu0 = cg_read(cg, "cpu.stat", "usage_usec") if cg else None; oom0 = cg_read(cg, "memory.events", "oom_kill") if cg else None
    arm = Arm(armname, n); samp = Sampler(0.5 if n < 25 else 1.0); samp.start()
    barrier = threading.Barrier(n) if with_iso else threading.Barrier(1)
    mark = {}; gate = threading.Barrier(n, action=lambda: mark.setdefault("t", time.time())); lat, results, agents = [], [], []
    ths = []
    for i in range(n):
        a = Agent(arm, i, base, barrier, lat); agents.append(a)
        t = threading.Thread(target=a.main, args=(allt, results, gate), daemon=True); t.start(); ths.append(t)
    for t in ths: t.join(900)
    wall = time.time() - mark.get("t", time.time())  # clock starts when every agent is connected: throughput excludes cold start
    time.sleep(0.6); samp.stop_.set(); samp.join(3)
    zyg = getattr(arm, "zygote_start_ms", None); arm.stop()
    cpu1 = cg_read(cg, "cpu.stat", "usage_usec") if cg else None; oom1 = cg_read(cg, "memory.events", "oom_kill") if cg else None
    per = {}
    for i, name, ok, why in results: per.setdefault(name, []).append(ok)
    expected = n * len(allt)
    npass = sum(1 for r in results if r[2])
    iso_ok = all(r[2] for r in results if r[1] == "iso") and sum(1 for r in results if r[1] == "iso") == n if with_iso else None
    cold = [a.cold_ms for a in agents if a.cold_ms is not None]
    row = dict(chrome_ver=pin(), commit=commit(), arm=armname, n=n, run=run, host_load1=os.getloadavg()[0], traces_per_agent=len(allt),
               traces_expected=expected, traces_recorded=len(results), traces_pass=npass, success=round(npass / expected, 4) if expected else 0,
               wall_s=round(wall, 2), ok_per_min=round(npass / wall * 60, 2) if wall else 0,
               p50_ms=pct(lat, .5) and round(pct(lat, .5), 1), p95_ms=pct(lat, .95) and round(pct(lat, .95), 1), actions=len(lat),
               cold_p50_ms=round(pct(cold, .5), 1) if cold else None, zygote_start_ms=round(zyg, 1) if zyg else None,
               peak_pss_mb=round(samp.peak / 1024, 1), peak_procs=samp.nproc, pss_per_ok_mb=round(samp.peak / 1024 / npass, 2) if npass else None,
               cpu_s=round(((cpu1 - cpu0) / 1e6) - time.process_time(), 2) if cpu0 is not None and cpu1 is not None else None,
               oom_kill=(oom1 - oom0) if oom0 is not None and oom1 is not None else None, caged=bool(cg) and cg_read(cg, "memory.max") == 8 << 30 and cg_read(cg, "memory.swap.max") == 0, isolation_ok=iso_ok,
               per_trace={k: f"{sum(v)}/{len(v)}" for k, v in sorted(per.items())},
               fail_reasons=sorted({r[3] for r in results if not r[2]})[:4])
    srv.shutdown(); return row


def mem_available_gb():
    for l in open("/proc/meminfo"):
        if l.startswith("MemAvailable:"): return int(l.split()[1]) / 1048576

COLS = ["chrome_ver", "commit", "arm", "n", "run", "host_load1", "traces_expected", "traces_recorded", "traces_pass", "success", "wall_s", "ok_per_min",
        "p50_ms", "p95_ms", "cold_p50_ms", "zygote_start_ms", "peak_pss_mb", "peak_procs", "pss_per_ok_mb", "cpu_s", "oom_kill", "caged", "isolation_ok", "per_trace", "fail_reasons"]

def tsv(row): return "\t".join(json.dumps(row[c]) if isinstance(row[c], (dict, list)) else str(row[c]) for c in COLS)

def caged_cell(arm, n, run, traces_json=None):
    """Run one cell in its own 8 GB cgroup; -> row dict. A cell the harness did not survive is an INFRA row, never silent."""
    while mem_available_gb() < MIN_AVAIL_GB:
        print(f"waiting: MemAvailable {mem_available_gb():.1f}G < {MIN_AVAIL_GB}G", file=sys.stderr, flush=True); time.sleep(30)
    cmd = CAGE + [sys.executable, "-I", os.path.abspath(__file__), "cell", "--arm", arm, "--n", str(n), "--run", str(run)]
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=1500)
    for l in reversed(p.stdout.splitlines()):
        if l.startswith("ROW "): return json.loads(l[4:])
    return dict(chrome_ver=pin(), commit=commit(), arm=arm, n=n, run=run, infra="cell died: " + (p.stderr or p.stdout)[-300:])

def cmd_run(a):
    arms = a.arms.split(","); ns = [int(x) for x in a.ns.split(",")]
    cells = [(arm, n) for n in ns for arm in arms]
    new = not os.path.exists(a.out)
    with open(a.out, "a") as f:
        if new: f.write("\t".join(COLS) + "\n")
        for run in range(1, a.runs + 1):
            order = cells[:]; random.Random(f"{a.seed}-{run}").shuffle(order)  # arms in randomised blocks
            for arm, n in order:
                r = caged_cell(arm, n, run)
                if "infra" in r: print("INFRA", r, file=sys.stderr, flush=True); open(a.out + ".infra", "a").write(json.dumps(r) + "\n"); continue
                f.write(tsv(r) + "\n"); f.flush(); print(f"{arm} N={n} run={run} success={r['success']} pss={r['peak_pss_mb']}MB p95={r['p95_ms']}ms iso={r['isolation_ok']}", flush=True)

def capacity_search(arm, cell_success, ns=NS, tol=0.05):
    """Largest N whose success is within `tol` of this arm's N=1 (monotone assumption: stop at the first miss, bisect the gap). cell_success(arm,n)->mean success."""
    base = cell_success(arm, 1); last_ok, first_bad = 1, None
    for n in ns[1:]:
        if cell_success(arm, n) >= base - tol: last_ok = n
        else: first_bad = n; break
    if first_bad is None: return last_ok, base
    lo, hi = last_ok, first_bad
    while hi - lo > 1 and (hi - lo) > max(1, lo // 10):
        mid = (lo + hi) // 2
        if cell_success(arm, mid) >= base - tol: lo = mid
        else: hi = mid
    return lo, base

def cmd_capacity(a):
    cache = {}
    def cs(arm, n):
        if n not in cache:
            rows = [caged_cell(arm, n, r) for r in range(1, a.runs + 1)]
            ok = [r["success"] for r in rows if "infra" not in r]; cache[n] = statistics.mean(ok) if ok else 0.0
            open(a.out, "a").write(json.dumps({"arm": arm, "n": n, "success": cache[n], "rows": len(ok), "chrome_ver": pin()}) + "\n")
        return cache[n]
    print(capacity_search(a.arm, cs))

def cmd_cell(a):
    r = run_cell(a.arm, a.n, a.run); print("ROW " + json.dumps(r), flush=True)
    os._exit(0)  # daemon agent threads may still hold sockets

def cmd_selftest(a):
    """Green only if: capacity logic is right, a deliberately broken trace is recorded FAILED, isolation holds, and a feasibility table exists."""
    bad = []
    curve = {1: .9, 5: .9, 10: .88, 25: .86, 50: .5, 100: .1}
    cap, _ = capacity_search("x", lambda arm, n: curve.get(n, .86 if n < 50 else .5))
    if cap < 25 or cap >= 50: bad.append(f"capacity logic: got {cap}, want in [25,50)")
    arm, n = a.arm, a.n
    tr = dict(fixtures.TRACES); tr = {k: tr[k] for k in ("form", "walk")}; tr.update(fixtures.BROKEN)
    r = run_cell(arm, n, 0, traces=tr, with_iso=True)
    per = r["per_trace"]
    if per.get("broken") != f"0/{n}": bad.append(f"broken trace not recorded as failed on all {n} agents: {per.get('broken')}")
    if r["traces_recorded"] != r["traces_expected"]: bad.append(f"traces dropped from the books: {r['traces_recorded']} of {r['traces_expected']}")
    if r["isolation_ok"] is not True: bad.append(f"isolation check failed: iso={per.get('iso')}")
    if per.get("form") != f"{n}/{n}": bad.append(f"form trace not feasible here: {per.get('form')}")
    print(json.dumps(r)); print("SELFTEST " + ("RED: " + "; ".join(bad) if bad else "GREEN")); sys.exit(1 if bad else 0)

def main():
    ap = argparse.ArgumentParser(); sub = ap.add_subparsers(dest="c", required=True)
    s = sub.add_parser("cell"); s.add_argument("--arm", required=True); s.add_argument("--n", type=int, required=True); s.add_argument("--run", type=int, default=1)
    s = sub.add_parser("run"); s.add_argument("--arms", default=",".join(ARMS)); s.add_argument("--ns", default="1,5"); s.add_argument("--runs", type=int, default=3)
    s.add_argument("--out", required=True); s.add_argument("--seed", default="conc1")
    s = sub.add_parser("capacity"); s.add_argument("--arm", required=True); s.add_argument("--runs", type=int, default=3); s.add_argument("--out", required=True)
    s = sub.add_parser("selftest"); s.add_argument("--arm", default="engine-proc-full"); s.add_argument("--n", type=int, default=3)
    a = ap.parse_args(); pin()
    {"cell": cmd_cell, "run": cmd_run, "capacity": cmd_capacity, "selftest": cmd_selftest}[a.c](a)

if __name__ == "__main__": main()
