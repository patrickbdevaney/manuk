# The Loop Driver — how the tick loop keeps itself running

The autonomous loop is driven by two layers that compose. Both are gated by the same on-disk budget
(`docs/loop/AUTOLOOP`), which is the single source of truth and is editable at any time.

## Layer 1 — the Stop hook (inner loop, live session)

`scripts/loop-continue.sh`, wired as a Claude Code `Stop` hook in `.claude/settings.local.json`.

Fires the instant the agent finishes a turn. While the budget has ticks remaining it returns a `block`
decision whose `reason` re-instructs the agent to start/finish the next tick — so a *live* session never
hands back; it grinds tick after tick with zero latency and no polling. When `TICK` reaches
`LOOP_UNTIL_TICK` it allows the stop and the loop ends with a report.

It also `touch`es `.git/manuk-loop-heartbeat` on every fire — the liveness signal Layer 2 reads.

Weakness (by construction): a hook can only fire inside a living session. If the session dies — OOM, a
closed terminal, a reboot — nothing fires it. That is Layer 2's job.

## Layer 2 — the resurrection daemon (outer loop, survives death)

`scripts/loop-daemon.sh`, run by cron every 30 min (`crontab -l`).

Asks one question: *is a manuk loop alive right now, and is there budget left?* It reads two independent
liveness signals — the heartbeat file (touched by the Stop hook every turn; covers the interactive
session) and a launched-PID file (covers a fresh headless session before its first Stop). If either is
warm, it exits. Only when both are cold and budget remains does it relaunch a fresh, detached, headless
`claude` in the repo — which then self-continues via the Stop hook exactly as before. The cron only has to
*start* one; Layer 1 keeps it alive. A crash-loop guard (`MIN_GAP_MIN`) prevents relaunch storms.

## Controls

| Action | How |
| --- | --- |
| Set the budget (how many ticks to run) | `./scripts/autoloop.sh set <TICK>` or edit `docs/loop/AUTOLOOP` |
| See ticks remaining | `./scripts/autoloop.sh remaining` |
| **Stop the loop now (soft)** | `./scripts/autoloop.sh set <=current TICK>` — both layers see budget spent |
| **Stop the loop now (hard off-switch)** | `touch .git/manuk-loop-DISABLED` — both layers halt regardless of budget |
| Re-enable after a hard stop | `rm .git/manuk-loop-DISABLED` |
| Watch the daemon's decisions | `tail -f .git/manuk-loop-daemon.log` |
| Activate the hook in an already-open terminal | open `/hooks` once, or restart the session (see below) |

## The one caveat — the settings watcher

Claude Code only watches for `.claude/settings.local.json` in directories that already had a settings file
when the session started. Because this file was created mid-session, the **current** terminal will not
fire the Stop hook until you open `/hooks` once (which reloads config) or restart. **Every new session —
including every session the daemon launches — picks it up automatically.** So the durability machine is
fully live now; only the specific terminal it was authored in needs the one-time reload.

## Files (all machine-local, kept out of git via `.git/info/exclude`)

- `.claude/settings.local.json` — the Stop-hook wiring
- `.git/manuk-loop-heartbeat` — liveness timestamp
- `.git/manuk-loop.pid` — last headless launch PID
- `.git/manuk-loop-DISABLED` — hard off-switch (absent = enabled)
- `.git/manuk-loop-daemon.log` — the daemon's decision log
