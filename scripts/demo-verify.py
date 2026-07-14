#!/usr/bin/env python3
"""G_DEMO_LIVE — drive the wasm demo in a real browser and assert the engine actually ran.

**This gate exists because three separate measurements lied in a single tick (PROCESS #36).**

  1. `--virtual-time-budget` froze the clock, so every pipeline stage read "0ms" and I nearly went back
     into Rust that was already correct.
  2. `--dump-dom` fires at `load`, which does **not** wait for an async wasm boot — so it reported an
     engine that never ran, every time, no matter what.
  3. `--screenshot` waits *sometimes*. It caught the render once and missed it the next time, on the same
     code. A flaky observer is worse than none: it makes a working build look broken at random.

All three are the same defect wearing different clothes — **the instrument was blind to the thing it was
reporting as absent.** The fix is to stop inferring "did it run?" from a side-effect that happens to be
observable, and instead *ask the page*, over the DevTools protocol, after it has actually finished.

What it asserts, and why each one is the difference between a demo and a screenshot of a demo:

  * the boot placeholder is **gone**   → the wasm module resolved and executed;
  * the nav has its class groups       → `pages.json` loaded and the corpus is wired up;
  * the canvas has **many distinct colours** → tiny-skia rasterized into it. This is the one that matters:
    it is the difference between "the page loaded" and "the engine painted". Note *colours*, not
    "non-white pixels" — the first version asked the latter and was vacuous, because an untouched canvas
    is transparent BLACK and satisfied it. It passed a mutation that deleted the paint call entirely;
  * parse/cascade/layout are **> 0ms**  → the provenance panel is reading a real high-resolution clock and
    not a frozen or coarse one. An all-zero panel is a broken build, never a fact about the engine.
"""

import asyncio
import json
import shutil
import subprocess
import sys
import tempfile
import urllib.request

PORT = 8901
CDP = 9222


def wait_for_cdp(chrome, timeout=40):
    """Wait for Chrome to actually open its debug port.

    The first version slept 3 seconds and then connected. On my laptop Chrome was up in well under
    that; on a GitHub runner it was not, so the probe hit `Connection refused` and **failed the demo
    DEPLOY** — the gate written to protect the demo was the only thing stopping it from shipping.
    That is PROCESS #31 exactly (*my instrument broke the build it was measuring*), and the cause was
    the same: a fixed sleep standing in for the condition I actually cared about. So: poll the
    condition. And if Chrome died, say what it said, rather than reporting a refused connection.
    """
    import time

    deadline = time.time() + timeout
    while time.time() < deadline:
        if chrome.poll() is not None:
            err = (chrome.stderr.read() or b"").decode(errors="replace")[-800:]
            fail(f"the browser exited before opening its debug port (rc={chrome.returncode}).\n{err}")
        try:
            with urllib.request.urlopen(f"http://localhost:{CDP}/json", timeout=1) as r:
                if json.load(r):
                    return
        except Exception:
            time.sleep(0.5)
    fail(f"the browser never opened its debug port on :{CDP} within {timeout}s.")


