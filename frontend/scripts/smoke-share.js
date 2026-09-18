// Browser smoke test for share links.
//
//   SMOKE_URL=http://127.0.0.1:18096 npm run smoke:share
//
// Needs a NabiMan server serving build/ (a throwaway data dir and password)
// and `npx playwright install chromium`. It walks the whole flow the way a
// person would: create a link, open it in a browser with no session, get in
// with the password, type into the shared shell, then check that a read-only
// link cannot type, that a viewer never reshapes the owner's pane, and that a
// revoked link stops working.
const { chromium } = require('playwright');
const { execSync } = require('child_process');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const pane = () => {
  try {
    return execSync("tmux list-panes -a -F '#{pane_width}x#{pane_height}' 2>/dev/null | head -1", { encoding: 'utf8' }).trim();
  } catch { return '?'; }
};

const results = [];
const check = (name, ok) => { results.push([name, ok]); console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}`); };

async function signIn(browser) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await ctx.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: BASE });
  const page = await ctx.newPage();
  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');
  await page.click('.cat-btn:has-text("원격")');
  await page.click('.rw-item:has-text("로컬 셸")');
  await page.waitForTimeout(700);
  const connect = await page.$('.rw-connect-card button.btn-primary');
  if (connect) await connect.click();
  await page.waitForSelector('.xterm-screen', { timeout: 20000 });
  await page.waitForTimeout(3000);
  return page;
}

async function createLink(page, { password, readOnly }) {
  await page.click('button:has-text("공유")');
  await page.waitForSelector('.share-dialog');
  await page.click('.share-dialog button:has-text("24시간")');
  if (password) await page.fill('.share-dialog input[type="password"]', password);
  if (readOnly) await page.check('.share-dialog input[type="checkbox"]');
  await page.click('.share-dialog button:has-text("링크 만들기")');
  await page.waitForSelector('.share-created input');
  const url = await page.$eval('.share-created input', (el) => el.value);
  await page.click('.share-dialog button:has-text("닫기")');
  await page.waitForTimeout(400);
  return url;
}

(async () => {
  const browser = await chromium.launch();
  const owner = await signIn(browser);
  await owner.click('.xterm-screen');
  await owner.keyboard.type('echo OWNER-LINE');
  await owner.keyboard.press('Enter');
  await owner.waitForTimeout(1200);

  const url = await createLink(owner, { password: 'sharepw', readOnly: false });
  check('link looks like a share URL', /\/share\/[0-9a-f]{32}$/.test(url));

  const guestCtx = await browser.newContext({ viewport: { width: 900, height: 600 } });
  const guest = await guestCtx.newPage();
  await guest.goto(url, { waitUntil: 'networkidle' });
  await guest.waitForSelector('.share-gate');
  await guest.fill('.share-gate input', 'nope');
  await guest.click('.share-gate button');
  await guest.waitForTimeout(800);
  check('wrong password is refused', Boolean(await guest.$('.login-error')));

  const paneBefore = pane();
  await guest.fill('.share-gate input', 'sharepw');
  await guest.click('.share-gate button');
  await guest.waitForSelector('.xterm-screen', { timeout: 20000 });
  await guest.waitForTimeout(3000);
  check('visitor sees the shared screen', (await guest.$eval('.xterm-rows', (el) => el.textContent)).includes('OWNER-LINE'));
  check('visitor did not reshape the pane', paneBefore === pane());

  await guest.click('.xterm-screen');
  await guest.keyboard.type('echo GUEST-LINE');
  await guest.keyboard.press('Enter');
  await guest.waitForTimeout(1500);
  check('visitor types into the same shell', (await owner.$eval('.xterm-rows', (el) => el.textContent)).includes('GUEST-LINE'));

  const box = await guest.$eval('.xterm-screen', (el) => { const b = el.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 }; });
  await guest.mouse.move(box.x, box.y);
  for (let i = 0; i < 3; i++) {
    await guest.keyboard.down('Control');
    await guest.mouse.wheel(0, -120);
    await guest.keyboard.up('Control');
    await guest.waitForTimeout(200);
  }
  await guest.waitForTimeout(1000);
  check('zooming a viewer leaves the pane alone', paneBefore === pane());

  const roUrl = await createLink(owner, { password: '', readOnly: true });
  const roCtx = await browser.newContext({ viewport: { width: 1000, height: 700 } });
  const ro = await roCtx.newPage();
  await ro.goto(roUrl, { waitUntil: 'networkidle' });
  await ro.waitForSelector('.xterm-screen', { timeout: 20000 });
  await ro.waitForTimeout(2500);
  check('a link without a password opens straight up', Boolean(await ro.$('.xterm-screen')));
  await ro.click('.xterm-screen');
  await ro.keyboard.type('echo SHOULD-NOT-APPEAR');
  await ro.waitForTimeout(1500);
  check('read-only visitor cannot type', !(await ro.$eval('.xterm-rows', (el) => el.textContent)).includes('SHOULD-NOT-APPEAR'));

  const token = url.split('/share/')[1];
  await owner.click('button:has-text("공유")');
  await owner.waitForSelector('.share-dialog');
  owner.once('dialog', (d) => d.accept());
  await owner.locator('.share-list tr', { hasText: token.slice(0, 8) }).locator('button:has-text("취소")').click();
  await owner.waitForTimeout(1500);
  const revokedCtx = await browser.newContext();
  const revoked = await revokedCtx.newPage();
  await revoked.goto(url, { waitUntil: 'networkidle' });
  await revoked.waitForTimeout(1500);
  check('a revoked link stops working', (await revoked.textContent('body')).includes('열 수 없는 링크'));

  await browser.close();
  const failed = results.filter(([, ok]) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
