// Browser smoke test for the remote workspace on its own (/remote) and the
// terminal settings dialog's size.
//
//   SMOKE_URL=http://127.0.0.1:18096 npm run smoke:standalone
const { chromium, devices } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};
async function signIn(page, path = '/') {
  await page.goto(`${BASE}${path}`, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
}

(async () => {
  const browser = await chromium.launch();
  const problems = [];
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));
  await page.evaluate(() => {}).catch(() => {});

  // 1. Signing in at /remote opens the workspace alone.
  await signIn(page, '/remote');
  await page.locator('.remote-shell').waitFor({ timeout: 15000 });
  check('/remote opens the remote workspace alone', await page.locator('.cat-nav').count() === 0 && await page.locator('.rw-rail').isVisible());
  check('with a slim bar and its title', (await page.locator('.remote-shell-bar').boundingBox()).height <= 48 && (await page.title()) === 'NabiMan 원격', await page.title());
  const rw = await page.locator('.rw').boundingBox();
  check('the workspace fills the window', rw.height >= 720 - 48 - 16 && rw.width >= 1280 - 16, JSON.stringify(rw));
  check('the address stays /remote', new URL(page.url()).pathname === '/remote' && !new URL(page.url()).hash, page.url());
  await page.locator('.rw-item', { hasText: '로컬 셸' }).click();
  await page.click('.rw-tabs button:has-text("터미널")');
  const connect = page.locator('.rw-connect button', { hasText: '접속' });
  if (await connect.count()) await connect.click();
  await page.waitForSelector('.rw-work .xterm-screen', { timeout: 20000 });
  check('a terminal works there', await page.locator('.rw-work .xterm-screen').isVisible());
  await page.screenshot({ path: `${OUT}/smoke-standalone.png` });

  // 2. The terminal settings dialog: wide, and never cut off.
  await page.locator('.rw-head-actions button[title="터미널 설정"]').click();
  const dialog = page.locator('.term-settings');
  await dialog.waitFor();
  const box = await dialog.boundingBox();
  check('the settings dialog is wide', box.width >= 800, String(box.width));
  check('and fits in the window', box.y >= 0 && box.y + box.height <= 720, JSON.stringify(box));
  const reach = await dialog.evaluate((el) => { el.scrollTop = el.scrollHeight; return el.scrollTop + el.clientHeight >= el.scrollHeight - 1; });
  check('its bottom can always be reached', reach);
  await page.screenshot({ path: `${OUT}/smoke-settings-dialog.png` });
  await dialog.locator('.term-settings-form').getByRole('button', { name: '취소' }).click();

  // 3. Folding, and the way back to NabiMan with the same sign-in.
  await page.locator('.remote-shell-bar button[aria-label^="상단 접기"]').click();
  check('▴ folds the bar away', await page.locator('.remote-shell-bar').count() === 0 && await page.locator('.chrome-handle').isVisible());
  await page.locator('.chrome-handle').click();
  await page.locator('.remote-shell-bar a', { hasText: '나비맨으로' }).click();
  await page.waitForSelector('.cat-nav', { timeout: 15000 });
  check('나비맨으로 goes back, still signed in', await page.locator('.app-header').isVisible());
  await page.click('.cat-btn:has-text("원격")');
  await page.locator('.rw-standalone').click();
  await page.locator('.remote-shell').waitFor({ timeout: 15000 });
  check('↗ in the workspace opens /remote, still signed in', new URL(page.url()).pathname === '/remote');

  // 4. On a phone.
  const phone = await (await browser.newContext({ ...devices['iPhone 13'] })).newPage();
  phone.on('pageerror', (e) => problems.push(`phone: ${e.message.split('\n')[0]}`));
  await signIn(phone, '/remote');
  await phone.locator('.remote-shell .rw-rail').waitFor({ timeout: 15000 });
  check('on a phone, /remote shows the list', await phone.locator('.rw-item', { hasText: '로컬 셸' }).isVisible()
    && await phone.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
  await phone.locator('.rw-item', { hasText: '로컬 셸' }).tap();
  await phone.locator('.rw-back').waitFor();
  check('and the work full screen', (await phone.locator('.rw-work').boundingBox()).y === 0);

  check('no page errors', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
