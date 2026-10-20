// Browser smoke test for the remote workspace on a phone (iPhone 13, touch).
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_MOBILE_DIR=/tmp/mobile-files npm run smoke:mobile
//
// Needs a NabiMan server serving build/, the local shell, and a folder on this
// machine holding a sub-folder "inner" with "a.txt" and "b.txt" in it.
const { chromium, devices } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const DIR = process.env.SMOKE_MOBILE_DIR;
if (!DIR) { console.log('FATAL: set SMOKE_MOBILE_DIR'); process.exit(1); }

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};

/** A finger held still on `locator` long enough to count as a long press. */
async function longPress(page, locator) {
  const box = await locator.boundingBox();
  const at = { clientX: box.x + box.width / 2, clientY: box.y + box.height / 2, pointerType: 'touch', isPrimary: true, bubbles: true };
  await locator.dispatchEvent('pointerdown', at);
  await page.waitForTimeout(700);
  await locator.dispatchEvent('pointerup', at);
  await locator.dispatchEvent('click', at);
}

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ ...devices['iPhone 13'] });
  const page = await context.newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));

  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');
  await page.evaluate((dir) => {
    localStorage.setItem('nabiman_sftp_path_local', dir);
    localStorage.removeItem('nabiman_remote_selection');
  }, DIR);

  // 1. The header is one line, its controls under ⋯.
  const header = await page.locator('.app-header').boundingBox();
  check('the header takes one line', header.height < 90, String(header.height));
  await page.locator('.header-more').tap();
  check('⋯ opens the header controls', await page.locator('.header-actions.is-open .logout-btn').isVisible());
  await page.locator('.header-more').tap();

  // 2. The list first, the work full screen on a tap, Back returns.
  await page.locator('.cat-btn', { hasText: '원격' }).tap();
  await page.locator('.rw-item', { hasText: '로컬 셸' }).waitFor();
  check('the list shows, the work does not', await page.locator('.rw-rail').isVisible() && !(await page.locator('.rw-work').isVisible()));
  const scroll = await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
  check('nothing scrolls sideways', scroll);
  await longPress(page, page.locator('.rw-item', { hasText: '로컬 셸' }));
  const menu = page.locator('.rw-menu');
  await menu.waitFor({ timeout: 5000 }).catch(() => {});
  const items = await menu.locator('[role="menuitem"]').allTextContents();
  check('a long press opens the entry\'s menu', items.some((i) => i.includes('연결')), items.join(' / '));
  check('without floating windows', !items.some((i) => i.includes('새 창')));
  check('and does not open the entry', !(await page.locator('.rw-work').isVisible()));
  await page.keyboard.press('Escape');
  await page.locator('.rw-item', { hasText: '로컬 셸' }).tap();
  await page.locator('.rw-back').waitFor();
  const work = await page.locator('.rw-work').boundingBox();
  check('a tap opens the work full screen', Math.round(work.width) === 390 && Math.round(work.y) === 0 && Math.round(work.height) >= 660, JSON.stringify(work));
  await page.goBack();
  await page.locator('.rw-rail').waitFor();
  check('the phone\'s Back returns to the list', !(await page.locator('.rw-work').isVisible()));
  await page.locator('.rw-item', { hasText: '로컬 셸' }).tap();

  // 3. The terminal, with the key bar.
  await page.locator('.rw-tabs button', { hasText: '터미널' }).tap();
  const connect = page.locator('.rw-connect button', { hasText: '접속' });
  if (await connect.count()) await connect.tap();
  await page.locator('.xterm-screen').waitFor({ timeout: 20000 });
  await page.waitForFunction(() => /\$\s*$/m.test([...document.querySelectorAll('.rw-work .xterm-rows > div')].map((r) => r.textContent).join('\n')), null, { timeout: 15000 }).catch(() => {});
  const keys = page.locator('.mkeys');
  check('a key bar sits under the terminal', await keys.isVisible());
  const rows = () => page.locator('.rw-work .xterm-rows > div').allTextContents();
  await page.locator('.rw-work .xterm-screen').tap();
  await page.keyboard.type('echo mobile-$((40+2))');
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => [...document.querySelectorAll('.rw-work .xterm-rows > div')].some((r) => r.textContent.trim() === 'mobile-42'), null, { timeout: 10000 }).catch(() => {});
  const before = (await rows()).filter((r) => r.trim() === 'mobile-42').length;
  await keys.getByRole('button', { name: '위' }).tap();
  await page.keyboard.press('Enter');
  await page.waitForFunction((n) => [...document.querySelectorAll('.rw-work .xterm-rows > div')].filter((r) => r.textContent.trim() === 'mobile-42').length > n, before, { timeout: 10000 }).catch(() => {});
  const after = (await rows()).filter((r) => r.trim() === 'mobile-42').length;
  check('↑ on the bar recalls the last command', after > before, `${before} → ${after}`);
  check('the bar keeps the keyboard\'s focus', await page.evaluate(() => document.activeElement?.classList.contains('xterm-helper-textarea')));
  await page.keyboard.type('sleep 30');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(500);
  await keys.getByRole('button', { name: 'Ctrl' }).tap();
  check('Ctrl holds for the next key', (await keys.getByRole('button', { name: 'Ctrl' }).getAttribute('aria-pressed')) === 'true');
  await page.keyboard.type('c');
  await page.waitForFunction(() => [...document.querySelectorAll('.rw-work .xterm-rows > div')].some((r) => r.textContent.includes('^C')), null, { timeout: 5000 }).catch(() => {});
  check('Ctrl then c interrupts', (await rows()).some((r) => r.includes('^C')));
  check('and Ctrl lets go after one key', (await keys.getByRole('button', { name: 'Ctrl' }).getAttribute('aria-pressed')) === 'false');
  await page.keyboard.type('ech');
  await keys.getByRole('button', { name: 'Tab' }).tap();
  await page.keyboard.type(' tab-ok');
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => [...document.querySelectorAll('.rw-work .xterm-rows > div')].some((r) => r.textContent.trim() === 'tab-ok'), null, { timeout: 8000 }).catch(() => {});
  check('Tab completes', (await rows()).some((r) => r.trim() === 'tab-ok'));
  const size = () => page.evaluate(() => getComputedStyle(document.querySelector('.rw-work .xterm-rows')).fontSize);
  const s0 = await size();
  await keys.getByRole('button', { name: '글자 크게' }).tap();
  await page.waitForTimeout(300);
  check('A+ makes the text larger', parseFloat(await size()) > parseFloat(s0), `${s0} → ${await size()}`);
  await keys.getByRole('button', { name: '글자 작게' }).tap();
  await page.screenshot({ path: `${OUT}/smoke-mobile-terminal.png` });

  // 4. The keyboard takes the bottom of the screen: the work shrinks above it.
  await page.setViewportSize({ width: 390, height: 420 });
  await page.waitForTimeout(500);
  const shrunk = await page.locator('.rw-work').boundingBox();
  const bar = await keys.boundingBox();
  check('the work follows the visible screen', Math.round(shrunk.height) <= 420 && bar.y + bar.height <= 421, `${shrunk.height} ${bar.y + bar.height}`);
  await page.setViewportSize({ width: 390, height: 664 });

  // 5. Files over the terminal: a tap opens a folder, a long press picks.
  await page.locator('.rw-icon-btn', { hasText: '📁' }).tap();
  const files = page.locator('.rw-split-files');
  await files.locator('.sftp-row', { hasText: 'inner' }).tap();
  await files.locator('.sftp-row', { hasText: 'a.txt' }).waitFor({ timeout: 10000 }).catch(() => {});
  check('a tap opens a folder', await files.locator('.sftp-row', { hasText: 'a.txt' }).count() === 1);
  await longPress(page, files.locator('.sftp-row', { hasText: 'a.txt' }));
  await page.locator('[role="menuitem"]').first().waitFor({ timeout: 5000 }).catch(() => {});
  check('a long press opens the file menu', await page.locator('[role="menuitem"]').count() > 0);
  await page.keyboard.press('Escape');
  await files.locator('.sftp-row', { hasText: 'b.txt' }).tap();
  const picked = await files.locator('.sftp-row.is-selected').count();
  check('then a tap adds to the selection', picked === 2, String(picked));
  await files.locator('.sftp-row', { hasText: 'a.txt' }).tap();
  await files.locator('.sftp-row', { hasText: 'b.txt' }).tap();
  check('and taps take them off again', await files.locator('.sftp-row.is-selected').count() === 0);
  await files.locator('.sftp-row', { hasText: 'a.txt' }).tap();
  await page.locator('.sftp-preview').waitFor({ timeout: 10000 }).catch(() => {});
  check('with nothing picked, a tap opens the file', await page.locator('.sftp-preview').count() === 1);
  await page.keyboard.press('Escape');
  await page.screenshot({ path: `${OUT}/smoke-mobile-files.png` });
  await page.locator('.rw-icon-btn', { hasText: '📁' }).tap();
  check('📁 again shows the terminal', !(await files.isVisible()));

  // 6. The rest under ⋯.
  await page.locator('.rw-icon-btn', { hasText: '⋯' }).tap();
  const sheet = page.locator('.rw-sheet');
  await sheet.waitFor();
  const sheetText = await sheet.textContent();
  check('⋯ offers a new session, sharing and settings', ['새 세션', '공유', '터미널 설정'].every((w) => sheetText.includes(w)), sheetText.slice(0, 80));
  await sheet.getByRole('button', { name: '닫기' }).tap();
  await sheet.waitFor({ state: 'detached' });
  check('the feedback button stays out of the way', !(await page.locator('[data-nabiman-feedback-launcher]').isVisible().catch(() => false)));

  check('no page errors', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
