#!/usr/bin/env bash
# ── AGENT-STREAM — render the grind agent's JSON action stream as readable terminal output.
#
# The headless agent already writes a rich JSONL event stream (one record per line: thinking / text /
# tool_use / tool_result) to ~/.claude/projects/-home-patrickd-manuk/*.jsonl. This tails the LIVE one
# and pretty-prints it so you can watch what the agent is doing without opening a 1.6MB json file.
#
#   scripts/agent-stream.sh                 # follow the live agent, colourised
#   scripts/agent-stream.sh --last 60       # last 60 events, then follow
#   scripts/agent-stream.sh --no-follow     # dump and exit (good for piping)
#   scripts/agent-stream.sh --thinking      # include the agent's reasoning blocks
#   scripts/agent-stream.sh --file PATH      # a specific transcript
#   scripts/agent-stream.sh --list          # list candidate transcripts and exit
set -uo pipefail
cd "$(dirname "$0")/.."
PROJ="$HOME/.claude/projects/-home-patrickd-manuk"

FILE=""; LAST=25; FOLLOW=1; THINKING=0
while [ $# -gt 0 ]; do
  case "$1" in
    --file)      FILE="$2"; shift 2 ;;
    --last)      LAST="$2"; shift 2 ;;
    --no-follow) FOLLOW=0; shift ;;
    --thinking)  THINKING=1; shift ;;
    --list)
      ls -t "$PROJ"/*.jsonl 2>/dev/null | while read -r f; do
        sz=$(du -h "$f" | cut -f1); when=$(stat -c '%y' "$f" | cut -c1-19)
        # the agent's transcript carries the grind-agent prompt; the observer's does not
        tag=$(head -c 4000 "$f" 2>/dev/null | grep -qm1 'Continue the autonomous Manuk tick loop NOW' && echo AGENT || echo other)
        printf '  %-6s %5s  %s  %s\n' "$tag" "$sz" "$when" "$(basename "$f")"
      done
      exit 0 ;;
    *) echo "unknown flag: $1 (see the header for usage)"; exit 2 ;;
  esac
done

# ── resolve the transcript. Default: the most-recently-written AGENT transcript (the one that holds
# the grind-agent prompt), NOT the observer's session, which also lives here and is often newer.
if [ -z "$FILE" ]; then
  FILE=$(ls -t "$PROJ"/*.jsonl 2>/dev/null | while read -r f; do
           head -c 4000 "$f" 2>/dev/null | grep -qm1 'Continue the autonomous Manuk tick loop NOW' && { echo "$f"; break; }; done)
fi
[ -z "$FILE" ] && FILE=$(ls -t "$PROJ"/*.jsonl 2>/dev/null | head -1)
[ -n "$FILE" ] && [ -f "$FILE" ] || { echo "✗ no transcript found in $PROJ"; exit 1; }
echo "▶ streaming $(basename "$FILE")  ($( [ "$FOLLOW" = 1 ] && echo 'following live — Ctrl-C to stop' || echo 'dump' ))"

RENDER='
import sys, json, datetime
THINK = __THINK__
C = {"dim":"\033[2m","b":"\033[1m","r":"\033[31m","g":"\033[32m","y":"\033[33m",
     "c":"\033[36m","m":"\033[35m","blue":"\033[34m","o":"\033[0m"}
def col(k,s): return C[k]+s+C["o"]
def clock(rec):
    t = rec.get("timestamp","")
    try: return datetime.datetime.fromisoformat(t.replace("Z","+00:00")).strftime("%H:%M:%S")
    except Exception: return "        "
def oneline(s, n=200):
    s = " ".join(str(s).split())
    return s if len(s)<=n else s[:n-1]+"…"
def emit(line):
    try: rec = json.loads(line)
    except Exception: return
    msg = rec.get("message") or {}
    content = msg.get("content")
    if not isinstance(content, list):  # user string prompts etc.
        if rec.get("type")=="user" and isinstance(content,str) and content.strip():
            print(col("dim", clock(rec)+"  » "+oneline(content,160)))
        return
    ts = clock(rec)
    for b in content:
        if not isinstance(b, dict): continue
        t = b.get("type")
        if t == "thinking":
            if THINK:
                tx = (b.get("thinking") or "").strip()
                if tx: print(col("dim", ts+"  ~ "+oneline(tx,200)))
        elif t == "text":
            tx = (b.get("text") or "").strip()
            if tx: print(ts+"  "+col("b","● ")+oneline(tx,240))
        elif t == "tool_use":
            name = b.get("name","?"); inp = b.get("input") or {}
            if name == "Bash":
                cmd = oneline(inp.get("command",""),200)
                desc = inp.get("description","")
                head = col("g", ts+"  $ ")+col("c",cmd)
                print(head + (col("dim","   # "+oneline(desc,60)) if desc else ""))
            elif name in ("Edit","Write","NotebookEdit"):
                print(col("y", ts+"  ✎ "+name+" ")+inp.get("file_path",""))
            elif name == "Read":
                print(col("blue", ts+"  ◂ Read ")+inp.get("file_path",""))
            elif name in ("Grep","Glob"):
                q = inp.get("pattern", inp.get("query",""))
                print(col("blue", ts+"  ⌕ "+name+" ")+oneline(q,80))
            else:
                print(col("m", ts+"  ⚙ "+name+" ")+oneline(json.dumps(inp),120))
        elif t == "tool_result":
            err = b.get("is_error")
            cont = b.get("content")
            if isinstance(cont, list):
                cont = " ".join(x.get("text","") for x in cont if isinstance(x,dict))
            cont = (cont or "").strip()
            first = oneline(cont.splitlines()[0] if cont else "", 160) if cont else ""
            n = len(cont.splitlines())
            tail = col("dim", f"  (+{n-1} lines)") if n>1 else ""
            if err:  print(col("r", ts+"  ✗ "+ (first or "error")) + tail)
            elif first: print(col("dim", ts+"  ⤷ "+first) + tail)
for line in sys.stdin:
    emit(line)
    sys.stdout.flush()
'
RENDER=${RENDER//__THINK__/$THINKING}

if [ "$FOLLOW" = 1 ]; then
  tail -n "$LAST" -f "$FILE" | python3 -c "$RENDER"
else
  tail -n "$LAST" "$FILE" | python3 -c "$RENDER"
fi
