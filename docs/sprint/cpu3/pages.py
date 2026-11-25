#!/usr/bin/env python3
"""CPU.3 page bench: pages.py <outfile.tsv> <variant,variant,..> <rounds>. For each round, each page, variants in rotated order:
one fresh `manuk-mcp --deterministic --backend engine`, navigate + observe; cpu-s = utime+stime over the process tree, peak PSS sampled at 20 ms.
Pages are served from demo/www/pages by python http.server. Records 1-min loadavg per row."""
import json,subprocess,sys,os,time,threading,psutil,http.server,functools,socketserver
out,variants,rounds=sys.argv[1],sys.argv[2].split(","),int(sys.argv[3])
BIN="/home/patrickd/cpu3bin/manuk-mcp-"
PAGES=["hackernews","rustlang","mdn-css","github","lobsters","python-docs","sqlite","pgdocs","rfc-editor","stripe-docs"]
root=os.path.join(os.path.dirname(os.path.abspath(__file__)),"../../../demo/www/pages")
H=functools.partial(http.server.SimpleHTTPRequestHandler,directory=root)
class Q(H.func):
    def log_message(self,*a):pass
srv=socketserver.TCPServer(("127.0.0.1",0),functools.partial(Q,directory=root));port=srv.server_address[1]
threading.Thread(target=srv.serve_forever,daemon=True).start()
def one(var,page):
    p=subprocess.Popen([BIN+var,"--deterministic","--backend","engine"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
    peak=[0.0];stop=[False]
    def tree():
        try:
            pr=psutil.Process(p.pid);return [pr]+pr.children(recursive=True)
        except psutil.Error:return []
    def samp():
        while not stop[0]:
            t=0
            for q in tree():
                try:t+=q.memory_full_info().pss
                except psutil.Error:pass
            peak[0]=max(peak[0],t/1048576);time.sleep(0.02)
    th=threading.Thread(target=samp);th.start()
    def cpu():
        t=0.0
        for q in tree():
            try:c=q.cpu_times();t+=c.user+c.system
            except psutil.Error:pass
        return t
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
    c0=cpu()
    rpc("tools/call",{"name":"navigate","arguments":{"url":f"http://127.0.0.1:{port}/{page}.html"}})
    c1=cpu()
    r=rpc("tools/call",{"name":"observe","arguments":{}})
    c2=cpu();stop[0]=True;th.join();p.kill();p.wait()
    ok="result" in r and not r["result"].get("isError")
    return round(c1-c0,3),round(c2-c1,3),round(peak[0],1),ok
with open(out,"a") as f:
    for rd in range(rounds):
        for pg in PAGES:
            order=variants[rd%len(variants):]+variants[:rd%len(variants)]
            for v in order:
                nav,obs,pss,ok=one(v,pg)
                f.write("\t".join(map(str,[rd,pg,v,nav,obs,round(nav+obs,3),pss,ok,os.getloadavg()[0],"145.0.7632.116"]))+"\n");f.flush()
