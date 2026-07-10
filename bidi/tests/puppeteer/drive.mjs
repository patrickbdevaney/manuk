import puppeteer from 'puppeteer-core';
const ws = process.argv[2];
const browser = await puppeteer.connect({ browserWSEndpoint: ws, protocol: 'webDriverBiDi' });
console.log('CONNECTED');
const page = await browser.newPage();
await page.goto('data:text/html,<title>PptrOK</title><body><h1>Hi</h1></body>');
console.log('NAVIGATED', page.url().slice(0, 30));
try {
  const shot = await page.screenshot({ encoding: 'base64' });
  console.log('SCREENSHOT bytes=' + Buffer.from(shot, 'base64').length);
} catch (e) { console.log('SCREENSHOT_ERROR:', String(e.message).split('\n')[0]); }
try {
  console.log('TITLE:', await page.title());
} catch (e) { console.log('TITLE_ERROR:', String(e.message).split('\n')[0]); }
await browser.disconnect();
console.log('DONE');
