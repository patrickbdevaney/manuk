#!/usr/bin/env python3
"""CPU.4.a gate. gate.py [--mutant]. For mdn-css and rfc-editor, 3 interleaved rounds of before/after (before = the escape hatches
MANUK_NO_SKIP_CLEAN_RESTYLE=1 MANUK_NO_CASCADE_NOTE=1 that restore the old pass structure; after = default), real manuk-mcp navigate+observe
over JSON-RPC with MANUK_NET_TRACE. Reports cascade_pass/layout_pass counts and cpu-s (wait4 rusage). Fails unless:
 mdn-css cascade passes <= 4 and layout passes <= 4 (after, median), total cpu-s down >= 25% (median), observe output byte-identical
 before vs after on both pages. --mutant: 'after' also gets the escape hatches, so the gate must go red."""
import subprocess,sys,os,json,tempfile,threading,http.server,functools,socketserver,statistics as st
H=os.path.dirname(os.path.abspath(__file__));R=os.path.join(H,"../../..")
MCP=os.path.join(R,"target/release/manuk-mcp");root=os.path.join(R,"demo/www/pages")
CACHE="/tmp/cpu4a-cache";os.makedirs(CACHE,exist_ok=True)
class Q(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*a):pass
srv=socketserver.ThreadingTCPServer(("127.0.0.1",0),functools.partial(Q,directory=root));port=srv.server_address[1]
threading.Thread(target=srv.serve_forever,daemon=True).start()
OLD={"MANUK_NO_SKIP_CLEAN_RESTYLE":"1","MANUK_NO_CASCADE_NOTE":"1"}
def run(page,old):
    tf=tempfile.NamedTemporaryFile(delete=False,dir=CACHE);tf.close();os.chmod(tf.name,0o666)
    env=dict(os.environ,MANUK_NET_TRACE=tf.name,MANUK_CACHE_DIR=CACHE);env.update(OLD if old else {})
    p=subprocess.Popen([MCP,"--deterministic","--backend","engine"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,env=env)
    n=[0];out=[]
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
    rpc("tools/call",{"name":"navigate","arguments":{"url":f"http://127.0.0.1:{port}/{page}.html"}})
    o=rpc("tools/call",{"name":"observe","arguments":{}})
    p.stdin.close()
    _,_,ru=os.wait4(p.pid,0)
    cpu=ru.ru_utime+ru.ru_stime
    c=l=0
    for ln in open(tf.name):
        k=ln.split("\t")[1] if "\t" in ln else ""
        c+=k=="cascade_pass";l+=k=="layout_pass"
    os.unlink(tf.name)
    return c,l,cpu,json.dumps(o,sort_keys=True)
bad=[];rows=[]
for pg in("mdn-css","rfc-editor"):
    B=[];A=[];ob=set();oa=set()
    for r in range(3):
        for old in(True,False):
            c,l,cpu,o=run(pg,old or "--mutant" in sys.argv)
            (B if old else A).append((c,l,cpu));(ob if old else oa).add(o)
    m=lambda X,i:st.median(x[i] for x in X)
    rows.append((pg,m(B,0),m(B,1),m(B,2),m(A,0),m(A,1),m(A,2)))
    # observe output carries live timing-free text only; identical sets required
    if ob!=oa or len(ob)!=1:bad.append(f"{pg}: observe output differs before/after (before {len(ob)} variants, after {len(oa)})")
    if pg=="mdn-css":
        if m(A,0)>4 or m(A,1)>4:bad.append(f"mdn-css after passes cascade {m(A,0)} layout {m(A,1)} > 4")
        if m(A,2)>0.75*m(B,2):bad.append(f"mdn-css cpu-s {m(B,2):.3f} -> {m(A,2):.3f} is not >=25% down")
print("page\tbefore cascade/layout/cpu-s\tafter cascade/layout/cpu-s")
for r in rows:print(f"{r[0]}\t{r[1]:.0f}/{r[2]:.0f}/{r[3]:.3f}\t{r[4]:.0f}/{r[5]:.0f}/{r[6]:.3f}")
print("\n".join(bad) if bad else "CPU.4.a gate: ok");sys.exit(1 if bad else 0)
