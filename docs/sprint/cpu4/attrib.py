#!/usr/bin/env python3
"""CPU.4: attribute CPU of a11y-dump (navigate + a11y tree + serialise) to phases, no engine change.
attrib.py <outfile.tsv> <rounds> <page,page,...> [dump|mcp]   (mcp = manuk-mcp navigate+observe over JSON-RPC, trace file inside the confine-writable cache dir)
A sampler reads utime+stime of the process (all threads) every ~2 ms from /proc/<pid>/stat; the MANUK_NET_TRACE lines
(t_us since first event, `epoch` line = unix us at t_us 0) are mapped onto that CPU curve. Phase CPU = cpu(end)-cpu(start).
Nested spans (cascade/layout inside 'cascade+layout+blocking scripts') are subtracted from the enclosing phase."""
import subprocess,sys,os,time,threading,tempfile,http.server,functools,socketserver
out,rounds,pages=sys.argv[1],int(sys.argv[2]),sys.argv[3].split(",")
ARM=sys.argv[4] if len(sys.argv)>4 else "dump"
MCP=os.path.join(os.path.dirname(os.path.abspath(__file__)),"../../../target/release/manuk-mcp")
CACHE="/tmp/cpu4-cache";os.makedirs(CACHE,exist_ok=True)
DRV={}
HZ=os.sysconf("SC_CLK_TCK")
root=os.path.join(os.path.dirname(os.path.abspath(__file__)),"../../../demo/www/pages")
class Q(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*a):pass
srv=socketserver.ThreadingTCPServer(("127.0.0.1",0),functools.partial(Q,directory=root));port=srv.server_address[1]
threading.Thread(target=srv.serve_forever,daemon=True).start()
BIN=os.path.join(os.path.dirname(os.path.abspath(__file__)),"../../../target/release/a11y-dump")
SEEN={};COMM={}
def cpu_of(pid):
    """sum of per-thread run time (ns, /proc/<pid>/task/*/schedstat); a thread that exits keeps its last value (SEEN)."""
    try:
        for tid in os.listdir(f"/proc/{pid}/task"):
            try:
                SEEN[tid]=int(open(f"/proc/{pid}/task/{tid}/schedstat").read().split()[0])
                if tid not in COMM:COMM[tid]=open(f"/proc/{pid}/task/{tid}/comm").read().strip()
            except Exception:pass
    except Exception:return None
    return sum(SEEN.values())/1e9
def interp(samples,t):
    # samples: list of (wall,cpu) ascending
    import bisect
    ts=[s[0] for s in samples];i=bisect.bisect_left(ts,t)
    if i<=0:return samples[0][1]
    if i>=len(samples):return samples[-1][1]
    (t0,c0),(t1,c1)=samples[i-1],samples[i]
    return c0+(c1-c0)*((t-t0)/(t1-t0) if t1>t0 else 0)
def drive_mcp(p,page):
    import json
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
    DRV["nav_start"]=time.time()
    rpc("tools/call",{"name":"navigate","arguments":{"url":f"http://127.0.0.1:{port}/{page}.html"}})
    DRV["nav_end"]=time.time()
    rpc("tools/call",{"name":"observe","arguments":{}})
    DRV["obs_end"]=time.time()
    p.stdin.close()
