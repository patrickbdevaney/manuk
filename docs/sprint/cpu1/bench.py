#!/usr/bin/env python3
"""CPU.1 bench: usage bench.py <manuk-mcp> <url> [--dump] [extra mcp args e.g. --backend engine]; cpu = whole process tree. One fresh process; prints cpu-s (utime+stime
from /proc/PID/stat) for navigate, observe, find, click. --dump prints observe text for byte-compare."""
import json,subprocess,sys,os,re
import psutil
binp,url=sys.argv[1],sys.argv[2]
extra=[a for a in sys.argv[3:] if a!="--dump"]
p=subprocess.Popen([binp,"--deterministic"]+extra,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
n=0
tck=os.sysconf("SC_CLK_TCK")
def cpu():
    tot=0.0
    pr=psutil.Process(p.pid)
    for q in [pr]+pr.children(recursive=True):
        try: c=q.cpu_times(); tot+=c.user+c.system
        except psutil.Error: pass
    return tot
def rpc(m,pr,notify=False):
    global n
    d={"jsonrpc":"2.0","method":m}
    if pr is not None:d["params"]=pr
    if not notify:n+=1;d["id"]=n
    p.stdin.write(json.dumps(d)+"\n");p.stdin.flush()
    if notify:return
    while True:
        r=json.loads(p.stdout.readline())
        if r.get("id")==n:return r
def call(name,args):
    r=rpc("tools/call",{"name":name,"arguments":args})
    return "\n".join(b.get("text","") for b in r["result"]["content"])
rpc("initialize",{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"x","version":"0"}})
rpc("notifications/initialized",None,True)
res={}
def t(k,f):
    c=cpu();o=f();res[k]=round(cpu()-c,2);return o
t("nav",lambda:call("navigate",{"url":url}))
ob=t("observe",lambda:call("observe",{}))
fd=t("find",lambda:call("find",{"query":"Pick 7"}))
mm=re.search(r'\[(h\d+)\] button',fd)
h=mm.group(1) if mm else None
if h: t("click",lambda:call("act",{"action":"click","handle":h}))
else: res["click"]=None
print(json.dumps(res))
if "--dump" in sys.argv: print(ob); print("FIND:",fd)
p.kill()
