#!/usr/bin/env bash
# sprint-loop.sh — the graph-directed product-sprint loop (docs/research/PRODUCT-SPRINT.md).
#
# Runs on the RTX 3090 desktop, BESIDE its other work (GPU asset farm, judge, crons): every agent runs in
# a low-priority, memory-capped cgroup so the farm always wins the CPU and the box can never OOM.
#
# One invocation = one graph node. The node's `model` column picks the model (opus for design-heavy
# nodes, sonnet for mechanical ones). The agent flips the node's status, lands it, may append child
# nodes, and exits; the loop then picks the next ready node.
#
#   start:  nohup ./scripts/sprint-loop.sh >/dev/null 2>&1 &
#   pause:  touch .git/sprint-DISABLED     resume: rm .git/sprint-DISABLED
#   log:    docs/sprint/loop.log (untracked)
set -uo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
KILL=.git/sprint-DISABLED
WORKING=.git/sprint-working
LOG=docs/sprint/loop.log
STALL_SECS=${STALL_SECS:-2700}      # no `touch $WORKING` for 45 min → hung, kill the tree
MIN_FREE_GB=${MIN_FREE_GB:-30}      # pause before the build can fill /home (shared with the farm)
CLAUDE=$(command -v claude 2>/dev/null || echo "$HOME/.local/bin/claude")

exec 9>/tmp/manuk-sprint-loop.lock
flock -n 9 || { echo "sprint-loop already running"; exit 0; }

say() { echo "$(date '+%F %T')  $*" >>"$LOG"; }
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
export DBUS_SESSION_BUS_ADDRESS=${DBUS_SESSION_BUS_ADDRESS:-unix:path=$XDG_RUNTIME_DIR/bus}

model_id() { case "$1" in opus) echo claude-opus-5-5 ;; sonnet) echo claude-sonnet-5-5 ;; haiku) echo claude-haiku-5-5 ;; *) echo "$1" ;; esac; }

# ── model escalation: the node's `model_floor` is the CHEAPEST model tried; every attempt that exits without
# landing the node moves one tier up the ladder. Opus gets two attempts, then the node is BLOCKED.
LADDER=(haiku sonnet opus)
tier_of() { local i; for i in "${!LADDER[@]}"; do [ "${LADDER[$i]}" = "$1" ] && { echo "$i"; return; }; done; echo 1; }

# ── oracle Chrome pin: docs/sprint/CHROME_PIN names a Chrome-for-Testing version; the oracle resolves Chrome by
# NAME on PATH (tests/wpt/src/chrome.rs, agent a11y-score), so shims ahead of /usr/bin pin it without touching
# engine code. The system Chrome stays whatever apt updates it to.
PIN_HOME=$HOME/.local/chrome-cft
ensure_chrome_pin() {
  local ver; ver=$(tr -d '[:space:]' <docs/sprint/CHROME_PIN)
  local dir=$PIN_HOME/$ver bin=$PIN_HOME/$ver/chrome-linux64/chrome
  if [ ! -x "$bin" ]; then
    say "installing pinned oracle Chrome $ver"
    mkdir -p "$dir" && curl -fsSL -o "$dir/c.zip" \
      "https://storage.googleapis.com/chrome-for-testing-public/$ver/linux64/chrome-linux64.zip" \
      && (cd "$dir" && unzip -q -o c.zip && rm c.zip) || { say "⚠ Chrome pin $ver download failed"; return 1; }
  fi
  # Chrome blocks forever on the GNOME keyring from a headless/ssh session (cookie-store encryption), so the pinned
  # binary is wrapped with --password-store=basic. Its sandbox needs /etc/apparmor.d/chrome-cft (userns) on Ubuntu.
  if [ ! -f "$bin.real" ]; then
    mv "$bin" "$bin.real"
    printf '#!/bin/sh\nexec %s.real --password-store=basic "$@"\n' "$bin" >"$bin"; chmod +x "$bin"
  fi
  mkdir -p "$PIN_HOME/bin"
  for n in google-chrome-stable google-chrome chromium chromium-browser; do ln -sfn "$bin" "$PIN_HOME/bin/$n"; done
  export PATH="$PIN_HOME/bin:$PATH" MANUK_CHROME_PIN="$ver"
}