async def probe():
    tabs = json.load(urllib.request.urlopen(f"http://localhost:{CDP}/json"))
    ws_url = next(t for t in tabs if t["type"] == "page")["webSocketDebuggerUrl"]
    import websockets

    async with websockets.connect(ws_url, max_size=None) as c:
        seq = 0

        async def call(method, params=None):
            nonlocal seq
            seq += 1
            await c.send(json.dumps({"id": seq, "method": method, "params": params or {}}))
            while True:
                msg = json.loads(await c.recv())
                if msg.get("id") == seq:
                    return msg

        await call("Runtime.enable")
        await call("Page.navigate", {"url": f"http://localhost:{PORT}/"})

        # Poll rather than sleep a fixed time: the whole point is not to guess when it is done.
        for _ in range(40):
            await asyncio.sleep(0.5)
            r = await call(
                "Runtime.evaluate",
                {
                    "expression": """JSON.stringify({
                      boot:   !!document.getElementById('boot'),
                      groups: document.querySelectorAll('#bar .grp').length,
                      layers: (document.getElementById('layers')||{}).innerText || '',
                      // COUNT DISTINCT COLOURS — do not ask "is any channel != 255".
                      //
                      // That was this check's first version and it was VACUOUS: an untouched canvas is
                      // transparent *black* (0,0,0,0), so "some channel != 255" is true of a canvas
                      // nothing has ever drawn to. It reported PAINTED for a blank demo, and a mutation
                      // that deleted the putImageData call sailed straight through it.
                      //
                      // A real render has many colours; an empty canvas has exactly one. So: sample, and
                      // demand variety. That is a property only actual rasterized content has.
                      colours: (()=>{ const c=document.querySelector('canvas');
                        if(!c || !c.width || !c.height) return 0;
                        const w=Math.min(c.width,200), h=Math.min(c.height,200);
                        const d=c.getContext('2d').getImageData(0,0,w,h).data, s=new Set();
                        for(let i=0;i<d.length;i+=4)
                          s.add((d[i]<<24)|(d[i+1]<<16)|(d[i+2]<<8)|d[i+3]);
                        return s.size; })(),
                    })""",
                    "returnByValue": True,
                },
            )
            st = json.loads(r["result"]["result"]["value"])
            if not st["boot"] and st["colours"] > 2 and st["groups"]:
                return st
        return st


def fail(msg):
    print(f"   \033[31m✗ G_DEMO_LIVE: {msg}\033[0m", file=sys.stderr)
    sys.exit(1)


def main():
    srv = subprocess.Popen(
        [sys.executable, "-m", "http.server", str(PORT)],
        cwd="demo/www",
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    chrome = None
    profile = tempfile.mkdtemp(prefix="manuk-cdp-")
    for exe in ("chromium", "google-chrome", "google-chrome-stable", "chromium-browser"):
        try:
            chrome = subprocess.Popen(
                [exe, "--headless=new", "--no-sandbox", "--disable-gpu",
                 # A CI container has a tiny /dev/shm; without this Chrome dies on startup with a
                 # crash that looks nothing like the cause.
                 "--disable-dev-shm-usage",
                 # And it needs a writable profile it does not have to guess at.
                 f"--user-data-dir={profile}",
                 f"--remote-debugging-port={CDP}", "about:blank"],
                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
            )
            break
        except FileNotFoundError:
            continue
    if chrome is None:
        srv.terminate()
        print("   ⚠ no chromium on PATH — the demo is UNVERIFIED, not verified")
        return
    try:
        wait_for_cdp(chrome)
        st = asyncio.run(probe())

        if st["boot"]:
            fail("the boot placeholder is still there — the wasm module never executed.")
        if not st["groups"]:
            fail("the nav has no class groups — pages.json did not load.")
        # >2 distinct colours. A blank canvas has exactly 1. Anti-aliased text alone has dozens.
        if st["colours"] <= 2:
            fail(f"the canvas has {st['colours']} distinct colour(s) — the engine parsed and laid out, "
                 f"but its pixels never reached the canvas. It did not PAINT.")

        # The clock. "0ms" everywhere means the panel is reading a frozen or coarse clock, and a
        # provenance panel that cannot see time is not provenance.
        ms = [float(x) for x in __import__("re").findall(r"([\d.]+)ms", st["layers"])]
        if not ms or not any(v > 0 for v in ms):
            fail(f"the provenance panel reports no elapsed time: {st['layers']!r} (PROCESS #36)")

        print(f"   \033[32m✓\033[0m the engine ran in a real browser: "
              f"{st['groups']} nav groups, canvas PAINTED ({st['colours']} colours), stages {ms}")
    finally:
        chrome.terminate()
        srv.terminate()
        shutil.rmtree(profile, ignore_errors=True)


if __name__ == "__main__":
    main()
