#!/usr/bin/env python3
"""CPU.2.a gate. usage: gate.py <manuk-mcp> [--mutant]. Reuses the CPU.2 fixtures (../cpu2/fx). Per fixture, in a fresh
`manuk-mcp --deterministic --backend engine` with MANUK_BUILD_TRACE=1: navigate, observe, observe, find, act(click), observe.
Bars: observe on an unchanged DOM (first and second) costs 0 cascades and 0 layouts; the a11y text of every call is
byte-identical to the run with MANUK_EVAL_DIRT_MUTATE=1 (the old dirt-leaving behaviour); the click still changes the text.
--mutant sets MANUK_EVAL_DIRT_MUTATE=1 for the 'fixed' side too -> must exit 1 (observe re-cascades)."""
import json,subprocess,sys,os,re,threading,http.server,functools,time
binp=sys.argv[1]; here=os.path.dirname(os.path.abspath(__file__))
H=functools.partial(http.server.SimpleHTTPRequestHandler,directory=here+"/../cpu2/fx"); H.log_message=lambda *a:None
srv=http.server.ThreadingHTTPServer(("127.0.0.1",0),H); port=srv.server_address[1]
threading.Thread(target=srv.serve_forever,daemon=True).start()
def run(page,extra):
    env=dict(os.environ,MANUK_BUILD_TRACE="1",**extra)
    p=subprocess.Popen([binp,"--deterministic","--backend","engine"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=env)
    lines=[]; threading.Thread(target=lambda:[lines.append(l) for l in p.stderr],daemon=True).start()
    n=[0]
    def rpc(m,pa,notify=False):
        d={"jsonrpc":"2.0","method":m}
        if pa is not None:d["params"]=pa
        if not notify:n[0]+=1;d["id"]=n[0]
        p.stdin.write(json.dumps(d)+"\n");p.stdin.flush()
        if notify:return
        while True:
            r=json.loads(p.stdout.readline())
            if r.get("id")==n[0]:return r
    out={}
    def call(k,name,args):
        mark=len(lines); r=rpc("tools/call",{"name":name,"arguments":args})
        txt="\n".join(b.get("text","") for b in r["result"]["content"]); time.sleep(0.3)
        b=[l for l in lines[mark:] if l.startswith("BUILDS "+name+" ")]
        m=re.search(r"cascades=(\d+) layouts=(\d+)",b[-1])
        out[k]=dict(cascades=int(m[1]),layouts=int(m[2]),text=txt); return txt
    rpc("initialize",{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"x","version":"0"}})
    rpc("notifications/initialized",None,True)
    call("navigate","navigate",{"url":f"http://127.0.0.1:{port}/{page}"})
    call("observe","observe",{}); call("observe_b","observe",{})
    fd=call("find","find",{"query":"Pick 7"}); m=re.search(r"\[(h\d+)\] button",fd)
    call("act","act",{"action":"click","handle":m.group(1)}); call("observe_c","observe",{})
    p.kill(); return out
bad=0
def chk(c,msg):
    global bad; print(("ok   " if c else "RED  ")+msg); bad+=not c
mut={"MANUK_EVAL_DIRT_MUTATE":"1"}
for pg in ["small.html","div1500.html","t1500.html"]:
    new=run(pg,mut if "--mutant" in sys.argv else {}); old=run(pg,mut)
    for k in ("observe","observe_b"):
        chk(new[k]["cascades"]==0 and new[k]["layouts"]==0,f"{pg}: {k} on unchanged DOM = {new[k]['cascades']} cascades {new[k]['layouts']} layouts (want 0/0; old behaviour {old[k]['cascades']}/{old[k]['layouts']})")
    chk(all(new[k]["text"]==old[k]["text"] for k in new),f"{pg}: every tool output byte-identical to the dirt-leaving build")
    chk("picked" in new["observe_c"]["text"].lower(),f"{pg}: observe after the click shows the changed text")
sys.exit(1 if bad else 0)
