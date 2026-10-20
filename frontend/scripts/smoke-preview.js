// Browser smoke test for the file panel's preview and its delete question.
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_PREVIEW_DIR=/tmp/preview-files npm run smoke:preview
//
// Needs a NabiMan server serving build/ and a folder on this machine (the
// local shell's files) holding: a-red.png (64×32), b-shape.svg (120×80, with a
// <script> that must never run), c-blue.png (1600×1200), broken.png (not an
// image), notes.txt, del-1.txt and del-2.txt. The two del- files are deleted.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const DIR = process.env.SMOKE_PREVIEW_DIR;
if (!DIR) { console.log('FATAL: set SMOKE_PREVIEW_DIR'); process.exit(1); }

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
  page.on('dialog', (d) => { problems.push(`browser dialog: ${d.message()}`); d.dismiss(); });

  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');
  await page.evaluate((dir) => { localStorage.setItem('nabiman_sftp_path_local', dir); localStorage.removeItem('nabiman_sftp_open'); }, DIR);
  await page.click('.cat-btn:has-text("원격")');
  await page.locator('.rw-item', { hasText: '로컬 셸' }).click();
  await page.click('.rw-tabs button:has-text("터미널")');
  const connect = await page.$('.rw-connect button:has-text("접속")');
  if (connect) await connect.click();
  const panel = page.locator('.rw-split-files');
  await panel.getByText('a-red.png', { exact: true }).waitFor({ timeout: 20000 });
  const row = (name) => panel.locator('.sftp-row', { hasText: name }).first();
  const dialog = page.locator('.sftp-preview');
  const facts = () => dialog.locator('.preview-facts').textContent();

  // 1. Double-click an image: it opens in place, at its real size.
  await row('a-red.png').dblclick();
  await dialog.locator('img').waitFor({ timeout: 15000 });
  await page.waitForFunction(() => document.querySelector('.sftp-preview img')?.naturalWidth > 0);
  check('double-click opens the image', (await dialog.locator('img').getAttribute('alt')) === 'a-red.png');
  await page.waitForFunction(() => /×/.test(document.querySelector('.preview-facts')?.textContent || ''));
  check('its size is shown', (await facts()).includes('64 × 32'), await facts());
  check('it counts the folder\'s images', (await facts()).includes('1 / 4'), await facts());

  // 2. Arrows step through; an SVG shows without running its script.
  await page.keyboard.press('ArrowRight');
  await page.waitForFunction(() => document.querySelector('.sftp-preview img')?.alt === 'b-shape.svg');
  await page.waitForFunction(() => /120 × 80/.test(document.querySelector('.preview-facts')?.textContent || ''), null, { timeout: 10000 }).catch(() => {});
  check('→ shows the next image, an SVG', (await facts()).includes('120 × 80'), await facts());
  check('the SVG\'s script never runs', (await page.title()) !== 'pwned');
  await page.keyboard.press('ArrowRight');
  await page.waitForFunction(() => document.querySelector('.sftp-preview .preview-title')?.textContent === 'broken.png');
  await page.waitForFunction(() => /그림으로 열 수 없습니다/.test(document.querySelector('.preview-stage')?.textContent || ''), null, { timeout: 10000 }).catch(() => {});
  check('a file that is not really an image says so', (await dialog.locator('.preview-stage').textContent()).includes('그림으로 열 수 없습니다'));
  await page.keyboard.press('ArrowRight');
  await page.waitForFunction(() => document.querySelector('.sftp-preview img')?.alt === 'c-blue.png');
  await page.waitForFunction(() => document.querySelector('.sftp-preview img')?.naturalWidth === 1600);
  const fitted = await dialog.locator('img').boundingBox();
  const stage = await dialog.locator('.preview-stage').boundingBox();
  check('a large image is fitted to the window', fitted.width <= stage.width + 1 && fitted.height <= stage.height + 1);
  await page.screenshot({ path: `${OUT}/smoke-preview-fit.png` });
  await page.keyboard.press('f');
  const actual = await dialog.locator('img').boundingBox();
  check('F shows it at its own size', Math.round(actual.width) === 1600, String(actual.width));
  await page.keyboard.press('ArrowRight');
  await page.waitForFunction(() => document.querySelector('.sftp-preview img')?.alt === 'a-red.png');
  check('→ wraps around to the first', true);

  // 3. Escape closes it; Space on a selected image opens it again.
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
  check('Escape closes the preview, leaving the terminal pane alone', await page.isVisible('.xterm-screen'));
  await row('b-shape.svg').click();
  await page.keyboard.press(' ');
  await dialog.locator('img').waitFor({ timeout: 15000 });
  check('Space previews the selected image', (await dialog.locator('img').getAttribute('alt')) === 'b-shape.svg');
  await page.click('.sftp-preview button[aria-label="닫기"]');
  await dialog.waitFor({ state: 'detached' });

  // 4. The menu offers a preview only for what can be previewed.
  const menuFor = async (name) => {
    await row(name).click({ button: 'right' });
    await page.waitForSelector('[role="menuitem"]');
    const items = await page.$$eval('[role="menuitem"]', (els) => els.map((el) => el.textContent.trim()));
    await page.keyboard.press('Escape');
    return items;
  };
  check('an image\'s menu has 미리보기', (await menuFor('a-red.png')).some((i) => i.includes('미리보기')));
  check('a text file\'s menu does not (yet)', !(await menuFor('notes.txt')).some((i) => i.includes('미리보기')));

  // 5. Deleting asks in the app's own modal, naming what goes.
  await row('del-1.txt').click();
  await row('del-2.txt').click({ modifiers: ['Control'] });
  await page.keyboard.press('Delete');
  const ask = page.getByRole('alertdialog');
  await ask.waitFor();
  check('Delete asks in a modal listing both files', (await ask.textContent()).includes('del-1.txt') && (await ask.textContent()).includes('del-2.txt'));
  check('the question starts on 취소', await page.evaluate(() => document.activeElement?.textContent === '취소'));
  await page.screenshot({ path: `${OUT}/smoke-preview-confirm.png` });
  await page.keyboard.press('Escape');
  await ask.waitFor({ state: 'detached' });
  check('Escape keeps the files', await row('del-1.txt').isVisible() && await row('del-2.txt').isVisible());
  await page.keyboard.press('Delete');
  await ask.waitFor();
  await ask.getByRole('button', { name: '삭제' }).click();
  await panel.getByText('del-1.txt', { exact: true }).waitFor({ state: 'detached', timeout: 10000 }).catch(() => {});
  check('삭제 removes them', !(await panel.getByText('del-1.txt', { exact: true }).count()) && !(await panel.getByText('del-2.txt', { exact: true }).count()));

  check('no page errors and no browser dialogs', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
