// W3.20.b.c.b.a: trace which registerTool/provideContext calls reach an init-script hook on Chrome 149.
// Usage: node trace149.mjs <url>   (env MANUK_CHROME)
import { spawn } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
const CHROME = process.env.MANUK_CHROME || '/home/patrickd/.local/chrome-cft/149.0.7827.155/chrome-linux64/chrome';
const url = process.argv[2];
const c = spawn(CHROME, ['--headless=new', '--no-sandbox', '--disable-gpu', '--password-store=basic', `--user-data-dir=${mkdtempSync(join(tmpdir(), 'tr-'))}`,
  '--enable-features=WebMCP,WebMCPTesting,DevToolsWebMCPSupport', '--enable-webmcp-testing', '--enable-experimental-web-platform-features', '--remote-debugging-pipe', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
let id = 1, buf = ''; const pend = new Map();
c.stdio[4].setEncoding('utf8');
c.stdio[4].on('data', (d) => { buf += d; let i; while ((i = buf.indexOf('\0')) >= 0) { const m = JSON.parse(buf.slice(0, i)); buf = buf.slice(i + 1); if (m.id && pend.has(m.id)) { pend.get(m.id)(m); pend.delete(m.id); } } });
const cdp = (method, params = {}, sessionId) => new Promise((r) => { const n = id++; pend.set(n, r); c.stdio[3].write(JSON.stringify({ id: n, method, params, sessionId }) + '\0'); });
const HOOK = `(() => { const process_alias = () => !!${process.env.ALIAS || 1}; window.__tr = []; const same = navigator.modelContext === document.modelContext;
  window.__tr.push('init nav=' + typeof navigator.modelContext + ' doc=' + typeof document.modelContext + ' same=' + same + ' ctor=' + (document.modelContext && document.modelContext.constructor.name));
  if (process_alias()) try { Object.defineProperty(document, 'modelContext', { value: navigator.modelContext, configurable: true }); } catch (e) { window.__tr.push('alias failed ' + e); }
  for (const [lab, o] of [['nav', navigator.modelContext], ['doc', document.modelContext]]) { if (!o) continue;
    for (const m of ['registerTool', 'provideContext', 'unregisterTool', 'clearContext']) { const f = o[m]; if (typeof f !== 'function') continue;
      o[m] = function (a) { window.__tr.push(lab + '.' + m + ' ' + (a && a.name || (a && a.tools && a.tools.length + ' tools') || '')); try { const r = f.apply(this, arguments); if (r && r.catch) r.catch((e) => window.__tr.push(m + ' rejected ' + (a && a.name) + ': ' + e)); return r; } catch (e) { window.__tr.push(m + ' threw ' + (a && a.name) + ': ' + e); throw e; } }; } } })();`;
const { result: { targetId } } = await cdp('Target.createTarget', { url: 'about:blank' });
const { result: { sessionId } } = await cdp('Target.attachToTarget', { targetId, flatten: true });
await cdp('Page.enable', {}, sessionId);
await cdp('Page.addScriptToEvaluateOnNewDocument', { source: HOOK }, sessionId);
await cdp('Page.navigate', { url }, sessionId);
await new Promise((r) => setTimeout(r, 9000));
const r = await cdp('Runtime.evaluate', { expression: 'JSON.stringify({tr: window.__tr, native: document.modelContext ? null : null})', returnByValue: true }, sessionId);
console.log(r.result.result.value);
c.kill();
