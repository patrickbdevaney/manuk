// RG.2.0.b.a.b: does Chrome's AX tree drop the controls behind an open popover (zilloft-5 Price popover,
// gocalendar-2 view popover / "+N more" popover)? Same click sequence as the manuk runs, read via CDP
// Accessibility.getFullAXTree in the pinned Chrome. Prints JSON: per state, the baseline named controls and
// whether each is still present and not ignored in the AX tree after the popover opens.
// Run: MANUK_CHROME_PIN=145.0.7632.116 node docs/sprint/rg/R20ba-chrome-ax.js
const { chromium } = require('/home/patrickd/.npm/_npx/381cec31605a419a/node_modules/playwright-core');
const exe = process.env.HOME + '/.local/chrome-cft/bin/google-chrome';
const CONTROL_ROLES = new Set(['button', 'link', 'combobox', 'option', 'menuitem', 'textbox', 'checkbox', 'radio', 'listbox']);

// Named controls in the AX tree: role -> name -> number of non-ignored nodes carrying it.
async function axControls(page) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Accessibility.enable');
  const { nodes } = await cdp.send('Accessibility.getFullAXTree');
  await cdp.detach();
  const out = {};
  for (const n of nodes) {
    const role = n.role && n.role.value;
    const name = ((n.name && n.name.value) || '').trim();
    if (!CONTROL_ROLES.has(role) || !name || n.ignored) continue;
    const k = `${role}|${name}`;
    out[k] = (out[k] || 0) + 1;
  }
  return out;
}

// Baseline names come from the AX tree before the popover opens; `present` is what survives after.
async function measure(page, label, openPopover, watch) {
  await page.waitForTimeout(1500);
  const before = await axControls(page);
  const baseline = Object.keys(before).filter(k => watch.test(k));
  await openPopover();
  await page.waitForTimeout(1000);
  const after = await axControls(page);
  const hiddenDom = await page.evaluate(() => {
    const els = [...document.querySelectorAll('button,a,[role=button]')].filter(e => { const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0; });
    return { visible: els.length, behind_aria_hidden: els.filter(e => e.closest('[aria-hidden=true]')).length };
  });
  const rows = baseline.map(k => ({ control: k, present: (after[k] || 0) > 0 }));
  return { state: label, baseline_count: baseline.length, present_after: rows.filter(r => r.present).length, absent_after: rows.filter(r => !r.present).map(r => r.control), dom: hiddenDom };
}

(async () => {
  const browser = await chromium.launch({ executablePath: exe, headless: true, args: ['--no-sandbox'] });
  const ver = browser.version();
  const result = { chrome_ver: ver, pin: process.env.MANUK_CHROME_PIN || null, runs: [] };
  const runs = Number(process.env.R20BAB_RUNS || 3);
  for (let r = 1; r <= runs; r++) {
    const run = { run: r, states: [] };
    // zilloft-5: homepage -> search "San Jose, CA" -> results -> Price popover.
    {
      const page = await browser.newPage({ viewport: { width: 1024, height: 768 } });
      await page.goto('https://evals-zilloft.vercel.app', { waitUntil: 'load' });
      await page.waitForTimeout(2000);
      await page.fill('input[aria-label^="Search homes"]', 'San Jose, CA');
      await page.click('button[aria-label="Start search"]');
      await page.waitForURL(/\/homes/, { timeout: 30000 }).catch(() => {});
      await page.waitForTimeout(4000);
      run.states.push(await measure(page, 'zilloft-5 Price popover', async () => {
        await page.getByRole('button', { name: /^Price/ }).first().click();
      }, /^(button|link)\|(Price|Beds & Baths|Home Type|For Sale|Save search|Go to homepage)$/));
      await page.close();
    }
    // gocalendar-2: /calendar -> view selector "5 days" popover -> Month -> "+N more" click.
    {
      const page = await browser.newPage({ viewport: { width: 1024, height: 768 } });
      await page.goto('https://evals-gocalendar.vercel.app/calendar', { waitUntil: 'load' });
      await page.waitForTimeout(2500);
      run.states.push(await measure(page, 'gocalendar-2 view popover (5 days)', async () => {
        await page.getByText('5 days', { exact: true }).first().click();
      }, /^(button|link|combobox|option)\|/));
      await page.keyboard.press('Escape');
      await page.waitForTimeout(800);
      await page.getByText('5 days', { exact: true }).first().click().catch(() => {});
      await page.waitForTimeout(800);
      await page.getByRole('option', { name: /^Month/ }).first().click().catch(async () => {
        await page.getByText('Month', { exact: true }).first().click();
      });
      await page.waitForTimeout(1500);
      run.states.push(await measure(page, 'gocalendar-2 +N more popover (Month)', async () => {
        await page.getByText(/^\+\d+ more/).first().click();
      }, /^(button|link|combobox|option)\|/));
      await page.close();
    }
    result.runs.push(run);
  }
  console.log(JSON.stringify(result, null, 1));
  await browser.close();
})().catch(e => { console.error(e); process.exit(1); });
