#!/usr/bin/env python3
"""W6.A1: extract final answer + last 3 tool results of every failed run of the 3 current arms.
usage: w6a1_extract.py OUTDIR  -> OUTDIR/rows.tsv and OUTDIR/batch-NN.txt (25 runs each)"""
import sys,os,json
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
import w6a1_match as m
SRC=[('rg2b-real-manuk','manuk'),('rg2b-real-playwright','playwright'),('w6b0-cdplayer-real','cdp-layer')]
def digest(fn):
    results=[];final=''
    for line in open(fn):
        try:d=json.loads(line)
        except: continue
        if d.get('type')=='user' and isinstance(d.get('message'),dict):
            for c in d['message'].get('content',[]) if isinstance(d['message'].get('content'),list) else []:
                if isinstance(c,dict) and c.get('type')=='tool_result':
                    cc=c.get('content')
                    if isinstance(cc,list): cc=' '.join(x.get('text','') for x in cc if isinstance(x,dict))
                    results.append(str(cc))
        if d.get('type')=='assistant':
            for c in d['message'].get('content',[]):
                if c.get('type')=='text': final=c['text']
    return final,results
def main(out):
    os.makedirs(out,exist_ok=True)
    rows=[]
    for t,arm in SRC:
        for task,run,fn in m.run(f'bench-queue/out/{t}.tsv',arm,arm):
            assert fn,(arm,task,run)
            rows.append((arm,task,run,fn))
    rows.sort(key=lambda r:(r[1],r[2],r[0]))   # arms interleaved so batches are mixed
    with open(f'{out}/rows.tsv','w') as f:
        f.write('arm\ttask\trun\ttranscript\n')
        for r in rows: f.write('\t'.join(r)+'\n')
    for i in range(0,len(rows),25):
        o=[]
        for arm,task,run,fn in rows[i:i+25]:
            final,res=digest(fn)
            o.append(f"=== ID {arm}|{task}|r{run}  (tool results: {len(res)})\nFINAL: {final[:900]}")
            for r in res[-3:]: o.append('  LAST: '+r[:300].replace('\n',' | '))
        open(f'{out}/batch-{i//25:02d}.txt','w').write('\n'.join(o)+'\n')
    print(len(rows),'rows')
main(sys.argv[1])
