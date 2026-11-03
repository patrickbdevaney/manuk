#!/usr/bin/env python3
# R0.4.a.c.a.b.a.a.a.a.a.a.a.a.b: capture Chrome's full AX tree for a URL (link/button nodes: ignored?, has layout box?).
# usage: sf-chrome-ax.py URL OUT_JSON [PORT]   (viewport 1024x768 like manuk's a11y dump)
import asyncio, json, subprocess, sys, tempfile, time, urllib.request, shutil, os
import websockets
url, out = sys.argv[1:3]; port = int(sys.argv[3]) if len(sys.argv) > 3 else 9633
ud = tempfile.mkdtemp(prefix="chrome-ax.")
p = subprocess.Popen(["google-chrome", "--headless=new", "--no-sandbox", "--disable-gpu", "--disable-blink-features=AutomationControlled", "--user-agent=Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36", f"--remote-debugging-port={port}",
                      f"--user-data-dir={ud}", "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    for _ in range(60):
        try: tabs = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json")); break
        except Exception: time.sleep(0.5)
    ws_url = [t for t in tabs if t["type"] == "page"][0]["webSocketDebuggerUrl"]
    async def run():
        async with websockets.connect(ws_url, max_size=None) as ws:
            n = 0
            async def call(m, **pr):
                nonlocal n
                n += 1; i = n
                await ws.send(json.dumps({"id": i, "method": m, "params": pr}))
                while True:
                    r = json.loads(await asyncio.wait_for(ws.recv(), 90))
                    if r.get("id") == i: return r
            await call("Page.enable")
            await call("Emulation.setDeviceMetricsOverride", width=1024, height=768, deviceScaleFactor=1, mobile=False)
            await call("Page.navigate", url=url)
            t0 = time.time()
            while time.time() - t0 < 20:
                try: await asyncio.wait_for(ws.recv(), 1)
                except asyncio.TimeoutError: break
            await asyncio.sleep(5)
            ev=await call("Runtime.evaluate", expression="document.title+' | '+location.href+' | '+document.body.innerText.length+' | '+document.querySelectorAll('a,button').length")
            print(ev["result"]["result"].get("value"), file=sys.stderr)
            await call("Accessibility.enable"); await call("DOM.enable")
            tree = (await call("Accessibility.getFullAXTree"))["result"]["nodes"]
            rows = []
            for nd in tree:
                role = nd.get("role", {}).get("value", "")
                if role not in ("link", "button"): continue
                rec = {"role": role, "name": (nd.get("name", {}).get("value") or "").strip(), "ignored": bool(nd.get("ignored")), "box": None}
                if not rec["ignored"] and "backendDOMNodeId" in nd:
                    b = await call("DOM.getBoxModel", backendNodeId=nd["backendDOMNodeId"])
                    if "result" in b:
                        q = b["result"]["model"]["border"]; xs, ys = q[0::2], q[1::2]
                        rec["box"] = [max(xs) - min(xs), max(ys) - min(ys)]
                rows.append(rec)
            json.dump({"chrome_ver": os.environ["MANUK_CHROME_PIN"], "all_nodes": len(tree), "nodes": rows}, open(out, "w"))
    asyncio.run(run())
finally:
    p.terminate(); shutil.rmtree(ud, ignore_errors=True)
