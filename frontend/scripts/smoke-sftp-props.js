// Browser smoke test for a file panel's 속성 (properties) dialog.
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_SFTP_SERVER=sftp-e2e \
//   SMOKE_PROPS_DIR=/tmp/props-e2e node scripts/smoke-sftp-props.js
//
// SMOKE_PROPS_DIR, on the saved server, holds `project/` — src/ with f1–f3.txt
// and deep/blob.bin (5000 bytes), .git/HEAD, a link `rootlink` to / — and
// `notes.txt` ("hello props\n"). The test only reads it.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const SERVER = process.env.SMOKE_SFTP_SERVER || 'sftp-e2e';
const DIR = process.env.SMOKE_PROPS_DIR;
const NOTES_SHA = '555932ef83001d3d5001cc21c9250e85149a52798f4a8da999d5c76e51cfc13b';

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};

(async () => {
  if (!DIR) throw new Error('Set SMOKE_PROPS_DIR to the prepared folder on the server');
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));

  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');
  const serverId = await page.evaluate(async (name) => {
    const res = await fetch('/api/remote-servers', { headers: { Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}` } });
    return ((await res.json()).data || []).find((s) => s.name === name)?.id;
  }, SERVER);
  check('the test server is saved', Boolean(serverId), SERVER);
  await page.evaluate(([id, dir]) => {
    localStorage.setItem(`nabiman_sftp_path_${id}`, dir);
    localStorage.setItem('nabiman_sftp_hidden', '0');
    localStorage.removeItem('nabiman_sftp_open');
  }, [serverId, DIR]);
  await page.click('.cat-btn:has-text("원격")');
  await page.locator('.rw-item', { hasText: SERVER }).click();
  await page.click('.rw-tabs button:has-text("터미널")');
  await page.click('.rw-connect button:has-text("접속")');
  await page.waitForSelector('.sftp .sftp-row:has-text("project")', { timeout: 20000 });

  const dialog = page.locator('.sftp-props');
  const field = (label) => dialog.locator(`dt:text-is("${label}") + dd`);
  const close = async () => { await dialog.locator('button:has-text("닫기")').click(); await dialog.waitFor({ state: 'detached' }); };

  // 1. A folder: what is directly in it, then everything under it.
  await page.click('.sftp-row:has-text("project")', { button: 'right' });
  await page.click('[role="menuitem"]:has-text("속성")');
  await dialog.waitFor();
  await page.waitForFunction(() => !document.querySelector('.sftp-props-busy') && document.querySelector('.sftp-props dd[aria-live]'));
  const inside = await field('바로 안').innerText();
  check('what is directly inside, hidden ones counted apart', /폴더 2개 · 파일 0개 · 링크 1개/.test(inside) && /숨김 1개/.test(inside), inside);
  const all = await field('하위 전체').innerText();
  check('everything under it, links not followed', /폴더 3개 · 파일 5개 · 링크 1개/.test(all) && /5,023 B/.test(all), all.replace(/\n/g, ' / '));
  check('its path and permissions', (await field('위치').innerText()) === `${DIR}/project` && /rwx/.test(await field('권한').innerText()));
  await page.screenshot({ path: `${OUT}/sftp-props-folder.png` });
  await close();

  // 2. A file: exact size, and the SHA-256 when asked.
  await page.click('.sftp-row:has-text("notes.txt")', { button: 'right' });
  await page.click('[role="menuitem"]:has-text("속성")');
  await dialog.waitFor();
  check('a file shows its exact size', /12 B/.test(await field('크기').innerText()));
  await dialog.locator('button:has-text("계산")').click();
  const hash = await dialog.locator('.sftp-props-hash').innerText({ timeout: 10000 });
  check('its SHA-256 matches', hash === (process.env.SMOKE_NOTES_SHA || NOTES_SHA), hash);
  await close();

  // 3. Two rows chosen: summed up.
  await page.click('.sftp-row:has-text("project")');
  await page.click('.sftp-row:has-text("notes.txt")', { modifiers: ['Control'] });
  await page.click('.sftp-row:has-text("notes.txt")', { button: 'right' });
  await page.click('[role="menuitem"]:has-text("속성")');
  await dialog.waitFor();
  await page.waitForFunction(() => !document.querySelector('.sftp-props-busy') && document.querySelector('.sftp-props dd[aria-live]'));
  const chosen = await field('선택').innerText();
  const sum = await field('선택 항목 전체').innerText();
  check('a selection is summed up', /2개 선택됨 — 폴더 1개 · 파일 1개/.test(chosen) && /파일 6개/.test(sum) && /5,035 B/.test(sum), `${chosen} | ${sum.replace(/\n/g, ' / ')}`);
  await close();

  // 4. The open folder itself, from the empty space; its permissions button leads on.
  await page.click('.sftp-body', { button: 'right', position: { x: 40, y: 300 } });
  await page.click('[role="menuitem"]:has-text("이 폴더 속성")');
  await dialog.waitFor();
  check('the open folder\'s properties', (await field('위치').innerText()) === DIR);
  await dialog.locator('button:has-text("변경")').click();
  await page.waitForSelector('.sftp-attrs');
  check('the permissions button opens the permissions dialog for it', /props-e2e/.test(await page.locator('.sftp-attrs-what').innerText()));
  await page.locator('.sftp-attrs button:has-text("취소")').click();

  check('no page errors', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(failed ? `${failed} FAILED` : 'all passed');
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.error('FATAL', e.message.split('\n')[0]); process.exit(1); });
