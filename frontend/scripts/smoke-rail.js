// Browser smoke test for the remote workspace's left rail.
//
//   SMOKE_URL=http://127.0.0.1:18096 npm run smoke:rail
//
// Needs a NabiMan server serving build/ (a throwaway data dir and password)
// and `npx playwright install chromium`. A plain click on a target has to show
// the sessions running there without attaching to any of them, and the
// right-click menu has to connect, open a new session, edit and delete — from
// the mouse and from the keyboard, and without spilling off the screen.
const { chromium } = require('playwright');
const { autoConfirm, answerConfirm } = require('./confirm-modal');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const SERVER = 'smoke-rail-target';

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));

  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');

  // A server to right-click on. Nothing listens there; the menu does not care.
  const created = await page.evaluate(async (name) => {
    const res = await fetch('/api/remote-servers', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}`,
      },
      body: JSON.stringify({ name, host: '127.0.0.1', port: 2, user: 'nobody', auth_method: 'key', tags: [], memo: '' }),
    });
    return res.json();
  }, SERVER);
  check('a throwaway server was saved', Boolean(created && created.success), created && created.message);

  await page.click('.cat-btn:has-text("원격")');
  await page.waitForSelector('.rw-rail');
  const local = page.locator('.rw-item', { hasText: '로컬 셸' });
  // The icon is aria-hidden, so a menu item's accessible name is just its label.
  const menuItem = (name) => page.getByRole('menuitem', { name, exact: true });

  // 1. A plain click shows the sessions, and dials nothing.
  await local.click();
  await page.waitForTimeout(800);
  check('a click shows the target\'s sessions', await page.isVisible('h3:has-text("로컬 셸 세션")'));
  check('a click does not open a terminal', !(await page.$('.xterm-screen')));
  // Shells left by an earlier run may still be there; count from here.
  const before = (await page.$$('.session-card')).length;

  // 2. The right-click menu, then 연결.
  await local.click({ button: 'right' });
  await page.waitForSelector('.rw-menu');
  const items = await page.$$eval('.rw-menu [role="menuitem"]', (els) => els.map((e) => e.textContent.trim()));
  check('the local shell menu has connect and new session', items.some((i) => i.endsWith('연결')) && items.some((i) => i.endsWith('새 세션')),
    items.join(' / '));
  await page.screenshot({ path: `${OUT}/rail-menu-local.png` });
  await menuItem('연결').click();
  await page.waitForSelector('.xterm-screen', { timeout: 20000 });
  await page.waitForTimeout(2500);
  check('연결 opens the terminal', Boolean(await page.$('.xterm-screen')));
  await page.click('.xterm-screen');
  await page.keyboard.type('echo RAIL-MARK-ONE');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(1500);

  // 3. Back on the list, the running shell shows up with what is on screen.
  await local.click();
  await page.waitForTimeout(1500);
  const cards = await page.$$('.session-card');
  const preview = await page.$$eval('.session-preview', (els) => els.map((e) => e.textContent).join('\n'));
  check('the session list shows the running shell', cards.length === before + 1 && preview.includes('RAIL-MARK-ONE'),
    `${before} → ${cards.length} card(s)`);

  // 4. 새 세션 from the menu starts a second shell next to it.
  await local.click({ button: 'right' });
  await menuItem('새 세션').click();
  await page.waitForSelector('.xterm-screen', { timeout: 20000 });
  await page.waitForTimeout(2500);
  const screenText = await page.$eval('.xterm-rows', (el) => el.textContent);
  check('새 세션 opens a fresh shell', !screenText.includes('RAIL-MARK-ONE'));
  await local.click();
  await page.waitForTimeout(1500);
  check('both shells are listed', (await page.$$('.session-card')).length === before + 2);

  // 5. Keyboard: Shift+F10 on a focused entry, arrows, Escape back to it.
  const server = page.locator('.rw-item', { hasText: SERVER });
  await server.focus();
  await page.keyboard.press('Shift+F10');
  await page.waitForSelector('.rw-menu');
  const focusedFirst = await page.evaluate(() => document.activeElement?.getAttribute('role'));
  await page.keyboard.press('ArrowDown');
  const focusedSecond = await page.evaluate(() => document.activeElement?.textContent?.trim());
  check('Shift+F10 opens the menu with focus inside', focusedFirst === 'menuitem' && focusedSecond.endsWith('새 세션'),
    `${focusedFirst} → ${focusedSecond}`);
  await page.keyboard.press('Escape');
  const backOnEntry = await page.evaluate(() => document.activeElement?.classList.contains('rw-item'));
  check('Escape closes it and returns focus', !(await page.$('.rw-menu')) && backOnEntry);

  // 6. Near the bottom-right corner the menu flips to stay on screen.
  const box = await server.boundingBox();
  await page.setViewportSize({ width: 1440, height: Math.round(box.y + box.height + 20) });
  // The smaller window may scroll the list; a scroll closes menus, so settle first.
  await server.scrollIntoViewIfNeeded();
  await page.waitForTimeout(300);
  const near = await server.boundingBox();
  await server.click({ button: 'right', position: { x: near.width - 4, y: near.height - 4 } });
  await page.waitForSelector('.rw-menu');
  const menuBox = await page.$eval('.rw-menu', (el) => {
    const r = el.getBoundingClientRect();
    return { bottom: r.bottom, right: r.right, vh: window.innerHeight, vw: window.innerWidth };
  });
  check('the menu stays inside the window', menuBox.bottom <= menuBox.vh && menuBox.right <= menuBox.vw,
    JSON.stringify(menuBox));
  await page.screenshot({ path: `${OUT}/rail-menu-server.png` });
  await page.keyboard.press('Escape');
  await page.setViewportSize({ width: 1440, height: 900 });

  // 7. 수정 opens the edit form for that server.
  await server.click({ button: 'right' });
  await menuItem('수정').click();
  await page.waitForTimeout(800);
  check('수정 opens the edit form', await page.isVisible('h3:has-text("서버 정보 수정")'));

  // 8. 삭제 asks first; cancelling keeps the server.
  await server.click({ button: 'right' });
  await menuItem('삭제').click();
  await answerConfirm(page, false);
  await page.waitForTimeout(1200);
  check('cancelling a delete keeps the server', await server.count() === 1);

  await server.click({ button: 'right' });
  await menuItem('삭제').click();
  await answerConfirm(page, true);
  await page.waitForTimeout(2000);
  check('삭제 removes the server', await page.locator('.rw-item', { hasText: SERVER }).count() === 0);

  check('no page errors', problems.length === 0, problems.slice(0, 3).join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
