// Browser smoke test for dictating into a terminal.
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_VOICE_URL=http://127.0.0.1:19999 \
//   SMOKE_VOICE_KEY=vk_test_dictation_key npm run smoke:dictation
//
// Needs a NabiMan server serving build/ and a stand-in speech service that
// takes that key and "hears" `echo dictation-works 엔터` (as the scratch
// fake_voice.py does). Chromium's fake microphone supplies the audio. The
// spoken "엔터" must press Enter (so the shell prints the echo) and never be
// typed — in the workspace pane, a floating window, and a share link's page.
//
// With SMOKE_PAUSE=1 the service hears only `echo dictation-works` and ends it
// at a pause; Enter at a pause is on unless an administrator turned it off, so
// the echo must still run, in all three places.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const VOICE_URL = process.env.SMOKE_VOICE_URL || 'http://127.0.0.1:19999';
const VOICE_KEY = process.env.SMOKE_VOICE_KEY || 'vk_test_dictation_key';
const PAUSE = process.env.SMOKE_PAUSE === '1';

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};

const api = (page, path, init = {}) => page.evaluate(async ({ path, init }) => {
  const res = await fetch(path, { ...init, headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}` } });
  return res.json();
}, { path, init });

/** Rows of the terminal inside `scope`. */
const rows = (scope) => scope.locator('.xterm-rows > div').allTextContents();
const printed = async (scope) => (await rows(scope)).some((r) => r.trim() === 'dictation-works');

async function dictate(page, scope, label) {
  const before = (await rows(scope)).filter((r) => r.trim() === 'dictation-works').length;
  const mic = scope.getByRole('button', { name: '음성 받아쓰기 시작' });
  await mic.click();
  await scope.getByRole('button', { name: '음성 받아쓰기 끝내기' }).waitFor({ timeout: 15000 });
  await page.waitForFunction(({ sel, n }) => [...document.querySelectorAll(`${sel} .xterm-rows > div`)]
    .filter((r) => r.textContent.trim() === 'dictation-works').length > n, { sel: label.sel, n: before }, { timeout: 30000 }).catch(() => {});
  const after = (await rows(scope)).filter((r) => r.trim() === 'dictation-works').length;
  check(`${label.name}: what is said is typed and ${PAUSE ? 'a pause' : '"엔터"'} presses Enter`, after > before, `${before} → ${after}`);
  check(`${label.name}: the word "엔터" is never typed`, !(await rows(scope)).some((r) => r.includes('엔터')));
  await scope.getByRole('button', { name: '음성 받아쓰기 끝내기' }).click();
  await scope.getByRole('button', { name: '음성 받아쓰기 시작' }).waitFor({ timeout: 15000 });
}

(async () => {
  const browser = await chromium.launch({ args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream'] });
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await context.grantPermissions(['microphone'], { origin: BASE });
  const page = await context.newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));

  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');
  const set = await api(page, '/api/terminal/dictation/settings', { method: 'PUT', body: JSON.stringify({ url: VOICE_URL, address: '', key: VOICE_KEY }) });
  check('dictation is set up', set.data?.available === true, JSON.stringify(set).slice(0, 120));
  if (PAUSE) check('Enter at a pause is on', set.data?.auto_enter === true && set.data?.endpoint_ms > 0, JSON.stringify(set.data));
  await page.reload({ waitUntil: 'networkidle' });
  await page.waitForSelector('.app-header');

  // 1. The workspace pane.
  await page.click('.cat-btn:has-text("원격")');
  await page.locator('.rw-item', { hasText: '로컬 셸' }).click();
  await page.click('.rw-tabs button:has-text("터미널")');
  const connect = await page.$('.rw-connect button:has-text("접속")');
  if (connect) await connect.click();
  await page.waitForSelector('.xterm-screen', { timeout: 20000 });
  await page.waitForTimeout(2000);
  const pane = page.locator('.rw-work');
  await dictate(page, pane, { name: 'workspace pane', sel: '.rw-work' });
  await page.screenshot({ path: `${OUT}/smoke-dictation-pane.png` });

  // 2. A floating window.
  await page.click('.rw-head-actions button:has-text("분리")');
  const win = page.locator('.term-window').last();
  await win.locator('.xterm-screen').waitFor({ timeout: 20000 });
  await page.waitForTimeout(2000);
  await dictate(page, win, { name: 'floating window', sel: '.term-window:last-of-type' });

  // 3. A share link's visitor, through a link that lets them type.
  const sessions = (await api(page, '/api/terminal/sessions')).data || [];
  const live = sessions.filter((s) => s.alive && s.label === 'local').sort((a, b) => b.last_active_unix - a.last_active_unix)[0];
  const link = await api(page, `/api/terminal/sessions/${live.id}/share-link`, { method: 'POST', body: JSON.stringify({ expires_in_secs: 3600, password: '', read_only: false, files: 'none' }) });
  const readOnly = await api(page, `/api/terminal/sessions/${live.id}/share-link`, { method: 'POST', body: JSON.stringify({ expires_in_secs: 3600, password: '', read_only: true, files: 'none' }) });
  const visitorCtx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  await visitorCtx.grantPermissions(['microphone'], { origin: BASE });
  const visitor = await visitorCtx.newPage();
  visitor.on('pageerror', (e) => problems.push(`visitor: ${e.message.split('\n')[0]}`));
  await visitor.goto(`${BASE}/share/${link.data.token}`, { waitUntil: 'networkidle' });
  await visitor.waitForSelector('.xterm-screen', { timeout: 20000 });
  await visitor.waitForTimeout(2000);
  await dictate(visitor, visitor.locator('.share-page'), { name: 'share link', sel: '.share-page' });
  await visitor.screenshot({ path: `${OUT}/smoke-dictation-share.png` });
  await visitor.goto(`${BASE}/share/${readOnly.data.token}`, { waitUntil: 'networkidle' });
  await visitor.waitForSelector('.xterm-screen', { timeout: 20000 });
  check('a read-only link offers no microphone', !(await visitor.getByRole('button', { name: '음성 받아쓰기 시작' }).count()));
  for (const l of [link, readOnly]) await api(page, `/api/terminal/share-links/${l.data.token}`, { method: 'DELETE' });

  check('no page errors', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
