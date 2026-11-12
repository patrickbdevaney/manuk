// RG.2.E1.a.a.a.a.a: pinned Chrome's box of the zilloft-2 "Map view of search results" region after a search.
const { chromium } = require('/home/patrickd/.npm/_npx/381cec31605a419a/node_modules/playwright-core');
(async () => {
  const b = await chromium.launch({ executablePath: process.env.HOME + '/.local/chrome-cft/bin/google-chrome', headless: true, args: ['--no-sandbox'] });
  const p = await b.newPage({ viewport: { width: 1280, height: 800 } });
  await p.goto('https://evals-zilloft.vercel.app', { waitUntil: 'load' });
  await p.waitForTimeout(3000);
  await p.getByRole('textbox').first().fill('San Francisco');
  await p.getByRole('button', { name: 'Start search' }).click();
  await p.waitForTimeout(4000);
  const r = await p.evaluate(() => {
    const e = [...document.querySelectorAll('[aria-label]')].find(e => /Map view/.test(e.getAttribute('aria-label')));
    if (!e) return { found: false };
    const x = e.getBoundingClientRect();
    return { found: true, box: [x.x, x.y, x.width, x.height].map(Math.round) };
  });
  console.log(JSON.stringify({ chrome_ver: b.version(), ...r }));
  await b.close();
})().catch(e => { console.error(e); process.exit(1); });
