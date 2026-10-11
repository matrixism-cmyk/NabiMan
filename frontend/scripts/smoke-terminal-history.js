// Use an isolated server. Creates and ends a local shell.
// SMOKE_PASSWORD=... node scripts/smoke-terminal-history.js
const { chromium } = require('playwright');
if (!process.env.SMOKE_PASSWORD) throw new Error('SMOKE_PASSWORD is required');
(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, locale: 'en-US' });
  const sockets = [];
  const sent = [];
  const received = [];
  page.on('websocket', socket => {
    sockets.push(socket);
    socket.on('framesent', frame => sent.push(frame.payload.toString()));
    socket.on('framereceived', frame => received.push(frame.payload.toString()));
  });
  try {
    await page.goto(process.env.SMOKE_URL || 'http://127.0.0.1:19096');
    await page.fill('input[type=text]', 'admin');
    await page.fill('input[type=password]', process.env.SMOKE_PASSWORD);
    await page.click('button[type=submit]');
    await page.waitForSelector('.app-header');
    await page.locator('.cat-btn').filter({ hasText: 'Remote' }).click();
    await page.locator('.rw-item').filter({ hasText: 'Local shell' }).click();
    // A target opens on its session list; the terminal is one tab over.
    await page.locator('.rw-tabs button').filter({ hasText: 'Terminal' }).click();
    const connect = page.getByRole('button', { name: 'Connect', exact: true });
    if (await connect.count()) await connect.click();
    await page.waitForSelector('.xterm-helper-textarea');
    await page.waitForTimeout(800);
    await page.getByRole('button', { name: /Detach/ }).click();
    await page.waitForTimeout(800);
    await page.locator('.term-window .xterm-helper-textarea').focus();
    await page.keyboard.insertText("for i in $(seq 1 200); do printf 'HISTORY-%03d\\n' $i; done");
    await page.keyboard.press('Enter');
    await page.waitForTimeout(600);
    const gutter = await page.locator('.terminal-history-controls').boundingBox();
    await page.mouse.move(gutter.x + 12, gutter.y + 100);
    for (let i = 0; i < 5; i++) { await page.mouse.wheel(0, -120); await page.waitForTimeout(150); }
    const text = await page.locator('.term-window .xterm-rows').innerText();
    const markers = text.match(/\[\d+\/\d+\]/g) || [];
    if (markers.length !== 1) throw new Error(`Expected one history marker, got ${markers.length}: ${text}`);
    if (Number(markers[0].slice(1).split('/')[0]) === 0) throw new Error('History did not scroll');
    const geometry = await page.evaluate(() => {
      const rect = s => document.querySelector('.term-window ' + s).getBoundingClientRect();
      return { text: rect('.xterm-screen').right, gutter: rect('.terminal-history-controls').left,
        end: rect('.terminal-history-controls').right, resize: rect('.term-resize-e').left };
    });
    if (geometry.text > geometry.gutter || geometry.end > geometry.resize) throw new Error('Terminal areas overlap');
    const socketCount = sockets.length;
    const sessionsBefore = received.filter(frame => frame.startsWith('\x02SESSION:'));
    sent.length = 0;
    received.length = 0;
    await page.getByRole('button', { name: 'Redraw screen', exact: true }).click();
    await page.waitForTimeout(600);
    if (!sent.some(frame => /^\x01RESIZE:\d+:\d+$/.test(frame))) throw new Error('Redraw did not request tmux refresh');
    if (sent.some(frame => !frame.startsWith('\x01RESIZE:'))) throw new Error('Redraw sent application input');
    if (sockets.length !== socketCount || received.some(frame => frame.startsWith('\x02SESSION:'))) throw new Error('Redraw reconnected');
    if (!sessionsBefore.length || !received.length) throw new Error('No server redraw received');
    const redrawn = await page.locator('.term-window .xterm-rows').innerText();
    if (redrawn !== text) throw new Error('Redraw changed the history position or screen contents');
    console.log('PASS: redraw receives server output and preserves connection, content, and history position');
    console.log('PASS: tmux scrolling, no redraw residue, separate text/history/resize areas');
    const scrollbar = page.getByRole('scrollbar', { name: 'Terminal history position' });
    await scrollbar.waitFor();
    await page.waitForFunction(() => document.querySelector('[role=scrollbar]')?.getAttribute('aria-disabled') === 'false');
    const track = await scrollbar.boundingBox();
    const max = Number(await scrollbar.getAttribute('aria-valuemax'));
    await page.mouse.click(track.x + track.width / 2, track.y + track.height / 2);
    await page.waitForTimeout(350);
    let value = Number(await scrollbar.getAttribute('aria-valuenow'));
    if (!(await scrollbar.evaluate(el => el === document.activeElement))) throw new Error('History update stole scrollbar focus');
    if (Math.abs(value - max / 2) > 2) throw new Error(`Track click failed: ${value}/${max}`);
    const thumb = await page.locator('.term-window .terminal-history-thumb').boundingBox();
    await page.mouse.move(thumb.x + thumb.width / 2, thumb.y + thumb.height / 2);
    await page.mouse.down();
    // Pointer capture must keep dragging even outside the narrow track.
    await page.mouse.move(track.x + 100, track.y, { steps: 8 });
    await page.mouse.up();
    await page.waitForTimeout(350);
    value = Number(await scrollbar.getAttribute('aria-valuenow'));
    if (value !== 0) throw new Error(`Drag did not reach oldest history: ${value}`);
    const oldest = await page.locator('.term-window .xterm-rows').innerText();
    if (!(oldest.match(/\[\d+\/\d+\]/g) || []).length) throw new Error('Drag did not enter history');
    await scrollbar.focus();
    await page.keyboard.press('End');
    await page.waitForTimeout(350);
    value = Number(await scrollbar.getAttribute('aria-valuenow'));
    if (value !== Number(await scrollbar.getAttribute('aria-valuemax'))) throw new Error('End did not return to live output');
    const live = await page.locator('.term-window .xterm-rows').innerText();
    if (/\[\d+\/\d+\]/.test(live) || !live.includes('HISTORY-200')) throw new Error('Live screen was not restored');
    await page.locator('.term-window .xterm-helper-textarea').focus();
    await page.keyboard.insertText("printf 'AFTER-SCROLL-OK\\n'");
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);
    if (!(await page.locator('.term-window .xterm-rows').innerText()).includes('AFTER-SCROLL-OK')) throw new Error('Shell no longer accepts input');
    if (sockets.length !== socketCount) throw new Error('History navigation reconnected');
    console.log('PASS: track click, drag outside track, keyboard End, live output and shell input');
    if (process.env.SMOKE_SCREENSHOT) await page.screenshot({ path: process.env.SMOKE_SCREENSHOT });
  } finally {
    await page.evaluate(async () => {
      const headers = { Authorization: 'Bearer ' + sessionStorage.getItem('nabiman_token') };
      const ids = new Set(Object.keys(localStorage).filter(k => k.startsWith('nabiman_term_sid_')).map(k => localStorage.getItem(k)).filter(Boolean));
      for (const id of ids) await fetch('/api/terminal/sessions/' + encodeURIComponent(id), { method: 'DELETE', headers });
    }).catch(() => {});
    await browser.close();
  }
})().catch(error => { console.error(error); process.exit(1); });
