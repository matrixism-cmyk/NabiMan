// Browser smoke test: loads the built app in Chromium, signs in, opens the
// remote workspace, and fails on anything the browser complains about.
//
//   npm run smoke                     # against http://127.0.0.1:18096
//   SMOKE_URL=... SMOKE_PASSWORD=... npm run smoke
//
// Needs a NabiMan server serving build/ and `npx playwright install chromium`.
// A bundle can pass every unit test and still die on the first line a browser
// runs (an unreplaced `process.env`, for one), which is what this catches.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const problems = [];
  page.on('pageerror', (e) => problems.push(`pageerror: ${e.message.split('\n')[0]}`));
  page.on('console', (m) => { if (m.type() === 'error') problems.push(`console: ${m.text().slice(0, 140)}`); });
  page.on('requestfailed', (r) => problems.push(`request failed: ${r.url()} (${r.failure()?.errorText})`));
  page.on('response', (r) => { if (r.status() >= 400 && !r.url().includes('/api/')) problems.push(`http ${r.status()}: ${r.url()}`); });

  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.screenshot({ path: `${OUT}/01-login.png` });
  console.log('LOGIN screen text:', (await page.textContent('body')).replace(/\s+/g, ' ').trim().slice(0, 80));

  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header', { timeout: 15000 });
  await page.waitForTimeout(2500);
  await page.screenshot({ path: `${OUT}/02-dashboard.png` });
  console.log('CATEGORIES:', await page.$$eval('.cat-btn', els => els.map(e => e.textContent.trim()).join(' / ')));

  await page.click('.cat-btn:has-text("원격")');
  await page.waitForTimeout(2500);
  await page.screenshot({ path: `${OUT}/03-remote.png`, fullPage: false });
  const rail = await page.$$eval('.rw-item', els => els.map(e => e.textContent.trim().slice(0, 30)));
  console.log('REMOTE rail items:', rail.length ? rail.join(' | ') : '(none)');
  console.log('WORKSPACE rendered:', Boolean(await page.$('.rw-work')));

  const real = problems.filter((p) => !p.includes('401'));
  console.log('PROBLEMS:', real.length ? JSON.stringify(real.slice(0, 6), null, 1) : 'none');
  await browser.close();
  if (real.length) process.exit(1);
})().catch(e => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
