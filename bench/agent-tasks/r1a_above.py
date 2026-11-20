#!/usr/bin/env python3
"""W6.R1.a: list Chrome elements (box tops above the header's y) on a REAL replica, to explain the AGF y offset."""
import asyncio, json, os, sys, tempfile, time, urllib.request
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import r1_agf as R
JS = r"""(()=>{const out=[];for(const e of document.body.querySelectorAll('*')){const r=e.getBoundingClientRect();
if(!r.width&&!r.height)continue;const cs=getComputedStyle(e);
out.push({tag:e.tagName.toLowerCase(),cls:(e.getAttribute('class')||'').slice(0,60),id:e.id,role:e.getAttribute('role')||'',
x:r.x,y:r.y,w:r.width,h:r.height,pos:cs.position,disp:cs.display,
txt:(e.children.length?'':(e.textContent||'')).trim().slice(0,50),kids:e.children.length});}
return JSON.stringify({sy:scrollY,dh:document.documentElement.scrollHeight,els:out});})()"""
async def main(site, out):
    import websockets
    prof = tempfile.mkdtemp(prefix="r1a-"); proc, port = R.launch_chrome(prof)
    try:
        pages = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list"))
        page = next(p for p in pages if p["type"] == "page")
        async with websockets.connect(page["webSocketDebuggerUrl"], max_size=None) as ws:
            c = R.Cdp(ws); asyncio.create_task(c.pump())
            for m in ["Page.enable", "Runtime.enable"]: await c.call(m)
            await c.call("Emulation.setDeviceMetricsOverride", {"width": 1024, "height": 768, "deviceScaleFactor": 1, "mobile": False})
            await c.call("Page.navigate", {"url": R.url_of(site)})
            await c.wait_event("Page.loadEventFired", 40); await asyncio.sleep(4)
            r = await c.call("Runtime.evaluate", {"expression": JS, "returnByValue": True})
            open(out, "w").write(r["result"]["value"])
    finally: proc.kill()
asyncio.run(main(sys.argv[1], sys.argv[2]))
