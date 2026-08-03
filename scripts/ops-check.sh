#!/usr/bin/env bash
# ops-check.sh — VERIFY THE OPERATIONAL INVARIANTS of the loop substrate, and auto-heal the safe ones.
#
# docs/loop/RELIABILITY-DOCTRINE.md Category B ("the substrate's state is ASSUMED, not VERIFIED"). Every
# paid-for operational incident — dead crons, disk creep, stale systemd scopes, uncontained/double-spawned
# agent, a poisoned wall-bank — becomes a STANDING checked invariant instead of a thing the observer
# happens to notice. This is the "verify every post-condition, assume nothing" half of the doctrine, made
# mechanical. Run by cron (*/15) AND by every observer heartbeat.
#
# FAIL-SAFE BY CONSTRUCTION — a bug in this file must NEVER touch the loop:
#   * it only READS, performs a WHITELIST of provably-safe auto-heals, and ALERTS to a log;
#   * it NEVER kills the live agent or supervisor, NEVER pkills by pattern, NEVER edits engine/ / STATUS /
#     the tick flow, NEVER re-baselines the wall (that is an observer judgement — it only ALERTS);
#   * every action is best-effort and it always `exit 0`, so ops-check can never fail a tick or a cron.
set -uo pipefail
cd "$(dirname "$0")/.." 2>/dev/null || exit 0
LOG=.git/manuk-ops-check.log
now(){ date -u +%H:%M:%S 2>/dev/null || echo "--:--:--"; }
note(){ printf '%s %s\n' "$(now)" "$*" >> "$LOG" 2>/dev/null; [ -t 2 ] && printf '  %s\n' "$*" >&2; }
heal=0; alert=0
grind_pids(){ for p in $(ps -C claude -o pid= 2>/dev/null); do tr '\0' ' ' </proc/"$p"/cmdline 2>/dev/null | grep -q 'model claude-opus' && echo "$p"; done; }
scope_of(){ grep -oE 'run-r[0-9a-f]+\.scope' /proc/"$1"/cgroup 2>/dev/null | head -1; }

# 1. CRON INTEGRITY — a `crontab -l | … | crontab -` round-trip once double-escaped the quotes, so cron
#    ran `bash -lc \'` and died with "unexpected EOF" EVERY fire; hygiene + the watchdog were dead for
#    hours, silently. AUTO-HEAL: un-escape \' back to '. Then confirm the two critical crons are present.
CT=$(crontab -l 2>/dev/null || true)
nesc=$(printf '%s' "$CT" | grep -c "\\\\'" 2>/dev/null); nesc=${nesc:-0}
if [ -n "$CT" ] && [ "$nesc" -ne 0 ]; then
  if printf '%s' "$CT" | sed "s/\\\\'/'/g" | crontab - 2>/dev/null; then note "HEALED: un-escaped corrupted crontab quotes (the silent-cron-death bug)"; heal=$((heal+1)); fi
fi
for c in disk-hygiene.sh loop-watchdog.sh; do
  printf '%s' "$CT" | grep -q "$c" || { note "ALERT: cron '$c' is MISSING from the crontab"; alert=$((alert+1)); }
done

# 2. HYGIENE LIVENESS — it prints 'now NNG free' every run; a stale log means the reaper is dead and disk
#    creeps monotonically. (>90m stale is well past its */3-min cron.)
if [ -f .git/manuk-hygiene.log ]; then
  age=$(( $(date +%s 2>/dev/null || echo 0) - $(stat -c %Y .git/manuk-hygiene.log 2>/dev/null || echo 0) ))
  [ "$age" -gt 5400 ] && { note "ALERT: disk-hygiene log stale ${age}s (>90m) — the reaper may be dead"; alert=$((alert+1)); }
fi

# 3. DISK — df on /HOME (the repo's mount). `df /` once produced a wrong-mount misdiagnosis that burned a
#    10-minute verify; always measure the repo's own mount.
DP=$(df --output=pcent /home 2>/dev/null | tail -1 | tr -dc '0-9' || echo 0)
# Alert at 93%, not 92%: disk safely hovers at ~92% (the 86G debug tree is the LIVE gate suite, not
# reclaimable cruft — hygiene is NOT behind). 93% is the real action point: status-update stops banking the
# wall at disk>=93 (wall-bank-guard), so THAT is when to act (scheduled cold-clean on an idle box, or nextest).
[ "${DP:-0}" -ge 93 ] && { note "ALERT: /home is ${DP}% used (>=93%) — wall-bank guard territory; cold-clean target/debug on an idle box or flag it"; alert=$((alert+1)); }

