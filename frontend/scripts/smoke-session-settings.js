// Browser smoke test for a session's own settings over the account's, and a
// share link visitor's own view.
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_RESTART="<restarts the server>" npm run smoke:session-settings
//
// Needs a NabiMan server serving build/ with dictation set up (any service:
// only the microphone's hint is read). SMOKE_RESTART, when given, restarts
// it to check a session keeps its settings.
const { execSync } = require('child_process');
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const RESTART = process.env.SMOKE_RESTART;

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};

let page;
(async () => {
  const browser = await chromium.launch({ args: ['--no-sandbox'] });
  const context = await browser.newContext({ viewport: { width: 1500, height: 950 } });
  page = await context.newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));
  await page.addInitScript(() => localStorage.setItem('nabiman_lang', 'ko'));

  const signIn = async () => {
    await page.goto(BASE, { waitUntil: 'networkidle' });
    // Sign-ins outlive a restart: after one, the app may already be open.
    await page.waitForSelector('.app-header, input[type="password"]');
    if (await page.$('.app-header')) return;
    await page.fill('input[type="text"]', 'admin');
    await page.fill('input[type="password"]', PASSWORD);
    await page.click('button[type="submit"]');
    await page.waitForSelector('.app-header');
  };
  const api = (path, init = {}) => page.evaluate(async ({ path, init }) => {
    const res = await fetch(path, { ...init, headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}` } });
    return res.json();
  }, { path, init });
  const fontSize = (scope) => scope.locator('.xterm-rows').first().evaluate((el) => getComputedStyle(el).fontSize);
  const openLocal = async () => {
    await page.click('.cat-btn:has-text("원격")');
    await page.locator('.rw-item', { hasText: '로컬 셸' }).click();
    await page.click('.rw-tabs button:has-text("터미널")');
    const connect = await page.$('.rw-connect button:has-text("접속")');
    if (connect) await connect.click();
    await page.waitForSelector('.rw-work .xterm-rows', { timeout: 20000 });
    await page.waitForTimeout(1500);
  };

  await signIn();
  const account = (await api('/api/terminal/settings')).data;
  await openLocal();
  const pane = page.locator('.rw-work');
  check('the pane starts with the account\'s text size', await fontSize(pane) === `${account.font_size}px`, await fontSize(pane));

  // 1. This session's own size and dictation, from the ⚙ dialog.
  await page.click('.rw-head-actions button[title="터미널 설정"]');
  const dialog = page.locator('.term-settings');
  check('from a session, the dialog opens on that session', (await dialog.locator('[role="tab"][aria-selected="true"]').textContent()).startsWith('이 세션'));
  const field = (id) => dialog.locator('.session-field', { has: page.locator(`label[for="${id}"]`) });
  await field('ss-size').locator('.session-follow input').uncheck();
  await dialog.locator('#ss-size').fill('21');
  const voice = await dialog.locator('#ss-pause').count() === 0 ? null : true;
  if (voice || await field('ss-pause').count()) {
    await field('ss-pause').locator('.session-follow input').uncheck();
    await dialog.locator('#ss-pause').selectOption('5000');
    await field('ss-enter').locator('.session-follow input').uncheck();
    await dialog.locator('#ss-enter').uncheck();
  }
  await dialog.locator('button.btn-primary', { hasText: '저장' }).click();
  await dialog.waitFor({ state: 'detached' });
  await page.waitForTimeout(800);
  check('the session\'s own size applies at once', await fontSize(pane) === '21px', await fontSize(pane));
  const sid = (await api('/api/terminal/sessions')).data.filter((s) => s.alive).sort((a, b) => b.last_active_unix - a.last_active_unix)[0].id;
  const stored = (await api(`/api/terminal/sessions/${sid}/settings`)).data.overrides;
  check('only what was set is the session\'s own', stored.font_size === 21 && stored.font_family == null && stored.scrollback_lines == null, JSON.stringify(stored));
  const mic = await page.locator('.rw-head-actions .dictation-btn').first().getAttribute('title').catch(() => '');
  check('the microphone follows the session: 5 s pause, Enter only when said', !mic || (mic.includes('5초') && mic.includes('Enter는')), mic);

  // 2. The same session in a floating window looks the same; the account is untouched.
  await page.click('.rw-head-actions button:has-text("분리")');
  const win = page.locator('.term-window').last();
  await win.locator('.xterm-rows').waitFor({ timeout: 20000 });
  await page.waitForTimeout(1500);
  check('a floating window of the session uses its size too', await fontSize(win) === '21px', await fontSize(win));
  check('the account\'s settings are unchanged', (await api('/api/terminal/settings')).data.font_size === account.font_size);
  await win.locator('button[title="이 세션 설정"]').click();
  check('a floating window has its session\'s ⚙', await page.locator('.term-settings [role="tab"][aria-selected="true"]', { hasText: '이 세션' }).count() === 1);
  await page.locator('.term-settings button', { hasText: '취소' }).click();

  // 3. It outlasts a restart.
  if (RESTART) {
    execSync(RESTART, { stdio: 'ignore' });
    await signIn();
    const again = (await api(`/api/terminal/sessions/${sid}/settings`)).data;
    check('a restart keeps the session\'s settings', again?.overrides?.font_size === 21, JSON.stringify(again));
  }

  // 4. A share link's visitor sets their own view, without touching the owner's.
  const link = await api(`/api/terminal/sessions/${sid}/share-link`, { method: 'POST', body: JSON.stringify({ expires_in_secs: 3600, password: '', read_only: false, files: 'none' }) });
  const visitorCtx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const visitor = await visitorCtx.newPage();
  visitor.on('pageerror', (e) => problems.push(`visitor: ${e.message.split('\n')[0]}`));
  await visitor.goto(`${BASE}/share/${link.data.token}`, { waitUntil: 'networkidle' });
  await visitor.waitForSelector('.share-page .xterm-rows', { timeout: 20000 });
  const fontFamily = (scope) => scope.locator('.xterm-rows').first().evaluate((el) => getComputedStyle(el).fontFamily);
  await visitor.click('.share-head button[aria-label="보기 설정"]');
  await visitor.selectOption('#sv-font', 'jetbrains');
  await visitor.locator('.share-view-settings button', { hasText: '닫기' }).click();
  await visitor.waitForFunction(() => getComputedStyle(document.querySelector('.share-page .xterm-rows')).fontFamily.includes('JetBrains'), null, { timeout: 15000 }).catch(() => {});
  check('a visitor changes their own font', (await fontFamily(visitor.locator('.share-page'))).includes('JetBrains'), await fontFamily(visitor.locator('.share-page')));
  await visitor.reload({ waitUntil: 'networkidle' });
  await visitor.waitForSelector('.share-page .xterm-rows', { timeout: 20000 });
  await visitor.waitForFunction(() => getComputedStyle(document.querySelector('.share-page .xterm-rows')).fontFamily.includes('JetBrains'), null, { timeout: 15000 }).catch(() => {});
  check('and it stays in their browser', (await fontFamily(visitor.locator('.share-page'))).includes('JetBrains'));
  check('the owner\'s pane keeps its own font', !(await fontFamily(win)).includes('JetBrains'), await fontFamily(win));
  check('the owner\'s session settings are untouched by the visitor', (await api(`/api/terminal/sessions/${sid}/settings`)).data.overrides.font_size === 21);
  await api(`/api/terminal/share-links/${link.data.token}`, { method: 'DELETE' });

  // 5. Back to following the defaults, from the floating window's ⚙.
  await win.locator('.xterm-rows').waitFor({ timeout: 20000 });
  await win.locator('button[title="이 세션 설정"]').click();
  await page.locator('.term-settings button', { hasText: '모두 전체 설정 따르기' }).click();
  await page.locator('.term-settings button.btn-primary', { hasText: '저장' }).click();
  await page.waitForTimeout(1000);
  check('following the defaults again draws with the account\'s size', await fontSize(win) === `${account.font_size}px`, await fontSize(win));
  check('and nothing of the session is kept', JSON.stringify((await api(`/api/terminal/sessions/${sid}/settings`)).data.overrides).replace(/null|"[a-z_]+":|[{},]/g, '') === '');

  await page.screenshot({ path: `${OUT}/smoke-session-settings.png` });
  check('no page errors', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.log('FATAL:', e.message.split('\n').slice(0, 3).join(' / '));
  await page?.screenshot({ path: `${OUT}/smoke-session-settings-failed.png` }).catch(() => undefined);
  process.exit(1);
});
