#!/usr/bin/env bash
# W6.H1 live gate: a real winit loop under Xvfb, real X key events. The scripted agent calls the MCP
# `handoff` tool; the run must PAUSE (no finish) until the human presses Ctrl+Shift+R, then finish.
# usage: scripts/live_handoff_gate.sh [noresume]   (noresume = the mutation: the human never resumes)
set -uo pipefail
cd "$(dirname "$0")/.."
MODE=${1:-resume}
BIN=target/release/manuk
FIX=$PWD/tests/fixtures/live_agent_page.html
D=:98
Xvfb $D -screen 0 1024x768x24 >/dev/null 2>&1 & XP=$!
trap 'kill $BIN_PID $XP 2>/dev/null' EXIT
sleep 1
export RUST_LOG=info,wgpu=error,winit=error,naga=error DISPLAY=$D MANUK_AGENT_FIXTURE=2:100:handoff
LOG=$(mktemp)
$BIN browse "file://$FIX" >"$LOG" 2>&1 & BIN_PID=$!
sleep 4
python3 -I - "$MODE" <<'PY'
import ctypes, sys, time
x11 = ctypes.CDLL('libX11.so.6'); xt = ctypes.CDLL('libXtst.so.6')
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XKeysymToKeycode.restype = ctypes.c_ubyte
d = ctypes.c_void_p(x11.XOpenDisplay(None))
def kc(n): return x11.XKeysymToKeycode(d, ctypes.c_ulong(x11.XStringToKeysym(n.encode())))
def key(name, mods=()):
    for m in mods: xt.XTestFakeKeyEvent(d, kc(m), 1, 0)
    xt.XTestFakeKeyEvent(d, kc(name), 1, 0); xt.XTestFakeKeyEvent(d, kc(name), 0, 0)
    for m in reversed(mods): xt.XTestFakeKeyEvent(d, kc(m), 0, 0)
    x11.XFlush(d); time.sleep(0.15)
root = x11.XDefaultRootWindow(d)
rr, pr = ctypes.c_ulong(), ctypes.c_ulong(); kids = ctypes.POINTER(ctypes.c_ulong)(); n = ctypes.c_uint()
x11.XQueryTree(d, ctypes.c_ulong(root), ctypes.byref(rr), ctypes.byref(pr), ctypes.byref(kids), ctypes.byref(n))
for i in range(n.value): x11.XSetInputFocus(d, ctypes.c_ulong(kids[i]), 2, 0)
xt.XTestFakeMotionEvent(d, 0, 500, 400, 0); x11.XFlush(d); time.sleep(0.3)
key('j', ['Control_L'])
for c in 'go': key(c)
key('Return')
time.sleep(2.5)           # the agent has called handoff and is waiting on the human
open('/tmp/live-handoff.mid', 'w').write('x')
if sys.argv[1] == 'resume':
    key('r', ['Control_L', 'Shift_L'])
time.sleep(3)
PY
sleep 1
[ -n "${DEBUG:-}" ] && tail -20 "$LOG"
python3 -I - "$MODE" "$LOG" <<'PY'
import sys
mode, log = sys.argv[1:3]
t = open(log, errors='replace').read()
finished = '[agent] fixture done' in t
print(f"mode={mode} finished={finished}")
if mode == 'resume':
    sys.exit(0 if finished else 1)
sys.exit(0 if not finished else 1)   # noresume: the run must still be waiting on the human
PY
