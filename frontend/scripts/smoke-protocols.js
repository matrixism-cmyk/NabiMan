// Browser smoke test for servers reached by something other than SSH+SFTP.
//
//   SMOKE_URL=http://127.0.0.1:18096 npm run smoke:protocols
//
// Needs a NabiMan server serving build/ (a throwaway data dir and password),
// `npx playwright install chromium`, and throwaway servers:
//   - an FTP server on SMOKE_FTP_PORT (2121) taking web@example.com / ftp-secret,
//     with a folder `docs` holding `read me.txt`;
//   - saved servers named proto-telnet (a Telnet server that asks for
//     admin / tel-secret, which is stored) and proto-sftp-only (SSH with the
//     shell refused, set to SFTP only), as scratch e2e scripts create them.
// An FTP-only server is added through the form, and every view must offer
// only what each server is set up for.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const FTP_PORT = process.env.SMOKE_FTP_PORT || '2121';
const NAME = `proto-ui-ftp-${Date.now() % 100000}`;

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

const tabs = async (page) => (await page.$$eval('.rw-tabs button', (els) => els.map((el) => el.textContent.trim())))
  .map((t) => t.replace(/\s*\d+$/, ''));

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

  // 1. Adding an FTP-only server through the form.
  await page.click('.rw-item-add');
  const form = page.locator('.rw-form');
  await form.locator('input[placeholder="web-prod-01"]').fill(NAME);
  await form.locator('input[placeholder="192.168.1.100"]').fill('127.0.0.1');
  await form.locator('#srv-terminal').selectOption('none');
  await form.locator('#srv-files').selectOption('ftp');
  check('the SSH port gives way to the FTP port', !(await form.locator('#srv-port').count())
    && await form.locator('#srv-ftp-port').isVisible());
  await form.locator('#srv-ftp-port').fill(FTP_PORT);
  check('plain FTP is flagged as unencrypted', (await form.locator('.proto-warning').textContent()).includes('FTP'));
  check('no SSH key choice for FTP', !(await form.locator('option[value="key"]').count()));
  await form.locator('input[placeholder="root"]').fill('web@example.com');
  await form.locator('input[type="password"]').fill('ftp-secret');
  await form.locator('button[type="submit"]').click();
  await page.waitForSelector('.rw-tabs button:has-text("설정").is-active', { timeout: 15000 });
  await page.waitForFunction(() => /확인 결과와 같습니다|바꿨습니다/.test(document.body.innerText), null, { timeout: 30000 }).catch(() => {});
  const said = await page.locator('section[aria-labelledby^="detect-"] [role="status"]').textContent().catch(() => '');
  check('a new server is checked by itself', said.includes('FTP 전용'), said);
  check('no host key form for an FTP server', !(await page.locator('[id^="host-trust-"]').count()));

  const item = page.locator('.rw-item', { hasText: NAME });
  check('the list shows an FTP badge marked unencrypted', await item.locator('.proto-badge.proto-ftp.is-plain').isVisible());
  check('the list shows the FTP address', (await item.locator('.rw-item-sub').textContent()).includes(`web@example.com@127.0.0.1:${FTP_PORT}`));

  // 2. Only what it offers: files, status, settings.
  const ftpTabs = await tabs(page);
  check('an FTP-only server has a files tab and no terminal or commands',
    ftpTabs.includes('파일') && !ftpTabs.includes('터미널') && !ftpTabs.includes('세션') && !ftpTabs.includes('실행'), ftpTabs.join(','));
  await page.click('.rw-tabs button:has-text("파일")');
  await page.waitForSelector('.rw-view-files .sftp-row, .rw-view-files [role="row"]', { timeout: 20000 }).catch(() => {});
  const listing = await page.locator('.rw-view-files').textContent();
  check('the file view lists the FTP server', listing.includes('docs'), listing.slice(0, 120));
  check('its hint does not mention a terminal', !listing.includes('터미널'));
  await page.locator('.rw-view-files').getByText('docs', { exact: true }).dblclick();
  await page.waitForTimeout(1500);
  check('a folder opens over FTP', (await page.locator('.rw-view-files').textContent()).includes('read me.txt'));
  await page.screenshot({ path: `${OUT}/smoke-protocols-ftp.png` });

  // The menu closes on scroll, so the entry is scrolled to first.
  await item.scrollIntoViewIfNeeded();
  await page.waitForTimeout(400);
  await item.click({ button: 'right' });
  await page.waitForSelector('[role="menuitem"]');
  const menu = await page.$$eval('[role="menuitem"]', (els) => els.map((el) => el.textContent.trim()));
  check('its menu opens files and offers no terminal', menu.some((m) => m.includes('파일 열기')) && !menu.some((m) => m.includes('접속')), menu.join(','));
  await page.keyboard.press('Escape');

  await page.click('.rw-tabs button:has-text("상태")');
  await page.click('.rw-section-head button:has-text("확인")');
  await page.waitForSelector('.proto-checks li', { timeout: 20000 });
  check('the status check reports FTP', (await page.locator('.proto-checks').textContent()).includes('FTP'));

  // 3. Telnet: logs in by itself, at the pane's size, without a file panel.
  await page.click('.rw-item:has-text("proto-telnet")');
  const telnetTabs = await tabs(page);
  check('a Telnet server has sessions and a terminal but no commands', telnetTabs.includes('터미널') && !telnetTabs.includes('실행'), telnetTabs.join(','));
  await page.click('.rw-tabs button:has-text("터미널")');
  const connect = await page.$('.rw-connect-card button.btn-primary');
  if (connect) await connect.click();
  await page.waitForSelector('.xterm-screen', { timeout: 20000 });
  const rows = () => page.$$eval('.xterm-rows > div', (els) => els.map((el) => el.textContent));
  await page.waitForFunction(() => [...document.querySelectorAll('.xterm-rows > div')].some((el) => el.textContent.includes('telnet$')), null, { timeout: 20000 }).catch(() => {});
  check('the stored password logs in by itself', (await rows()).some((r) => r.includes('telnet$')));
  await page.click('.xterm-screen');
  await page.keyboard.type('echo "T=$TERM"; stty size\n');
  await page.waitForTimeout(1500);
  const screen = (await rows()).join('\n');
  check('the terminal type reaches the server', screen.includes('T=xterm-256color'));
  const size = screen.match(/^(\d+) (\d+)\s*$/m);
  check('the window size reaches the server', Boolean(size) && Number(size[1]) > 10 && Number(size[2]) > 40, size && size[0]);
  check('no file panel beside a server without files', !(await page.locator('.rw-split-tab, .rw-split-files').count()));
  await page.screenshot({ path: `${OUT}/smoke-protocols-telnet.png` });

  // 4. SFTP only: the shell is refused, the files are not.
  await page.click('.rw-item:has-text("proto-sftp-only")');
  const sftpTabs = await tabs(page);
  check('an SFTP-only server opens on its files', sftpTabs.includes('파일') && !sftpTabs.includes('터미널'), sftpTabs.join(','));
  await page.waitForTimeout(2500);
  check('its files are listed', (await page.locator('.rw-view-files .sftp-crumbs').count()) > 0);

  // 5. The protocol filter.
  await page.selectOption('.rw-rail-filters select', 'telnet');
  const shown = await page.$$eval('.rw-rail-list .rw-item .rw-item-name', (els) => els.map((el) => el.textContent));
  check('the protocol filter keeps only Telnet servers', shown.includes('proto-telnet') && !shown.includes(NAME), shown.join(','));
  await page.selectOption('.rw-rail-filters select', '');

  check('no page errors', problems.length === 0, problems.join(' | '));
  const servers = (await api(page, '/api/remote-servers')).data || [];
  for (const s of servers.filter((s) => s.name === NAME)) await api(page, `/api/remote-servers/${s.id}`, { method: 'DELETE' });
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
