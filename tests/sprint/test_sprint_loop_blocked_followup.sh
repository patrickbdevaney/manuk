#!/usr/bin/env bash
# gate for W6.J1.b.a.a.a.a.e: an agent BLOCK on a node with an open follow-up stays BLOCKED (sprint-next then prints the
# follow-up); with no open follow-up it is reset to TODO and escalated.
#   test_sprint_loop_blocked_followup.sh           -> real loop (green)
#   test_sprint_loop_blocked_followup.sh --mutant  -> loop without the open-follow-up check must be RED
set -u; cd "$(dirname "$0")/../.."; SRC=$PWD/scripts/sprint-loop.sh; MUT=${1:-}
T=$(mktemp -d); trap 'pkill -f "$T/root" 2>/dev/null; [ -n "${KEEP:-}" ] || rm -rf "$T"' EXIT
run() { # $1 = suffix, $2 = follow-up status ("" = no follow-up); prints "<X status> <sprint-next>"
  local R=$T/root$1; rm -rf $R; mkdir -p $R/scripts $R/docs/sprint $R/.git $T/bin $T/pin/9.9.9.9/chrome-linux64
  cp scripts/sprint-next.py scripts/sprint-pack.py scripts/sprint-ledger.py scripts/sprint-graph.py $R/scripts/ 2>/dev/null
  cp "$SRC" $R/scripts/sprint-loop.sh
  [ "$MUT" = --mutant ] && sed -i 's/ && \[ "\$(open_followups "\$node")" -eq 0 \]//' $R/scripts/sprint-loop.sh
  echo base >$R/docs/sprint/PACK-BASE.md; echo 9.9.9.9 >$R/docs/sprint/CHROME_PIN; : >$R/docs/sprint/loop.log; : >$R/docs/sprint/JOURNAL.md
  printf 'id\tring\tpri\tmodel_floor\tdeps\tstatus\ttitle\tgate\tnotes\nX\t0\t1\tsonnet\t-\tTODO\tt\tg\tn\n' >$R/docs/sprint/GRAPH.tsv
  [ -n "$2" ] && printf 'X.a\t0\t1\tsonnet\tX\t%s\tt\tg\tn\n' "$2" >>$R/docs/sprint/GRAPH.tsv
  printf '#!/bin/sh\nexit 0\n' >$T/pin/9.9.9.9/chrome-linux64/chrome; chmod +x $T/pin/9.9.9.9/chrome-linux64/chrome
  touch $R/.git/sprint-chrome-checked
  printf '#!/bin/sh\nwhile [ "${1#-}" != "$1" ]; do case "$1" in -p|--setenv) shift;; esac; shift; done\nexec "$@"\n' >$T/bin/systemd-run
  printf '#!/bin/sh\nexit 0\n' >$T/bin/curl
  cat >$T/bin/claude <<STUB
#!/bin/sh
[ -e $T/ran$1 ] && exit 1
touch $T/ran$1
cd $R && python3 scripts/sprint-graph.py set X status BLOCKED >/dev/null
echo '{"type":"result","is_error":false,"result":"r"}'
exit 1
STUB
  chmod +x $T/bin/*
  ( PATH=$T/bin:$PATH PIN_HOME=$T/pin MIN_FREE_GB=0 SPRINT_SLEEP=1 XDG_RUNTIME_DIR=$T bash $R/scripts/sprint-loop.sh >/dev/null 2>&1 ) &
  for _ in $(seq 40); do grep -qE "BLOCKED by the agent" $R/docs/sprint/loop.log 2>/dev/null && break; sleep 0.5; done
  pkill -f "$R/scripts/sprint-loop.sh" 2>/dev/null; sleep 1
  echo "$(awk -F'\t' '$1=="X"{print $6}' $R/docs/sprint/GRAPH.tsv) $(cd $R && python3 scripts/sprint-next.py | cut -f1)"
}
with=$(run A TODO); without=$(run B "")
echo "with-followup=[$with] no-followup=[$without]"
[ "$with" = "BLOCKED X.a" ] && [ "$without" = "TODO X" ] && { echo "ok   open follow-up keeps X BLOCKED and ready; none -> reset to TODO"; exit 0; }
echo "FAIL"; exit 1
