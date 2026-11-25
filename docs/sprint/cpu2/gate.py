#!/usr/bin/env python3
"""CPU.2 gate. usage: gate.py <manuk-mcp> [--mutant: key never moves -> must exit 1]. Serves ./fx, runs navigate/observe/find/act(click)/type per fixture
in a fresh `manuk-mcp --deterministic --backend engine` with MANUK_BUILD_TRACE=1 (a stderr line per tool call:
trees/cascades/layouts), for the page cache ON and OFF (MANUK_NO_A11Y_CACHE=1), and checks:
  act (click changes one text node) <= 2 tree builds on every fixture; find after observe, no DOM change <= 1;
  t1500 act cpu-s (cache on) <= 0.45 (CPU.1 left it at 0.91); every tool output identical cache on vs off.
Exit 1 on any failure. cpu = utime+stime of the process tree."""
import json,subprocess,sys,os,re,threading,http.server,functools,psutil
binp=sys.argv[1]; here=os.path.dirname(os.path.abspath(__file__))
H=functools.partial(http.server.SimpleHTTPRequestHandler,directory=here+"/fx")
H.log_message=lambda *a:None
srv=http.server.ThreadingHTTPServer(("127.0.0.1",0),H); port=srv.server_address[1]
threading.Thread(target=srv.serve_forever,daemon=True).start()
def run(page,env_extra):
    env=dict(os.environ,MANUK_BUILD_TRACE="1",**env_extra)
    p=subprocess.Popen([binp,"--deterministic","--backend","engine"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=env)
    lines=[]
    threading.Thread(target=lambda:[lines.append(l) for l in p.stderr],daemon=True).start()
    n=[0]
    def cpu():
        pr=psutil.Process(p.pid);t=0.0
        for q in [pr]+pr.children(recursive=True):
            try:c=q.cpu_times();t+=c.user+c.system
            except psutil.Error:pass
        return t
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
        mark=len(lines);c=cpu()
        r=rpc("tools/call",{"name":name,"arguments":args})
        txt="\n".join(b.get("text","") for b in r["result"]["content"])
        import time;time.sleep(0.2)
        b=[l for l in lines[mark:] if l.startswith("BUILDS "+name+" ")]
        m=re.search(r"trees=(\d+) cascades=(\d+) layouts=(\d+)",b[-1]) if b else None
        out[k]=dict(trees=int(m[1]),cascades=int(m[2]),layouts=int(m[3]),cpu=round(cpu()-c,2),text=txt)
        return txt
    rpc("initialize",{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"x","version":"0"}})
    rpc("notifications/initialized",None,True)
    call("navigate","navigate",{"url":f"http://127.0.0.1:{port}/{page}"})
    call("observe","observe",{})
    fd=call("find","find",{"query":"Pick 7"})
    m=re.search(r"\[(h\d+)\] button",fd)
    call("act","act",{"action":"click","handle":m.group(1)})
    call("observe2","observe",{})
    if page=="small.html":
        t=call("find2","find",{"query":"Name"}); mm=re.search(r"\[(h\d+)\] (textbox|input|searchbox)",t) or re.search(r"\[(h\d+)\]",t)
        call("type","type",{"handle":mm.group(1),"text":"abc"})
    p.kill(); return out
bad=0; res={}
pages=["small.html","div1500.html","t1500.html"]
for pg in pages:
    for mode,env in (("on",{"MANUK_A11Y_CACHE_MUTATE":"stale"} if "--mutant" in sys.argv else {}),("off",{"MANUK_NO_A11Y_CACHE":"1"})):
        runs=[run(pg,env) for _ in range(3)]   # host load is noisy: counts are exact, cpu is the min of 3 runs
        best=min(runs,key=lambda o:o["act"]["cpu"]); res[(pg,mode)]=best
print(f"{'page':12} {'cache':5} {'call':8} trees cascades layouts   cpu-s")
for (pg,mode),o in res.items():
    for k,v in o.items(): print(f"{pg:12} {mode:5} {k:8} {v['trees']:5} {v['cascades']:8} {v['layouts']:7} {v['cpu']:8}")
def chk(c,msg):
    global bad
    print(("ok   " if c else "RED  ")+msg); bad+=not c
for pg in pages:
    on=res[(pg,"on")]; off=res[(pg,"off")]
    chk(on["act"]["trees"]<=2,f"{pg}: act builds {on['act']['trees']} trees (<=2; cache off {off['act']['trees']})")
    chk(on["find"]["trees"]<=1,f"{pg}: find after observe builds {on['find']['trees']} trees (<=1; cache off {off['find']['trees']})")
    same=all(on[k]["text"]==off[k]["text"] for k in on)
    chk(same,f"{pg}: every tool output byte-identical cache on vs off")
    chk("PICKED" in on["observe2"]["text"] or "picked" in on["observe2"]["text"] or pg=="small.html" and "picked" in on["observe2"]["text"],f"{pg}: observe after the click shows the changed text (fresh tree)")
a,b=res[("t1500.html","on")]["act"]["cpu"],res[("t1500.html","off")]["act"]["cpu"]
chk(a<=0.45,f"t1500 act cpu {a} <= 0.45 (CPU.1 baseline 0.91)")
print(f"(t1500 act cpu: cache off {b}, on {a}; the 2x bar is against CPU.1's recorded 0.91, same host class)")
json.dump({f"{k[0]}|{k[1]}":{c:{x:y for x,y in v.items() if x!='text'} for c,v in o.items()} for k,o in res.items()},open(here+"/gate.json","w"),indent=1)
sys.exit(1 if bad else 0)
