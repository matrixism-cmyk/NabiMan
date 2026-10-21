// Browser smoke test for staying signed in across restarts, and for signing
// out after a time without use (최고관리자 → 환경설정).
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_RESTART="<command that restarts it>" npm run smoke:signin
//
// SMOKE_RESTART must restart the NabiMan under test and return once it answers.
const { execSync } = require('child_process');
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const RESTART = process.env.SMOKE_RESTART;
if (!RESTART) { console.log('FATAL: set SMOKE_RESTART'); process.exit(1); }

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};
async function signIn(page) {
  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');
}
const me = (page) => page.evaluate(async () => (await fetch('/api/users/me', { headers: { Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}` } })).status);
const setIdle = (page, minutes) => page.evaluate(async (m) => (await fetch('/api/app-settings', {
  method: 'PUT', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}` },
  body: JSON.stringify({ idle_logout_minutes: m }),
})).json(), minutes);

(async () => {
  const browser = await chromium.launch();
  const problems = [];

  // 1. A restart keeps the sign-in; a sign-out stays signed out.
  const a = await (await browser.newContext()).newPage();
  a.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));
  await signIn(a);
  await setIdle(a, 0);
  const b = await (await browser.newContext()).newPage();
  await signIn(b);
  const bToken = await b.evaluate(() => sessionStorage.getItem('nabiman_token'));
  await b.locator('.logout-btn').click();
  await b.waitForSelector('input[type="password"]');
  execSync(RESTART, { stdio: 'ignore' });
  check('the server was restarted', true);
  await a.reload({ waitUntil: 'networkidle' });
  check('after a restart the page is still signed in', await a.locator('.app-header').count() === 1 && await me(a) === 200);
  const bStatus = await a.evaluate(async (t) => (await fetch('/api/users/me', { headers: { Authorization: `Bearer ${t}` } })).status, bToken);
  check('a signed-out token stays dead after a restart', bStatus === 401, String(bStatus));

  // 2. 환경설정 sets the idle time.
  await a.locator('.cat-btn', { hasText: '최고관리자' }).click();
  await a.locator('.sidebar-btn', { hasText: '환경설정' }).click();
  await a.locator('.app-settings select').selectOption('5');
  await a.locator('.app-settings button', { hasText: '저장' }).click();
  await a.locator('.app-settings .message').waitFor();
  check('환경설정 saves the idle time', (await a.locator('.app-settings .message').textContent()).includes('저장'));
  execSync(RESTART, { stdio: 'ignore' });
  await a.reload({ waitUntil: 'networkidle' });
  await a.locator('.cat-btn', { hasText: '최고관리자' }).click();
  await a.locator('.sidebar-btn', { hasText: '환경설정' }).click();
  check('and keeps it across a restart', await a.locator('.app-settings select').inputValue() === '5');

  // 3. With the clock in our hands: warn at four minutes, sign out at five.
  const ctx = await browser.newContext();
  const c = await ctx.newPage();
  c.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));
  await c.clock.install();
  await signIn(c);
  await c.waitForTimeout(500);
  await c.clock.runFor(3 * 60_000);
  check('no warning while there is time', await c.locator('.idle-warning').count() === 0);
  await c.clock.runFor(65_000);
  check('a minute before, it warns', await c.locator('.idle-warning').count() === 1, await c.locator('.idle-warning').textContent().catch(() => ''));
  await c.locator('.idle-warning button', { hasText: '계속 사용' }).click();
  check('계속 사용 puts it off', await c.locator('.idle-warning').count() === 0);
  await c.clock.runFor(4 * 60_000);
  await c.keyboard.press('Shift');
  await c.clock.runFor(4 * 60_000);
  check('a key counts as use', await c.locator('.app-header').count() === 1);
  await c.clock.runFor(70_000 + 5_000);
  await c.waitForSelector('input[type="password"]', { timeout: 10000 }).catch(() => {});
  check('with no use, it signs out', await c.locator('input[type="password"]').count() === 1);
  check('and says why', await c.getByText('한동안 사용하지 않아 자동으로 로그아웃되었습니다.').count() === 1);
  check('the server has let the sign-in go', await me(c) === 401);

  await setIdle(a, 0);
  check('no page errors', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
