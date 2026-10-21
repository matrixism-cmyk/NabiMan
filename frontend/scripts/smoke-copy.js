// Browser smoke test for copying between two servers' file panels.
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_COPY_FROM=sftp-e2e SMOKE_COPY_DIR=/tmp/copy-src \
//   SMOKE_COPY_TO=proto-ftp SMOKE_COPY_TO_DIR=/copy-smoke npm run smoke:copy
//
// Needs a NabiMan server serving build/ with two saved servers: one with a
// terminal and SFTP (its session goes into a floating window, files panel
// open), and one reached for its files (opened in the workspace pane). In
// SMOKE_COPY_DIR on the first: copy-a.txt, a folder copy-dir/ with a file in
// it, and huge.bin (a few GB, sparse is fine). SMOKE_COPY_TO_DIR on the
// second must be an empty folder; the test empties it again at the end.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const FROM = process.env.SMOKE_COPY_FROM || 'sftp-e2e';
const TO = process.env.SMOKE_COPY_TO || 'proto-ftp';
const DIR = process.env.SMOKE_COPY_DIR;
const TO_DIR = process.env.SMOKE_COPY_TO_DIR || '/copy-smoke';
if (!DIR) { console.log('FATAL: set SMOKE_COPY_DIR'); process.exit(1); }

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};

