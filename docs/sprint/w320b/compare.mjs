// W3.20.b: tools/list on Chrome WebMCP demo pages, manuk-mcp (--backend chrome) vs Chrome's own registry.
// Reference = navigator.modelContextTesting.listTools() in the pinned Chrome (launched with
// --enable-features=WebMCPTesting, no manuk bridge). The DevTools WebMCP panel does not exist in the pinned
// 145 build (no WebMCP inspector domain in chrome.real), so the native registry is the reference the panel
// would read. Usage: node compare.mjs <out-dir>   (env: MANUK_MCP_BIN, MANUK_CHROME, MANUK_CHROME_PIN)
import { spawn } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import readline from 'node:readline';

const OUT = process.argv[2] || 'docs/sprint/w320b';
const CHROME = process.env.MANUK_CHROME || '/home/patrickd/.local/chrome-cft/145.0.7632.116/chrome-linux64/chrome.real';
const MCP = process.env.MANUK_MCP_BIN || 'target/release/manuk-mcp';
const PIN = process.env.MANUK_CHROME_PIN || 'unset';
const BASE = 'https://googlechromelabs.github.io/webmcp-tools/demos/';
const DEMOS = ['pizza-maker', 'coffee-shop', 'ticket-booking', 'smart-home', 'doors', 'french-bistro'];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const sortKeys = (v) =>
  Array.isArray(v) ? v.map(sortKeys) : v && typeof v === 'object' ? Object.fromEntries(Object.keys(v).sort().map((k) => [k, sortKeys(v[k])])) : v;
const canon = (v) => JSON.stringify(sortKeys(v ?? null));
const asObj = (s) => {
  if (typeof s !== 'string') return s;
  try { return JSON.parse(s); } catch { return s; }
};

// ---- reference: pinned Chrome over --remote-debugging-pipe (fd3 in, fd4 out, NUL-terminated JSON) ----
const profile = mkdtempSync(join(tmpdir(), 'wmc-ref-'));
const chrome = spawn(CHROME, ['--headless=new', '--no-sandbox', '--disable-gpu', '--password-store=basic', `--user-data-dir=${profile}`,
  '--enable-features=WebMCP,WebMCPTesting', '--remote-debugging-pipe', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
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

const NATIVE_EXPR = `(async () => {
  const out = { modelContext: typeof navigator.modelContext, testing: typeof navigator.modelContextTesting };
  if (navigator.modelContextTesting) {
    let t = await navigator.modelContextTesting.listTools();
    if (typeof t === 'string') t = JSON.parse(t);
    out.tools = t;
  }
  return JSON.stringify(out);
})()`;

const DOC_ALIAS = `if (!document.modelContext && navigator.modelContext) Object.defineProperty(document, 'modelContext', { value: navigator.modelContext, configurable: true });`;

async function nativeTools(url) {
  const { targetId } = await cdp('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp('Target.attachToTarget', { targetId, flatten: true });
  await cdp('Page.enable', {}, sessionId);
  // The demos register on document.modelContext, a spelling absent from the pinned 145 build; alias it to the
  // native navigator.modelContext before page script so the native registry sees the registrations.
  await cdp('Page.addScriptToEvaluateOnNewDocument', { source: DOC_ALIAS }, sessionId);
  let loaded;
  const done = new Promise((r) => (loaded = r));
  const f = (m) => { if (m.method === 'Page.loadEventFired' && m.sessionId === sessionId) { listeners.splice(listeners.indexOf(f), 1); loaded(); } };
  listeners.push(f);
  await cdp('Page.navigate', { url }, sessionId);
  await Promise.race([done, sleep(30000)]);
  await sleep(5000); // module scripts, CDN tailwind, origin-trial registration
  const r = await cdp('Runtime.evaluate', { expression: NATIVE_EXPR, awaitPromise: true, returnByValue: true }, sessionId);
  await cdp('Target.closeTarget', { targetId }).catch(() => {});
  if (r.exceptionDetails) throw new Error('native eval threw: ' + JSON.stringify(r.exceptionDetails).slice(0, 300));
  return JSON.parse(r.result.value);
}

// ---- manuk side: one manuk-mcp --backend chrome session over stdio JSON-RPC ----
const mcp = spawn(MCP, ['--backend', 'chrome'], { stdio: ['pipe', 'pipe', 'ignore'], env: process.env });
const mcpPending = new Map();
let mid = 0;
readline.createInterface({ input: mcp.stdout }).on('line', (line) => {
  let m; try { m = JSON.parse(line); } catch { return; }
  if (m.id !== undefined && mcpPending.has(m.id)) { mcpPending.get(m.id)(m); mcpPending.delete(m.id); }
});
const rpc = (method, params = {}) => new Promise((res) => {
  const id = ++mid;
  mcpPending.set(id, res);
  mcp.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
});
const note = (m) => mcp.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: m }) + '\n');

