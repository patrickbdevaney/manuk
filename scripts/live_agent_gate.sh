#!/usr/bin/env bash
# W3.17.a live-window gate: a real winit loop under Xvfb, driven by real X key events (XTest), a
# scripted 10-step agent run. Mode "run" = full run; "esc" = Esc pressed mid-run.
# usage: scripts/live_agent_gate.sh run|esc [out.png]
set -uo pipefail
cd "$(dirname "$0")/.."
MODE=${1:-run}; PNG=${2:-/tmp/live-agent.png}
BIN=target/release/manuk
FIX=$PWD/tests/fixtures/live_agent_page.html
D=:97
Xvfb $D -screen 0 1024x768x24 >/dev/null 2>&1 & XP=$!
trap 'kill $BIN_PID $XP 2>/dev/null' EXIT
sleep 1
export RUST_LOG=info,wgpu=error,winit=error,naga=error DISPLAY=$D MANUK_AGENT_FIXTURE=10:200
LOG=$(mktemp)
$BIN browse "file://$FIX" >"$LOG" 2>&1 & BIN_PID=$!
sleep 4
python3 -I - "$MODE" "$PNG" <<'PY'
import ctypes, sys, time, subprocess
x11 = ctypes.CDLL('libX11.so.6'); xt = ctypes.CDLL('libXtst.so.6')
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XKeysymToKeycode.restype = ctypes.c_ubyte
d = ctypes.c_void_p(x11.XOpenDisplay(None))
def key(name, mods=()):
    ks = x11.XStringToKeysym(name.encode())
    kc = x11.XKeysymToKeycode(d, ctypes.c_ulong(ks))
    for m in mods: xt.XTestFakeKeyEvent(d, x11.XKeysymToKeycode(d, ctypes.c_ulong(x11.XStringToKeysym(m.encode()))), 1, 0)
    xt.XTestFakeKeyEvent(d, kc, 1, 0); xt.XTestFakeKeyEvent(d, kc, 0, 0)
    for m in reversed(mods): xt.XTestFakeKeyEvent(d, x11.XKeysymToKeycode(d, ctypes.c_ulong(x11.XStringToKeysym(m.encode()))), 0, 0)
    x11.XFlush(d); time.sleep(0.15)
# No window manager under Xvfb: focus the browser's top-level window ourselves.
root = x11.XDefaultRootWindow(d)
rr, pr = ctypes.c_ulong(), ctypes.c_ulong(); kids = ctypes.POINTER(ctypes.c_ulong)(); n = ctypes.c_uint()
x11.XQueryTree(d, ctypes.c_ulong(root), ctypes.byref(rr), ctypes.byref(pr), ctypes.byref(kids), ctypes.byref(n))
for i in range(n.value): x11.XSetInputFocus(d, ctypes.c_ulong(kids[i]), 2, 0)
xt.XTestFakeMotionEvent(d, 0, 500, 400, 0); x11.XFlush(d); time.sleep(0.3)
mode, png = sys.argv[1:3]
key('j', ['Control_L'])
for c in 'go': key(c)
key('Return')
time.sleep(0.7)
subprocess.run(f"xwd -root -silent | python3 -I -c \"import sys;open('{png}.xwd','wb').write(sys.stdin.buffer.read())\"", shell=True)
if mode == 'esc':
    key('Escape')
time.sleep(3.5)
PY
sleep 1
[ -n "${DEBUG:-}" ] && tail -20 "$LOG"
python3 -I scripts/live_agent_check.py "$MODE" "$PNG.xwd" "$LOG"
