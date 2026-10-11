const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const origin = new URL(process.argv[2]).origin;
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  const urls = [], sessions = [], closes = [], errors = [], streamEnds = [];
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    page.on('pageerror', error => errors.push('Page error: ' + error.message.replaceAll(fixture.token, '[redacted]').replaceAll(fixture.refresh, '[redacted]').replace(/ticket=[a-f0-9]{64}/g, 'ticket=[redacted]')));
    page.on('request', req => {
      if (req.url().includes('token=') || req.url().includes(fixture.token)) errors.push('Credential in request URL');
    });
    page.on('requestfinished', req => {
      if (req.url().includes('/dashboard/live/stream')) streamEnds.push(true);
    });
    page.on('websocket', ws => {
      urls.push(ws.url());
      if (ws.url().includes('token=') || ws.url().includes(fixture.token)) errors.push('JWT in WebSocket URL');
      ws.on('framereceived', frame => {
        if (typeof frame.payload === 'string' && frame.payload.startsWith('\x02SESSION:')) sessions.push(frame.payload.slice(9));
      });
    });
    await page.addInitScript(({ token, refresh, origin }) => {
      if (window.top !== window || window.location.origin !== origin) return;
      localStorage.setItem('nabiman_lang', 'ko');
      if (!sessionStorage.getItem('ticket_smoke_bootstrapped')) {
        sessionStorage.setItem('nabiman_token', token);
        sessionStorage.setItem('nabiman_refresh', refresh);
        sessionStorage.setItem('nabiman_role', 'admin');
        sessionStorage.setItem('ticket_smoke_bootstrapped', '1');
      }
    }, { ...fixture, origin });
    // Observe close codes from the browser itself, including admission revocation.
    await page.addInitScript(origin => {
      if (window.top !== window || window.location.origin !== origin) return;
      window.__ticketCloseCodes = JSON.parse(sessionStorage.getItem('ticket_smoke_close_codes') || '[]');
      const nativeFetch = window.fetch;
      window.fetch = async (...args) => {
        const response = await nativeFetch(...args);
        if (String(args[0]).includes('/dashboard/live/stream') && response.ok && response.body) {
          const observer = response.clone().body.getReader();
          (async () => {
            while (!(await observer.read()).done) { /* Observe HTTP EOF without changing the UI's reader. */ }
            sessionStorage.setItem('ticket_smoke_stream_ended', '1');
          })().catch(() => {});
        }
        return response;
      };
      const Native = window.WebSocket;
      window.WebSocket = class extends Native {
        constructor(...args) { super(...args); this.addEventListener('close', event => { window.__ticketCloseCodes.push(event.code); sessionStorage.setItem('ticket_smoke_close_codes', JSON.stringify(window.__ticketCloseCodes)); }); }
      };
    }, origin);
    await page.goto(process.argv[2] + '/#remote/remote');
    await page.locator('.rw-item').filter({ hasText: '로컬 셸' }).click();
    // A target opens on its session list; the terminal is one tab over.
    await page.locator('.rw-tabs button').filter({ hasText: '터미널' }).click();
    const connect = page.getByRole('button', { name: '접속', exact: true });
    if (await connect.count()) await connect.click();
    for (let i = 0; i < 200 && !sessions.length; i++) await page.waitForTimeout(100);
    if (!sessions.length || !urls.some(url => /[?&]ticket=[a-f0-9]{64}/.test(url))) throw new Error('Account terminal did not connect through a one-use ticket');
    const beforeDetach = sessions.length;
    await page.getByRole('button', { name: /분리/ }).click();
    await page.locator('.term-window .xterm-helper-textarea').waitFor();
    for (let i = 0; i < 200 && sessions.length <= beforeDetach; i++) await page.waitForTimeout(100);
    if (sessions.length <= beforeDetach) throw new Error('Detached window failed fresh ticket admission');
    await page.locator('.term-window .xterm-helper-textarea').focus();
    await page.keyboard.insertText("printf 'NABIMAN-TICKET-OK\\n'");
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => [...document.querySelectorAll('.term-window .xterm-rows')].some(el => el.innerText.split('\n').some(line => line.trim() === 'NABIMAN-TICKET-OK')));
    await page.screenshot({ path: path.resolve(__dirname, '../../screenshots/terminal-ticket.png'), fullPage: true });
    await page.goto(process.argv[2] + '/#system/files');
    await page.getByRole('heading', { name: '파일 관리자', exact: true }).waitFor();
    const input = page.locator('.panel .filter-input').first();
    await input.fill(path.dirname(fixture.download_path));
    await input.press('Enter');
    const row = page.getByRole('row').filter({ hasText: path.basename(fixture.download_path) });
    const pending = page.waitForEvent('download');
    const downloadResponse = page.waitForResponse(response => response.url().includes('/api/files/download?'));
    await row.getByRole('button', { name: '다운로드', exact: true }).click();
    const download = await pending;
    if ((await downloadResponse).headers()['cache-control'] !== 'no-store') throw new Error('Private file download allowed caching');
    if (fs.readFileSync(await download.path(), 'utf8') !== 'Isolated download proof\n') throw new Error('Header-authenticated file download content mismatch');
    const stream = page.waitForRequest(req => req.url().includes('/dashboard/live/stream'));
    const streamResponse = page.waitForResponse(response => response.url().includes('/dashboard/live/stream'));
    await page.goto(process.argv[2] + '/#mec/mecLive');
    const request = await stream;
    const response = await streamResponse;
    if (response.status() !== 200 || !response.headers()['content-type'].includes('text/event-stream') || response.headers()['cache-control'] !== 'no-store' || streamEnds.length) throw new Error('Dashboard stream was not active before logout');
    if (request.url().includes('token=') || request.headers().authorization !== 'Bearer ' + fixture.token) throw new Error('Dashboard stream failed header authentication');
    const result = await page.evaluate(async token => {
      const response = await fetch('/api/auth/logout', { method: 'POST', headers: { Authorization: 'Bearer ' + token } });
      return response.status;
    }, fixture.token);
    if (result !== 200) throw new Error('Isolated logout failed');
    await page.waitForFunction(() => window.__ticketCloseCodes.includes(1008), undefined, { timeout: 10000 });
    closes.push(...await page.evaluate(() => window.__ticketCloseCodes));
    if (errors.length) throw new Error(errors.join('; '));
    await page.waitForFunction(() => sessionStorage.getItem('ticket_smoke_stream_ended') === '1', undefined, { timeout: 10000 });
    console.log('PASS: actual terminal UI uses ticket; file download and dashboard use headers; logout closes live socket with 1008 and dashboard stream.');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
