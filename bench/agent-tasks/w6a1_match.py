import csv,json,glob,os,sys,re
csv.field_size_limit(10**9)
def final_of(fn):
    final='';n=0
    try:
        for line in open(fn):
            try:d=json.loads(line)
            except: continue
            if d.get('type')=='assistant':
                for c in d['message'].get('content',[]):
                    if c.get('type')=='text': final=c['text']
            if d.get('type')=='result' and isinstance(d.get('result'),str): final=d['result'] or final
    except Exception as e: pass
    return final
norm=lambda s:re.sub(r'\s+','',s)[:150]
def run(tsv,arm,prefix):
    rows=list(csv.DictReader(open(tsv),delimiter='\t',quoting=csv.QUOTE_NONE))
    hit=miss=0;out=[]
    for r in rows:
        if r['pass']!='0': continue
        a=norm(r['answer'].strip('"').replace('""','"'))
        found=None
        for d in glob.glob(f"target/real-runs/{prefix}_{r['task']}_r{r['run']}*"):
            fn=d+'/transcript.jsonl'
            if os.path.exists(fn) and a and norm(final_of(fn))==a: found=fn;break
        if found: hit+=1
        else: miss+=1
        out.append((r['task'],r['run'],found))
    print(tsv,hit,miss)
    return out
if __name__=='__main__':
    for t,arm in [('rg2b-real-manuk','manuk'),('rg2b-real-playwright','playwright'),('w6b0-cdplayer-real','cdp-layer')]:
        run(f'bench-queue/out/{t}.tsv',arm,arm)