# ── Chrome drift: once a week, if a newer stable MAJOR exists than the pin, append a re-pin node (once per major).
# The node re-baselines on BOTH versions before the pin moves, so no number is ever compared across versions.
check_chrome_drift() {
  local stamp=.git/sprint-chrome-checked
  [ -f "$stamp" ] && [ $(( $(date +%s) - $(stat -c %Y "$stamp") )) -lt 604800 ] && return
  touch "$stamp"
  local latest; latest=$(curl -fsSL https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions.json \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["channels"]["Stable"]["version"])' 2>/dev/null) || return
  local pin; pin=$(tr -d '[:space:]' <docs/sprint/CHROME_PIN)
  [ "${latest%%.*}" -gt "${pin%%.*}" ] 2>/dev/null || return
  grep -q "^CHROME.${latest%%.*}	" docs/sprint/GRAPH.tsv && return
  printf 'CHROME.%s\t4\t9\tsonnet\tCHROME.PIN\tTODO\tRe-pin oracle Chrome %s → %s: install it beside the pin, run the trend fidelity + a11y sweeps and the agent-task Chrome arm on BOTH (3 runs each), record per-metric deltas in docs/sprint/CHROME-REPIN.md, then move docs/sprint/CHROME_PIN and re-baseline SCOREBOARD\tdeltas table exists with 3-run bands for both versions; every row after the move carries the new chrome_ver; old rows untouched\tappended automatically by sprint-loop.sh on stable drift\n' \
    "${latest%%.*}" "$pin" "$latest" >>docs/sprint/GRAPH.tsv
  say "Chrome stable $latest > pin $pin — appended CHROME.${latest%%.*} re-pin node"
}

watchdog() {  # kill the agent's tree if the working flag goes stale (hung, not slow)
  local pid=$1
  while kill -0 "$pid" 2>/dev/null; do
    sleep 60
    local age=$(( $(date +%s) - $(stat -c %Y "$WORKING" 2>/dev/null || date +%s) ))
    if [ "$age" -ge "$STALL_SECS" ]; then
      say "⏰ working flag stale ${age}s — killing agent scope (pid $pid)"
      # the scope's cgroup holds claude AND every cargo/rustc grandchild; cgroup.kill takes them all
      local cg; cg=$(awk -F: '$1=="0"{print $3}' "/proc/$pid/cgroup" 2>/dev/null)
      if [ -n "$cg" ] && [ -w "/sys/fs/cgroup$cg/cgroup.kill" ]; then echo 1 >"/sys/fs/cgroup$cg/cgroup.kill"
      else kill -KILL "$pid" 2>/dev/null; fi
      return
    fi
  done
}

