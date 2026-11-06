#!/usr/bin/env bash
# R3.4 gate: 64 children forked from ONE loaded page process (quiesced: 1 thread at fork).
# Each child acts differently and reports an a11y digest. Green requires, for N=64:
#   replied=N hung=0 own_token=N distinct_digests=N distinct_math_random=N (Math.random after the parent drew) sockets_at_fork=0 parent_unchanged
#   + spot-checked digests (idx 0,31,63) equal a never-forked reference process
#   + script watchdog cuts a spinning child (spin_cut_ms < 3000 for every child)
#   + live HTTPS: net_ok=N and (parent + N) distinct TLS ClientHello randoms (strace)
# `--mutants` also runs the broken inputs and requires each to go RED.
set -u
cd "$(dirname "$0")/.."
touch .git/sprint-working 2>/dev/null
N=${N:-64}
BIN=target/release/fork-probe
FIX=agent/tests/fixtures/fork_probe.html
NET=${NET_URL:-https://example.com}
CHROME_VER=${MANUK_CHROME_PIN:-unset}
TS=$(date +%Y%m%d-%H%M%S)
OUT=docs/bench/r3-4-fork-$TS.tsv
fail=0
need() { [ "$1" = "$2" ] || { echo "RED: $3 (got $1, want $2)"; fail=1; }; }

run() { # run <label> <env...> -- <mode> [net]  -> writes /tmp/r34.$label.{out,err}
  local label=$1; shift
  local envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  env "${envs[@]}" timeout 300 $BIN $FIX "$N" "$@" >/tmp/r34.$label.out 2>/tmp/r34.$label.err
}
field() { grep -o "$2=[^ ]*" /tmp/r34.$1.err | tail -1 | cut -d= -f2; }

echo "== quiesced fork, N=$N (+spin)"
run q MANUK_MAX_DRAIN_MS=300 FORK_PROBE_SPIN=1 -- quiesced
for k in replied own_token distinct_digests distinct_math_random sockets_at_fork hung parent_unchanged; do
  v=$(field q $k)
  case $k in
    replied|own_token|distinct_digests|distinct_math_random) need "$v" "$N" "$k" ;;
    sockets_at_fork|hung) need "$v" 0 "$k" ;;
    parent_unchanged) need "$v" true "$k" ;;
  esac
done
grep -q 'threads at fork: 1$' /tmp/r34.q.err || { echo "RED: not single-threaded at fork"; fail=1; }
slow=$(awk -F'\t' 'NR>1 && match($7,/spin_cut_ms=[0-9]+/){v=substr($7,RSTART+12,RLENGTH-12); if(v+0>=3000)c++; n++} END{print (n==0?"none":c+0)}' /tmp/r34.q.out)
need "$slow" 0 "children not cut by the script deadline"

echo "== reference digests (never-forked)"
for i in 0 31 63; do
  ref=$($BIN $FIX 1 ref:$i 2>/dev/null | cut -f2)
  got=$(awk -F'\t' -v i=$i '$1==i{print $2}' /tmp/r34.q.out)
  need "$got" "$ref" "digest of child $i vs fresh process"
done

echo "== live HTTPS, N=$N (strace for ClientHello randoms)"
if command -v strace >/dev/null; then
  env MANUK_MAX_DRAIN_MS=300 strace -f -qq -e trace=write,sendto,writev,sendmsg -xx -s 64 -o /tmp/r34.st \
    timeout 300 $BIN $FIX "$N" quiesced "$NET" >/tmp/r34.n.out 2>/tmp/r34.n.err
  need "$(field n net_ok)" "$N" "net_ok"
  need "$(field n sockets_at_fork)" 0 "net sockets_at_fork"
  need "$(field n hung)" 0 "net hung"
  python3 -I - /tmp/r34.st "$N" <<'PY' || fail=1
import re,sys
n=int(sys.argv[2]); rands={}
for l in open(sys.argv[1]):
    m=re.match(r'(\d+)\s+(\w+)\(.*?"((?:\\x16\\x03\\x01)(?:\\x[0-9a-f]{2})+)',l)
    if not m: continue
    b=bytes(int(x,16) for x in re.findall(r'\\x([0-9a-f]{2})',m.group(3)))
    if len(b)>=43 and b[5]==1: rands.setdefault(m.group(1),[]).append(b[11:43].hex())
allr=[r for v in rands.values() for r in v]
print(f"tls: pids={len(rands)} clienthellos={len(allr)} distinct_randoms={len(set(allr))}")
ok = len(rands)==n+1 and len(set(allr))==len(allr)>=n+1
sys.exit(0 if ok else (print(f"RED: want {n+1} pids with {n+1} distinct ClientHello randoms") or 1))
PY
else echo "RED: strace missing"; fail=1; fi

# Measurements: the quiesced run's per-child rows, with the pin.
{
  echo -e "chrome_ver\tidx\tdigest\trss_kb\tprivate_dirty_kb\tchild_fork_us\tparent_fork_us"
  awk -F'\t' -v cv="$CHROME_VER" 'NR>1 && $2!="-"{print cv"\t"$1"\t"$2"\t"$3"\t"$4"\t"$5"\t"$9}' /tmp/r34.q.out
} > "$OUT"
python3 -I - "$OUT" <<'PY'
import sys,statistics as st
rows=[l.rstrip('\n').split('\t') for l in open(sys.argv[1])][1:]
col=lambda i:sorted(int(r[i]) for r in rows)
p=lambda v,q:v[min(len(v)-1,int(q*len(v)))]
for name,i in (("rss_kb",3),("private_dirty_kb",4),("child_fork_us",5),("parent_fork_us",6)):
    v=col(i); print(f"{name}: p50={p(v,.5)} p99={p(v,.99)} max={v[-1]}")
PY

if [ "${1:-}" = "--mutants" ]; then
  echo "== mutants (each must go RED)"
  N=8 ; export N
  run m1 -- naive;                 [ "$(field m1 hung)" != 0 ] && echo "ok: naive fork -> hung=$(field m1 hung)" || { echo "NOT RED: naive"; fail=1; }
  run m2 FORK_PROBE_NO_RESEED=1 -- quiesced; [ "$(field m2 distinct_math_random)" = 1 ] && echo "ok: no reseed -> distinct_math_random=1" || { echo "NOT RED: no-reseed ($(field m2 distinct_math_random))"; fail=1; }
  run m3 FORK_PROBE_KEEP_SOCKET=1 -- quiesced; [ "$(field m3 sockets_at_fork)" != 0 ] && echo "ok: leaked socket -> sockets_at_fork=$(field m3 sockets_at_fork)" || { echo "NOT RED: socket"; fail=1; }
fi
[ $fail = 0 ] && echo "GREEN ($OUT)" || echo "RED"
exit $fail
