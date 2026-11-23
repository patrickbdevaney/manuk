#!/usr/bin/env node
// R3.1.b gate: Chrome control arm for the R3.1 determinism fixture.
// The same page (tests/fixtures/determinism.html, or $FIXTURE) is loaded in headless Chrome
// idle and under PROCS busy-loop processes, RUNS each, in two arms: no Playwright clock, and
// page.clock.install(). The no-clock arm MUST give >1 distinct digest over 2*RUNS runs (the
// control must be able to differ, or the R3.1 gate proves nothing). The clock arm is reported
// honestly; manuk's target is 1 but Chrome's Math.random/crypto are not on page.clock.
// usage: node scripts/r31b-chrome-control.mjs [runs=5] [procs=20] [--append]
//   env: MANUK_CHROME_BIN (Chrome binary), MANUK_CHROME_PIN (chrome_ver), PLAYWRIGHT_CORE (module dir), FIXTURE
// exit 0 iff the no-clock arm has >1 distinct digest; --append writes rows to docs/sprint/SCOREBOARD.tsv.
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, appendFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const require = createRequire(import.meta.url);
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const PW = process.env.PLAYWRIGHT_CORE || '/home/patrickd/.npm/_npx/381cec31605a419a/node_modules/playwright-core';
const { chromium } = require(PW);
const CHROME = process.env.MANUK_CHROME_BIN || '/home/patrickd/.local/chrome-cft/bin/chromium';
const CHROME_VER = process.env.MANUK_CHROME_PIN || '145.0.7632.116';
const FIX_PATH = process.env.FIXTURE || path.join(ROOT, 'tests/fixtures/determinism.html');
const FIX = 'file://' + path.resolve(FIX_PATH);
const args = process.argv.slice(2);
const APPEND = args.includes('--append');
const [RUNS, PROCS] = args.filter((a) => a !== '--append').map(Number);
const RUN_N = RUNS || 5, PROC_N = PROCS || 20;
const sha = (s) => createHash('sha256').update(s).digest('hex');
const FIX_HASH = sha(readFileSync(FIX_PATH));
const withTimeout = (p, ms, what) =>
  Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error(what + ' timeout')), ms))]);

function stressOn(n) {
  return Array.from({ length: n }, () => spawn('sh', ['-c', 'while :; do :; done'], { stdio: 'ignore' }));
}
function stressOff(ps) {
  for (const p of ps) { try { p.kill('SIGKILL'); } catch {} }
}

async function runOnce(browser, withClock) {
  const ctx = await browser.newContext();
  try {
    const page = await ctx.newPage();
    if (withClock) await page.clock.install({ time: new Date('2026-01-01T00:00:00Z') });
    await page.goto(FIX, { waitUntil: 'commit', timeout: 15000 });
    await page.waitForFunction(() => document.getElementById('h')?.textContent === 'done', null, { timeout: 15000 });
    const html = await withTimeout(page.evaluate(() => document.documentElement.outerHTML), 10000, 'evaluate');
    return { digest: sha(html).slice(0, 16), err: '' };
  } catch (e) {
    // A page whose script never returns (e.g. a busy-wait on a clock that does not advance) is an outcome.
    return { digest: 'HANG', err: String(e.message).split('\n')[0].slice(0, 120) };
  } finally {
    await withTimeout(ctx.close(), 10000, 'close').catch(() => {});
  }
}

async function arm(withClock) {
  const browser = await chromium.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox'] });
  const rows = [];
  try {
    for (const phase of ['idle', 'stress']) {
      const ps = phase === 'stress' ? stressOn(PROC_N) : [];
      try {
        for (let i = 1; i <= RUN_N; i++) {
          const r = await runOnce(browser, withClock);
          rows.push({ phase, run: rows.length + 1, ...r });
        }
      } finally { stressOff(ps); }
    }
  } finally {
    await withTimeout(browser.close(), 15000, 'browser close').catch(() => {});
  }
  return rows;
}

const ts = new Date().toISOString().slice(0, 19);
const HEADER = readFileSync(path.join(ROOT, 'docs/sprint/SCOREBOARD.tsv'), 'utf8').split('\n')[0].split('\t');
function row(arm, f) {
  const v = { ts, arm, suite_hash: FIX_HASH, chrome_ver: CHROME_VER, task: path.basename(FIX_PATH), model: 'none', ...f };
  return HEADER.map((k) => String(v[k] ?? '').replace(/[\t\n]/g, ' ')).join('\t');
}
const summary = {};
const out = [];
for (const [label, withClock] of [['chrome-noclock', false], ['chrome-pwclock', true]]) {
  const rows = await arm(withClock);
  const distinct = new Set(rows.map((r) => r.digest)).size;
  const hangs = rows.filter((r) => r.digest === 'HANG').length;
  summary[label] = { distinct, hangs, n: rows.length };
  console.log(`arm=${label} runs=${rows.length} distinct=${distinct} hangs=${hangs} chrome_ver=${CHROME_VER}`);
  for (const r of rows) console.log(`  run=${r.run} phase=${r.phase} digest=${r.digest}${r.err ? ' err=' + r.err : ''}`);
  for (const r of rows) {
    out.push(row(label, { run: r.run, pass: r.digest === 'HANG' ? 0 : 1, kind: 'control', slice: 'determinism',
      reasons: r.err, answer: `${r.phase} digest=${r.digest}` }));
  }
  out.push(row(label, { run: 0, kind: 'summary', slice: 'determinism',
    reasons: `distinct=${distinct} of ${rows.length} runs (idle ${RUN_N} + stress ${RUN_N}, ${PROC_N} busy procs) hangs=${hangs} clock=${withClock ? 'page.clock.install' : 'none'} gate=${withClock ? 'report-only' : (distinct > 1 ? 'PASS' : 'FAIL')}` }));
}

if (APPEND) appendFileSync(path.join(ROOT, 'docs/sprint/SCOREBOARD.tsv'), out.join('\n') + '\n');
const ok = summary['chrome-noclock'].distinct > 1;
console.log(ok
  ? `PASS: no-clock Chrome gives ${summary['chrome-noclock'].distinct} distinct digests over ${summary['chrome-noclock'].n} runs (>1); page.clock gives ${summary['chrome-pwclock'].distinct}`
  : `FAIL: no-clock Chrome gives ${summary['chrome-noclock'].distinct} distinct digest(s) over ${summary['chrome-noclock'].n} runs (want >1); page.clock gives ${summary['chrome-pwclock'].distinct}`);
process.exit(ok ? 0 : 1);
