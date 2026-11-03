#!/bin/bash
# R0.4.a.c.a.b.a.a.a.a.a.a.a.a.b.a gate: pruned salesforce release dump has <=7 nobox link/button, list has <=10 Ungrounded.
set -u; cd "$(dirname "$0")/.."
D=${1:-docs/bench/sf-a11y-release-pruned-20261103.json}; L=${2:-docs/bench/sf-list-release-pruned-20261103.txt}
n=$(python3 -c "import json,sys;r=json.load(open('$D'));print(sum(1 for n in r if n['role'] in('link','button') and (not n['w'] or not n['h'])))")
u=$(grep -c Ungrounded "$L")
echo "nobox=$n ungrounded=$u chrome_ver=${MANUK_CHROME_PIN:?}"
[ "$n" -le 7 ] && [ "$u" -le 10 ] && [ "$n" -gt -1 ] && echo GREEN || { echo RED; exit 1; }