# 4. systemd HEALTH + STALE-SCOPE AUTO-CLEAN. Empty claude scopes accumulate on every agent relaunch (and
#    en masse a 'degraded' systemd is what forces the UNCONTAINED fallback). reset-failed + stop the empties.
#    (systemctl --user needs the user session; from cron it may no-op — that is fine, the observer run does it.)
if command -v systemctl >/dev/null 2>&1; then
  st=$(systemctl --user is-system-running 2>/dev/null || echo unknown)
  if [ "$st" != "running" ] && [ "$st" != "unknown" ]; then systemctl --user reset-failed 2>/dev/null && { note "HEALED: systemd --user was '$st' → reset-failed"; heal=$((heal+1)); }; fi
  LIVE=""; for p in $(grind_pids); do LIVE=$(scope_of "$p"); [ -n "$LIVE" ] && break; done
  for s in $(systemctl --user list-units --type=scope --no-legend 2>/dev/null | awk '/claude/{print $1}'); do
    [ "$s" = "$LIVE" ] && continue
    cg="/sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/app.slice/$s/cgroup.procs"
    if [ -z "$(cat "$cg" 2>/dev/null)" ]; then systemctl --user stop "$s" 2>/dev/null && { note "HEALED: stopped empty stale scope $s (superseded agent launch)"; heal=$((heal+1)); }; fi
  done
fi

# 5. AGENT: SINGLE + CONTAINED. Double-spawn (a resurrection daemon launching a duplicate) and an
#    uncontained agent (systemd-run unavailable at launch → OOM/machine-hang risk) are both ALERT-ONLY:
#    bouncing the live agent is disruptive and must be done at a wall-free moment — the observer's call,
#    never automated here.
NG=$(grind_pids | wc -l | tr -dc '0-9'); NG=${NG:-0}
if [ "$NG" -gt 1 ]; then note "ALERT: ${NG} grind agents running (DOUBLE-SPAWN?) — observer must investigate by PID"; alert=$((alert+1)); fi
if [ "$NG" -eq 1 ]; then
  gp=$(grind_pids | head -1)
  [ -z "$(scope_of "$gp")" ] && { note "ALERT: grind agent $gp is UNCONTAINED — observer bounce at a wall-free moment (systemd recovered ⇒ relaunch is contained)"; alert=$((alert+1)); }
fi
if [ "$NG" -eq 0 ]; then
  pgrep -f 'bash .*/scripts/loop-forever.sh' >/dev/null 2>&1 || { note "ALERT: NO grind agent AND NO supervisor — the loop is DOWN"; alert=$((alert+1)); }
fi

