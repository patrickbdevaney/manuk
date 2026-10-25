#!/usr/bin/env python3
# R0.4.a.b.a.a.a.a.a.a: store the main-document bytes Chrome itself receives for a URL (CDP Network.getResponseBody),
# and the DOM after JS ran. usage: usagov-chrome-bytes.py URL OUT_HTML OUT_DOM [PORT]
import asyncio, json, subprocess, sys, tempfile, time, urllib.request, base64, shutil
import websockets
url, out_html, out_dom = sys.argv[1:4]; port = int(sys.argv[4]) if len(sys.argv) > 4 else 9622
ud = tempfile.mkdtemp(prefix="chrome-bytes.")
p = subprocess.Popen(["google-chrome", "--headless=new", "--no-sandbox", "--disable-gpu", f"--remote-debugging-port={port}",
                      f"--user-data-dir={ud}", "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    for _ in range(60):
        try:
            tabs = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json")); break
        except Exception: time.sleep(0.5)
    ws_url = [t for t in tabs if t["type"] == "page"][0]["webSocketDebuggerUrl"]
    async def run():
        async with websockets.connect(ws_url, max_size=None) as ws:
            n = 0; main = None
            async def call(m, **pr):
                nonlocal n, main
                n += 1; i = n
                await ws.send(json.dumps({"id": i, "method": m, "params": pr}))
                while True:
                    r = json.loads(await asyncio.wait_for(ws.recv(), 60))
                    if r.get("method") == "Network.responseReceived" and r["params"].get("type") == "Document" and main is None:
                        main = r["params"]["requestId"]
                    if r.get("id") == i: return r
            await call("Network.enable"); await call("Page.enable")
            await call("Page.navigate", url=url)
            t0 = time.time()
            while time.time() - t0 < 15:  # drain events until load finishes
                try: r = json.loads(await asyncio.wait_for(ws.recv(), 1))
                except asyncio.TimeoutError: break
                if r.get("method") == "Network.responseReceived" and r["params"].get("type") == "Document" and main is None:
                    main = r["params"]["requestId"]
            await asyncio.sleep(3)
            b = await call("Network.getResponseBody", requestId=main)
            body = b["result"]["body"]; raw = base64.b64decode(body) if b["result"]["base64Encoded"] else body.encode()
            open(out_html, "wb").write(raw)
            d = await call("Runtime.evaluate", expression="document.documentElement.outerHTML")
            open(out_dom, "w").write("<!DOCTYPE html>" + d["result"]["result"]["value"])
    asyncio.run(run())
finally:
    p.terminate(); shutil.rmtree(ud, ignore_errors=True)
