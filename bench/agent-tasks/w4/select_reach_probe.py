#!/usr/bin/env python3 -I
"""W4.P0.f.a.a.a.a.a.a.a.a: can the cdp-layer's own input primitives reach a Select time option on the clone?

Fresh pinned Chrome profile per run, raw CDP. Sets date 20 Jul 2024 and 4 people with the same clicks as
royal_slot_check.py, then opens the "1:00 PM" time combobox and tries to reach the target option the way the
cdp shim does: window.scrollBy (the shim's scroll_by) and ArrowDown key presses sent as rawKeyDown/keyUp
(the shim's press_key). It does NOT scrollIntoView and does NOT click the option by rect, which is what
royal_slot_check does and which is why it cannot tell reachability.

After every step it reads the target option's rect and the point at its centre. The target is REACHABLE when
the option is in the listbox's visible area and elementFromPoint at its centre lands on that option (a click
there would select it). Writes TSV rows to stdout, chrome_ver=$MANUK_CHROME_PIN.

  select_reach_probe.py [--runs N] [--target "7:00 PM"] [--max-keys 40] [--method key|scroll] [--mutate-missing]

--method scroll   each step is window.scrollBy(0,120) (the shim's scroll_by) instead of an ArrowDown key.

--mutate-missing  targets an option that does not exist: reachable must be False in every run (the gate goes red
                  if the probe ever reports a reachable option that is not in the DOM).
Exit 0 always after measuring; the verdict is read from the rows.
"""
import asyncio, json, os, subprocess, sys, tempfile, urllib.request
import websockets

PIN = os.environ.get("MANUK_CHROME_PIN") or sys.exit("MANUK_CHROME_PIN unset")
CHROME = os.path.expanduser(f"~/.local/chrome-cft/{PIN}/chrome-linux64/chrome")
SITE = "https://evals-opendining.vercel.app"
RESTAURANT = "The Royal Dine"


def arg(name, default):
    return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else default


class Page:
    def __init__(self, ws):
        self.ws, self.n = ws, 0

    async def send(self, method, **params):
        self.n += 1
        i = self.n
        await self.ws.send(json.dumps({"id": i, "method": method, "params": params}))
        while True:
            r = json.loads(await self.ws.recv())
            if r.get("id") == i:
                return r.get("result", r)

    async def ev(self, expr):
        r = await self.send("Runtime.evaluate", expression=expr, returnByValue=True, awaitPromise=True)
        return r.get("result", {}).get("value")

    async def click_xy(self, xy):
        if not xy:
            raise RuntimeError("click target not found")
        for t in ("mousePressed", "mouseReleased"):
            await self.send("Input.dispatchMouseEvent", type=t, x=xy[0], y=xy[1], button="left", clickCount=1)

    async def center(self, js):
        """Centre of an element WITHOUT scrolling it into view (the probe must not do the shim's job for it)."""
        return await self.ev(f"(()=>{{const e={js}; if(!e) return null; const b=e.getBoundingClientRect(); return [b.x+b.width/2,b.y+b.height/2]}})()")

    async def key(self, k):
        """The shim's press_key for a named key: rawKeyDown then keyUp."""
        vk = {"ArrowDown": 40, "ArrowUp": 38, "Enter": 13}[k]
        for t in ("rawKeyDown", "keyUp"):
            await self.send("Input.dispatchKeyEvent", type=t, key=k, code=k, windowsVirtualKeyCode=vk,
                            nativeVirtualKeyCode=vk, modifiers=0)


# Reads the target option (by exact text) in the open listbox: is it in the DOM, is it inside the listbox's visible
# rect, and is it the element hit at its centre (so a click would select it).
STATE_JS = """(t => {
  const opts = [...document.querySelectorAll("li[role=option]")].filter(l => l.getBoundingClientRect().width > 0);
  const o = opts.find(l => l.innerText.trim() === t);
  const lb = o && (o.closest("[role=listbox]") || o.parentElement);
  const visList = opts.map(l => l.innerText.trim());
  if (!o) return {inDom: false, visible: visList.length, reachable: false, listed: visList.join(",")};
  const r = o.getBoundingClientRect(), L = lb.getBoundingClientRect();
  const inside = r.height > 0 && r.top >= L.top - 1 && r.bottom <= L.bottom + 1;
  const cx = r.x + r.width / 2, cy = r.y + r.height / 2;
  const hit = document.elementFromPoint(cx, cy);
  const hitOpt = !!hit && (hit === o || o.contains(hit));
  return {inDom: true, visible: visList.length, inside, hitOpt, reachable: inside && hitOpt,
          y: Math.round(r.top), listed: visList.join(",")};
})"""


