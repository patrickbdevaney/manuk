import csv,sys,collections
G=['dashdish-11','dashdish-2','opendining-4','udriver-8','zilloft-2']
r=list(csv.DictReader(open(sys.argv[1]),delimiter='\t'))
assert all(x['chrome_ver']=='145.0.7632.116' for x in r)
p=collections.Counter(); 
for x in r: p[x['task']]+=int(x['pass'])
ok=[t for t in G if p[t]>=2]; print(sys.argv[1], len(ok),'/5', ok)
sys.exit(0 if len(ok)>=3 else 1)
