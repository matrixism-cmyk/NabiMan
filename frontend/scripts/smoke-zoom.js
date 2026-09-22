// Browser smoke test for how a terminal remembers the size it was zoomed to.
//
//   SMOKE_URL=http://127.0.0.1:18096 npm run smoke:zoom
//
// Needs a NabiMan server serving build/ (a throwaway data dir and password)
// and `npx playwright install chromium`. Zoom belongs to the session, not to
// the pane showing it, and the pane's stored screen and session id have to
// survive a reload — a page load once pruned them, which quietly cost the
// operator their zoom and sent the pane off to start a fresh shell.
const { chromium } = require('playwright');
const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const font = (p) => p.$eval('.xterm-rows', (el) => getComputedStyle(el).fontSize);
const store = (p, prefix) => p.evaluate((pre) => Object.keys(localStorage)
  .filter((k) => k.startsWith(pre)).sort().map((k) => `${k.slice(pre.length)}=${localStorage.getItem(k)}`), prefix);
const results = [];
const check = (name, ok, detail = '') => { results.push(ok); console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`); };
const zoom = async (p, steps) => {
  const box = await p.$eval('.xterm-screen', (el) => { const b = el.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 }; });
  await p.mouse.move(box.x, box.y);
  for (let i = 0; i < Math.abs(steps); i++) {
    await p.keyboard.down('Control');
    await p.mouse.wheel(0, steps > 0 ? -120 : 120);
    await p.keyboard.up('Control');
    await p.waitForTimeout(150);
  }
  await p.waitForTimeout(700);
};

(async () => {
  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await ctx.newPage();
  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');
  await page.click('.cat-btn:has-text("원격")');
  await page.click('.rw-item:has-text("로컬 셸")');
  await page.waitForTimeout(700);
  const c = await page.$('.rw-connect-card button.btn-primary'); if (c) await c.click();
  await page.waitForSelector('.xterm-screen', { timeout: 20000 });
  await page.waitForTimeout(3500);

  await page.click('.xterm-screen');
  await page.keyboard.type('echo ZOOM-MARK-A');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(1200);
  await zoom(page, 4);
  const zoomA = await font(page);
  const sidBefore = await store(page, 'nabiman_term_sid_');
  console.log('session A zoom   :', zoomA, '| fonts:', JSON.stringify(await store(page, 'nabiman_term_font_')));
  check('zoom is stored under the session, not the pane',
    (await store(page, 'nabiman_term_font_')).some((k) => k.startsWith('sess_')));

  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.waitForSelector('.xterm-screen', { timeout: 20000 });
  await page.waitForTimeout(4000);
  check('a reload keeps the zoom', (await font(page)) === zoomA, `${await font(page)} vs ${zoomA}`);
  const sidAfter = await store(page, 'nabiman_term_sid_');
  check('a reload re-attaches to the same session',
    JSON.stringify(sidAfter) === JSON.stringify(sidBefore), `${JSON.stringify(sidAfter)} vs ${JSON.stringify(sidBefore)}`);
  check('a reload lands back in the same shell',
    (await page.$eval('.xterm-rows', (el) => el.textContent)).includes('ZOOM-MARK-A'));

  // A second session in the same pane, zoomed the other way.
  await page.click('button:has-text("새 세션")');
  await page.waitForTimeout(5000);
  await zoom(page, -5);
  const zoomB = await font(page);
  check('the second session takes its own size', zoomB !== zoomA, `${zoomB} vs ${zoomA}`);
  console.log('fonts now        :', JSON.stringify(await store(page, 'nabiman_term_font_')));

  // Back to the first session through the session list.
  await page.click('.rw-item:has-text("전체 현황")');
  await page.waitForTimeout(600);
  await page.click('.rw-tabs button:has-text("세션")');
  await page.waitForTimeout(2000);
  const cards = await page.$$('.session-card');
  console.log('session cards    :', cards.length);
  const sidA = sidBefore[0].split('=')[1];
  console.log('joining session  :', sidA);
  await page.locator('.session-card', { hasText: 'ZOOM-MARK-A' }).first()
    .locator('button:has-text("참여")').click();
  await page.waitForTimeout(5000);
  const backFont = await font(page);
  check('rejoining a session restores that session\'s zoom', backFont === zoomA,
    `${backFont} (A=${zoomA}, B=${zoomB})`);
  check('rejoining lands in that session\'s screen',
    (await page.$eval('.xterm-rows', (el) => el.textContent)).includes('ZOOM-MARK-A'));
  console.log('joined font      :', backFont);
  console.log('sids             :', JSON.stringify(await store(page, 'nabiman_term_sid_')));

  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
