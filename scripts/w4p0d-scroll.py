import subprocess,json,sys
p=subprocess.Popen(["target/release/manuk-mcp"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
i=0
def rpc(m,pa):
    global i; i+=1
    p.stdin.write(json.dumps({"jsonrpc":"2.0","id":i,"method":m,"params":pa})+"\n"); p.stdin.flush()
    while True:
        v=json.loads(p.stdout.readline())
        if v.get("id")==i: return v
def tool(n,a):
    r=rpc("tools/call",{"name":n,"arguments":a})
    return "\n".join(b.get("text","") for b in r["result"]["content"])
rpc("initialize",{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"t","version":"0"}})
tool("navigate",{"url":sys.argv[1]})
last=""
for _ in range(int(sys.argv[4]) if len(sys.argv)>4 else 4): last=tool("scroll",{"dy":int(sys.argv[2])})
t=tool("extract",{"format":"text"})
print(last[:200]); print("PASS" if sys.argv[3] in t else "FAIL"); p.kill()
