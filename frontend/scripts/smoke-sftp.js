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

  // 4b. Dragged out of the browser, a file is saved from a single-use link.
  await page.hover('.sftp-row:has-text("hello.txt")');
  await page.waitForTimeout(800);
  const downloadUrl = await page.evaluate(() => {
    const row = Array.from(document.querySelectorAll('.sftp-row')).find((r) => r.textContent.includes('hello.txt'));
    const dt = new DataTransfer();
    row.dispatchEvent(new DragEvent('dragstart', { dataTransfer: dt, bubbles: true }));
    row.dispatchEvent(new DragEvent('dragend', { dataTransfer: dt, bubbles: true }));
    return dt.getData('DownloadURL');
  });
  const linkUrl = downloadUrl.split(':').slice(2).join(':');
  check('a file dragged out carries a desktop download', downloadUrl.startsWith('application/octet-stream:hello.txt:http'),
    downloadUrl.slice(0, 60));
  // The browser fetches it without the page's sign-in, and only once.
  const first = await fetch(linkUrl);
  const firstBody = await first.text();
  const again = await fetch(linkUrl);
  check('the link downloads once without a sign-in, then is spent',
    first.status === 200 && firstBody === 'hello from the browser\n' && again.status === 401, `${first.status} → ${again.status}`);

  // 5. Up through the ".." row, then delete the folder with its contents.
  await page.dblclick('.sftp-row.is-up');
  await page.waitForSelector('.sftp-row:has-text("받은 파일")');
  // 5a. A folder downloads as one ZIP archive of everything in it.
  await page.click('.sftp-row:has-text("받은 파일")', { button: 'right' });
  const [zipDownload] = await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('menuitem', { name: '폴더 내려받기 (ZIP)', exact: true }).click(),
  ]);
  const zipPath = await zipDownload.path();
  const listed = require('child_process').execFileSync('python3', ['-c',
    'import sys, zipfile; z = zipfile.ZipFile(sys.argv[1]); assert z.testzip() is None; '
    + 'print(sorted(z.namelist())); print(z.read("받은 파일/hello.txt").decode(), end="")', zipPath]).toString();
  check('a folder downloads as a ZIP of its contents', zipDownload.suggestedFilename() === '받은 파일.zip'
    && listed.includes("'받은 파일/hello.txt'") && listed.endsWith('hello from the browser\n'), listed.split('\n')[0]);

  await page.click('.sftp-row:has-text("받은 파일")');
  await page.keyboard.press('Delete');
  await page.waitForSelector('.sftp-row:has-text("받은 파일")', { state: 'detached', timeout: 10000 });
  await page.click('.sftp-row:has-text("두번째.txt")');
  await page.keyboard.press('Delete');
  await page.waitForSelector('.sftp-row:has-text("두번째.txt")', { state: 'detached', timeout: 10000 });
  check('delete removes a folder with its contents, and a file', await page.isVisible('.sftp-empty'));

  // 6. Folded away and back after a reload, where it was.
  await page.click('.sftp-tools button[aria-label="파일 패널 숨기기"]');
  check('the panel folds to a tab', await page.isVisible('.rw-split-tab') && !(await page.$('.sftp')));
  await page.reload({ waitUntil: 'networkidle' });
  await page.waitForSelector('.rw-split-tab', { timeout: 20000 });
  await page.click('.rw-split-tab');
  await page.waitForFunction((dir) => document.querySelector('.sftp-crumbs .is-here')?.textContent === dir.split('/').pop(), DIR);
  check('after a reload it reopens in the same folder', true);

  // 7. The local shell has this machine's files beside it.
  await page.locator('.rw-item', { hasText: '로컬 셸' }).click();
  await page.click('.rw-tabs button:has-text("터미널")');
  const connectLocal = await page.$('.rw-connect button:has-text("접속")');
  if (connectLocal) await connectLocal.click();
  // The path bar is there before the listing; wait for the rows themselves.
  const localRows = await page.waitForSelector('.rw-view-terminal:not(.rw-view-hidden) .sftp-row', { timeout: 20000 })
    .then(() => true, () => false);
  check('the local shell has a file panel', localRows);

  // 8. A floating window starts with the panel folded, and opens it on request.
  await page.locator('.rw-item', { hasText: SERVER }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: '새 창으로 열기', exact: true }).click();
  await page.waitForSelector('.term-window .rw-split-tab', { timeout: 20000 });
  await page.click('.term-window .rw-split-tab');
  await page.waitForSelector('.term-window .sftp-row', { timeout: 20000 });
  check('a floating window opens the panel from its tab', true);
  await page.screenshot({ path: `${OUT}/sftp-window.png` });

  // 9. Share links: files only when the link grants them, and only as far.
  const api = (path, init = {}) => page.evaluate(async ([p, i]) => {
    const res = await fetch(p, { ...i, headers: { 'Content-Type': 'application/json',
      Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}`, ...(i.headers || {}) } });
    return { status: res.status, body: await res.json().catch(() => null) };
  }, [path, init]);
  const sessions = (await api('/api/terminal/sessions')).body.data;
  const shared = sessions.find((x) => x.alive && x.label.includes(':22022'));
  const makeLink = async (files) => (await api(`/api/terminal/sessions/${shared.id}/share-link`, {
    method: 'POST', body: JSON.stringify({ expires_in_secs: 3600, password: '', read_only: true, files }) })).body.data.token;
  const readLink = await makeLink('read');
  const writeLink = await makeLink('write');
  const plainLink = await makeLink('none');
  require('fs').writeFileSync(`${DIR}/shared.txt`, 'for the visitor\n');

  const visitor = await browser.newContext({ viewport: { width: 1280, height: 800 }, acceptDownloads: true });
  const guest = await visitor.newPage();
  guest.on('pageerror', (e) => problems.push(`guest: ${e.message.split('\n')[0]}`));
  guest.on('dialog', (d) => d.accept());
  const openShare = async (token) => {
    await guest.evaluate((dir) => {
      Object.keys(localStorage).filter((k) => k.startsWith('nabiman_sftp_path_share_')).forEach((k) => localStorage.setItem(k, dir));
    }, DIR).catch(() => {});
    await guest.goto(`${BASE}/share/${token}`, { waitUntil: 'networkidle' });
    await guest.evaluate(([t, dir]) => localStorage.setItem(`nabiman_sftp_path_share_${t.slice(0, 16)}`, dir), [token, DIR]);
    await guest.reload({ waitUntil: 'networkidle' });
  };

  await openShare(plainLink);
  await guest.waitForSelector('.xterm-screen', { timeout: 20000 });
  await guest.waitForTimeout(1500);
  check('a link without files shows no file panel', !(await guest.$('.sftp')) && !(await guest.$('.rw-split-tab')));
  const noKey = await guest.evaluate(async (t) => (await fetch(`/api/share/${t}/files/list`)).status, plainLink);
  check('and its file API refuses a visitor', noKey === 401, String(noKey));

  await openShare(readLink);
  await guest.waitForSelector('.sftp-row:has-text("shared.txt")', { timeout: 20000 });
  check('a browse-only link lists files for the visitor', true);
  check('without write controls', !(await guest.$('.sftp-tools button[aria-label="파일 올리기"]'))
    && !(await guest.$('.sftp-tools button[aria-label="새 폴더"]')));
  await guest.click('.sftp-row:has-text("shared.txt")', { button: 'right' });
  const [guestDownload] = await Promise.all([
    guest.waitForEvent('download'),
    guest.getByRole('menuitem', { name: '내려받기', exact: true }).click(),
  ]);
  check('the visitor can download', require('fs').readFileSync(await guestDownload.path(), 'utf8') === 'for the visitor\n');
  await guest.screenshot({ path: `${OUT}/sftp-share.png` });

  await openShare(writeLink);
  await guest.waitForSelector('.sftp-tools button[aria-label="새 폴더"]', { timeout: 20000 });
  const dt = await guest.evaluateHandle(() => { const t = new DataTransfer(); t.items.add(new File(['from a visitor\n'], 'visitor.txt')); return t; });
  await guest.dispatchEvent('.sftp-body', 'dragover', { dataTransfer: dt });
  await guest.dispatchEvent('.sftp-body', 'drop', { dataTransfer: dt });
  await guest.waitForSelector('.sftp-row:has-text("visitor.txt")', { timeout: 15000 });
  check('a writable link lets the visitor upload', require('fs').readFileSync(`${DIR}/visitor.txt`, 'utf8') === 'from a visitor\n');

  // Revoking the link ends file access at once.
  await api(`/api/terminal/share-links/${writeLink}`, { method: 'DELETE' });
  await guest.click('.sftp-tools button[aria-label="새로고침"]');
  await guest.waitForSelector('.sftp-msg.is-error', { timeout: 10000 });
  check('revoking the link ends file access', true, await guest.textContent('.sftp-msg'));
  await visitor.close();
  for (const token of [readLink, plainLink]) await api(`/api/terminal/share-links/${token}`, { method: 'DELETE' });

  check('no page errors', problems.length === 0, problems.slice(0, 3).join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
