// Chrome-paint probe for sampled disagreement rows. Args: sample.json pagesDir outJson shotsDir
const { chromium } = require('/home/patrickd/.npm/_npx/381cec31605a419a/node_modules/playwright-core');
const fs = require('fs');
const [,, samplePath, pagesDir, outPath, shotsDir] = process.argv;
const samples = JSON.parse(fs.readFileSync(samplePath, 'utf8'));
(async () => {
  const browser = await chromium.launch({ executablePath: '/home/patrickd/.local/chrome-cft/bin/chromium', args: ['--no-sandbox'] });
  const ver = browser.version();
  const results = [];
  for (const s of samples) {
    const html = fs.readFileSync(`${pagesDir}/${s.sample_id}.html`, 'utf8');
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
    // block all network: no external loads; scripts still run as Chrome would
    await page.route('**/*', r => r.request().url().startsWith('about:') || r.request().url().startsWith('data:') ? r.continue() : r.abort());
    await page.setContent(html, { waitUntil: 'load' }).catch(() => {});
    // occlusion probe needs the whole document in the viewport: size it to the page height (capped)
    const sh = await page.evaluate(() => document.documentElement.scrollHeight);
    await page.setViewportSize({ width: 1280, height: Math.min(Math.max(900, sh), 20000) });
    const probes = [];
    for (const seg of s.segments) {
      const needle = seg.grams[0]; // a signature 5-gram from this segment
      const pr = await page.evaluate(({ needle, kind }) => {
        const norm = t => ' ' + t.toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim() + ' ';
        const n = ' ' + needle + ' ';
        const boxOf = el => {
          const r = el.getBoundingClientRect();
          return { w: r.width, h: r.height, x: r.x, y: r.y };
        };
        const parse = c => { const m = c.match(/rgba?\(([^)]+)\)/); if (!m) return null; const v = m[1].split(/[ ,\/]+/).filter(Boolean).map(Number); return { r: v[0], g: v[1], b: v[2], a: v.length > 3 ? v[3] : 1 }; };
        const lum = c => { const f = x => { x /= 255; return x <= 0.03928 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4); }; return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b); };
        const contrastOf = el => {
          const fg = parse(getComputedStyle(el).color); if (!fg) return 21;
          // composite the nearest opaque-enough background behind el (white page canvas if none)
          let bg = { r: 255, g: 255, b: 255, a: 1 }, cur = el, layers = [];
          while (cur && cur.nodeType === 1) { const c = parse(getComputedStyle(cur).backgroundColor); if (c && c.a > 0) layers.push(c); cur = cur.parentElement; }
          for (let i = layers.length - 1; i >= 0; i--) { const l = layers[i]; bg = { r: l.r * l.a + bg.r * (1 - l.a), g: l.g * l.a + bg.g * (1 - l.a), b: l.b * l.a + bg.b * (1 - l.a), a: 1 }; }
          const fa = fg.a; const fc = { r: fg.r * fa + bg.r * (1 - fa), g: fg.g * fa + bg.g * (1 - fa), b: fg.b * fa + bg.b * (1 - fa) };
          const L1 = lum(fc), L2 = lum(bg); return (Math.max(L1, L2) + 0.05) / (Math.min(L1, L2) + 0.05);
        };
        // effective paint of an element: no display none / visibility / zero-opacity chain / zero box / occluded centre
        const paintState = el => {
          let eff = 1, cur = el, why = [];
          while (cur && cur.nodeType === 1) {
            const cs = getComputedStyle(cur);
            if (cs.display === 'none') return { st: 'nobox', why: 'display:none ancestor' };
            if (cs.visibility === 'hidden' || cs.visibility === 'collapse') return { st: 'hidden', why: 'visibility' };
            eff *= parseFloat(cs.opacity);
            if (['SCRIPT','STYLE','TEMPLATE','NOSCRIPT','TITLE','HEAD'].includes(cur.tagName)) return { st: 'nobox', why: cur.tagName };
            cur = cur.parentElement;
          }
          const b = boxOf(el);
          if (b.w <= 0 || b.h <= 0) return { st: 'nobox', why: 'zero box' };
          const cx = b.x + b.w / 2, cy = b.y + b.h / 2;
          const top = document.elementFromPoint(cx, cy);
          if (eff <= 0.01) return { st: 'hidden', why: 'opacity~0' };
          if (!top || !(el === top || el.contains(top) || top.contains(el))) return { st: 'hidden', why: 'occluded or offscreen' };
          const ratio = contrastOf(el);
          if (ratio < 3) return { st: 'hidden', why: 'low-contrast ' + ratio.toFixed(2) + ':1' };
          return { st: 'visible', why: 'box+opacity+unoccluded contrast ' + ratio.toFixed(2) + ':1' };
        };
        const found = [];
        if (kind !== 'attr' && kind !== 'comment') {
          const tw = document.createTreeWalker(document.body || document.documentElement, NodeFilter.SHOW_TEXT);
          let node; while ((node = tw.nextNode())) {
            if (norm(node.textContent).includes(n)) {
              const el = node.parentElement;
              if (!el) continue;
              found.push({ carrier: 'text', tag: el.tagName, ...paintState(el) });
            }
          }
        }
        if (kind === 'attr' || kind === 'comment' || found.length === 0) {
          // attribute carriers
          for (const el of document.querySelectorAll('*')) {
            for (const a of el.attributes) {
              if (norm(a.value).includes(n)) {
                // alt/title/data-*/aria-label/placeholder/value are not painted text at rest
                const painted = (a.name === 'alt' && el.tagName === 'IMG' && el.complete && el.naturalWidth === 0 && el.getBoundingClientRect().width > 0)
                  || ((a.name === 'value' || a.name === 'placeholder') && ['INPUT','TEXTAREA'].includes(el.tagName) && paintState(el).st === 'visible');
                found.push({ carrier: 'attr:' + a.name, tag: el.tagName, st: painted ? 'visible' : 'not-painted-at-rest', why: painted ? 'broken img alt / input text' : 'attribute value is not painted text' });
              }
            }
          }
          // comment carriers
          const cw = document.createTreeWalker(document, NodeFilter.SHOW_COMMENT);
          let c; while ((c = cw.nextNode())) if (norm(c.data).includes(n)) found.push({ carrier: 'comment', st: 'not-painted-at-rest', why: 'comment node' });
        }
        return found;
      }, { needle, kind: seg.kind });
      probes.push({ kind: seg.kind, needle, carriers: pr });
    }
    // element screenshot of the first painted carrier, if any, for later eyeballing
    const painted = probes.flatMap(p => p.carriers).filter(c => c.st === 'visible');
    const paintBin = painted.length ? 'visible' : probes.some(p => p.carriers.some(c => c.st === 'hidden')) ? 'rendered-hidden' : 'non-rendered';
    if (painted.length && shotsDir) await page.screenshot({ path: `${shotsDir}/${s.sample_id}.png`, fullPage: true }).catch(() => {});
    results.push({ sample_id: s.sample_id, construction: s.construction, manuk_render: s.manuk_render, chrome_paint: paintBin, probes });
    await page.close();
  }
  await browser.close();
  fs.writeFileSync(outPath, JSON.stringify({ chrome_ver: ver, results }, null, 1));
  console.log('chrome', ver, 'rows', results.length);
})();
