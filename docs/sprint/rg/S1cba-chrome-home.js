// RG.S1.c.b.a: count restaurant cards on the OpenDining home page in the pinned Chrome (CfT 145), via Playwright DOM.
// Run: MANUK_CHROME_PIN=145.0.7632.116 node docs/sprint/rg/S1cba-chrome-home.js
const { chromium } = require('/home/patrickd/.npm/_npx/381cec31605a419a/node_modules/playwright-core');
const exe = process.env.HOME + '/.local/chrome-cft/bin/google-chrome';
const URL = 'https://evals-opendining.vercel.app';
(async () => {
  const browser = await chromium.launch({ executablePath: exe, headless: true, args: ['--no-sandbox'] });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForTimeout(Number(process.env.S1CBA_WAIT || 6) * 1000);
  const ver = browser.version();
  const r = await page.evaluate(() => {
    const links = [...document.querySelectorAll('a,button')].filter(e => /View details for /.test(e.getAttribute('aria-label') || e.textContent || ''));
    const names = links.map(e => ((e.getAttribute('aria-label') || e.textContent).match(/View details for ([^"]+?)(?: Book|$)/) || [])[1] || e.textContent.trim().slice(0, 40));
    const visible = links.filter(e => { const b = e.getBoundingClientRect(); const s = getComputedStyle(e); return b.width > 0 && b.height > 0 && s.visibility !== 'hidden' && s.display !== 'none'; });
    const inView = visible.filter(e => { const b = e.getBoundingClientRect(); return b.bottom > 0 && b.right > 0 && b.top < innerHeight && b.left < innerWidth; });
    const ratings = [...document.querySelectorAll('[aria-label], *')].filter(e => e.children.length === 0 && /^Rating: \d/.test(e.textContent.trim())).length;
    return { dom_links: links.length, distinct_names: [...new Set(names)], visible_links: visible.length, in_viewport_links: inView.length, rating_leaves: ratings };
  });
  console.log(JSON.stringify({ chrome_ver: ver, url: URL, ...r }, null, 1));
  await browser.close();
})().catch(e => { console.error(e); process.exit(1); });
