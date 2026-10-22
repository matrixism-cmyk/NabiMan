// Browser smoke test for a remote-only member (role "remote"): signing in
// opens the remote workspace and nothing else, with nothing of this machine's
// or the administrators' on offer.
//
//   SMOKE_URL=http://127.0.0.1:18096 npm run smoke:member
const { chromium, devices } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const MEMBER = 'smoke-member';
const MEMBER_PASSWORD = 'smoke-member-pw1';

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};

async function api(path, { token, method = 'GET', body } = {}) {
  const res = await fetch(`${BASE}${path}`, {
    method,
    headers: { 'Content-Type': 'application/json', ...(token ? { Authorization: `Bearer ${token}` } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  return { status: res.status, json: await res.json().catch(() => ({})) };
}

async function signIn(page, path = '/') {
  await page.goto(`${BASE}${path}`, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', MEMBER);
  await page.fill('input[type="password"]', MEMBER_PASSWORD);
  await page.click('button[type="submit"]');
}

(async () => {
  // An administrator makes the member, fresh each run.
  const admin = (await api('/api/auth/login', { method: 'POST', body: { username: 'admin', password: PASSWORD } })).json.data.token;
  for (const u of (await api('/api/users', { token: admin })).json.data) {
    if (u.username === MEMBER) await api(`/api/users/${u.id}`, { token: admin, method: 'DELETE' });
  }
  const made = await api('/api/users', { token: admin, method: 'POST', body: { username: MEMBER, password: MEMBER_PASSWORD, role: 'remote' } });
  check('an administrator makes a remote-only member', made.json.success, made.json.message);

  const browser = await chromium.launch();
  const problems = [];
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));

  // 1. Signing in anywhere opens the remote workspace, and only that.
  await signIn(page, '/');
  await page.locator('.remote-shell .rw-rail').waitFor({ timeout: 15000 });
  check('signing in at / opens /remote', new URL(page.url()).pathname === '/remote', page.url());
  check('no NabiMan menus at all', await page.locator('.cat-nav, .sidebar, .app-header').count() === 0);
  check('no way back to NabiMan', await page.locator('.remote-shell-bar a', { hasText: '나비맨으로' }).count() === 0);
  check('the password can still be changed', await page.locator('.remote-shell-bar button', { hasText: '비밀번호 변경' }).isVisible());

  // 2. Nothing of this machine's or the administrators' is offered.
  const rail = await page.locator('.rw-rail').innerText();
  check('no local shell, quick connect or fleet check in the list',
    !rail.includes('로컬 셸') && !rail.includes('임시 접속') && !rail.includes('전체 확인'), rail.replace(/\n/g, ' | '));
  const tabs = await page.locator('.rw-tabs').innerText();
  check('no share links or SSH key tabs', !tabs.includes('공유 링크') && !tabs.includes('SSH 키'), tabs.replace(/\n/g, ' | '));
  await page.click('.rw-tabs button:has-text("설정")');
  await page.locator('.rw-settings').waitFor();
  check('settings hold no server-wide dictation settings',
    await page.locator('.dictation-settings').count() === 0 && await page.locator('.rw-settings .term-settings-form').count() === 1);
  await page.screenshot({ path: `${OUT}/smoke-member.png` });

  // 3. Their own server: a password, at an address outside this machine.
  await page.locator('.rw-item', { hasText: '+ 서버 추가' }).click();
  const form = page.locator('.rw-form');
  check('the server form asks no way to log in but a password',
    await form.locator('label', { hasText: /^인증$/ }).count() === 0 && await form.locator('input[type="password"]').isVisible());
  await form.locator('input[placeholder="web-prod-01"]').fill('inside');
  await form.locator('input[placeholder="192.168.1.100"]').fill('127.0.0.1');
  await form.locator('input[type="password"]').fill('anything');
  await form.locator('button[type="submit"]').click();
  await form.locator('.message').waitFor({ timeout: 10000 });
  const said = await form.locator('.message').innerText();
  check('an address inside this machine is refused', said.includes('내부 주소'), said);

  // 4. A shell of this machine's is refused even when asked for directly.
  const notice = await page.evaluate(async () => {
    const token = sessionStorage.getItem('nabiman_token');
    const query = 'cols=80&rows=24';
    const res = await fetch('/api/terminal/tickets', {
      method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ query }),
    }).then((r) => r.json());
    if (!res.success) return `ticket refused: ${res.message}`;
    const ws = new WebSocket(`${location.origin.replace(/^http/, 'ws')}/api/terminal?${query}&ticket=${res.data.ticket}`);
    ws.binaryType = 'arraybuffer';
    return new Promise((resolve) => {
      let text = '';
      ws.onmessage = (e) => { text += typeof e.data === 'string' ? e.data : new TextDecoder().decode(e.data); };
      ws.onclose = () => resolve(text);
      setTimeout(() => { ws.close(); resolve(text); }, 5000);
    });
  });
  check('a local shell is refused', notice.includes('등록한 서버로만'), notice.slice(0, 120));

  // 5. On a phone, the same.
  const phone = await (await browser.newContext({ ...devices['iPhone 13'] })).newPage();
  phone.on('pageerror', (e) => problems.push(`phone: ${e.message.split('\n')[0]}`));
  await signIn(phone, '/');
  await phone.locator('.remote-shell .rw-rail').waitFor({ timeout: 15000 });
  const phoneRail = await phone.locator('.rw-rail').innerText();
  check('on a phone, only the workspace and no local shell', new URL(phone.url()).pathname === '/remote'
    && !phoneRail.includes('로컬 셸') && await phone.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
  await phone.screenshot({ path: `${OUT}/smoke-member-phone.png` });

  check('no page errors', problems.length === 0, problems.join(' | '));
  await browser.close();
  for (const u of (await api('/api/users', { token: admin })).json.data) {
    if (u.username === MEMBER) await api(`/api/users/${u.id}`, { token: admin, method: 'DELETE' });
  }
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