async function manukTools(url) {
  const nav = await rpc('tools/call', { name: 'navigate', arguments: { url } });
  if (nav.result?.isError) throw new Error('manuk navigate failed: ' + JSON.stringify(nav.result).slice(0, 300));
  await sleep(5000);
  const l = await rpc('tools/list', {});
  return l.result.tools.filter((t) => t.name.startsWith('webmcp_')).map((t) => ({
    name: t.name.slice('webmcp_'.length),
    schema: asObj(t.inputSchema),
    description: t.description,
  }));
}

// ---- compare ----
await rpc('initialize', { protocolVersion: '2025-03-26', capabilities: {}, clientInfo: { name: 'w320b', version: '0' } });
note('notifications/initialized');
await sleep(500);
const version = await cdp('Browser.getVersion').catch(() => ({}));
const rows = [], transcript = {};
for (const d of DEMOS) {
  const url = BASE + d + '/';
  const rec = { url, native: null, manuk: null, error: null };
  try {
    const nat = await nativeTools(url);
    rec.nativeMeta = { modelContext: nat.modelContext, testing: nat.testing };
    rec.native = (nat.tools || []).map((t) => ({ name: t.name, schema: asObj(t.inputSchema), description: t.description })).sort((a, b) => a.name.localeCompare(b.name));
    rec.manuk = (await manukTools(url)).sort((a, b) => a.name.localeCompare(b.name));
    const nn = rec.native.map((t) => t.name), mn = rec.manuk.map((t) => t.name);
    const missing = nn.filter((n) => !mn.includes(n));
    const extra = mn.filter((n) => !nn.includes(n));
    const schemaDiff = rec.native.filter((t) => { const m = rec.manuk.find((x) => x.name === t.name); return m && canon(m.schema) !== canon(t.schema); }).map((t) => t.name);
    const pass = nn.length > 0 && missing.length === 0 && extra.length === 0 && schemaDiff.length === 0;
    Object.assign(rec, { missing, extra, schemaDiff, pass });
  } catch (e) {
    rec.error = String(e.message || e);
    rec.pass = false;
  }
  transcript[d] = rec;
  rows.push([d, rec.pass ? 'PASS' : 'FAIL', rec.native ? rec.native.length : '', rec.manuk ? rec.manuk.length : '',
    rec.native ? rec.native.map((t) => t.name).join(',') : '', rec.manuk ? rec.manuk.map((t) => t.name).join(',') : '',
    (rec.missing || []).join(',') || '-', (rec.extra || []).join(',') || '-', (rec.schemaDiff || []).join(',') || '-', rec.error || '-', PIN].join('\t'));
  console.error(`${d}: ${rec.pass ? 'PASS' : 'FAIL'} native=${rec.native?.length} manuk=${rec.manuk?.length} missing=${rec.missing} extra=${rec.extra} schemaDiff=${rec.schemaDiff} ${rec.error || ''}`);
}
const header = ['demo', 'verdict', 'native_n', 'manuk_n', 'native_names', 'manuk_names', 'missing_in_manuk', 'extra_in_manuk', 'schema_diff', 'error', 'chrome_ver'].join('\t');
const pagesPass = Object.values(transcript).filter((r) => r.pass).length;
writeFileSync(join(OUT, 'RESULT.tsv'), [header, ...rows].join('\n') + `\n# pages_pass=${pagesPass} of ${DEMOS.length} chrome_ver=${PIN} browser=${version.product || 'n/a'}\n`);
writeFileSync(join(OUT, 'transcript.json'), JSON.stringify({ chrome_ver: PIN, browser: version.product, transcript }, null, 2) + '\n');
console.error(`pages_pass=${pagesPass}`);
mcp.kill();
chrome.kill();
process.exit(0);