say "sprint-loop started (pid $$)"
while :; do
  if [ -f "$KILL" ]; then sleep 60; continue; fi
  free_gb=$(df -BG --output=avail /home | tail -1 | tr -dc 0-9)
  if [ "$free_gb" -lt "$MIN_FREE_GB" ]; then say "⚠ /home free ${free_gb}G < ${MIN_FREE_GB}G — paused"; sleep 600; continue; fi

  ensure_chrome_pin || { sleep 600; continue; }
  check_chrome_drift

  next=$(python3 scripts/sprint-next.py)
  if [ "$next" = NONE ]; then say "graph has no ready node — idle (add nodes or unblock one)"; sleep 900; continue; fi
  case "$next" in
    BADGRAPH*) say "⚠ GRAPH.tsv is malformed (${next#BADGRAPH }) — paused until fixed"; sleep 600; continue ;;
    *$'\t'*) ;;
    *) say "⚠ sprint-next.py gave no node (${next:-empty output}) — paused"; sleep 600; continue ;;
  esac
  node=${next%%$'\t'*}; floor=${next##*$'\t'}
  attempts=$(grep -c "■ node $node exited" "$LOG" 2>/dev/null || true); attempts=${attempts:-0}
  ft=$(tier_of "$floor"); t=$(( ft + attempts )); [ "$t" -gt 2 ] && t=2
  if [ "$attempts" -ge $(( 2 - ft + 2 )) ]; then
    python3 - "$node" <<'EOF'
import csv, sys
from pathlib import Path
p = Path("docs/sprint/GRAPH.tsv"); rows = list(csv.reader(p.open(), delimiter="\t"))
for r in rows[1:]:
    if r[0] == sys.argv[1]:
        r[5] = "BLOCKED"; r[8] = (r[8] + " | auto-BLOCKED: failed on every tier up to opus").strip(" |")
csv.writer(p.open("w", newline=""), delimiter="\t", lineterminator="\n").writerows(rows)
EOF
    say "⛔ node $node auto-BLOCKED after $attempts attempts (floor $floor → opus)"; continue
  fi
  model=$(model_id "${LADDER[$t]}")

  PROMPT="You are the headless product-sprint agent for manuk (no user to hand back to). Your ONE job this invocation is graph node ${node} in docs/sprint/GRAPH.tsv. Read docs/research/WAVE3-SYNTHESIS.md (the NORTH STAR), docs/research/PRODUCT-SPRINT.md (the plan), docs/sprint/SUCCESS.md (the pre-registered bars — never edit its thresholds or tune tasks to results), docs/research/SYNTHESIS.md (decided facts — do not re-derive), CONSTITUTION.MD, and the node row (title, gate, notes). Steps: (1) set the node status to IN_PROGRESS (if it already is, a previous invocation crashed — inspect the tree, keep good WIP or 'git checkout -- .' and restart the node clean). (2) Do the work. A node is DONE only when its gate has been run and can go red (prove it with a mutation or a deliberately-broken input). (3) Land it: if the diff touches engine/ or tests/wpt, land via ./scripts/tick.sh (the full wall; never wrap it in 'timeout'); otherwise run cargo test for the touched crates + cargo build --release for touched binaries, then commit with an explicit pathspec (never git add -A), after checking git diff --cached, with a message starting '${node}:', using 'git commit --no-verify' (the pre-commit hook enforces the old tick journal, which sprint nodes outside the wall do not write — so the secret scan below is mandatory); push to origin main. (4) Set status DONE (or BLOCKED with the reason in notes) and append one line to docs/sprint/JOURNAL.md: '${node}: <what now works, in user-visible terms> | gate: <the check> | evidence: <numbers or transcript excerpt>'. (5) If the work revealed necessary follow-ups, APPEND new rows to GRAPH.tsv (ids like ${node}.a, deps set, a falsifiable gate each, model_floor = the CHEAPEST model you judge able to land it: haiku for mechanical runs/measurements/docs, sonnet otherwise; the loop escalates on failure, so never pick opus as a floor) — the graph grows; never delete rows. Every measurement row you write carries chrome_ver=\$MANUK_CHROME_PIN; never diff numbers taken under different chrome_ver. Rules: touch .git/sprint-working at the top of every command so the watchdog sees you alive. Builds use the CARGO_BUILD_JOBS you were given; never start a second concurrent cargo build. Never commit .env, keys or tokens; before every commit run: git diff --cached | grep -niE 'ghp_|api_key|bearer|sk-[a-z0-9]' and abort on a hit. Claims in docs must come from measurements (no 'faster'/'lighter' without a SCOREBOARD row). Do not tag releases or publish anything beyond pushing commits. When the node is DONE or BLOCKED, exit."

  say "▶ node $node on $model"
  touch "$WORKING"
  systemd-run --user --scope --quiet \
      -p MemoryMax=28G -p MemoryHigh=24G -p MemorySwapMax=4G -p OOMPolicy=kill \
      -p CPUWeight=20 -p IOWeight=20 \
      --setenv=CARGO_BUILD_JOBS=16 --setenv=WPT_DIR="$HOME/wpt" --setenv=MANUK_SPRINT_NODE="$node" \
      nice -n 10 "$CLAUDE" --model "$model" --dangerously-skip-permissions --permission-mode bypassPermissions \
      -p "$PROMPT" >>"$LOG" 2>&1 &
  apid=$!
  watchdog "$apid" &
  wd=$!
  wait "$apid"; rc=$?
  kill "$wd" 2>/dev/null
  # only a FAILED attempt is logged as '■ node X exited' — that line is the escalation counter
  st=$(awk -F'\t' -v id="$node" '$1==id{print $6}' docs/sprint/GRAPH.tsv)
  if [ "$st" = DONE ]; then say "✓ node $node DONE on ${LADDER[$t]} (rc=$rc)"
  elif [ "$st" = BLOCKED ]; then say "⛔ node $node BLOCKED by the agent on ${LADDER[$t]}"
  else say "■ node $node exited rc=$rc on ${LADDER[$t]} without landing — next attempt escalates"; fi
  sleep 30
done
