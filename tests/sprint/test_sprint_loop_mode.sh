#!/usr/bin/env bash
# gate for W6.L1.a.b.a: SPRINT_PROMPT_MODE=alternate makes the loop alternate full,pack,full,pack; unset => all pack.
#   test_sprint_loop_mode.sh           -> real loop (green)
#   test_sprint_loop_mode.sh --mutant  -> loop without the alternate branch must be RED
set -u; cd "$(dirname "$0")/../.."; SRC=$PWD/scripts/sprint-loop.sh; MUT=${1:-}
T=$(mktemp -d); trap 'pkill -f "$T/root" 2>/dev/null; [ -n "${KEEP:-}" ] || rm -rf "$T"' EXIT
run() { # $1 = mode env value ("" = unset), $2 = suffix; prints recorded modes
  local R=$T/root$2; rm -rf $R; mkdir -p $R/scripts $R/docs/sprint $R/.git $T/bin $T/pin/9.9.9.9/chrome-linux64
  cp scripts/sprint-next.py scripts/sprint-pack.py scripts/sprint-ledger.py scripts/sprint-graph.py $R/scripts/ 2>/dev/null
  cp "$SRC" $R/scripts/sprint-loop.sh
  [ "$MUT" = --mutant ] && python3 - $R/scripts/sprint-loop.sh <<'PY'
import sys,re
p=sys.argv[1]; s=open(p).read()
s=re.sub(r'  if \[ "\$PMODE" = alternate \]; then\n.*?\n  fi\n','',s,count=1,flags=re.S); open(p,'w').write(s)
PY
  echo base >$R/docs/sprint/PACK-BASE.md; echo 9.9.9.9 >$R/docs/sprint/CHROME_PIN; : >$R/docs/sprint/loop.log; : >$R/docs/sprint/JOURNAL.md
  printf 'id\tring\tpri\tmodel_floor\tdeps\tstatus\ttitle\tgate\tnotes\nX1\t0\t1\thaiku\t-\tTODO\tt\tg\tn\n' >$R/docs/sprint/GRAPH.tsv
  printf '#!/bin/sh\nexit 0\n' >$T/pin/9.9.9.9/chrome-linux64/chrome; chmod +x $T/pin/9.9.9.9/chrome-linux64/chrome
  touch $R/.git/sprint-chrome-checked
  printf '#!/bin/sh\nwhile [ "${1#-}" != "$1" ]; do case "$1" in -p|--setenv) shift;; esac; shift; done\nexec "$@"\n' >$T/bin/systemd-run
  printf '#!/bin/sh\nexit 0\n' >$T/bin/curl
  cat >$T/bin/claude <<STUB
#!/bin/sh
for a; do p="\$a"; done
case "\$p" in *"Read docs/research/WAVE3-SYNTHESIS.md"*) echo full;; *"Read ONLY .git/sprint-pack"*) echo pack;; *) echo other;; esac >>$T/prompts$2
echo '{"type":"result","is_error":false,"result":"r"}'
exit 1
STUB
  chmod +x $T/bin/*
  ( if [ -n "$1" ]; then export SPRINT_PROMPT_MODE=$1; else unset SPRINT_PROMPT_MODE; fi
    PATH=$T/bin:$PATH PIN_HOME=$T/pin MIN_FREE_GB=0 SPRINT_SLEEP=1 XDG_RUNTIME_DIR=$T bash $R/scripts/sprint-loop.sh >/dev/null 2>&1 ) &
  for _ in $(seq 90); do [ "$(wc -l <$T/prompts$2 2>/dev/null || echo 0)" -ge 4 ] && break; sleep 1; done
  pkill -f "$R/scripts/sprint-loop.sh" 2>/dev/null
  head -4 $T/prompts$2 2>/dev/null | tr '\n' ' '
}
alt=$(run alternate A); unset_=$(run "" B)
echo "alternate=[$alt] unset=[$unset_]"
[ "$alt" = "full pack full pack " ] && [ "$unset_" = "pack pack pack pack " ] && { echo "ok   alternate -> full,pack,full,pack; unset -> all pack"; exit 0; }
echo "FAIL modes wrong"; exit 1
