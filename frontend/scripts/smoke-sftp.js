// Browser smoke test for the file panel beside a remote terminal.
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_SFTP_SERVER=sftp-e2e \
//   SMOKE_SFTP_DIR=/tmp/sftp-scratch npm run smoke:sftp
//
// Needs a NabiMan server serving build/ with a saved server whose host key is
// approved and which accepts the server's SSH login (a throwaway sshd works),
// and an empty folder on it that the test may fill and empty. Everything it
// makes is inside SMOKE_SFTP_DIR.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const SERVER = process.env.SMOKE_SFTP_SERVER || 'sftp-e2e';
const DIR = process.env.SMOKE_SFTP_DIR;

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};

(async () => {
  if (!DIR) throw new Error('Set SMOKE_SFTP_DIR to an empty folder on the server');
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, acceptDownloads: true });
  const page = await context.newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));
  page.on('dialog', (d) => d.accept());

  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');

  // Open the panel straight in the scratch folder.
  const serverId = await page.evaluate(async (name) => {
    const res = await fetch('/api/remote-servers', {
      headers: { Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}` },
    });
    return ((await res.json()).data || []).find((s) => s.name === name)?.id;
  }, SERVER);
  check('the test server is saved', Boolean(serverId), SERVER);
  await page.evaluate(([id, dir]) => {
    localStorage.setItem(`nabiman_sftp_path_${id}`, dir);
    localStorage.removeItem('nabiman_sftp_open');
  }, [serverId, DIR]);

  await page.click('.cat-btn:has-text("원격")');
  await page.locator('.rw-item', { hasText: SERVER }).click();
  await page.click('.rw-tabs button:has-text("터미널")');
  await page.click('.rw-connect button:has-text("접속")');
  await page.waitForSelector('.xterm-screen', { timeout: 20000 });
  await page.waitForSelector('.sftp', { timeout: 20000 });
  await page.waitForFunction((dir) => document.querySelector('.sftp-crumbs .is-here')?.textContent === dir.split('/').pop(), DIR);
  check('the panel opens beside the terminal in the remembered folder', true);
  const termBox = await page.locator('.rw-split-term').boundingBox();
  const panelBox = await page.locator('.rw-split-files').boundingBox();
  check('the terminal keeps most of the width', termBox.width > panelBox.width * 2 && panelBox.x > termBox.x,
    `${Math.round(termBox.width)} / ${Math.round(panelBox.width)}`);

  // 1. A folder, then desktop files dropped onto it.
  await page.click('.sftp-tools button[aria-label="새 폴더"]');
  await page.fill('.sftp-edit', '받은 파일');
  await page.keyboard.press('Enter');
  await page.waitForSelector('.sftp-row:has-text("받은 파일")');
  const dropOn = async (selector, files) => {
    const dt = await page.evaluateHandle((list) => {
      const transfer = new DataTransfer();
      for (const [name, text] of list) transfer.items.add(new File([text], name, { type: 'text/plain' }));
      return transfer;
    }, files);
    await page.dispatchEvent(selector, 'dragover', { dataTransfer: dt });
    await page.dispatchEvent(selector, 'drop', { dataTransfer: dt });
  };
  await dropOn('.sftp-body', [['hello.txt', 'hello from the browser\n'], ['두번째.txt', '둘\n']]);
  await page.waitForSelector('.sftp-row:has-text("두번째.txt")', { timeout: 15000 });
  check('desktop files dropped on the panel are uploaded here', await page.isVisible('.sftp-row:has-text("hello.txt")'));
  await page.screenshot({ path: `${OUT}/sftp-panel.png` });

  // 2. A row dragged onto a folder moves into it.
  await page.dragAndDrop('.sftp-row:has-text("hello.txt")', '.sftp-row:has-text("받은 파일")');
  await page.waitForSelector('.sftp-row:has-text("hello.txt")', { state: 'detached', timeout: 10000 });
  await page.dblclick('.sftp-row:has-text("받은 파일")');
  await page.waitForSelector('.sftp-row:has-text("hello.txt")');
  check('a row dropped on a folder moves into it', true);

  // 3. Dragged onto the terminal, a row types its path.
  await page.click('.xterm-screen');
  await page.keyboard.type('cat ');
  await page.dragAndDrop('.sftp-row:has-text("hello.txt")', '.xterm-screen');
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => document.querySelector('.xterm-rows')?.textContent.includes('hello from the browser'),
    null, { timeout: 10000 }).catch(() => {});
  const screenText = await page.$eval('.xterm-rows', (el) => el.textContent);
  check('a row dropped on the terminal types its path', screenText.includes('hello from the browser'),
    screenText.slice(-160).replace(/\s+/g, ' '));

  // 4. Download from the menu.
  await page.click('.sftp-row:has-text("hello.txt")', { button: 'right' });
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('menuitem', { name: '내려받기', exact: true }).click(),
  ]);
  const saved = await download.path();
  const body = require('fs').readFileSync(saved, 'utf8');
  check('download returns the uploaded bytes', body === 'hello from the browser\n' && download.suggestedFilename() === 'hello.txt');

  // 5. Up through the ".." row, then delete the folder with its contents.
  await page.dblclick('.sftp-row.is-up');
  await page.waitForSelector('.sftp-row:has-text("받은 파일")');
  await page.click('.sftp-row:has-text("받은 파일")');
  await page.keyboard.press('Delete');
  await page.waitForSelector('.sftp-row:has-text("받은 파일")', { state: 'detached', timeout: 10000 });
  await page.click('.sftp-row:has-text("두번째.txt")');
  await page.keyboard.press('Delete');
  await page.waitForSelector('.sftp-row:has-text("두번째.txt")', { state: 'detached', timeout: 10000 });
  check('delete removes a folder with its contents, and a file', await page.isVisible('.sftp-empty'));

  // 6. Folded away and back after a reload, where it was.
  await page.click('.sftp-tools button[aria-label="파일 패널 닫기"]');
  check('the panel folds to a tab', await page.isVisible('.rw-split-tab') && !(await page.$('.sftp')));
  await page.reload({ waitUntil: 'networkidle' });
  await page.waitForSelector('.rw-split-tab', { timeout: 20000 });
  await page.click('.rw-split-tab');
  await page.waitForFunction((dir) => document.querySelector('.sftp-crumbs .is-here')?.textContent === dir.split('/').pop(), DIR);
  check('after a reload it reopens in the same folder', true);

  check('no page errors', problems.length === 0, problems.slice(0, 3).join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
