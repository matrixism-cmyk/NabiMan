// Browser smoke test for the file panel's preview and its delete question.
// (Video, audio and other files: smoke-media.js.)
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_PREVIEW_DIR=/tmp/preview-files npm run smoke:preview
//
// Needs a NabiMan server serving build/ and a folder on this machine (the
// local shell's files) holding: a-red.png (64×32), b-shape.svg (120×80, with a
// <script> that must never run), c-blue.png (1600×1200), broken.png (not an
// image), notes.txt, script.py (7 lines, one very long), memo-euckr.txt (EUC-KR
// Korean), big.log (12000 lines, over 300 KB), LICENSE (binary), pack.zip,
// report.pdf (3 pages: "First page NabiMan", …), locked.pdf (the same, with
// the password pdf-secret), fake.pdf (not a PDF), del-1.txt and del-2.txt.
// The two del- files are deleted.
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
  // Every file has a preview now: a-red, b-shape, big.log, broken, c-blue, …, script.py.
  check('it counts the folder\'s files', (await facts()).includes('1 / 15'), await facts());

  // 2. Arrows step through; an SVG shows without running its script.
  await page.keyboard.press('ArrowRight');
  await page.waitForFunction(() => document.querySelector('.sftp-preview img')?.alt === 'b-shape.svg');
  await page.waitForFunction(() => /120 × 80/.test(document.querySelector('.preview-facts')?.textContent || ''), null, { timeout: 10000 }).catch(() => {});
  check('→ shows the next image, an SVG', (await facts()).includes('120 × 80'), await facts());
  check('the SVG\'s script never runs', (await page.title()) !== 'pwned');
  await page.keyboard.press('ArrowRight');
  await page.waitForFunction(() => document.querySelector('.sftp-preview .preview-title')?.textContent === 'big.log');
  check('→ goes on into the texts', true);
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
  await page.keyboard.press('End');
  await page.waitForFunction(() => document.querySelector('.sftp-preview .preview-title')?.textContent === 'script.py');
  await page.keyboard.press('ArrowRight');
  await page.waitForFunction(() => document.querySelector('.sftp-preview img')?.alt === 'a-red.png');
  check('End, then → wraps around to the first', true);

  // 2b. Text and code.
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
  await row('script.py').dblclick();
  await page.waitForFunction(() => document.querySelectorAll('.sftp-preview .tp-row').length === 7, null, { timeout: 15000 }).catch(() => {});
  check('source opens with a row per line', await dialog.locator('.tp-row').count() === 7);
  await page.waitForFunction(() => document.querySelector('.sftp-preview .hljs-keyword'), null, { timeout: 10000 }).catch(() => {});
  check('and is coloured as Python', (await dialog.locator('.hljs-keyword').first().textContent()) === 'def' && (await facts()).includes('python'), await facts());
  const before = await dialog.locator('.tp-row').nth(4).boundingBox();
  await page.keyboard.press('w');
  const after = await dialog.locator('.tp-row').nth(4).boundingBox();
  check('W wraps the long line', after.height > before.height * 2, `${before.height} → ${after.height}`);
  await page.screenshot({ path: `${OUT}/smoke-preview-code.png` });
  await page.keyboard.press('w');
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
  await row('memo-euckr.txt').dblclick();
  await page.waitForFunction(() => /EUC-KR/.test(document.querySelector('.preview-facts')?.textContent || ''), null, { timeout: 15000 }).catch(() => {});
  check('an EUC-KR file reads as Korean', (await dialog.locator('.text-preview').textContent()).includes('한글 메모입니다'), await facts());
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
  await row('big.log').dblclick();
  await page.waitForFunction(() => document.querySelector('.sftp-preview .text-preview.is-block'), null, { timeout: 15000 }).catch(() => {});
  check('a large text opens as one light block', await dialog.locator('.text-preview.is-block').count() === 1 && (await facts()).includes('12,000줄'), await facts());
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
  await row('LICENSE').dblclick();
  await dialog.getByText(/바이너리 파일/).waitFor({ timeout: 15000 }).catch(() => {});
  check('a binary file named like text says so', await dialog.getByText(/바이너리 파일/).count() === 1);
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });

  // 2c. PDF.
  await row('report.pdf').dblclick();
  await page.waitForFunction(() => document.querySelectorAll('.sftp-preview .pdf-page').length === 3, null, { timeout: 20000 }).catch(() => {});
  check('a PDF opens with all its pages', await dialog.locator('.pdf-page').count() === 3 && (await facts()).includes('3쪽'), await facts());
  await page.waitForFunction(() => /First page NabiMan/.test(document.querySelector('.sftp-preview .pdf-page .textLayer')?.textContent || ''), null, { timeout: 15000 }).catch(() => {});
  check('its text is there to select', (await dialog.locator('.pdf-page').first().locator('.textLayer').textContent()).includes('First page NabiMan'));
  const painted = await page.evaluate(() => {
    const c = document.querySelector('.sftp-preview .pdf-page canvas');
    if (!c || !c.width) return false;
    const px = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
    for (let i = 0; i < px.length; i += 4) if (px[i] < 128 && px[i + 3] > 0) return true;
    return false;
  });
  check('the first page is drawn', painted);
  await page.screenshot({ path: `${OUT}/smoke-preview-pdf.png` });
  const zoomOf = () => dialog.locator('.pdf-zoom').textContent();
  const fitted100 = await zoomOf();
  await dialog.locator('.pdf-scroll').focus();
  await page.keyboard.press('+');
  check('+ zooms in', (await zoomOf()) !== fitted100, `${fitted100} → ${await zoomOf()}`);
  await page.keyboard.press('0');
  check('0 fits the width again', (await zoomOf()) === fitted100);
  await dialog.getByRole('textbox', { name: '쪽 번호' }).fill('3');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(600);
  const third = await page.evaluate(() => {
    const box = document.querySelector('.sftp-preview .pdf-scroll');
    const p3 = document.querySelector('.sftp-preview .pdf-page[data-page="3"]');
    return p3.getBoundingClientRect().top - box.getBoundingClientRect().top;
  });
  check('a page number jumps to it', Math.abs(third) < 60 || (await dialog.locator('.pdf-scroll').evaluate((b) => b.scrollTop + b.clientHeight >= b.scrollHeight - 2)), String(third));
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
  await row('locked.pdf').dblclick();
  const pw = dialog.getByLabel('PDF 암호');
  await pw.waitFor({ timeout: 20000 });
  await pw.fill('wrong');
  await page.keyboard.press('Enter');
  await dialog.getByText('암호가 맞지 않습니다').waitFor({ timeout: 10000 }).catch(() => {});
  check('a wrong PDF password is said so', await dialog.getByText('암호가 맞지 않습니다').count() === 1);
  await dialog.getByLabel('PDF 암호').fill('pdf-secret');
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => document.querySelectorAll('.sftp-preview .pdf-page').length === 3, null, { timeout: 15000 }).catch(() => {});
  check('the right password opens it', await dialog.locator('.pdf-page').count() === 3);
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
  await row('fake.pdf').dblclick();
  await dialog.getByText(/PDF를 읽을 수 없습니다/).waitFor({ timeout: 15000 }).catch(() => {});
  check('a broken PDF says so', await dialog.getByText(/PDF를 읽을 수 없습니다/).count() === 1);
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });

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

  // 4. Every file's menu offers a preview, an archive's too.
  await row('pack.zip').dblclick();
  // pack.zip here is only a ZIP's first four bytes; smoke-media lists a real one.
  await dialog.getByText(/ZIP의 목록을 읽을 수 없습니다/).waitFor({ timeout: 15000 }).catch(() => {});
  check('a broken ZIP says it cannot be listed', await dialog.getByText(/ZIP의 목록을 읽을 수 없습니다/).count() === 1);
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
  const menuFor = async (name) => {
    await row(name).click({ button: 'right' });
    await page.waitForSelector('[role="menuitem"]');
    const items = await page.$$eval('[role="menuitem"]', (els) => els.map((el) => el.textContent.trim()));
    await page.keyboard.press('Escape');
    return items;
  };
  check('an image\'s menu has 미리보기', (await menuFor('a-red.png')).some((i) => i.includes('미리보기')));
  check('a text file\'s menu has it too', (await menuFor('notes.txt')).some((i) => i.includes('미리보기')));
  check('an archive\'s menu has it as well', (await menuFor('pack.zip')).some((i) => i.includes('미리보기')));

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
