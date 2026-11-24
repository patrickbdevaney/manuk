#!/usr/bin/env python3
"""DASH.MEM.1: RSS-over-time of ONE manuk-mcp --backend engine driving a dashdish task's setup+early calls.
usage: dash-mem-trace.py BIN OUT.tsv TASK [idle_s]   (run under systemd-run --user --scope -p MemoryMax=6G -p MemorySwapMax=0)
Phases: setup(config,finish) -> start navigate -> observe interactive -> observe viewport -> idle. Samples every 1s."""
import json, subprocess, sys, threading, time, os, queue
bin_, out, task = sys.argv[1:4]; idle = int(sys.argv[4]) if len(sys.argv) > 4 else 60
site = "https://evals-dashdish.vercel.app"
p = subprocess.Popen([bin_, "--backend", "engine"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
t0 = time.time(); phase = ["spawn"]; stop = [False]
f = open(out, "w"); f.write("t_s\tphase\trss_kb\tanon_kb\n")
def rss():
    try:
        d = dict(l.split(":") for l in open(f"/proc/{p.pid}/status") if l.startswith(("VmRSS", "RssAnon")))
        return int(d["VmRSS"].split()[0]), int(d["RssAnon"].split()[0])
    except Exception: return 0, 0
def samp():
    while not stop[0]:
        r, a = rss(); f.write(f"{time.time()-t0:.1f}\t{phase[0]}\t{r}\t{a}\n"); f.flush(); time.sleep(1)
threading.Thread(target=samp, daemon=True).start()
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l) for l in p.stdout], daemon=True).start()
meta = {"io.modelcontextprotocol/protocolVersion": "2025-06-18", "io.modelcontextprotocol/clientInfo": {"name": "claude-code", "version": "0"}, "io.modelcontextprotocol/clientCapabilities": {}}
n = [0]
def send(m, params, wait=True, to=120):
    n[0] += 1; i = n[0]
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": m, "params": params}) + "\n"); p.stdin.flush()
    while wait:
        try: l = q.get(timeout=to)
        except queue.Empty: return None
        try: j = json.loads(l)
        except ValueError: continue
        if j.get("id") == i: return j
def call(name, args, ph):
    phase[0] = ph; s = time.time()
    r = send("tools/call", {"name": name, "arguments": args, "_meta": dict(meta)})
    print(f"{ph} {name} {time.time()-s:.1f}s rss={rss()[0]//1024}M", file=sys.stderr)
    return r
send("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "claude-code", "version": "0"}})
p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); p.stdin.flush()
call("navigate", {"url": f"{site}/config?run_id=0&task_id={task}&latency=0"}, "setup-config")
call("navigate", {"url": f"{site}/finish"}, "setup-finish")
call("navigate", {"url": site}, "start")
call("observe", {"mode": "interactive"}, "observe-interactive")
call("observe", {"mode": "viewport"}, "observe-viewport")
if os.environ.get("DM_EVAL"):
    r = call(os.environ.get("DM_TOOL","evaluate"), json.loads(os.environ["DM_EVAL"]), "eval"); print("EVAL", json.dumps(r)[:int(os.environ.get("DM_N","1500"))], file=sys.stderr)
phase[0] = "idle"; time.sleep(idle)
stop[0] = True; p.terminate(); f.close()
