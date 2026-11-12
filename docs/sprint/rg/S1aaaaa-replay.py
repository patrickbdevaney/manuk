import json,subprocess,sys,re,time
p=subprocess.Popen(['/home/patrickd/manuk/target/release/manuk-mcp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
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
    r=rpc('tools/call',{"name":n,"arguments":a})
    try: return ''.join(c.get('text','') for c in r['result']['content'])
    except Exception: return str(r)[:500]
def find(o,pat):
    for l in o.split('\n'):
        if re.search(pat,l) and '[h' in l:
            return l[l.index('[h')+1:l.index(']',l.index('[h'))]
def click(label,exact=False):
    o=tool('observe',{"mode":"interactive"})
    h=find(o,re.escape(label)+('"' if exact else ''))
    if not h: return label+": NOHANDLE"
    r=tool('act',{"handle":h,"action":"click"})
    return label+": "+r[:600].replace('\n',' | ')
tool('navigate',{"url":"https://evals-zilloft.vercel.app"})
o=tool('observe',{"mode":"interactive"})
tid=find(o,'textbox')
print(tool('type',{"handle":tid,"text":"San Francisco"})[:80])
print(click('Start search'))
print(click('Beds & Baths')[:300])
o=tool('observe',{"mode":"interactive"})
L=[l for l in o.split('\n') if '[h' in l]
def hid(l): return l[l.index('[h')+1:l.index(']',l.index('[h'))]
a=[l for l in L if 'button "3+"' in l]; b=[l for l in L if 'button "2+"' in l]; r=[l for l in L if 'exact match' in l]
seq=[('exact',hid(r[0])),('beds3+',hid(a[0])),('baths2+',hid(b[1]))]
for n,h in seq:
    t=tool('act',{"handle":h,"action":"click"}); print(n,'NOEFFECT' if '+0 ~0 -0' in t else 'effect',t[:90].replace('\n',' '))
o=tool('observe',{"mode":"interactive"})
ap=[l for l in o.split('\n') if 'APPLY' in l][0]
t=tool('act',{"handle":hid(ap),"action":"click"}); print('apply',t[:100].replace('\n',' '))
c=tool('observe',{"mode":"full"}); open('/tmp/rg/full.txt','w').write(c)
print([l[:100] for l in c.split('\n') if re.search(r'result|bd|\bba\b',l,re.I)][:6]); print(len(re.findall(r'3 ?bds?',c)),len(re.findall(r'2 ?ba',c)))
print('CARDS',len(re.findall(r'button "\d+ bds?"',c)),'3bds',len(re.findall(r'button "3 bds"',c)),'2ba',len(re.findall(r'button "2 ba"',c)))
