// Browser smoke test for clipboard behaviour in a terminal pane.
//
//   SMOKE_URL=http://127.0.0.1:18096 npm run smoke:paste
//
// Needs a NabiMan server serving build/ and `npx playwright install chromium`.
// Every paste route is easy to break and hard to notice, so they are all here:
// Ctrl+V, Ctrl+Shift+V, middle-click, right-click, a multi-line paste, a paste
// while a selection holds the pane in copy-mode, and an image on the clipboard
// (which is uploaded and pasted as its path on the server).
const { chromium } = require('playwright');
const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const results = [];
const check = (name, ok) => { results.push(ok); console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}`); };
const screen = (p) => p.$eval('.xterm-rows', el => el.textContent);
(async () => {
  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await ctx.grantPermissions(['clipboard-read','clipboard-write'], { origin: BASE });
  const page = await ctx.newPage();
  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]','admin'); await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]'); await page.waitForSelector('.app-header');
  await page.click('.cat-btn:has-text("원격")'); await page.click('.rw-item:has-text("로컬 셸")');
  await page.waitForTimeout(700);
  const c = await page.$('.rw-connect-card button.btn-primary'); if (c) await c.click();
  await page.waitForSelector('.xterm-screen', { timeout: 20000 }); await page.waitForTimeout(3000);
  await page.click('.xterm-screen');

  // 1. Ctrl+V — delivered as a paste event, the way a browser does it
  await page.evaluate(() => {
    const dt = new DataTransfer();
    dt.setData('text/plain', 'CTRLV-PASTE');
    const target = document.querySelector('.xterm-helper-textarea') || document.querySelector('.xterm-screen');
    target.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
  });
  await page.waitForTimeout(1200);
  check('Ctrl+V (paste event) pasted', (await screen(page)).includes('CTRLV-PASTE'));

  // 2. Ctrl+Shift+V (terminal convention)
  await page.keyboard.press('Control+u');  // clear the line
  await page.evaluate(() => navigator.clipboard.writeText('CTRLSHIFTV-PASTE'));
  await page.keyboard.press('Control+Shift+V');
  await page.waitForTimeout(1200);
  check('Ctrl+Shift+V pasted', (await screen(page)).includes('CTRLSHIFTV-PASTE'));

  // 3. Middle click (X11 habit)
  await page.keyboard.press('Control+u');
  await page.evaluate(() => navigator.clipboard.writeText('MIDDLE-PASTE'));
  const box = await page.$eval('.xterm-screen', el => { const b = el.getBoundingClientRect(); return { x: b.x + 100, y: b.y + 40 }; });
  await page.mouse.move(box.x, box.y);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.up({ button: 'middle' });
  await page.waitForTimeout(1200);
  check('middle-click pasted', (await screen(page)).includes('MIDDLE-PASTE'));

  // 4. Right click (ours)
  await page.keyboard.press('Control+u');
  await page.evaluate(() => navigator.clipboard.writeText('RIGHT-PASTE'));
  await page.mouse.click(box.x, box.y, { button: 'right' });
  await page.waitForTimeout(1200);
  check('right-click pasted', (await screen(page)).includes('RIGHT-PASTE'));

  // 5. Multi-line paste
  await page.keyboard.press('Control+u');
  await page.evaluate(() => navigator.clipboard.writeText('echo LINE-A\necho LINE-B\n'));
  await page.mouse.click(box.x, box.y, { button: 'right' });
  await page.waitForTimeout(1800);
  const s = await screen(page);
  check('multi-line paste ran both lines', s.includes('LINE-A') && s.includes('LINE-B'));

  // 6. Paste while tmux sits in copy-mode (after selecting something)
  await page.keyboard.press('Control+u');
  await page.mouse.dblclick(box.x, box.y);
  await page.waitForTimeout(900);
  await page.evaluate(() => navigator.clipboard.writeText('AFTER-SELECT'));
  await page.keyboard.press('Control+Shift+V');
  await page.waitForTimeout(1200);
  check('Ctrl+Shift+V while in copy-mode', (await screen(page)).includes('AFTER-SELECT'));
  // 7. Ctrl+Shift+V must paste once, not twice: the browser pastes plain text
  //    by itself, and the key handler used to paste on top of that.
  await page.keyboard.press('Control+u');
  await page.evaluate(() => navigator.clipboard.writeText('ONCE-ONLY-TOKEN'));
  await page.keyboard.press('Control+Shift+V');
  await page.waitForTimeout(1500);
  const line = await page.$eval('.xterm-rows', (el) => el.textContent);
  check('Ctrl+Shift+V pasted exactly once',
    (line.match(/ONCE-ONLY-TOKEN/g) || []).length === 1);

  // 8. An image on the clipboard: a terminal cannot carry the bytes, so the
  //    server keeps the file and the pane receives the path.
  await page.keyboard.press('Control+u');
  await page.evaluate(() => {
    // Smallest valid PNG, as a file on the clipboard.
    const b64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8AAAwAB/gFvXFMZAAAAAElFTkSuQmCC';
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    const dt = new DataTransfer();
    dt.items.add(new File([bytes], 'shot.png', { type: 'image/png' }));
    const target = document.querySelector('.xterm-helper-textarea') || document.querySelector('.xterm-screen');
    target.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
  });
  await page.waitForTimeout(2000);
  const withImage = await screen(page);
  check('image paste arrived as a saved path', /pastes\/\w+\/paste-[\d-]+-\w+\.png/.test(withImage));

  const all = await screen(page);
  check('no control-prefix junk on screen', !all.includes('PASTE:'));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(e => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
