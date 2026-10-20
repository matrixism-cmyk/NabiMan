// Browser smoke test for renaming a session where it is open.
//
//   SMOKE_URL=http://127.0.0.1:18096 npm run smoke:rename
//
// Needs a NabiMan server serving build/ (a throwaway data dir and password)
// and `npx playwright install chromium`. The name has to be editable from the
// terminal's header and from a floating window's title bar, not only from the
// session list, and typing it must neither reach the shell nor drag the window.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const HEAD_NAME = `smoke-head-${Date.now() % 100000}`;
const WINDOW_NAME = `smoke-window-${Date.now() % 100000}`;

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};

const api = (page, path, init = {}) => page.evaluate(async ({ path, init }) => {
  const res = await fetch(path, {
    ...init,
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}` },
  });
  return res.json();
}, { path, init });

const titleOf = async (page, id) => {
  const res = await api(page, '/api/terminal/sessions');
  return (res.data || []).find((s) => s.id === id)?.title;
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
  await page.click('.cat-btn:has-text("원격")');
  await page.click('.rw-item:has-text("로컬 셸")');
  await page.click('.rw-tabs button:has-text("터미널")');
  await page.waitForTimeout(700);
  const connect = await page.$('.rw-connect-card button.btn-primary');
  if (connect) await connect.click();
  await page.waitForSelector('.xterm-screen', { timeout: 20000 });
  await page.waitForTimeout(2500);

  // 1. The header: the pencil beside the share button.
  const head = page.locator('.rw-head-actions .session-name-edit');
  check('the open session\'s name shows in the header', await head.isVisible());
  await head.getByRole('button', { name: '이름 바꾸기' }).click();
  await head.getByRole('textbox', { name: '세션 이름' }).fill(HEAD_NAME);
  await page.keyboard.press('Enter');
  await page.waitForTimeout(1500);
  check('the header shows the new name', (await head.textContent()).includes(HEAD_NAME));
  const sessions = (await api(page, '/api/terminal/sessions')).data || [];
  const id = sessions.find((s) => s.title === HEAD_NAME)?.id;
  check('the server stored the header rename', Boolean(id));
  // tmux's status line (the last row) carries the name too; the shell must not.
  const rows = await page.$$eval('.xterm-rows > div', (els) => els.map((el) => el.textContent));
  check('tmux\'s status line shows the name', rows[rows.length - 1].includes(HEAD_NAME));
  check('typing the name did not reach the shell', !rows.slice(0, -1).some((row) => row.includes(HEAD_NAME)));
  await page.screenshot({ path: `${OUT}/smoke-rename-head.png` });

  await page.click('.rw-tabs button:has-text("세션")');
  await page.waitForTimeout(800);
  check('the session list has the same name', await page.isVisible(`.session-card .session-name:has-text("${HEAD_NAME}")`));

  // 2. A floating window on the same session.
  await page.click('.rw-tabs button:has-text("터미널")');
  await page.click('.rw-head-actions button:has-text("분리")');
  const win = page.locator('.term-window').last();
  await win.waitFor();
  const bar = win.locator('.session-name-edit-bar');
  await page.waitForFunction((name) => [...document.querySelectorAll('.term-window .session-name-edit-bar')]
    .some((el) => el.textContent.includes(name)), HEAD_NAME, { timeout: 15000 }).catch(() => {});
  check('the window\'s title bar shows the session name', (await bar.textContent()).includes(HEAD_NAME));

  const before = await win.boundingBox();
  await bar.getByRole('button', { name: '이름 바꾸기' }).click();
  const input = bar.getByRole('textbox', { name: '세션 이름' });
  const box = await input.boundingBox();
  await page.mouse.move(box.x + 10, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + 150, box.y + 120, { steps: 5 });
  await page.mouse.up();
  const after = await win.boundingBox();
  check('selecting in the name box does not drag the window', before.x === after.x && before.y === after.y);
  await input.fill(WINDOW_NAME);
  await page.keyboard.press('Enter');
  await page.waitForTimeout(1500);
  check('the title bar shows the new name', (await bar.textContent()).includes(WINDOW_NAME));
  check('the server stored the window rename', (await titleOf(page, id)) === WINDOW_NAME);
  await page.screenshot({ path: `${OUT}/smoke-rename-window.png` });

  await bar.getByRole('button', { name: '이름 바꾸기' }).click();
  await bar.getByRole('textbox', { name: '세션 이름' }).fill('not saved');
  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);
  check('Escape leaves the name as it was', (await titleOf(page, id)) === WINDOW_NAME
    && (await bar.textContent()).includes(WINDOW_NAME));

  check('no page errors', problems.length === 0, problems.join(' | '));
  if (id) await api(page, `/api/terminal/sessions/${id}`, { method: 'DELETE' });
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
