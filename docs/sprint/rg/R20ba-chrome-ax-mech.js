// RG.2.0.b.a.b.a: why does Chrome's AX tree "drop" the Month-grid day cells after the gocalendar-2 "+N more"
// click, with no aria-hidden on siblings? Probe one dropped cell (a Month-grid .rbc-button-link day button) and
// one kept control (the toolbar), before and after the click: DOM attachment, ancestor inert/aria-hidden/
// aria-modal/role, dialog/overlay count, which rbc view is mounted, and the CDP AX node.
// Run: MANUK_CHROME_PIN=145.0.7632.116 node docs/sprint/rg/R20ba-chrome-ax-mech.js
// MUTANT=1: the probe reads an aria-hidden-free ancestor chain as hidden (gate must go red).
const { chromium } = require('/home/patrickd/.npm/_npx/381cec31605a419a/node_modules/playwright-core');
const exe = process.env.HOME + '/.local/chrome-cft/bin/google-chrome';
const mutant = process.env.MUTANT === '1';

const probe = ([sel, mutant]) => {
  const el = document.querySelector(sel);
  const view = ['rbc-month-view', 'rbc-time-view', 'rbc-agenda-view'].filter(c => document.querySelector('.' + c));
  const base = { selector: sel, attached: !!el && document.contains(el), month_cells_in_dom: document.querySelectorAll('.rbc-month-view .rbc-button-link').length,
    mounted_views: view, dialogs: document.querySelectorAll('[role=dialog],[aria-modal],dialog,[popover],.rbc-overlay').length };
  if (!el) return base;
  const chain = [];
  for (let n = el; n && n.nodeType === 1; n = n.parentElement) {
    const sig = n.inert === true || n.getAttribute('aria-hidden') === 'true' || n.hasAttribute('hidden');
    chain.push({ tag: n.tagName.toLowerCase(), cls: (n.className + '').slice(0, 40), role: n.getAttribute('role'), inert: n.inert === true, ariaHidden: n.getAttribute('aria-hidden'), ariaModal: n.getAttribute('aria-modal'), hidden: sig && !mutant ? true : (mutant ? true : false) });
  }
  return { ...base, hidden_ancestors: chain.filter(c => c.hidden).length, chain_len: chain.length };
};

async function ax(page, sel) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('DOM.enable'); await cdp.send('Accessibility.enable');
  try {
    const { root } = await cdp.send('DOM.getDocument', { depth: 0 });
    const { nodeId } = await cdp.send('DOM.querySelector', { nodeId: root.nodeId, selector: sel });
    if (!nodeId) return { in_ax: false, reason: 'node detached from DOM' };
    const { nodes } = await cdp.send('Accessibility.getPartialAXTree', { nodeId, fetchRelatives: false });
    const n = nodes[0] || {};
    return { in_ax: true, ignored: !!n.ignored, role: n.role && n.role.value };
  } finally { await cdp.detach(); }
}

(async () => {
  const browser = await chromium.launch({ executablePath: exe, headless: true, args: ['--no-sandbox'] });
  const page = await browser.newPage({ viewport: { width: 1024, height: 768 } });
  await page.goto('https://evals-gocalendar.vercel.app/calendar', { waitUntil: 'load' });
  await page.waitForTimeout(2500);
  await page.getByText('5 days', { exact: true }).first().click();
  await page.waitForTimeout(800);
  await page.getByRole('option', { name: /^Month/ }).first().click().catch(() => page.getByText('Month', { exact: true }).first().click());
  await page.waitForTimeout(1500);
  const CELL = '.rbc-month-view .rbc-button-link', KEPT = 'button:not(.rbc-button-link):not([role=gridcell])';
  const out = { chrome_ver: browser.version(), pin: process.env.MANUK_CHROME_PIN || null, mutant, before: {}, after: {} };
  const grab = async k => { out[k] = { dropped_cell: { ...(await page.evaluate(probe, [CELL, mutant])), ax: await ax(page, CELL) }, kept_control: { ...(await page.evaluate(probe, [KEPT, mutant])), ax: await ax(page, KEPT) } }; };
  await grab('before');
  out.click_text = await page.getByText(/^\+\d+ more/).first().textContent();
  await page.getByText(/^\+\d+ more/).first().click();
  await page.waitForTimeout(1000);
  await grab('after');
  console.log(JSON.stringify(out, null, 1));
  await browser.close();
})().catch(e => { console.error(e); process.exit(1); });
