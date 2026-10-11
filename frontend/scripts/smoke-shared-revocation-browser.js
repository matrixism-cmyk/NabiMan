const { chromium, request } = require('playwright');
const fs = require('fs');
const path = require('path');
let proofPhase = 'startup';
function requireProof(condition, message) { if (!condition) throw new Error(message); }
(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const base = new URL(process.argv[2]).origin;
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  const http = await request.newContext();
  const errors = [], sessions = [];
  async function api(endpoint, method = 'GET', body, token = fixture.token) {
    const response = await http.fetch(base + endpoint, { method, headers: { Authorization: 'Bearer ' + token }, data: body });
    let value; try { value = await response.json(); } catch { value = {}; }
    return { status: response.status(), value, headers: response.headers() };
  }
  async function guest(link, password) {
    const context = await browser.newContext();
    const page = await context.newPage();
    page.on('pageerror', error => errors.push('Guest: ' + error.message.split('\n')[0]));
    page.on('request', req => {
      if (new URL(req.url()).pathname.startsWith('/api/share/') && req.headers().authorization) errors.push('Control-plane credential sent by anonymous share');
    });
    await page.addInitScript(origin => {
      if (window.top !== window || window.location.origin !== origin) return;
      localStorage.setItem('nabiman_lang', 'ko');
      sessionStorage.setItem('nabiman_token', 'unused-control-plane-fixture');
      window.__shareCloseCodes = []; window.__shareSocketCount = 0; window.__shareSockets = []; window.__shareSocketUrls = [];
      const Native = window.WebSocket;
      window.WebSocket = class extends Native {
        constructor(...args) { super(...args); window.__shareSocketCount++; window.__shareSockets.push(this); window.__shareSocketUrls.push(String(args[0])); this.addEventListener('close', event => window.__shareCloseCodes.push(event.code)); }
      };
    }, base);
    await page.goto(base + '/share/' + link.token);
    if (password) {
      await page.locator('.share-gate input').fill('wrong-fixture-password');
      await page.locator('.share-gate button').click();
      await page.locator('.login-error').waitFor();
      await page.locator('.share-gate input').fill(password);
      await page.locator('.share-gate button').click();
    }
    await page.locator('.xterm-helper-textarea').waitFor();
    await page.waitForFunction(() => document.querySelector('.xterm-rows')?.innerText.includes('SHARE-OWNER-READY'));
    return page;
  }
  try {
    const owner = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    owner.on('pageerror', error => errors.push('Owner: ' + error.message.split('\n')[0]));
    owner.on('websocket', ws => ws.on('framereceived', frame => { if (typeof frame.payload === 'string' && frame.payload.startsWith('\x02SESSION:')) sessions.push(frame.payload.slice(9)); }));
    await owner.addInitScript(({ token, refresh, origin }) => { if (window.top !== window || window.location.origin !== origin) return; localStorage.setItem('nabiman_lang', 'ko'); sessionStorage.setItem('nabiman_token', token); sessionStorage.setItem('nabiman_refresh', refresh); sessionStorage.setItem('nabiman_role', 'admin'); }, { ...fixture, origin: base });
    await owner.goto(base + '/#remote/remote');
    proofPhase = 'owner terminal';
    await owner.locator('.rw-item').filter({ hasText: '로컬 셸' }).click();
    // A target opens on its session list; the terminal is one tab over.
    await owner.locator('.rw-tabs button').filter({ hasText: '터미널' }).click();
    const connect = owner.getByRole('button', { name: '접속', exact: true });
    if (await connect.count()) await connect.click();
    await owner.locator('.xterm-helper-textarea').waitFor();
    for (let i = 0; i < 150 && !sessions.length; i++) await owner.waitForTimeout(100);
    requireProof(sessions.length === 1, 'Owner terminal did not connect');
    const session = sessions[0];
    await owner.locator('.xterm-helper-textarea').focus();
    await owner.keyboard.insertText("printf 'SHARE-OWNER-READY\\n'"); await owner.keyboard.press('Enter');
    await owner.waitForFunction(() => document.querySelector('.xterm-rows')?.innerText.includes('SHARE-OWNER-READY'));
    const sharePath = '/api/terminal/sessions/' + session + '/share-link';
    proofPhase = 'expiry validation and share UI';
    for (const expiry of [0, 30 * 86400 + 1]) requireProof((await api(sharePath, 'POST', { expires_in_secs: expiry })).status === 400, 'Unbounded or excessive share expiry accepted');
    // Exercise the actual creation UI and its safe default.
    await owner.getByTitle('이 터미널만 볼 수 있는 링크를 만들어 다른 사람에게 전달합니다.').click();
    await owner.locator('.share-dialog').waitFor();
    requireProof(await owner.locator('.share-dialog input[type=checkbox]').isChecked(), 'Share UI did not default to read-only');
    await owner.locator('.share-dialog input[type=password]').fill('isolated-share-password');
    const createdResponse = owner.waitForResponse(r => r.url().endsWith(sharePath) && r.request().method() === 'POST');
    await owner.getByRole('button', { name: '링크 만들기', exact: true }).click();
    const response = await createdResponse; const created = await response.json();
    requireProof(response.status() === 200 && created.success && created.data.read_only && created.data.usable && /^[a-f0-9]{64}$/.test(created.data.token), 'Share creation receipt invalid');
    requireProof(response.headers()['cache-control'] === 'no-store', 'Share creation allowed caching');
    requireProof(created.data.password_hash === null, 'Password hash leaked in owner receipt');
    await owner.getByRole('button', { name: '닫기', exact: true }).click();
    const readonly = created.data;
    proofPhase = 'first password guest';
    const first = await guest(readonly, 'isolated-share-password');
    proofPhase = 'second password guest';
    const second = await guest(readonly, 'isolated-share-password');
    await first.locator('.xterm-helper-textarea').focus();
    await first.keyboard.insertText("printf 'READONLY-MUST-NOT-EXECUTE\\n'"); await first.keyboard.press('Enter');
    await first.waitForTimeout(300);
    requireProof(!(await owner.locator('.xterm-rows').innerText()).includes('READONLY-MUST-NOT-EXECUTE'), 'Read-only input reached owner shell');
    const writableResponse = await api(sharePath, 'POST', { expires_in_secs: 3600, read_only: false });
    proofPhase = 'writable guest';
    requireProof(writableResponse.value.success, 'Explicit writable share failed');
    const writable = await guest(writableResponse.value.data);
    proofPhase = 'fresh share admission after network reconnect';
    const before = await first.evaluate(() => window.__shareSocketCount);
    await first.evaluate(() => window.__shareSockets[window.__shareSockets.length - 1].close(4001, 'isolated network interruption'));
    await first.waitForFunction(count => window.__shareSocketCount > count && window.__shareSockets[window.__shareSockets.length - 1].readyState === WebSocket.OPEN, before, { timeout: 10000 });
    const observed = await first.evaluate(() => window.__shareSocketUrls);
    const proofs = observed.map(value => new URL(value).searchParams.get('ticket'));
    requireProof(proofs.every(value => /^[a-f0-9]{64}$/.test(value)) && new Set(proofs).size === proofs.length, 'Reconnect reused or forged a share admission');
    const storage = await first.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }));
    requireProof(!storage.includes('isolated-share-password') && proofs.every(value => !storage.includes(value)), 'Password or admission persisted in browser storage');
    const used = new URL(observed[0]);
    requireProof((await http.get(base + used.pathname + used.search, { headers: { Origin: base } })).status() === 401, 'Already-consumed browser proof replay accepted');
    // Exercise the real HTTP boundary with correctly authenticated proofs.
    const nativeIssue = async (query = '') => {
      const response = await http.post(base + '/api/share/' + readonly.token + '/auth', { headers: { Origin: base }, data: { password: 'isolated-share-password', query } });
      const receipt = await response.json();
      requireProof(response.status() === 200 && response.headers()['cache-control'] === 'no-store' && receipt.data.expires_in_seconds === 30, 'Native share admission receipt failed');
      return receipt.data.ticket;
    };
    proofPhase = 'query Origin and target binding';
    const queryProof = await nativeIssue('cols=80');
    requireProof((await http.get(base + '/api/share/' + readonly.token + '/ws?cols=81&ticket=' + queryProof, { headers: { Origin: base } })).status() === 401, 'Changed geometry admitted');
    const originProof = await nativeIssue();
    requireProof((await http.get(base + '/api/share/' + readonly.token + '/ws?ticket=' + originProof, { headers: { Origin: 'https://other.invalid' } })).status() === 401, 'Changed Origin admitted');
    requireProof((await http.get(base + '/api/share/' + readonly.token + '/ws?ticket=' + originProof, { headers: { Origin: base } })).status() === 401, 'Mismatched Origin proof was reusable');
    const targetProof = await nativeIssue();
    requireProof((await http.get(base + '/api/share/' + writableResponse.value.data.token + '/ws?ticket=' + targetProof, { headers: { Origin: base } })).status() === 401, 'Another link accepted proof');
    const account = await api('/api/terminal/tickets', 'POST', { query: '' });
    requireProof(account.value.success && (await http.get(base + '/api/share/' + readonly.token + '/ws?ticket=' + account.value.data.ticket, { headers: { Origin: base } })).status() === 401, 'Account ticket accepted as anonymous share proof');
    const shareOnly = await nativeIssue();
    requireProof((await http.get(base + '/api/terminal?ticket=' + shareOnly, { headers: { Origin: base } })).status() === 401, 'Share proof escalated to account terminal');
    requireProof((await http.get(base + '/api/share/' + readonly.token + '/ws?ticket=9999999999.legacy-signed-proof', { headers: { Origin: base } })).status() === 401, 'Legacy reusable proof accepted');
    proofPhase = 'actual thirty-second admission expiry';
    const expiredProof = await nativeIssue();
    await first.waitForTimeout(31000);
    requireProof((await http.get(base + '/api/share/' + readonly.token + '/ws?ticket=' + expiredProof, { headers: { Origin: base } })).status() === 401, 'Expired admission accepted');
    proofPhase = 'live connection limit and lease release';
    const bounded = await api(sharePath, 'POST', { expires_in_secs: 3600, read_only: true });
    const capacityPage = await guest(bounded.value.data);
    const connectExtra = async () => capacityPage.evaluate(async token => {
      const response = await fetch('/api/share/' + token + '/auth', { method: 'POST', credentials: 'omit', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ password: '', query: '' }) });
      if (!response.ok) return response.status;
      const receipt = await response.json();
      const socket = new WebSocket((location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/api/share/' + token + '/ws?ticket=' + receipt.data.ticket);
      window.__extraSharedSockets = window.__extraSharedSockets || []; window.__extraSharedSockets.push(socket);
      return await new Promise(resolve => { socket.onopen = () => resolve(101); socket.onerror = () => resolve(0); });
    }, bounded.value.data.token);
    for (let i = 0; i < 7; i++) requireProof(await connectExtra() === 101, 'Shared connection capacity underflow');
    requireProof(await connectExtra() === 429, 'Ninth shared attachment accepted');
    await capacityPage.evaluate(() => window.__extraSharedSockets[0].close(4001, 'release isolated lease'));
    await capacityPage.waitForFunction(() => window.__extraSharedSockets[0].readyState === WebSocket.CLOSED);
    await capacityPage.waitForTimeout(200);
    requireProof(await connectExtra() === 101, 'Closed attachment failed to release capacity');
    await capacityPage.context().close();
    proofPhase = 'password attempt throttling';
    let limited = false;
    for (let i = 0; i < 31; i++) {
      const response = await http.post(base + '/api/share/' + readonly.token + '/auth', { headers: { Origin: base, 'X-Forwarded-For': '192.0.2.' + i }, data: { password: 'wrong', query: '' } });
      if (response.status() === 429) { limited = response.headers()['retry-after'] === '60' && response.headers()['cache-control'] === 'no-store'; break; }
      requireProof(response.status() === 403, 'Wrong password unexpectedly admitted');
    }
    requireProof(limited, 'Share brute-force attempts were not bounded');
    await writable.locator('.xterm-helper-textarea').focus();
    await writable.keyboard.insertText("printf 'SHARE-WRITE-OK\\n'"); await writable.keyboard.press('Enter');
    await owner.waitForFunction(() => document.querySelector('.xterm-rows')?.innerText.split('\n').some(line => line.trim() === 'SHARE-WRITE-OK'));
    const revoked = await api('/api/terminal/share-links/' + readonly.token, 'DELETE');
    proofPhase = 'readonly revocation';
    requireProof(revoked.value.success && revoked.headers['cache-control'] === 'no-store', 'Durable share revocation failed');
    for (const page of [first, second]) await page.waitForFunction(() => window.__shareCloseCodes.includes(1008), undefined, { timeout: 10000 });
    await first.waitForTimeout(500);
    requireProof(await first.evaluate(count => window.__shareSocketCount === count, observed.length), 'Revoked guest automatically reconnected');
    requireProof((await api('/api/share/' + readonly.token)).value.success === false, 'Revoked share metadata still accessible');
    const expiry = await api(sharePath, 'POST', { expires_in_secs: 6, read_only: true });
    proofPhase = 'expiry guest';
    const expiring = await guest(expiry.value.data);
    await expiring.waitForFunction(() => window.__shareCloseCodes.includes(1008), undefined, { timeout: 10000 });
    requireProof((await api('/api/share/' + expiry.value.data.token)).value.success === false, 'Expired share metadata still accessible');
    // The unrelated writable link remains connected and the owner shell survives revocation.
    requireProof(await writable.evaluate(() => !window.__shareCloseCodes.includes(1008)), 'Unrelated link was revoked');
    await owner.locator('.xterm-helper-textarea').focus();
    await owner.keyboard.insertText("printf 'OWNER-SHELL-SURVIVES\\n'"); await owner.keyboard.press('Enter');
    await writable.waitForFunction(() => document.querySelector('.xterm-rows')?.innerText.split('\n').some(line => line.trim() === 'OWNER-SHELL-SURVIVES'));
    requireProof(!(await first.locator('.xterm-rows').innerText()).includes('OWNER-SHELL-SURVIVES'), 'Revoked guest received new terminal output');
    const demote = await api('/api/users/' + fixture.owner_id, 'PUT', { role: 'viewer' }, fixture.recovery_token);
    proofPhase = 'owner demotion';
    requireProof(demote.status === 200 && demote.value.success, 'Fixture owner demotion failed');
    await writable.waitForFunction(() => window.__shareCloseCodes.includes(1008), undefined, { timeout: 10000 });
    requireProof((await api('/api/share/' + writableResponse.value.data.token)).value.success === false, 'Demoted owner share still accessible');
    proofPhase = 'owner restored without resurrecting old grants';
    const restored = await api('/api/users/' + fixture.owner_id, 'PUT', { role: 'admin' }, fixture.recovery_token);
    requireProof(restored.status === 200 && restored.value.success, 'Fixture owner restoration failed');
    requireProof((await api('/api/share/' + writableResponse.value.data.token)).value.success === false, 'Restored owner resurrected old share');
    const deniedAuth = await http.post(base + '/api/share/' + writableResponse.value.data.token + '/auth', { headers: { Origin: base }, data: { password: '', query: '' } });
    requireProof(deniedAuth.status() === 403, 'Restored owner issued proof for old share');
    const relogin = await http.post(base + '/api/auth/login', { data: { username: 'admin', password: fixture.owner_password } });
    const reloginValue = await relogin.json();
    requireProof(relogin.status() === 200 && reloginValue.success, 'Restored owner login failed');
    const newShare = await api(sharePath, 'POST', { expires_in_secs: 3600, read_only: true }, reloginValue.data.token);
    requireProof(newShare.value.success && newShare.value.data.usable, 'Fresh share approval denied after restoration');
    const renewed = await guest(newShare.value.data);
    await renewed.context().close();
    const restoredList = await api('/api/terminal/share-links', 'GET', undefined, reloginValue.data.token);
    requireProof(restoredList.value.success && restoredList.value.data.some(l => l.token === writableResponse.value.data.token && !l.usable), 'Old owner link incorrectly listed as usable');
    await first.screenshot({ path: path.resolve(__dirname, '../../screenshots/shared-terminal-revoked.png'), fullPage: true });
    requireProof(errors.length === 0, 'Browser page errors: ' + errors.join('; '));
    console.log('PASS: actual share UI, fresh one-use reconnect admission, replay/Origin/query/target/legacy denial, real 30-second expiry, password throttling, no control-plane JWT or persisted password/admission, live revoke/expiry/owner demotion, permanent denial after restoration and unrelated shell preservation.');
  } finally { await http.dispose(); await browser.close(); }
})().catch(error => { console.error('FAIL: isolated shared terminal browser proof at ' + proofPhase + ' (' + error.name + '): ' + error.message.split('\n')[0].replace(/[a-f0-9]{64}/g, '[opaque]').replace(/eyJ[\w.-]+/g, '[jwt]')); process.exit(1); });
