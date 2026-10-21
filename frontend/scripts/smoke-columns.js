// Browser smoke test for the file list's columns and the folding server list.
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_COLUMNS_DIR=/tmp/columns-files npm run smoke:columns
//
// The folder holds big.bin (50 KB, 2025), small.txt (2020), c2.txt, c10.txt,
// a-very-long-file-name-that-needs-a-wide-column-to-be-read.log and zfolder/.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const DIR = process.env.SMOKE_COLUMNS_DIR;
if (!DIR) { console.log('FATAL: set SMOKE_COLUMNS_DIR'); process.exit(1); }

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
  await page.evaluate((dir) => {
    localStorage.setItem('nabiman_sftp_path_local', dir);
    ['nabiman_sftp_open', 'nabiman_sftp_columns', 'nabiman_rail_folded', 'nabiman_sftp_width'].forEach((k) => localStorage.removeItem(k));
    localStorage.setItem('nabiman_sftp_width', '520');
  }, DIR);
  await page.click('.cat-btn:has-text("원격")');
  await page.locator('.rw-item', { hasText: '로컬 셸' }).click();
  await page.click('.rw-tabs button:has-text("터미널")');
  const connect = page.locator('.rw-connect button', { hasText: '접속' });
  if (await connect.count()) await connect.click();
  const panel = page.locator('.rw-split-files');
  await panel.locator('.sftp-row', { hasText: 'small.txt' }).waitFor({ timeout: 20000 });
  const names = () => panel.locator('.sftp-rows .sftp-c-name').allTextContents();
  const heads = () => panel.locator('.sftp-colhead .sftp-colhead-label').allTextContents();

  // 1. Headings, and sorting by them.
  check('the list has headings', JSON.stringify(await heads()) === JSON.stringify(['이름', '크기', '수정한 때']), (await heads()).join(','));
  check('names sort as people read them, folders first', JSON.stringify(await names()) === JSON.stringify(
    ['zfolder', 'a-very-long-file-name-that-needs-a-wide-column-to-be-read.log', 'big.bin', 'c2.txt', 'c10.txt', 'small.txt']), (await names()).join(' '));
  await panel.locator('.sftp-colhead-btn', { hasText: '크기' }).click();
  const bySize = await names();
  check('크기 sorts by size', bySize[0] === 'zfolder' && bySize[bySize.length - 1] === 'big.bin', bySize.join(' '));
  await panel.locator('.sftp-colhead-btn', { hasText: '크기' }).click();
  check('again, the other way', (await names())[1] === 'big.bin' && await panel.locator('.sftp-ch-size').getAttribute('aria-sort') === 'descending');
  await panel.locator('.sftp-colhead-btn', { hasText: '수정한 때' }).click();
  const byTime = await names();
  check('수정한 때 sorts by date', byTime[1] === 'small.txt' && byTime[2] === 'big.bin', byTime.join(' '));

  // 2. A right-click chooses the columns; they are kept.
  await panel.locator('.sftp-colhead').click({ button: 'right' });
  await page.getByRole('menuitem', { name: /권한/ }).click();
  await panel.locator('.sftp-colhead').click({ button: 'right' });
  await page.getByRole('menuitem', { name: /소유자/ }).click();
  check('권한 and 소유자 can be shown', (await heads()).includes('권한') && (await heads()).includes('소유자'), (await heads()).join(','));
  const mode = await panel.locator('.sftp-row', { hasText: 'small.txt' }).locator('.sftp-c-mode').textContent();
  check('with each file\'s permissions', /^[r-][w-][xsS-]/.test(mode), mode);
  await panel.locator('.sftp-colhead').click({ button: 'right' });
  await page.getByRole('menuitem', { name: /크기/ }).click();
  check('and a column can be hidden', !(await heads()).includes('크기'));
  await page.reload({ waitUntil: 'networkidle' });
  await panel.locator('.sftp-row', { hasText: 'small.txt' }).waitFor({ timeout: 20000 });
  check('the choice and the order survive a reload', JSON.stringify(await heads()) === JSON.stringify(['이름', '수정한 때', '권한', '소유자'])
    && (await names())[1] === 'small.txt', (await heads()).join(','));

  // 3. Widths: drag an edge; double-click it to fit.
  const width = (sel) => panel.locator(sel).first().evaluate((el) => el.getBoundingClientRect().width);
  const before = await width('.sftp-ch-mtime');
  const grip = await panel.locator('.sftp-ch-mtime .sftp-colhead-grip').boundingBox();
  await page.mouse.move(grip.x + 4, grip.y + 4);
  await page.mouse.down();
  await page.mouse.move(grip.x + 64, grip.y + 4, { steps: 6 });
  await page.mouse.up();
  const after = await width('.sftp-ch-mtime');
  check('dragging an edge widens the column', after - before > 40, `${before} → ${after}`);
  const long = panel.locator('.sftp-row', { hasText: 'a-very-long' }).locator('.sftp-c-name');
  const cut = await long.evaluate((el) => el.scrollWidth > el.clientWidth + 1);
  await panel.locator('.sftp-ch-name .sftp-colhead-grip').dblclick();
  await page.waitForTimeout(200);
  const whole = await long.evaluate((el) => el.scrollWidth <= el.clientWidth + 1);
  check('double-clicking the name\'s edge fits the longest name', cut && whole);
  await page.screenshot({ path: `${OUT}/smoke-columns.png` });

  // 4. No help line under the list; the count of a selection still shows.
  check('the help line is gone', await panel.locator('.sftp-hint').count() === 0);
  await panel.locator('.sftp-row', { hasText: 'c2.txt' }).click();
  await panel.locator('.sftp-row', { hasText: 'c10.txt' }).click({ modifiers: ['Control'] });
  check('picking several says how many', (await panel.locator('.sftp-hint').textContent()).includes('2'));
  await panel.locator('.sftp-colhead').click({ button: 'right' });
  await page.getByRole('menuitem', { name: /처음대로/ }).click();
  check('the columns can be put back as they were', JSON.stringify(await heads()) === JSON.stringify(['이름', '크기', '수정한 때']));

  // 5. The server list folds to a tab and back, remembered.
  const workBefore = (await page.locator('.rw-work').boundingBox()).width;
  await page.locator('.rw-rail-fold').click();
  const workAfter = (await page.locator('.rw-work').boundingBox()).width;
  check('« folds the server list to a tab', await page.locator('.rw-rail.is-folded .rw-rail-tab').isVisible() && workAfter - workBefore > 150, `${workBefore} → ${workAfter}`);
  await page.reload({ waitUntil: 'networkidle' });
  check('and it stays folded', await page.locator('.rw-rail.is-folded').count() === 1);
  await page.locator('.rw-rail-tab').click();
  check('the tab brings it back', await page.locator('.rw-rail-list').isVisible());

  check('no page errors', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