let page;
(async () => {
  const browser = await chromium.launch({ args: ['--no-sandbox'] });
  const context = await browser.newContext({ viewport: { width: 1600, height: 1000 } });
  page = await context.newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));
  page.on('dialog', (d) => { problems.push(`browser dialog: ${d.message()}`); d.dismiss(); });

  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');
  const api = (path, init = {}) => page.evaluate(async ({ path, init }) => {
    const res = await fetch(path, { ...init, headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}` } });
    return res.json();
  }, { path, init });
  const servers = (await api('/api/remote-servers')).data;
  const fromId = servers.find((s) => s.name === FROM).id;
  const toId = servers.find((s) => s.name === TO).id;
  await page.evaluate(({ fromId, toId, DIR, TO_DIR }) => {
    localStorage.setItem(`nabiman_sftp_path_${fromId}`, DIR);
    localStorage.setItem(`nabiman_sftp_path_${toId}`, TO_DIR);
    localStorage.setItem('nabiman_sftp_open_window', '1');
    localStorage.setItem('nabiman_lang', 'ko');
  }, { fromId, toId, DIR, TO_DIR });
  await page.reload({ waitUntil: 'networkidle' });
  await page.waitForSelector('.app-header');

  // The first server's session in a floating window, its files beside it.
  await page.click('.cat-btn:has-text("원격")');
  await page.locator('.rw-item', { hasText: FROM }).first().click();
  await page.click('.rw-tabs button:has-text("터미널")');
  const connect = await page.$('.rw-connect button:has-text("접속")');
  if (connect) await connect.click();
  await page.waitForSelector('.rw-work .xterm-screen', { timeout: 20000 });
  await page.click('.rw-head-actions button:has-text("분리")');
  const win = page.locator('.term-window').last();
  // Out of the way of the server list: down to the right, by its title bar.
  const bar = await win.locator('.term-window-title').boundingBox();
  await page.mouse.move(bar.x + 10, bar.y + bar.height / 2);
  await page.mouse.down();
  await page.mouse.move(900, 470, { steps: 8 });
  await page.mouse.up();
  const fromRow = (name) => win.locator('.sftp-row', { hasText: name }).first();
  await fromRow('copy-a.txt').waitFor({ timeout: 20000 });

  // The second server's files in the workspace pane.
  await page.locator('.rw-item', { hasText: TO }).first().click();
  await page.click('.rw-tabs button:has-text("파일")');
  const pane = page.locator('.rw-work .sftp');
  await pane.locator('.sftp-crumbs .is-here').waitFor({ timeout: 20000 });
  check('two servers\' panels are open side by side', await win.locator('.sftp').isVisible() && await pane.isVisible());

  const dragInto = async (names) => {
    await fromRow(names[0]).click();
    for (const n of names.slice(1)) await fromRow(n).click({ modifiers: ['Control'] });
    // The window covers part of the pane: drop near its top left, in the clear.
    await fromRow(names[0]).dragTo(pane.locator('.sftp-body'), { targetPosition: { x: 200, y: 120 } });
  };
  const ask = page.getByRole('alertdialog');
  const answer = async (label) => { await ask.getByRole('button', { name: label, exact: true }).click(); await ask.waitFor({ state: 'detached' }); };
  const copyRows = () => pane.locator('.sftp-transfer', { hasText: '⇄' });

  // 1. A file and a folder, copied after asking.
  await dragInto(['copy-a.txt', 'copy-dir']);
  await ask.waitFor();
  const question = await ask.textContent();
  check('dropping rows from another server asks first, naming both places and what goes', question.includes(FROM) && question.includes(TO)
    && question.includes('copy-a.txt') && question.includes('copy-dir'), question.slice(0, 160));
  await answer('복사');
  await pane.locator('.sftp-transfer.is-done', { hasText: '⇄' }).first().waitFor({ timeout: 30000 });
  check('the copy shows in the target panel and finishes', (await copyRows().first().textContent()).includes(`${FROM} → ${TO}`), await copyRows().first().textContent());
  await pane.locator('.sftp-row', { hasText: 'copy-a.txt' }).waitFor({ timeout: 10000 });
  check('the copied file and folder appear without a refresh', await pane.locator('.sftp-row', { hasText: 'copy-dir' }).count() === 1);
  const inside = (await api(`/api/terminal/sftp/${toId}/list?path=${encodeURIComponent(`${TO_DIR}/copy-dir`)}`)).data.entries.map((e) => e.name);
  check('the folder arrived with what was in it', inside.includes('inner.txt'), inside.join(','));
  check('the source still has its files (a copy, not a move)', await fromRow('copy-a.txt').count() === 1);
  await page.screenshot({ path: `${OUT}/smoke-copy.png` });

  // 2. Names already there are asked about; no keeps them, yes replaces them.
  await dragInto(['copy-a.txt']);
  await answer('복사');
  await ask.waitFor();
  check('a name already there is asked about, as a danger', (await ask.textContent()).includes('copy-a.txt') && await ask.getByRole('button', { name: '덮어쓰기' }).count() === 1);
  const before = await copyRows().count();
  await answer('취소');
  await page.waitForTimeout(800);
  check('saying no starts nothing', await copyRows().count() === before);
  await dragInto(['copy-a.txt']);
  await answer('복사');
  await ask.waitFor();
  await answer('덮어쓰기');
  await page.waitForFunction(({ n }) => document.querySelectorAll('.rw-work .sftp-transfer.is-done').length > n, { n: before - 0 }, { timeout: 30000 }).catch(() => {});
  check('saying yes replaces it', await pane.locator('.sftp-transfer.is-done', { hasText: '⇄' }).count() >= 2);

  // 3. A big copy can be cancelled from its row.
  await dragInto(['huge.bin']);
  await answer('복사');
  const running = pane.locator('.sftp-transfer.is-running', { hasText: '⇄' }).first();
  await running.waitFor({ timeout: 15000 });
  await page.waitForFunction(() => /\d+%/.test(document.querySelector('.rw-work .sftp-transfer.is-running')?.textContent || ''), null, { timeout: 15000 }).catch(() => {});
  check('a running copy shows how far it is', /\d+%/.test(await running.textContent()), await running.textContent());
  await running.getByRole('button', { name: '전송 취소' }).click();
  await pane.locator('.sftp-transfer.is-cancelled', { hasText: '⇄' }).waitFor({ timeout: 15000 });
  await page.waitForTimeout(1000);
  const left = (await api(`/api/terminal/sftp/${toId}/list?path=${encodeURIComponent(TO_DIR)}`)).data.entries.map((e) => e.name);
  check('a cancelled copy leaves no file, part or whole', !left.some((n) => n.includes('huge')), left.join(','));

  // 4. Within one server, a drag still moves (and asks nothing).
  await pane.locator('.sftp-row', { hasText: 'copy-a.txt' }).dragTo(pane.locator('.sftp-row', { hasText: 'copy-dir' }));
  await page.waitForTimeout(1500);
  check('within one panel a drag still moves, without asking', await ask.count() === 0
    && (await api(`/api/terminal/sftp/${toId}/list?path=${encodeURIComponent(`${TO_DIR}/copy-dir`)}`)).data.entries.some((e) => e.name === 'copy-a.txt'));

  for (const name of ['copy-dir', 'copy-a.txt']) {
    await api(`/api/terminal/sftp/${toId}/delete`, { method: 'POST', body: JSON.stringify({ path: `${TO_DIR}/${name}` }) });
  }
  check('no page errors and no browser dialogs', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.log('FATAL:', e.message.split('\n').slice(0, 3).join(' / '));
  await page?.screenshot({ path: `${OUT}/smoke-copy-failed.png` }).catch(() => undefined);
  process.exit(1);
});