# 6. WALL-BANK STALENESS (never auto-fix; ALERT). A poisoned LAST_WALL_TIME (banked high under load) makes
#    the ratchet refuse every later tick. If the LAST receipt was green on a QUIET box (load<1.5) yet its
#    real seconds are <½ the banked wall, the bank is stale-high and needs an observer re-baseline.
WB=$(grep -oP '^LAST_WALL_TIME:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
RS=$(grep -oP '^seconds:\s*\K[0-9]+' .git/manuk-verify-receipt 2>/dev/null || echo 0)
RL=$(grep -oP '^load1:\s*\K[0-9.]+' .git/manuk-verify-receipt 2>/dev/null | awk '{printf "%d",$1*10}' || echo 99)
RR=$(grep -oP '^result:\s*\K\w+' .git/manuk-verify-receipt 2>/dev/null || echo "?")
if [ "$RR" = green ] && [ "${RL:-99}" -lt 15 ] && [ "${WB:-0}" -gt 0 ] && [ "${RS:-0}" -gt 0 ] && [ $(( RS * 2 )) -lt "${WB:-0}" ]; then
  note "ALERT: LAST_WALL_TIME=${WB}s but a QUIET green receipt ran in ${RS}s — the bank is stale-high; re-baseline by hand"; alert=$((alert+1))
fi

# 7. MAP RECONCILIATION (RELIABILITY-DOCTRINE Cat A §3&4) — the capability map must stay backed by real
#    gates. ALERT, never block: making this a wall gate risks bricking every tick on an instrument false
#    positive (the tests/ blind spot proved they exist). Observer acts on the alert — real drift ⇒ steer the
#    agent to cite a gate / set status; false drift ⇒ fix map-reconcile's search. This is reconciliation
#    made STANDING (the doctrine's goal) without the wall-brick risk.
if [ -x scripts/map-reconcile.sh ]; then
  # Judge the COMMITTED map (HEAD), not the worktree — else this fires on the agent's transient mid-tick
  # map edits (WIP), which is noise. A standing alert should flag only LANDED drift. Gate search stays
  # worktree (a committed row citing a just-added gate is correctly backed).
  _hm=$(mktemp)
  if git show HEAD:docs/loop/CONSTELLATION.tsv > "$_hm" 2>/dev/null && [ -s "$_hm" ]; then
    _mout=$(MANUK_MAP="$_hm" bash scripts/map-reconcile.sh 2>/dev/null)
  else _mout=$(bash scripts/map-reconcile.sh 2>/dev/null); fi
  rm -f "$_hm"
  if printf '%s' "$_mout" | grep -q 'RECONCILED'; then MD=0     # drift 0 prints "RECONCILED", not "DRIFT TOTAL: 0"
  else MD=$(printf '%s' "$_mout" | grep -oE 'DRIFT TOTAL: [0-9]+' | grep -oE '[0-9]+' | head -1); fi
  [ "${MD:-0}" -gt 0 ] && { note "ALERT: map drift ${MD} row(s) in the COMMITTED map — a landed capability claims a gate that doesn't exist (real ⇒ steer agent; false ⇒ fix map-reconcile search)"; alert=$((alert+1)); }
fi

# 8. FIDELITY PROGRESS (user directive t685: scorability + coverage + placement/shape must keep climbing
#    to 95%). Record the freshest corpus sweep into the trend ledger and surface its flags: a STALE sweep
#    (the trend goes blind), a SCORABILITY REGRESSION, or the DENOMINATOR TRAP (coverage rose only because
#    a hard site dropped out). ALERT-only — fidelity has real run-to-run variance, so this is NEVER a gate.
if [ -x scripts/fidelity-progress.sh ]; then
  bash scripts/fidelity-progress.sh >/dev/null 2>&1 || true   # record-if-changed (safe append to its own ledger)
  fp=$(bash scripts/fidelity-progress.sh --check 2>/dev/null || true)
  if [ -n "$fp" ]; then
    while IFS= read -r _l; do [ -n "$_l" ] && { note "ALERT(fidelity): $_l"; alert=$((alert+1)); }; done <<< "$fp"
  fi
fi

# 8b. PHASE-0 MILESTONE STATE (owner-locked 2026-07-29: v1 render → v1 function → v2 re-cert, in order).
#     Durably surfaces WHICH milestone we are in + distance/slope, so the ordered path to Phase-0 cannot be
#     forgotten across relaunches/context resets. Informational — computed from the ledger, never a gate.
if [ -x scripts/phase0-milestones.sh ]; then
  ms=$(bash scripts/phase0-milestones.sh --oneline 2>/dev/null || true)
  [ -n "$ms" ] && note "$ms"
fi

# 8c. DEATH-TAIL (crossings-per-tick + reachability). Refreshes the derived ledger docs/loop/DEATH-TAIL.tsv
#     from the clean same-corpus burndown rows, surfaces the one-line bracket (linear vs decay ETA), and
#     ALERTS only when the death-tail SIGNAL is active (recent crossings-per-tick low AND decelerating) — the
#     early-warning that 95% may be unreachable without a fresh family-sized batch. Informational, never a gate.
if [ -x scripts/death-tail.sh ]; then
  bash scripts/death-tail.sh >/dev/null 2>&1 || true          # refresh the derived ledger
  dt=$(bash scripts/death-tail.sh --oneline 2>/dev/null || true)
  [ -n "$dt" ] && note "$dt"
  dta=$(bash scripts/death-tail.sh --check 2>/dev/null || true)
  if [ -n "$dta" ]; then
    while IFS= read -r _l; do [ -n "$_l" ] && { note "ALERT(death-tail): $_l"; alert=$((alert+1)); }; done <<< "$dta"
  fi
fi

# Summary line (the observer reads the tail of $LOG each heartbeat).
note "ops-check: ${heal} healed, ${alert} alert(s) [disk ${DP:-?}% · grind ${NG} · systemd ${st:-n/a} · mapdrift ${MD:-?}]"
[ "$alert" -gt 0 ] && exit 0   # alerts are informational; ops-check NEVER fails anything.
exit 0
