"""RG.S1.a.a.a.b: per-click drain ms on the zilloft /homes panel under manuk-mcp.
Usage: S1aaab-drain.py RUNS OUT.tsv [STDERR_LOG]   env MANUK_MAX_DRAIN_MS is passed through (default 120000 here).
Each act's wall time is the drain (dispatch + settle + a11y diff). MANUK_LAYOUT_PROFILE=1 stderr is kept per run."""
import json,subprocess,sys,re,time,os
runs=int(sys.argv[1]); out=sys.argv[2]; elog=sys.argv[3] if len(sys.argv)>3 else '/dev/null'
env=dict(os.environ); env["MANUK_ACT_TRACE"]="1"; env.setdefault('MANUK_MAX_DRAIN_MS','120000'); env['MANUK_LAYOUT_PROFILE']='1'
PIN=os.environ.get('MANUK_CHROME_PIN','')
rows=[]
for r in range(1,runs+1):
    err=open(elog,'a'); err.write(f'### run {r}\n'); err.flush()
    p=subprocess.Popen(['/home/patrickd/manuk/target/release/manuk-mcp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,text=True,env=env)
    i=0
    def rpc(m,pr):
        global i; i+=1
        p.stdin.write(json.dumps({"jsonrpc":"2.0","id":i,"method":m,"params":pr})+"\n"); p.stdin.flush()
        while True:
            v=json.loads(p.stdout.readline())
            if v.get('id')==i: return v
    rpc('initialize',{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"t","version":"0"}})
    p.stdin.write(json.dumps({"jsonrpc":"2.0","method":"notifications/initialized"})+"\n"); p.stdin.flush()
    def tool(n,a):
        t=time.time(); x=rpc('tools/call',{"name":n,"arguments":a}); ms=int((time.time()-t)*1000)
        try: s=''.join(c.get('text','') for c in x['result']['content'])
        except Exception: s=str(x)[:500]
        return s,ms
    def hid(l): return l[l.index('[h')+1:l.index(']',l.index('[h'))]
    def find(o,pat):
        for l in o.split('\n'):
            if re.search(pat,l) and '[h' in l: return hid(l)
    def rec(step,ms,txt,mark):
        rows.append((PIN,r,step,ms,env['MANUK_MAX_DRAIN_MS'],'NOEFFECT' if '+0 ~0 -0' in txt and 'class changed' not in txt else 'ok'))
        err.write(f'### run {r} step {step} {ms}ms\n'); err.flush()
    s,ms=tool('navigate',{"url":"https://evals-zilloft.vercel.app"}); rec('navigate',ms,s,0)
    o,_=tool('observe',{"mode":"interactive"}); tid=find(o,'textbox')
    s,ms=tool('type',{"handle":tid,"text":"San Francisco"}); rec('type',ms,s,0)
    def click(label):
        o,_=tool('observe',{"mode":"interactive"}); h=find(o,re.escape(label))
        if not h: rec('click:'+label,-1,'NOHANDLE',0); return
        s,ms=tool('act',{"handle":h,"action":"click"}); rec('click:'+label,ms,s,0)
    click('Start search'); click('Beds & Baths')
    o,_=tool('observe',{"mode":"interactive"})
    L=[l for l in o.split('\n') if '[h' in l]
    a=[l for l in L if 'button "3+"' in l]; b=[l for l in L if 'button "2+"' in l]; e=[l for l in L if 'exact match' in l]
    for n,l in (('exact',e),('beds3+',a),('baths2+',b[1:2] or b)):
        if not l: rec('click:'+n,-1,'NOHANDLE',0); continue
        s,ms=tool('act',{"handle":hid(l[0]),"action":"click"}); rec('click:'+n,ms,s,0)
    o,_=tool('observe',{"mode":"interactive"}); ap=[l for l in o.split('\n') if 'APPLY' in l]
    if ap:
        s,ms=tool('act',{"handle":hid(ap[0]),"action":"click"}); rec('click:apply',ms,s,0)
    p.stdin.close(); p.terminate(); err.close()
with open(out,'w') as f:
    f.write('chrome_ver\trun\tstep\tms\tmax_drain_ms\teffect\n')
    for x in rows: f.write('\t'.join(map(str,x))+'\n')
print(open(out).read())
