// W3.20.b.c.b.b: which WebMCP registry does each demo page register on, under Chrome 149 with no manuk bridge?
// For each page name given on the command line: load https://googlechromelabs.github.io/webmcp-tools/demos/<name>/
// in the pinned 149 build, wait for load + settle, then record where the page's tools live.
// Usage: node probe-register.mjs <out.tsv> <page> [<page> ...]   (env: MANUK_CHROME, MANUK_CHROME_PIN)
import { spawn } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const [OUT, ...PAGES] = process.argv.slice(2);
if (!OUT || PAGES.length === 0) { console.error('usage: probe-register.mjs <out.tsv> <page>...'); process.exit(2); }
const CHROME = process.env.MANUK_CHROME || '/home/patrickd/.local/chrome-cft/149.0.7827.155/chrome-linux64/chrome';
const PIN = process.env.MANUK_CHROME_PIN || 'unset';
const BASE = 'https://googlechromelabs.github.io/webmcp-tools/demos/';
const SETTLE_MS = 12000; // page polls document.modelContext up to 10s (20 x 500ms) before giving up
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const profile = mkdtempSync(join(tmpdir(), 'wmc-probe-'));
const chrome = spawn(CHROME, ['--headless=new', '--no-sandbox', '--disable-gpu', '--password-store=basic', `--user-data-dir=${profile}`,
  '--enable-features=WebMCP,WebMCPTesting,DevToolsWebMCPSupport', '--enable-webmcp-testing', '--enable-experimental-web-platform-features',
  '--enable-blink-features=WebMCP,WebMCPTesting,DevToolsWebMCPSupport', '--remote-debugging-pipe', 'about:blank'],
  { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
chrome.on('error', (e) => { console.error('chrome failed to start: ' + e.message); process.exit(2); });
let nextId = 1, buf = '';
const pending = new Map(), listeners = [];
chrome.stdio[4].setEncoding('utf8');
chrome.stdio[4].on('data', (chunk) => {
  buf += chunk;
  let i;
  while ((i = buf.indexOf('\0')) >= 0) {
    const msg = JSON.parse(buf.slice(0, i));
    buf = buf.slice(i + 1);
    if (msg.id !== undefined && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
    else for (const f of [...listeners]) f(msg);
  }
});
const cdp = (method, params = {}, sessionId) => new Promise((res, rej) => {
  const id = nextId++;
  pending.set(id, (m) => (m.error ? rej(new Error(JSON.stringify(m.error))) : res(m.result)));
  const o = { id, method, params };
  if (sessionId) o.sessionId = sessionId;
  chrome.stdio[3].write(JSON.stringify(o) + '\0');
});

// Everything the page could have registered on, read at settle time. No page code is modified.
const PROBE = `(async () => {
  const names = async (reg) => { try { return (await reg()).map((t) => t.name).sort(); } catch (e) { return ['ERR:' + e.message]; } };
  const d = document.modelContext, n = navigator.modelContext, t = navigator.modelContextTesting;
  const out = {
    doc_modelContext: typeof d,
    nav_modelContext: typeof n,
    nav_modelContextTesting: typeof t,
    doc_own: 'modelContext' in document,
    nav_proto_has_modelContext: 'modelContext' in Navigator.prototype,
    doc_tools: d && d.getTools ? await names(() => d.getTools()) : null,
    nav_tools: n && n.getTools ? await names(() => n.getTools()) : null,
    testing_tools: t && t.listTools ? await names(() => t.listTools()) : null,
    title: document.title,
  };
  return JSON.stringify(out);
})()`;

async function probe(name) {
  const url = BASE + name + '/';
  const errors = [];
  const { targetId } = await cdp('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp('Target.attachToTarget', { targetId, flatten: true });
  await cdp('Page.enable', {}, sessionId);
  await cdp('Runtime.enable', {}, sessionId);
  let loaded, status = 'n/a';
  const done = new Promise((r) => (loaded = r));
  const f = (m) => {
    if (m.sessionId !== sessionId) return;
    if (m.method === 'Page.loadEventFired') loaded();
    if (m.method === 'Runtime.exceptionThrown') errors.push((m.params.exceptionDetails.exception?.description || m.params.exceptionDetails.text || '?').split('\n')[0].slice(0, 160));
    if (m.method === 'Runtime.consoleAPICalled' && m.params.type === 'error') errors.push('console.error: ' + (m.params.args || []).map((a) => a.value ?? a.description ?? '').join(' ').slice(0, 160));
  };
  listeners.push(f);
  const nav = await cdp('Page.navigate', { url }, sessionId);
  if (nav.errorText) status = 'navigate:' + nav.errorText;
  await Promise.race([done, sleep(30000)]);
  await sleep(SETTLE_MS);
  const r = await cdp('Runtime.evaluate', { expression: PROBE, awaitPromise: true, returnByValue: true }, sessionId);
  await cdp('Target.closeTarget', { targetId }).catch(() => {});
  listeners.splice(listeners.indexOf(f), 1);
  if (r.exceptionDetails) return { name, status, error: 'eval: ' + (r.exceptionDetails.text || '').slice(0, 160) };
  return { name, status, ...JSON.parse(r.result.value), errors: errors.slice(0, 3) };
}

const rows = [];
for (const name of PAGES) {
  const rec = await probe(name);
  rows.push(rec);
  console.error(JSON.stringify(rec));
}
const cols = ['page', 'chrome_ver', 'doc_modelContext', 'nav_modelContext', 'nav_modelContextTesting', 'nav_proto_has_modelContext',
  'doc_tools', 'nav_tools', 'testing_tools', 'title', 'page_errors'];
const lines = [cols.join('\t')];
for (const r of rows) {
  const j = (v) => (v === undefined || v === null ? '-' : Array.isArray(v) ? (v.join(',') || '[]') : String(v));
  lines.push([r.name, PIN, j(r.doc_modelContext), j(r.nav_modelContext), j(r.nav_modelContextTesting), j(r.nav_proto_has_modelContext),
    j(r.doc_tools), j(r.nav_tools), j(r.testing_tools), j(r.title), j(r.error || r.errors?.join(' | ') || '-')].join('\t'));
}
writeFileSync(OUT, lines.join('\n') + '\n');
console.error(`wrote ${OUT} rows=${rows.length}`);
chrome.kill();
process.exit(0);
