#!/bin/bash
# R0.4.a.c.a.b.a.a.a.a.a.a.a.a.b gate: Chrome AX comparison classifies >=90% of manuk's salesforce nobox targets; red mutants.
set -u; cd "$(dirname "$0")/.."
R=docs/bench/sf-a11y-release-20261103.json; C=docs/bench/sf-chrome-ax-20261103.json; T=$(mktemp -d)
python3 scripts/sf-chrome-ax-compare.py $R $C $T/o.tsv >/dev/null || { echo "FAIL: real data not green"; exit 1; }
# mutant 1: empty chrome tree -> must be red
python3 -c "import json;d=json.load(open('$C'));d['nodes']=[];json.dump(d,open('$T/m1.json','w'))"
# mutant 2: wrong chrome_ver -> must be red
python3 -c "import json;d=json.load(open('$C'));d['chrome_ver']='1.0';json.dump(d,open('$T/m2.json','w'))"
# mutant 3: every manuk nobox name exposed with a box in chrome -> must be red
python3 -c "
import json;d=json.load(open('$C'));r=json.load(open('$R'))
d['nodes']+=[{'role':n['role'],'name':n['name'].strip(),'ignored':False,'box':[10,10]} for n in r if n['role'] in('link','button') and not n['w']]
json.dump(d,open('$T/m3.json','w'))"
for m in m1 m2 m3; do
  python3 scripts/sf-chrome-ax-compare.py $R $T/$m.json $T/o.tsv >/dev/null && { echo "FAIL: mutant $m stayed green"; exit 1; }
done
echo "GREEN: real classified >=90%, 3 mutants red"
