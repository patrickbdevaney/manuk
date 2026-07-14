#!/usr/bin/env bash
# Regenerate the WPT horizon counts from the LOCAL checkout — never fabricate them.
#
# Prints real, current testharness-file counts per top-level WPT directory, and the css/ sub-spec
# breakdown (css/ is the one directory that is many specs). Run on the EPOCH-audit cadence and paste the
# numbers into docs/wiki/wpt-horizon.md, or extend it to run `manuk-wpt wpt <dir>` for live pass rates.
set -euo pipefail
WPT="${WPT_DIR:-$HOME/wpt}"
[ -d "$WPT" ] || { echo "no WPT checkout at $WPT — run ./scripts/wpt-setup.sh"; exit 1; }

python3 - "$WPT" <<'PY'
import os, sys
WPT = sys.argv[1]
def count(d):
    h=a=r=0
    base=os.path.join(WPT,d)
    if not os.path.isdir(base): return None
    for root,_,files in os.walk(base):
        for f in files:
            rel=os.path.join(root,f)
            if f.endswith(('.html','.htm','.xht','.xhtml')):
                if f.endswith('-ref.html') or '-ref.' in f or '/reference/' in rel: r+=1
                else: h+=1
            elif f.endswith('.js') and ('.any.' in f or '.window.' in f): a+=1
    return h,a,r
print("=== WPT horizon — counted from", WPT, "===\n")
for d in sorted(x for x in os.listdir(WPT) if os.path.isdir(os.path.join(WPT,x)) and not x.startswith('.')):
    c=count(d)
    if c and c[0]+c[1]>0: print(f"  {d:18} .html={c[0]:5}  .any/.window.js={c[1]:4}  refs={c[2]}")
print("\n=== css/ sub-specs (css/ is many specs in one dir) ===")
cd=os.path.join(WPT,'css')
if os.path.isdir(cd):
    for s in sorted(os.listdir(cd)):
        c=count(os.path.join('css',s))
        if c and c[0]>0: print(f"  css/{s:26} .html={c[0]:5}  refs={c[2]}")
PY
