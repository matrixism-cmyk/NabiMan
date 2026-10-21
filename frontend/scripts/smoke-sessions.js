// Browser smoke test: restoring panes never starts shells nobody asked for.
//
//   SMOKE_URL=http://127.0.0.1:18096 npm run smoke:sessions
//
// Opens the local shell in the workspace and two floating windows, ends the
// windows' sessions (as the session list's 세션 종료 does), then reloads and
// signs in again: no new local shells may appear until "+" asks for one. A
// session ended while the browser was away is reported, not replaced.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};
const api = (page, path, init = {}) => page.evaluate(async ({ path, init }) => {
  const res = await fetch(path, { ...init, headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}` } });
  return res.json();
}, { path, init });
const local = async (page) => ((await api(page, '/api/terminal/sessions')).data || []).filter((s) => s.label === 'local' && s.alive).map((s) => s.id).sort();

async function signIn(page) {
  await page.goto(BASE, { waitUntil: 'networkidle' });
  if (await page.locator('.app-header').count()) return;
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');
}
const settle = (page) => page.waitForTimeout(4000);

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  let page = await context.newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));
  await signIn(page);
  // Start clean: no windows, no local shells.
  for (const id of await local(page)) await api(page, `/api/terminal/sessions/${id}`, { method: 'DELETE' });
  await page.evaluate(() => Object.keys(localStorage).filter((k) => k.startsWith('nabiman_term')).forEach((k) => localStorage.removeItem(k)));
  await page.reload({ waitUntil: 'networkidle' });

  await page.click('.cat-btn:has-text("원격")');
  const rail = page.locator('.rw-item', { hasText: '로컬 셸' });
  await rail.click();
  await page.click('.rw-tabs button:has-text("터미널")');
  const connect = page.locator('.rw-connect button', { hasText: '접속' });
  if (await connect.count()) await connect.click();
  await page.waitForSelector('.rw-work .xterm-screen', { timeout: 20000 });
  for (let i = 0; i < 2; i += 1) {
    await rail.scrollIntoViewIfNeeded();
    await rail.click({ button: 'right' });
    // The entry sits low in the list: pick the item without scrolling (a scroll closes the menu).
    await page.getByRole('menuitem', { name: '새 창으로 열기', exact: true }).evaluate((el) => el.click());
  }
  await page.waitForFunction(() => document.querySelectorAll('.term-window .xterm-screen').length === 2, null, { timeout: 20000 });
  await settle(page);
  const opened = await local(page);
  check('the pane and two windows have a shell each', opened.length === 3, String(opened.length));

  // Reloading re-attaches: still the same three.
  await page.reload({ waitUntil: 'networkidle' });
  await settle(page);
  check('a reload re-attaches to the same shells', JSON.stringify(await local(page)) === JSON.stringify(opened));

  // End the windows' shells, as 세션 종료 in the list does.
  const windowSids = await page.evaluate(() => Object.keys(localStorage)
    .filter((k) => k.startsWith('nabiman_term_sid_tw_')).map((k) => localStorage.getItem(k)));
  for (const id of windowSids) await api(page, `/api/terminal/sessions/${id}`, { method: 'DELETE' });
  await page.waitForTimeout(1500);
  const left = await local(page);
  check('their windows show the session ended', left.length === 1
    && await page.locator('.term-window', { hasText: '세션이 종료' }).count() + await page.locator('.term-window .xterm-rows', { hasText: '종료' }).count() >= 2, String(left.length));

  // Reload, and sign in afresh: no new shells.
  await page.reload({ waitUntil: 'networkidle' });
  await settle(page);
  check('a reload starts no shell for the ended windows', JSON.stringify(await local(page)) === JSON.stringify(left), String((await local(page)).length));
  await page.evaluate(() => { sessionStorage.clear(); });
  await signIn(page);
  await settle(page);
  check('nor does signing in again', JSON.stringify(await local(page)) === JSON.stringify(left), String((await local(page)).length));

  // A shell ended while the browser was away: reported, not replaced.
  const paneSid = left[0];
  await page.close();
  const api2 = await context.newPage();
  await signIn(api2);
  await api(api2, `/api/terminal/sessions/${paneSid}`, { method: 'DELETE' });
  await api2.close();
  page = await context.newPage();
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));
  await signIn(page);
  await settle(page);
  check('a shell ended while away is not replaced on return', (await local(page)).length === 0, String((await local(page)).length));

  // Asking for one still works: "+" in a window.
  await page.locator('.term-window').first().locator('button[title="새 세션"]').click();
  await settle(page);
  check('"+" starts a new shell when asked', (await local(page)).length === 1, String((await local(page)).length));
  await page.reload({ waitUntil: 'networkidle' });
  await settle(page);
  check('which the next reload re-attaches to', (await local(page)).length === 1);

  // Clean up the windows.
  await page.evaluate(() => Object.keys(localStorage).filter((k) => k.startsWith('nabiman_term')).forEach((k) => localStorage.removeItem(k)));
  for (const id of await local(page)) await api(page, `/api/terminal/sessions/${id}`, { method: 'DELETE' });
  check('no page errors', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
