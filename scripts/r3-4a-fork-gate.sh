#!/usr/bin/env bash
# R3.4.a gate: 64 children forked from ONE loaded >=2 MB-DOM page (a real corpus page with a script
# appended so it has a live JS context; the corpus snapshots have scripts stripped).
# Green for N=64: replied=N hung=0 own_token=N distinct_digests=N distinct_math_random=N
#   distinct_cached_random=N (a Math.random the page cached before the fork) distinct_iframe_random=N
#   sockets_at_fork=0 dom_bytes>=2MB, (parent + N) distinct TLS ClientHello randoms (strace), and the
#   MCP-level test (manuk-mcp --zygote-after-load) green. `--mutants`: naive fork and no-reseed go RED.
# Writes docs/bench/r3-4a-fork-<ts>.tsv (per-child rows, chrome_ver column) + prints p50/p99.
set -u
cd "$(dirname "$0")/.."
touch .git/sprint-working 2>/dev/null
N=${N:-64}
BIN=target/release/fork-probe
SRC=${SRC:-demo/www/pages/github.html}
PAGE=/tmp/r34a_big_spa.html
NET=${NET_URL:-https://example.com}
CHROME_VER=${MANUK_CHROME_PIN:-unset}
TS=$(date +%Y%m%d-%H%M%S)
OUT=docs/bench/r3-4a-fork-$TS.tsv
fail=0
need() { [ "$1" = "$2" ] || { echo "RED: $3 (got $1, want $2)"; fail=1; }; }
python3 -I - "$SRC" "$PAGE" <<'PY'
import sys
s=open(sys.argv[1]).read(); i=s.rfind('</body>')
open(sys.argv[2],'w').write(s[:i]+"<script>window.__who='parent';Math.random();Math.random();</script>"+s[i:])
PY
run() { local label=$1; shift; local envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  env "${envs[@]}" FORK_PROBE_REALMS=1 timeout 300 $BIN $PAGE "$N" "$@" >/tmp/r34a.$label.out 2>/tmp/r34a.$label.err; }
field() { grep -ao "$2=[^ ]*" /tmp/r34a.$1.err | tail -1 | cut -d= -f2; }

echo "== quiesced fork of a big page, N=$N"
run q MANUK_MAX_DRAIN_MS=300 -- quiesced
dom=$(grep -ao 'dom_bytes=[0-9]*/[0-9]*' /tmp/r34a.q.err | head -1 | cut -d/ -f2)
grep -ao 'page_nodes/dom_bytes=[^ ]*' /tmp/r34a.q.err | head -1
[ "${dom:-0}" -ge 2000000 ] || { echo "RED: DOM serialises to ${dom:-0} bytes (< 2 MB)"; fail=1; }
for k in replied own_token distinct_digests distinct_math_random distinct_cached_random distinct_iframe_random; do need "$(field q $k)" "$N" "$k"; done
need "$(field q sockets_at_fork)" 0 sockets_at_fork; need "$(field q hung)" 0 hung; need "$(field q parent_unchanged)" true parent_unchanged

echo "== live HTTPS from every child, N=$N"
if command -v strace >/dev/null; then
  env MANUK_MAX_DRAIN_MS=300 FORK_PROBE_REALMS=1 strace -f -qq -e trace=write,sendto,writev,sendmsg -xx -s 64 -o /tmp/r34a.st \
    timeout 300 $BIN $PAGE "$N" quiesced "$NET" >/tmp/r34a.n.out 2>/tmp/r34a.n.err
  need "$(field n net_ok)" "$N" net_ok; need "$(field n hung)" 0 "net hung"
  python3 -I - /tmp/r34a.st "$N" <<'PY' || fail=1
import re,sys
n=int(sys.argv[2]); rands={}
for l in open(sys.argv[1]):
    m=re.match(r'(\d+)\s+(\w+)\(.*?"((?:\\x16\\x03\\x01)(?:\\x[0-9a-f]{2})+)',l)
    if not m: continue
    b=bytes(int(x,16) for x in re.findall(r'\\x([0-9a-f]{2})',m.group(3)))
    if len(b)>=43 and b[5]==1: rands.setdefault(m.group(1),[]).append(b[11:43].hex())
allr=[r for v in rands.values() for r in v]
print(f"tls: pids={len(rands)} clienthellos={len(allr)} distinct_randoms={len(set(allr))}")
sys.exit(0 if len(rands)==n+1 and len(set(allr))==len(allr)>=n+1 else (print(f"RED: want {n+1} pids, {n+1} distinct ClientHello randoms") or 1))
PY
else echo "RED: strace missing"; fail=1; fi

{ echo -e "chrome_ver\tidx\tdigest\trss_kb\tprivate_dirty_kb\tchild_fork_us\tparent_fork_us\tdom_bytes"
  awk -F'\t' -v cv="$CHROME_VER" -v d="$dom" 'NR>1 && $2!="-"{print cv"\t"$1"\t"$2"\t"$3"\t"$4"\t"$5"\t"$9"\t"d}' /tmp/r34a.q.out; } > "$OUT"
python3 -I - "$OUT" <<'PY'
import sys
rows=[l.rstrip('\n').split('\t') for l in open(sys.argv[1])][1:]
p=lambda v,q:v[min(len(v)-1,int(q*len(v)))]
for name,i in (("rss_kb",3),("private_dirty_kb",4),("child_fork_us",5),("parent_fork_us",6)):
    v=sorted(int(r[i]) for r in rows); print(f"{name}: p50={p(v,.5)} p99={p(v,.99)} max={v[-1]}")
PY

echo "== shipped path: manuk-mcp --zygote-after-load (4 sessions, distinct dice incl. cached Math.random)"
cargo test --release -p manuk-mcp --test zygote_after_load >/tmp/r34a.mcp.log 2>&1 </dev/null
rc=$?; grep -aE "^test |test result" /tmp/r34a.mcp.log
[ $rc = 0 ] || { echo "RED: zygote_after_load"; fail=1; }

if [ "${1:-}" = "--mutants" ]; then
  echo "== mutants (each must go RED)"
  N=8; export N
  run m1 -- naive; [ "$(field m1 hung)" != 0 ] && echo "ok: naive fork -> hung=$(field m1 hung)" || { echo "NOT RED: naive"; fail=1; }
  run m2 FORK_PROBE_NO_RESEED=1 -- quiesced
  [ "$(field m2 distinct_cached_random)" = 1 ] && [ "$(field m2 distinct_math_random)" = 1 ] && echo "ok: no reseed -> math=1 cached=1" || { echo "NOT RED: no-reseed"; fail=1; }
fi
[ $fail = 0 ] && echo "GREEN ($OUT)" || echo "RED"
exit $fail
