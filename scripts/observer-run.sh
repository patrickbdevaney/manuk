#!/usr/bin/env bash
# ── OBSERVER-RUN — run an OBSERVER command inside a memory cap.
#
# WHY THIS EXISTS. On 2026-07-19 03:15 the machine hard-OOMed and rebooted, killing the terminal and
# the tick loop. The kernel log named the culprit exactly:
#
#   task_memcg=/user.slice/.../app-org.gnome.Terminal.slice/vte-spawn-....scope, task=ld
#   Out of memory: Killed process (ld) anon-rss:2083168kB — global_oom, constraint=CONSTRAINT_NONE
#
# That cgroup is the OBSERVER'S TERMINAL, not the agent's. The agent was contained the whole time
# (`run-*.scope`, memory.max=24G, oom_kill=0) and behaved. I ran `cargo build --release --workspace`
# plus a full `verify.sh` UNCAPPED alongside it: the agent legitimately reserves 24G of 31G, leaving
# ~7G, and parallel `ld` walked straight through that and took the box down.
#
# So all the containment work protected the machine FROM THE AGENT and left the observer as the one
# unguarded path to a global OOM. This closes it. Heavy observer work (cargo build, verify.sh,
# fidelity sweeps, wpt runs) goes through here or it does not run.
#
#   usage: scripts/observer-run.sh [--mem 6G] -- <command...>
#          scripts/observer-run.sh -- cargo build --release --workspace
set -uo pipefail
cd "$(dirname "$0")/.."

MEM="6G"
while [ $# -gt 0 ]; do
  case "$1" in
    --mem) MEM="$2"; shift 2 ;;
    --)    shift; break ;;
    *)     break ;;
  esac
done
[ $# -gt 0 ] || { echo "usage: scripts/observer-run.sh [--mem 6G] -- <command...>"; exit 2; }

# Headroom check BEFORE launching: if the agent is mid-build the box may already be committed, and
# the right move is to wait rather than to squeeze in beside it and risk the whole machine again.
AVAIL=$(free -g | awk '/^Mem:/{print $7}')
BUILD_ACTIVE=0
if pgrep -x rustc >/dev/null 2>&1 || pgrep -x cargo >/dev/null 2>&1; then BUILD_ACTIVE=1; fi
echo "▶ observer-run: cap=${MEM} · ${AVAIL}G available · agent build active=${BUILD_ACTIVE}"
if [ "$BUILD_ACTIVE" = 1 ] && [ "${AVAIL:-0}" -lt 8 ]; then
  echo "✗ REFUSING: the agent is building and only ${AVAIL}G is available."
  echo "  Running a second heavy job here is what OOMed the box at 03:15 and rebooted it mid-tick."
  echo "  Wait for the agent's build to finish, or re-run with an explicit smaller --mem."
  exit 1
fi

if command -v systemd-run >/dev/null 2>&1 && \
   systemd-run --user --scope --quiet -p MemoryMax="$MEM" -p MemorySwapMax=2G -p OOMPolicy=kill \
     "$@"; then
  exit 0
fi
rc=$?
# If the scope could not be created, do NOT silently fall back to running uncapped — that is exactly
# the failure mode (`systemd-run unavailable — launching UNCONTAINED`) that preceded an earlier hang.
if [ "$rc" -ne 0 ] && ! systemd-run --user --scope --quiet -p MemoryMax="$MEM" true 2>/dev/null; then
  echo "✗ systemd-run unavailable — NOT falling back to an uncapped run (that is how the box hangs)."
  echo "  Run the command manually if you accept the risk."
  exit 1
fi
exit "$rc"
