// RG.2.E1.a.a.a: what does pinned Chrome (CfT 145) say about the zilloft-2 results landmark after a search?
// Run: MANUK_CHROME_PIN=145.0.7632.116 node docs/sprint/rg/E1aaa-chrome.js
// Prints one JSON object: role / aria-modal of the "Search results for homes" main, its dialog ancestors,
// whether a control outside it is hit-tested inside its box (Chrome's own "blocks" test), and its box area.
const { chromium } = require('/home/patrickd/.npm/_npx/381cec31605a419a/node_modules/playwright-core');
const exe = process.env.HOME + '/.local/chrome-cft/bin/google-chrome';
const URL = 'https://evals-zilloft.vercel.app';
(async () => {
  const browser = await chromium.launch({ executablePath: exe, headless: true, args: ['--no-sandbox'] });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForTimeout(3000);
  await page.getByRole('textbox').first().fill('San Francisco');
  await page.getByRole('button', { name: 'Start search' }).click();
  await page.waitForTimeout(4000);
  const ver = browser.version();
  const r = await page.evaluate(() => {
    const main = [...document.querySelectorAll('main,[role=main],[aria-label]')].find(e => /Search results for homes/.test(e.getAttribute('aria-label') || ''));
    if (!main) return { found: false, url: location.href };
    const b = main.getBoundingClientRect();
    const vp = innerWidth * innerHeight;
    const ancestors = [];
    for (let e = main; e; e = e.parentElement) ancestors.push(e);
    const dialogish = ancestors.filter(e => /^(dialog|alertdialog)$/.test(e.getAttribute('role') || '') || e.tagName === 'DIALOG');
    const ariaModal = ancestors.filter(e => e.getAttribute('aria-modal') === 'true');
    // Chrome's hit test: controls outside main whose centre lands inside main
    const ctl = [...document.querySelectorAll('button,a,input,[role=button],[role=radio],[role=checkbox]')].filter(e => !main.contains(e));
    let blocked = 0, total = 0;
    for (const e of ctl) {
      const c = e.getBoundingClientRect();
      if (c.width === 0 || c.height === 0) continue;
      total++;
      const hit = document.elementFromPoint(c.x + c.width / 2, c.y + c.height / 2);
      if (hit && main.contains(hit)) blocked++;
    }
    return {
      found: true, url: location.href, tag: main.tagName, role: main.getAttribute('role'), aria_modal_self_or_ancestor: ariaModal.length > 0,
      dialog_ancestors: dialogish.length, box: [Math.round(b.x), Math.round(b.y), Math.round(b.width), Math.round(b.height)],
      covers_30pct_viewport: b.width * b.height >= 0.3 * vp, z_index: getComputedStyle(main).zIndex, position: getComputedStyle(main).position,
      controls_outside: total, controls_hit_inside_main: blocked,
    };
  });
  console.log(JSON.stringify({ chrome_ver: ver, ...r }, null, 1));
  await browser.close();
})().catch(e => { console.error(e); process.exit(1); });
