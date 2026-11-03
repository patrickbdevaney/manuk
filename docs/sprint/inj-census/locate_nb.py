# W3.15.a: sentence-level naive-Bayes locator for pages with no signature gram. Weak positives = train-malicious sentences holding a df>=10 signature gram; negatives = train-benign sentences; threshold = quantile of max-sentence score over 500 held-out train benign pages. Run from docs/sprint/inj-census (needs /tmp/bs/*.json).
import json,sys,re,math,collections,random
sys.path.insert(0,'.')
import census as C
random.seed(3)
tr=json.load(open('/tmp/bs/train.json'));t=json.load(open('/tmp/bs/test.json'))
S={g for g,v in json.load(open('sigs.json')).items() if v>=10}
def sentences(h):
    out=[]
    for k,v in C.segments(h):
        for s in re.split(r'(?<=[.!?])\s+|\n+',v):
            if len(s.split())>=5: out.append(s)
    return out
def feats(s):
    w=C.words(s); return set(w)|{a+' '+b for a,b in zip(w,w[1:])}
trm=[r for r in tr if r['label']=='yes'];trb=[r for r in tr if r['label']=='no']
random.shuffle(trm);random.shuffle(trb)
pos=collections.Counter();neg=collections.Counter();np_=nn=0
for r in trm[:3500]:
    for s in sentences(r['content']):
        if C.grams(s)&S:
            np_+=1;pos.update(feats(s))
for r in trb[:3500]:
    for s in sentences(r['content']):
        nn+=1;neg.update(feats(s))
print(np_,nn)
def sscore(s):
    sc=0.0
    for f in feats(s):
        p=pos.get(f,0);n=neg.get(f,0)
        if p+n<3: continue
        sc+=math.log(((p+0.5)/np_)/((n+0.5)/nn))
    return sc
def page(h,sub=None):
    return max([sscore(s) for s in sentences(h)] or [-9e9])
hold=[page(r['content']) for r in trb[3500:4000]]
hold.sort()
mal=[(i,r) for i,r in enumerate(t) if r['label']=='yes']
sigs=S
un=[(i,r) for i,r in mal if not C.payload(r['content'],sigs)]
us=[page(r['content']) for i,r in un]
tb=[page(r['content']) for r in [r for r in t if r['label']=='no'][:300]]
for q in (0.98,0.99,0.995,1.0):
    thr=hold[min(len(hold)-1,int(q*len(hold)))]
    print(q,round(thr,1),'unlocated recovered',sum(x>thr for x in us)/len(us),'test benign fp',sum(x>thr for x in tb)/len(tb))
json.dump({'un':us,'hold':hold,'tb':tb},open('/tmp/nbscores.json','w'))