def run(page):
    d_=CACHE if ARM=="mcp" else tempfile.gettempdir()
    tf=tempfile.NamedTemporaryFile(delete=False,dir=d_);tf.close();os.chmod(tf.name,0o666)
    env=dict(os.environ,MANUK_NET_TRACE=tf.name,MANUK_CACHE_DIR=CACHE)
    SEEN.clear();COMM.clear();BYCOMM.clear();DRV.clear()
    if ARM=="mcp":
        p=subprocess.Popen([MCP,"--deterministic","--backend","engine"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,env=env)
        threading.Thread(target=drive_mcp,args=(p,page),daemon=True).start()
    else:
        p=subprocess.Popen([BIN,f"http://127.0.0.1:{port}/{page}.html"],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,env=env)
    samples=[(time.time(),0.0)]
    while True:
        pid,st,ru=os.wait4(p.pid,os.WNOHANG)
        if pid:break
        c=cpu_of(p.pid)
        if c is not None:
            samples.append((time.time(),c))
            d={}
            for tid,v in SEEN.items():d[COMM.get(tid,'?')]=d.get(COMM.get(tid,'?'),0)+v/1e9
            BYCOMM.append((time.time(),d))
        time.sleep(0.002)
    p.returncode=0
    total=ru.ru_utime+ru.ru_stime
    samples.append((time.time(),max(samples[-1][1],total)))
    return tf.name,samples,total
TOTAL=[0.0];BYCOMM=[]
def attribute(tracefile,samples):
    L=[l.rstrip("\n").split("\t") for l in open(tracefile)]
    epoch=None;ev=[]
    for l in L:
        if l[1]=="epoch":epoch=int(l[2])/1e6
        else:ev.append((int(l[0])/1e6,l[1],l[2:]))
    cp=lambda t:interp(samples,t)
    C={}
    def add(k,v):C[k]=C.get(k,0.0)+v
    first=ev[0][0] if ev else 0
    add("startup (before first trace event, incl. JS prewarm)",cp(epoch+first)-0.0)
    nested=[]  # (start,end,kind,cpu)
    spans=[]
    marks={}
    for t,k,f in ev:
        if k in("cascade_pass","layout_pass"):
            d=int(f[0])/1e6;kk=k[:-5]
            c_=cp(epoch+t)-cp(epoch+t-d)
            nested.append((t-d,t,kk,c_))
            add(f"  pass[{kk}@page/lib.rs:{f[1]}] n",1);add(f"  pass[{kk}@page/lib.rs:{f[1]}] cpu",c_)
        elif k=="phase":
            d=int(f[1])/1e6
            spans.append((t-d,t,f[0],cp(epoch+t)-cp(epoch+t-d)))
        elif k=="mark":marks[f[0]]=t
        elif k=="dynscript_eval":
            d=int(f[0])/1e6;nested.append((t-d,t,"script",cp(epoch+t)-cp(epoch+t-d)))
    for (s,e,k,c) in nested:
        if k in("cascade","layout"):add(k,c)
    for (s,e,name,c) in spans:
        inner=sum(cc for (ss,ee,kk,cc) in nested if kk in("cascade","layout") and ss>=s-1e-6 and ee<=e+1e-6)
        rest=max(0.0,c-inner)
        if name=="html parse":add("parse (html)",rest)
        elif name in("external scripts","module graph prefetch","deferred scripts","DOMContentLoaded","load event","finish_loading: dynamic scripts"):add("script (JS exec + script fetch/compile)",rest)
        elif name=="cascade+layout+blocking scripts":add("script (JS exec + script fetch/compile)",rest)
        elif name=="blocking stylesheets" or name=="finish_loading: external CSS":add("stylesheet fetch+parse",rest)
        elif name in("subframes (pre-load)","finish_loading: subframes"):add("subframes",rest)
        elif "images" in name or "masks" in name or "background" in name or "page fetches" in name:add("images/masks/fetch (decode)",rest)
        else:add("other phase: "+name,rest)
    for (s_,e_,name,c) in spans:
        if c>0.05:
            a_=min(BYCOMM,key=lambda x:abs(x[0]-(epoch+s_)));b_=min(BYCOMM,key=lambda x:abs(x[0]-(epoch+e_)))
            for k2 in b_[1]:
                dv=b_[1][k2]-a_[1].get(k2,0)
                if dv>0.01:add(f"  thread[{name}] {k2}",dv)
    if ARM=="mcp":
        marks["navigate_returned"]=DRV["nav_end"]-epoch;marks["a11y_built"]=DRV["obs_end"]-epoch
    nav=marks.get("navigate_returned");a11y=marks.get("a11y_built")
    end=TOTAL[0]
    if nav is not None and a11y is not None:
        inner=sum(cc for (ss,ee,kk,cc) in nested if kk in("cascade","layout") and ss>=nav-1e-6 and ee<=a11y+1e-6)
        add("a11y tree (+ observe serialisation under mcp)",cp(epoch+a11y)-cp(epoch+nav)-inner)
        add("serialise+exit" if ARM!="mcp" else "mcp shutdown",end-cp(epoch+a11y))
        lastend=max([e for (s_,e,n_,c_) in spans if e<=nav]+[0])
        if ARM=="mcp":
            inner2=sum(cc for (ss,ee,kk,cc) in nested if kk in("cascade","layout") and ss>=lastend-1e-6 and ee<=nav+1e-6)
            add("navigate tail after last load phase (tree seed, snapshot, settle)",cp(epoch+nav)-cp(epoch+lastend)-inner2)
    return C
os.makedirs(os.path.dirname(os.path.abspath(out)),exist_ok=True)
with open(out,"a") as f:
    for rd in range(rounds):
        for pg in pages:
            tn,samples,tot=run(pg);TOTAL[0]=tot
            try:C=attribute(tn,samples)
            except Exception as e:C={"error "+str(e):0}
            (os.environ.get("KEEP_TRACE") and os.system(f"cp {tn} /tmp/keep-{pg}.trace")); os.unlink(tn)
            s=sum(v for k,v in C.items() if not k.startswith("  "))
            for k,v in C.items():f.write("\t".join([str(rd),pg,k,f"{v:.3f}"])+"\n")
            f.write("\t".join([str(rd),pg,"TOTAL_rusage",f"{tot:.3f}"])+"\n")
            f.write("\t".join([str(rd),pg,"SUM_attributed",f"{s:.3f}"])+"\n");f.flush()
