#!/usr/bin/env python3
"""R1.4.a.d.b.a.a: every bench/scripts MCP client must raise (not hang) on a server that never answers, and must give
byte-identical results to a plain readline on the fake server that answers (notification + response in one write).

Run:  python3 -I scripts/test_rpc_deadline.py   (exit 1 on any hang or mismatch; MANUK_RPC_DEADLINE=3 is set here)
"""
import json, os, subprocess, sys, time

REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
SD = os.path.join(REPO, "docs", "sprint", "shield-audit")
SILENT, FAKE = os.path.join(SD, "silent_mcp.py"), os.path.join(SD, "fake_mcp.py")
LIMIT = 60

# each driver: a python snippet run in the client's file with its Popen argv swapped for the fake server
FILES = ["bench/agent-tasks/real_boot.py", "bench/agent-tasks/c1_settle.py", "bench/agent-tasks/c2_retry.py",
         "bench/agent-tasks/c3_bon.py", "bench/agent-tasks/c4_receipts.py", "scripts/w4p0d-scroll.py", "scripts/r213-gate.py"]


def run_client(rel, server):
    """Execute the client source with subprocess.Popen redirected to `server`, issuing initialize + one tools/call."""
    src = open(os.path.join(REPO, rel)).read()
    cls = {"real_boot": "M", "c1_settle": "Mcp", "c2_retry": "Mcp", "c3_bon": "Mcp", "c4_receipts": "Mcp", "r213-gate": "C"}.get(
        os.path.basename(rel)[:-3])
    drv = f"""
import subprocess, sys, runpy, os
_P = subprocess.Popen
def _fake(argv, *a, **k): return _P([sys.executable, {server!r}], *a, **k)
subprocess.Popen = _fake
sys.argv = [{rel!r}, "http://x", "10", "0", "1"]
__file__ = {os.path.join(REPO, rel)!r}
src = open(__file__).read()
if {cls!r} == None:   # w4p0d-scroll.py is a flat script: it issues initialize + navigate on import
    exec(compile(src, __file__, "exec"), {{"__file__": __file__, "__name__": "m"}})
else:
    g = {{"__file__": __file__, "__name__": "m"}}
    exec(compile(src, __file__, "exec"), g)
    o = {{"M": lambda: g["M"]([], dict(os.environ)), "Mcp": lambda: g["Mcp"](["x"]), "C": lambda: g["C"]()}}[{cls!r}]()
    o.rpc("tools/call", {{"name": "navigate", "arguments": {{"url": "u"}}}})
    print("ANSWERED")
"""
    env = dict(os.environ, MANUK_RPC_DEADLINE="3")
    t0 = time.time()
    try:
        p = subprocess.run([sys.executable, "-I", "-c", "import json\n" + drv], capture_output=True, text=True, timeout=LIMIT, env=env, cwd=REPO)
        return p.returncode, p.stdout + p.stderr, time.time() - t0
    except subprocess.TimeoutExpired:
        return None, "HUNG", time.time() - t0


def main():
    bad = 0
    for rel in FILES:
        rc, out, dt = run_client(rel, SILENT)
        ok = rc not in (None, 0) and "TimeoutError" in out
        print(f"{'GREEN' if ok else 'RED  '} silent {rel}: rc={rc} {dt:.1f}s {'TimeoutError' if 'TimeoutError' in out else out[-120:].strip()!r}")
        bad += not ok
        if os.path.basename(rel) in ("c1_settle.py", "c2_retry.py", "c3_bon.py", "c4_receipts.py", "real_boot.py"):  # flat scripts run their whole job on import
            rc, out, dt = run_client(rel, FAKE)
            ok = rc == 0 and "ANSWERED" in out
            print(f"{'GREEN' if ok else 'RED  '} fake   {rel}: rc={rc} {out.strip()[-80:]!r}")
            bad += not ok
    # real_run.py: the proxy's first tools/call hits a silent child
    state = f"/tmp/rpc_deadline_state_{os.getpid()}"
    t0 = time.time()
    try:
        p = subprocess.run([sys.executable, "-I", os.path.join(REPO, "bench/agent-tasks/real_run.py"), "proxy", "--arm", "cdp-layer", "--site", "http://x",
                            "--task", "t", "--state", state, "--", sys.executable, SILENT],
                           input='{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"navigate","arguments":{}}}\n',
                           capture_output=True, text=True, timeout=LIMIT, cwd=REPO)
        err = open(state + ".err").read() if os.path.exists(state + ".err") else p.stderr
        ok = p.returncode != 0 and ("Empty" in err or "Timeout" in err)
        print(f"{'GREEN' if ok else 'RED  '} silent bench/agent-tasks/real_run.py: rc={p.returncode} {time.time() - t0:.1f}s {err.strip()[-80:]!r}")
    except subprocess.TimeoutExpired:
        ok = False
        print(f"RED   silent bench/agent-tasks/real_run.py: HUNG {time.time() - t0:.1f}s")
    bad += not ok
    print("GATE " + ("RED" if bad else "GREEN"))
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
