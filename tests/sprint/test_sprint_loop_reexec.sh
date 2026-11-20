#!/usr/bin/env bash
# gate for W6.L1.a.a.a.a: sprint-loop.sh re-execs itself between attempts when its own file changed. Builds a fake
# repo root in a temp dir with a COPY of the loop, a stub `claude` that records its argv and (on attempt 1) edits the
# copy from `--output-format json` to `stream-json`, then checks attempt 2 recorded stream-json.
#   test_sprint_loop_reexec.sh            -> real loop must re-exec (green)
#   test_sprint_loop_reexec.sh --mutant   -> loop with the re-exec block stripped must be RED (proves the gate can fail)
set -u; cd "$(dirname "$0")/../.."; SRC=$PWD/scripts/sprint-loop.sh; MUT=${1:-}
T=$(mktemp -d); trap 'kill $LPID 2>/dev/null; pkill -f "$T/root/scripts/sprint-loop.sh" 2>/dev/null; [ -n "${KEEP:-}" ] || rm -rf "$T"' EXIT
R=$T/root; mkdir -p $R/scripts $R/docs/sprint $R/.git $T/bin $T/pin/9.9.9.9/chrome-linux64
cp scripts/sprint-next.py scripts/sprint-pack.py scripts/sprint-ledger.py scripts/sprint-graph.py $R/scripts/ 2>/dev/null
cp "$SRC" $R/scripts/sprint-loop.sh
sed -i 's/--output-format stream-json/--output-format json/' $R/scripts/sprint-loop.sh
[ "$MUT" = --mutant ] && sed -i '/REEXEC-BEGIN/,/REEXEC-END/d' $R/scripts/sprint-loop.sh
echo 9.9.9.9 >$R/docs/sprint/CHROME_PIN; : >$R/docs/sprint/loop.log; : >$R/docs/sprint/JOURNAL.md
printf 'id\tring\tpri\tmodel_floor\tdeps\tstatus\ttitle\tgate\tnotes\nX1\t0\t1\thaiku\t-\tTODO\tt\tg\tn\n' >$R/docs/sprint/GRAPH.tsv
printf '#!/bin/sh\nexit 0\n' >$T/pin/9.9.9.9/chrome-linux64/chrome; chmod +x $T/pin/9.9.9.9/chrome-linux64/chrome
touch $R/.git/sprint-chrome-checked
# stubs: systemd-run drops its options and runs the command; curl always succeeds
printf '#!/bin/sh\nwhile [ "${1#-}" != "$1" ]; do case "$1" in -p|--setenv) shift;; esac; shift; done\nexec "$@"\n' >$T/bin/systemd-run
printf '#!/bin/sh\nexit 0\n' >$T/bin/curl
cat >$T/bin/claude <<STUB
#!/bin/sh
echo "\$*" >>$T/argv
if [ ! -f $T/edited ]; then touch $T/edited; sed -i 's/--output-format json/--output-format stream-json/' $R/scripts/sprint-loop.sh; fi
echo '{"type":"result","is_error":false,"result":"r"}'
exit 1
STUB
chmod +x $T/bin/*
PATH=$T/bin:$PATH PIN_HOME=$T/pin MIN_FREE_GB=0 SPRINT_SLEEP=1 XDG_RUNTIME_DIR=$T bash $R/scripts/sprint-loop.sh >/dev/null 2>&1 &
LPID=$!
for _ in $(seq 60); do [ "$(wc -l <$T/argv 2>/dev/null || echo 0)" -ge 2 ] && break; sleep 1; done
n=$(wc -l <$T/argv 2>/dev/null || echo 0)
a1=$(sed -n 1p $T/argv | grep -o 'output-format [a-z-]*'); a2=$(sed -n 2p $T/argv | grep -o 'output-format [a-z-]*')
echo "attempts=$n attempt1=[$a1] attempt2=[$a2]"
if [ "$n" -ge 2 ] && [ "$a1" = "output-format json" ] && [ "$a2" = "output-format stream-json" ]; then echo "ok   loop re-exec'd onto the edited file"; exit 0; fi
echo "FAIL running loop did not pick up the edited file"; exit 1
