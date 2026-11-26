#!/usr/bin/env python3
"""CPU.4.b probe: probe.py <arm: mcp|dump> [ENV=VAL ...]  -> per-thread-name cpu-s (sampled schedstat, exited threads keep last value) + IMGLOG lines + total wait4."""
import subprocess,sys,os,json,threading,time,http.server,functools,socketserver,collections
arm=sys.argv[1];env=dict(os.environ);[env.__setitem__(*a.split("=",1)) for a in sys.argv[2:]]
here=os.path.dirname(os.path.abspath(__file__));root=os.path.join(here,"../../../demo/www/pages")
class Q(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*a):pass
srv=socketserver.ThreadingTCPServer(("127.0.0.1",0),functools.partial(Q,directory=root));port=srv.server_address[1]
threading.Thread(target=srv.serve_forever,daemon=True).start()
B=os.environ.get("MANUK_BIN_DIR",os.path.join(here,"../../../target/release/"))
env["MANUK_IMG_LOG"]="1"
cmd=[B+"/manuk-mcp","--deterministic","--backend","engine"] if arm=="mcp" else [B+"a11y-dump",f"http://127.0.0.1:{port}/w3c.html"]
errf=open("/tmp/cpu4b.err","w")
p=subprocess.Popen(cmd,stdin=subprocess.PIPE if arm=="mcp" else None,stdout=subprocess.PIPE,stderr=errf,text=True,env=env)
SEEN={};COMM={};stop=False
def samp():
    while not stop:
        try:
            for t in os.listdir(f"/proc/{p.pid}/task"):
                try:
                    SEEN[t]=int(open(f"/proc/{p.pid}/task/{t}/schedstat").read().split()[0])
                    COMM.setdefault(t,open(f"/proc/{p.pid}/task/{t}/comm").read().strip())
                except Exception:pass
        except Exception:pass
        time.sleep(0.002)
th=threading.Thread(target=samp);th.start()
if arm=="mcp":
    n=[0]
    def rpc(m,pr,notify=False):
        d={"jsonrpc":"2.0","method":m}
        if pr is not None:d["params"]=pr
        if not notify:n[0]+=1;d["id"]=n[0]
        p.stdin.write(json.dumps(d)+"\n");p.stdin.flush()
        if notify:return
        while True:
            r=json.loads(p.stdout.readline())
            if r.get("id")==n[0]:return r
    rpc("initialize",{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"x","version":"0"}})
    rpc("notifications/initialized",None,True)
    rpc("tools/call",{"name":"navigate","arguments":{"url":f"http://127.0.0.1:{port}/w3c.html"}})
    r=rpc("tools/call",{"name":"observe","arguments":{}})
    open("/tmp/cpu4b.obs","w").write(json.dumps(r))
    p.stdin.close()
else:
    open("/tmp/cpu4b.obs","w").write(p.stdout.read())
_,st,ru=os.wait4(p.pid,0);p.returncode=0;stop=True;th.join()
by=collections.defaultdict(float)
for t,v in SEEN.items():by[COMM[t]]+=v/1e9
print("total_wait4",round(ru.ru_utime+ru.ru_stime,3))
for k,v in sorted(by.items(),key=lambda x:-x[1]):print(f"  {k}\t{v:.3f}")