async def one_run(run, target, max_keys, method="key"):
    prof = tempfile.mkdtemp(prefix="seltime-prof-")
    proc = subprocess.Popen([CHROME, "--headless=new", "--no-sandbox", "--remote-debugging-port=0", f"--user-data-dir={prof}",
                             "--window-size=1280,1000", "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        port = None
        for _ in range(200):
            f = os.path.join(prof, "DevToolsActivePort")
            if os.path.exists(f):
                port = open(f).read().split()[0]
                break
            await asyncio.sleep(0.1)
        if not port:
            raise RuntimeError("no DevToolsActivePort")
        tgt = [x for x in json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json")) if x["type"] == "page"][0]
        async with websockets.connect(tgt["webSocketDebuggerUrl"], max_size=None) as ws:
            p = Page(ws)
            await p.send("Page.enable")
            await p.send("Page.navigate", url=SITE)
            await asyncio.sleep(5)
            # date 20 Jul 2024 (same clicks as royal_slot_check.py)
            await p.click_xy(await p.center("document.getElementById(':r1:')"))
            await asyncio.sleep(1)
            await p.click_xy(await p.center("[...document.querySelectorAll('button')].filter(b=>b.innerText.trim()=='20' && b.getBoundingClientRect().width>0)[0]"))
            await asyncio.sleep(0.5)
            await p.click_xy(await p.center("[...document.querySelectorAll('button')].filter(b=>b.innerText.trim()=='OK' && b.getBoundingClientRect().width>0)[0]"))
            await asyncio.sleep(1)
            date_shown = await p.ev("document.getElementById(':r1:').value")
            # party 4
            await p.click_xy(await p.center("[...document.querySelectorAll('[role=combobox]')].filter(e=>e.innerText.trim()=='2 people')[0]"))
            await asyncio.sleep(1)
            await p.click_xy(await p.center("[...document.querySelectorAll('li[role=option]')].filter(l=>l.innerText.trim()=='4 people' && l.getBoundingClientRect().width>0)[0]"))
            await asyncio.sleep(1)
            # open the time combobox (it shows 1:00 PM), then use the shim's primitives only
            await p.click_xy(await p.center("[...document.querySelectorAll('[role=combobox]')].filter(e=>e.innerText.trim()=='1:00 PM')[0]"))
            await asyncio.sleep(1)
            state = lambda: p.ev(STATE_JS + f"({json.dumps(target)})")
            st = await state()
            steps, reached_at = 0, None
            while not st.get("reachable") and steps < max_keys:
                # one step = one ArrowDown key press (the cdp shim's press_key), or with --method scroll one
                # window.scrollBy(0,120) (the cdp shim's scroll_by), then re-read the target option
                if method == "scroll":
                    await p.ev("window.scrollBy(0,120);0")
                else:
                    await p.key("ArrowDown")
                await asyncio.sleep(0.15)
                steps += 1
                st = await state()
                if st.get("reachable"):
                    reached_at = steps
            return {"run": run, "date_shown": date_shown, "target": target, "steps": steps,
                    "reached_at_step": "" if reached_at is None else reached_at,
                    "in_dom": st.get("inDom"), "reachable": bool(st.get("reachable")),
                    "visible_options": st.get("visible"), "listed": st.get("listed", "")}
    finally:
        proc.kill()


async def main():
    runs = int(arg("--runs", "3"))
    max_keys = int(arg("--max-keys", "40"))
    target = "No Such Time" if "--mutate-missing" in sys.argv else arg("--target", "7:00 PM")
    method = arg("--method", "key")
    print("run\tchrome_ver\tmethod\tdate_shown\ttarget\tin_dom\treachable\tsteps\treached_at_step\tvisible_options")
    for r in range(1, runs + 1):
        o = await one_run(r, target, max_keys, method)
        print(f"{o['run']}\t{PIN}\t{method}\t{o['date_shown']}\t{o['target']}\t{o['in_dom']}\t{o['reachable']}\t{o['steps']}\t{o['reached_at_step']}\t{o['listed']}")


if __name__ == "__main__":
    asyncio.run(main())
