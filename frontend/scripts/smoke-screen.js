// Browser smoke test for room on screen and terminal fonts.
//
//   SMOKE_URL=http://127.0.0.1:18096 npm run smoke:screen
//
// The top folds away (the ▴ tab under the menu) and comes back (▾), remembered; the remote
// workspace has its own settings tab; terminals use the bundled coding font
// (D2Coding by default, Korean two cells wide), and a font picked in the
// settings applies to an open terminal; Esc inside a terminal does not end
// focus mode.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};
const api = (page, path, init = {}) => page.evaluate(async ({ path, init }) => {
  const res = await fetch(path, { ...init, headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}` } });
  return res.json();
}, { path, init });

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
  await page.evaluate(() => localStorage.removeItem('nabiman_chrome_hidden'));
  const settings = (await api(page, '/api/terminal/settings')).data;
  await api(page, '/api/terminal/settings', { method: 'PUT', body: JSON.stringify({ ...settings, font_family: 'd2coding' }) });
  await page.reload({ waitUntil: 'networkidle' });

  // 1. The top folds away and comes back, remembered.
  await page.click('.cat-btn:has-text("원격")');
  const top = async () => (await page.locator('.rw').boundingBox()).y;
  const before = await top();
  await page.locator('.chrome-fold').click();
  check('▴ folds away the logo and the menu', !(await page.locator('.app-header').isVisible()) && !(await page.locator('.cat-nav').isVisible()));
  const after = await top();
  check('the workspace moves to the top of the screen', after <= 8 && before - after > 100, `${before} → ${after}`);
  await page.reload({ waitUntil: 'networkidle' });
  check('and stays folded after a reload', !(await page.locator('.app-header').isVisible()));
  await page.screenshot({ path: `${OUT}/smoke-screen-folded.png` });
  await page.locator('.chrome-handle').click();
  check('▾ brings it back', await page.locator('.app-header').isVisible());

  // 2. The remote workspace's own settings.
  await page.locator('.rw-item', { hasText: '전체 현황' }).click();
  await page.locator('.rw-tabs button', { hasText: '환경설정' }).click();
  const box = page.locator('.rw-settings');
  await box.waitFor();
  check('the remote workspace has a settings tab', await box.locator('h3', { hasText: '터미널' }).count() >= 1);
  check('folding the top is no longer a setting there (the tab under the menu does it)', await box.locator('.rw-settings-check').count() === 0);
  check('the header has no fold button of its own any more', await page.locator('.app-header button[aria-label^="상단 접기"]').count() === 0);

  // 3. Terminals use D2Coding, loaded, Korean two cells wide.
  await page.locator('.rw-item', { hasText: '로컬 셸' }).click();
  await page.click('.rw-tabs button:has-text("터미널")');
  const connect = page.locator('.rw-connect button', { hasText: '접속' });
  if (await connect.count()) await connect.click();
  await page.waitForSelector('.rw-work .xterm-screen', { timeout: 20000 });
  // Keys typed before the connection opens go nowhere.
  await page.locator('.rw-head-actions .status-badge.up').waitFor({ timeout: 20000 });
  await page.locator('.rw-work .xterm-screen').click();
  await page.keyboard.type("printf '한글ab|\\nabcdef|\\n'");
  await page.keyboard.press('Enter');
  // Wait for the output itself, not a fixed time: a busy machine is slower.
  await page.waitForFunction(() => [...document.querySelectorAll('.rw-work .xterm-rows > div')]
    .some((r) => r.textContent.startsWith('abcdef|')), null, { timeout: 15000 }).catch(() => {});
  await page.waitForTimeout(500);
  const font = await page.evaluate(() => ({
    family: getComputedStyle(document.querySelector('.rw-work .xterm-rows')).fontFamily,
    loaded: document.fonts.check('14px "NabiMan D2Coding"'),
  }));
  check('the terminal uses the bundled D2Coding', font.family.includes('NabiMan D2Coding') && font.loaded, JSON.stringify(font));
  const widths = await page.evaluate(() => {
    const rows = [...document.querySelectorAll('.rw-work .xterm-rows > div')];
    const width = (text) => {
      const row = rows.find((r) => r.textContent === text || r.textContent.startsWith(text));
      if (!row) return null;
      const range = document.createRange();
      range.selectNodeContents(row);
      const rects = [...range.getClientRects()];
      return rects.length ? rects[rects.length - 1].right - rects[0].left : null;
    };
    return { korean: width('한글ab|'), latin: width('abcdef|') };
  });
  check('two Korean letters take the room of four Latin ones', widths.korean && widths.latin && Math.abs(widths.korean - widths.latin) < 1.5, JSON.stringify(widths));
  await page.screenshot({ path: `${OUT}/smoke-screen-d2coding.png` });

  // 4. Another font from the ⚙ settings applies to the open terminal.
  await page.locator('.rw-head-actions button[title="터미널 설정"]').click();
  const dialog = page.locator('.term-settings');
  // From a session the dialog opens on that session; the font for all is on the other side.
  const all = dialog.getByRole('tab', { name: '전체 기본값' });
  if (await all.count()) await all.click();
  await dialog.locator('#term-font').selectOption('jetbrains');
  check('the picker shows a sample in the font', (await dialog.locator('.term-font-sample').getAttribute('style')).includes('JetBrains'));
  await dialog.locator('.term-settings-form').getByRole('button', { name: '저장' }).click();
  await dialog.waitFor({ state: 'detached' });
  await page.waitForFunction(() => getComputedStyle(document.querySelector('.rw-work .xterm-rows')).fontFamily.includes('JetBrains'), null, { timeout: 10000 }).catch(() => {});
  const now = await page.evaluate(() => ({
    family: getComputedStyle(document.querySelector('.rw-work .xterm-rows')).fontFamily,
    hangul: document.fonts.check('14px "NabiMan Hangul"', '한'),
  }));
  check('a new font applies to the open terminal, Korean from D2Coding', now.family.includes('NabiMan JetBrains Mono') && now.family.includes('NabiMan Hangul'), JSON.stringify(now));
  check('and is kept for the account', (await api(page, '/api/terminal/settings')).data.font_family === 'jetbrains');
  await api(page, '/api/terminal/settings', { method: 'PUT', body: JSON.stringify({ ...settings, font_family: 'd2coding' }) });

  // 5. Esc in a terminal leaves focus mode alone.
  await page.locator('.app-header button[title="집중 모드"], .app-header button:has-text("⛶")').first().click();
  await page.waitForTimeout(500);
  await page.locator('.rw-work .xterm-screen').click();
  await page.keyboard.press('Escape');
  await page.waitForTimeout(300);
  check('Esc inside a terminal does not end focus mode', await page.locator('.app.focus-mode').count() === 1);

  check('no page errors', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
