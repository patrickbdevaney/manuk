import asyncio, json, os, subprocess, sys, tempfile, time, urllib.request
import websockets
CH=os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
async def run(url, js, wait, port=9334):
    d=tempfile.mkdtemp()
    p=subprocess.Popen([CH,"--headless=new","--no-sandbox","--disable-gpu",f"--remote-debugging-port={port}",f"--user-data-dir={d}","--window-size=1024,768","about:blank"],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    try:
        for _ in range(40):
            try: tabs=json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json")); break
            except Exception: time.sleep(0.5)
        ws=next(t for t in tabs if t["type"]=="page")["webSocketDebuggerUrl"]
        async with websockets.connect(ws,max_size=2**26) as w:
            n=[0]
            async def send(m,**pa):
                n[0]+=1; await w.send(json.dumps({"id":n[0],"method":m,"params":pa}))
                while True:
                    j=json.loads(await w.recv())
                    if j.get("id")==n[0]: return j.get("result",{})
            await send("Page.navigate",url=url); await asyncio.sleep(wait)
            r=await send("Runtime.evaluate",expression=js,returnByValue=True)
            return r.get("result",{}).get("value")
    finally:
        p.kill()

