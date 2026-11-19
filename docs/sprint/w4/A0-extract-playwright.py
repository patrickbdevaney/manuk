import csv, json, os, sys
RUNS='docs/sprint/w318/real-runs.tsv'
KEEP='docs/sprint/w4/transcripts-keep'
latest={}
with open(RUNS) as fh:
    rd=csv.DictReader(fh, delimiter='\t')
    for r in rd:
        if r['arm']=='playwright':
            latest[(r['task'],r['run'])]=r  # later rows overwrite
fails=[(k,v) for k,v in latest.items() if v['pass']=='0']
print('playwright latest rows',len(latest),'failed',len(fails), file=sys.stderr)
out=[]
missing=[]
for (task,run),v in sorted(fails):
    fn=f'{KEEP}/playwright_{task}_r{run}.jsonl'
    if not os.path.exists(fn): missing.append(fn); continue
    results=[]; final=''
    for line in open(fn):
        d=json.loads(line)
        if d.get('type')=='user':
            for c in d['message'].get('content',[]) if isinstance(d.get('message'),dict) else []:
                if isinstance(c,dict) and c.get('type')=='tool_result':
                    cc=c.get('content')
                    if isinstance(cc,list): cc=' '.join(x.get('text','') for x in cc if isinstance(x,dict))
                    results.append(str(cc))
        if d.get('type')=='assistant':
            for c in d['message'].get('content',[]):
                if c.get('type')=='text': final=c['text']
    out.append(f"=== playwright {task} r{run} | answer_col: {v['answer'][:300]!r}\nFINAL: {final[:600]!r}")
    for r_ in results[-3:]:
        out.append('  LAST: '+r_[:220].replace('\n',' | '))
print('missing',missing, file=sys.stderr)
open('/tmp/pw_extract.txt','w').write('\n'.join(out))
