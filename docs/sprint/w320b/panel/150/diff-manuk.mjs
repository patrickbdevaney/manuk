// W3.20.b.c.a.b: diff WebMCP tool names from the Chrome 150 panel capture (toolsAdded, names.txt per page)
// against manuk-mcp tools/list (--backend chrome, MANUK_CHROME=150) for the same demo page.
// Usage: node diff-manuk.mjs <outdir>   env: MANUK_MCP_BIN (default target/release/manuk-mcp), MANUK_CHROME (150 binary),
// MANUK_CHROME_PIN (must be 150.0.7871.124), DROP=<name> (mutation: remove one panel name before diffing; must go red).
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import readline from 'node:readline';

const OUT = process.argv[2] || '.';
const MCP = process.env.MANUK_MCP_BIN || '/home/patrickd/manuk/target/release/manuk-mcp';
const PIN = process.env.MANUK_CHROME_PIN || 'unset';
const DROP = process.env.DROP || '';
const BASE = 'https://googlechromelabs.github.io/webmcp-tools/demos/';
const PAGES = ['pizza-maker', 'french-bistro', 'hotel-chain'];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function manukNames(url) {
  const mcp = spawn(MCP, ['--backend', 'chrome'], { stdio: ['pipe', 'pipe', 'ignore'], env: process.env });
  const pend = new Map();
  let id = 0;
  readline.createInterface({ input: mcp.stdout }).on('line', (line) => {
    let m; try { m = JSON.parse(line); } catch { return; }
    if (m.id !== undefined && pend.has(m.id)) { pend.get(m.id)(m); pend.delete(m.id); }
  });
  const rpc = (method, params = {}) => new Promise((res) => { const i = ++id; pend.set(i, res); mcp.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: i, method, params }) + '\n'); });
  await rpc('initialize', { protocolVersion: '2025-03-26', capabilities: {}, clientInfo: { name: 'w320b-diff', version: '0' } });
  mcp.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
  const nav = await rpc('tools/call', { name: 'navigate', arguments: { url } });
  if (nav.result?.isError) { mcp.kill(); throw new Error('navigate failed: ' + JSON.stringify(nav.result).slice(0, 300)); }
  await sleep(6000);
  const l = await rpc('tools/list', {});
  mcp.kill();
  return l.result.tools.filter((t) => t.name.startsWith('webmcp_')).map((t) => t.name.slice('webmcp_'.length)).sort();
}

const rows = [['page', 'chrome_ver', 'panel_n', 'manuk_n', 'missing_in_manuk', 'extra_in_manuk', 'verdict'].join('\t')];
let allGreen = true;
for (const p of PAGES) {
  const panel = readFileSync(join(OUT, p, 'names.txt'), 'utf8').split('\n').map((s) => s.trim()).filter(Boolean).sort();
  const panelUsed = panel.filter((n) => n !== DROP);
  let manuk = [], err = '';
  try { manuk = await manukNames(BASE + p + '/'); } catch (e) { err = String(e.message || e); }
  const missing = panelUsed.filter((n) => !manuk.includes(n));
  const extra = manuk.filter((n) => !panelUsed.includes(n));
  const verdict = err ? 'ERROR' : (missing.length || extra.length) ? 'DIFF' : 'MATCH';
  if (verdict !== 'MATCH') allGreen = false;
  rows.push([p, PIN, panelUsed.length, manuk.length, missing.join(',') || '-', extra.join(',') || '-', verdict + (err ? ' ' + err : '')].join('\t'));
  console.error(`${p}: ${verdict} panel=${panelUsed.length} manuk=${manuk.length} missing=[${missing}] extra=[${extra}] ${err}`);
}
mkdirSync(OUT, { recursive: true });
writeFileSync(join(OUT, DROP ? 'diff-mutant.tsv' : 'diff.tsv'), rows.join('\n') + '\n');
console.log(rows.join('\n'));
process.exit(allGreen ? 0 : 1);
