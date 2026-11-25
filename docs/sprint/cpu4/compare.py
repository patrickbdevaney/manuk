#!/usr/bin/env python3
"""CPU.4: total cpu-s per page, same frozen page from the same loopback server, interleaved per round.
compare.py <out.tsv> <rounds> <page,...>   arms: manuk-mcp (navigate+observe, --deterministic --backend engine), a11y-dump, chrome (headless --dump-dom)
Chrome needs an already-initialised --user-data-dir (a fresh one hangs silently on first run here): /tmp/cpu4-chrome-profile was warmed by one prior run.
cpu-s = ru_utime+ru_stime from wait4 of the process we spawned (includes its reaped descendants: chrome's renderer/gpu/network children)."""
import subprocess,sys,os,json,time,threading,http.server,functools,socketserver
out,rounds,pages=sys.argv[1],int(sys.argv[2]),sys.argv[3].split(",")
here=os.path.dirname(os.path.abspath(__file__));root=os.path.join(here,"../../../demo/www/pages")
class Q(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*a):pass
srv=socketserver.ThreadingTCPServer(("127.0.0.1",0),functools.partial(Q,directory=root));port=srv.server_address[1]
threading.Thread(target=srv.serve_forever,daemon=True).start()
MCP=os.path.join(here,"../../../target/release/manuk-mcp");DUMP=os.path.join(here,"../../../target/release/a11y-dump")
CHROME=os.environ.get("CHROME","/home/patrickd/.local/chrome-cft/bin/google-chrome")
def wait(p):
    _,st,ru=os.wait4(p.pid,0);p.returncode=0;return ru.ru_utime+ru.ru_stime
def mcp(page):
    p=subprocess.Popen([MCP,"--deterministic","--backend","engine"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
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
    rpc("tools/call",{"name":"navigate","arguments":{"url":f"http://127.0.0.1:{port}/{page}.html"}})
    r=rpc("tools/call",{"name":"observe","arguments":{}})
    p.stdin.close();c=wait(p);return c,("result" in r and not r["result"].get("isError"))
def dump(page):
    p=subprocess.Popen([DUMP,f"http://127.0.0.1:{port}/{page}.html"],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);return wait(p),True
def chrome(page):
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        p=subprocess.Popen([CHROME,"--headless=new","--no-sandbox","--disable-gpu","--no-first-run","--disable-dev-shm-usage","--no-zygote","--disable-background-networking","--user-data-dir=/tmp/cpu4-chrome-profile","--dump-dom",f"http://127.0.0.1:{port}/{page}.html"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
        import threading as T;T.Timer(60,p.kill).start();o=p.stdout.read();c=wait(p);return c,len(o)>1000
arms={"manuk-mcp":mcp,"a11y-dump":dump,"chrome":chrome}
with open(out,"a") as f:
    for rd in range(rounds):
        for pg in pages:
            names=list(arms);k=rd%len(names);names=names[k:]+names[:k]
            for a in names:
                c,ok=arms[a](pg)
                f.write("\t".join(map(str,[rd,pg,a,f"{c:.3f}",ok,os.getloadavg()[0],"145.0.7632.116"]))+"\n");f.flush()
