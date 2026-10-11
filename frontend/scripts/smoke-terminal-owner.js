const fs = require('fs');
const { chromium } = require('playwright');
(async () => {
  const f = JSON.parse(fs.readFileSync(0, 'utf8'));
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage(); await page.goto(f.base);
    const result = await page.evaluate(async f => {
      const check = (ok, why) => { if (!ok) throw new Error(why); };
      const api = async (path, method = 'GET', body, token = f.token) => {
        const r = await fetch(path, { method, headers: { Authorization: 'Bearer ' + token, 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body) });
        return { status: r.status, value: await r.json() };
      };
      const wait = async (test, why) => { for (let i = 0; i < 200; i++) { if (test()) return; await new Promise(r => setTimeout(r, 50)); } throw new Error(why); };
      const connect = async (token, query = '') => {
        const admission = (await api('/api/terminal/tickets', 'POST', { query }, token)).value;
        check(admission.success, 'Admission ticket missing');
        const ws = new WebSocket(location.origin.replace(/^http/, 'ws') + '/api/terminal?' + (query ? query + '&' : '') + 'ticket=' + admission.data.ticket);
        const state = { ws, session: null, text: '', closed: null }; ws.binaryType = 'arraybuffer';
        ws.onmessage = e => { const text = typeof e.data === 'string' ? e.data : new TextDecoder().decode(e.data); state.text += text; if (text.startsWith('\x02SESSION:')) state.session = text.slice(9); };
        ws.onclose = e => state.closed = e.code;
        await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = () => reject(new Error('Fixture socket failed')); });
        return state;
      };
      const denied = async (token, id) => {
        for (const suffix of ['&join=1', '']) {
          const socket = await connect(token, 'session_id=' + id + suffix);
          await wait(() => socket.text.includes('\x02ENDED'), 'Unowned session did not return END');
          check(!socket.session && !socket.text.includes('OWNER-OLD-ID'), 'Unowned session attached or fell back to a local shell');
        }
        for (const [path, body] of [['name', { name: 'takeover' }], ['share', { shared: true }], ['timeout', { timeout_secs: 60 }], ['keepalive', { keepalive: true }], ['share-link', { expires_in_secs: 3600 }]]) {
          check(!(await api('/api/terminal/sessions/' + id + '/' + path, 'POST', body, token)).value.success, 'Unowned metadata mutation succeeded: ' + path);
        }
        check(!(await api('/api/terminal/sessions/' + id + '/scrollback', 'GET', undefined, token)).value.success, 'Unowned history leaked');
        check(!(await api('/api/terminal/sessions/' + id, 'DELETE', undefined, token)).value.success, 'Unowned session termination succeeded');
        const list = (await api('/api/terminal/sessions', 'GET', undefined, token)).value.data;
        check(!list.some(s => s.id === id), 'Unowned session was visible in account inventory');
      };
      if (f.resume) {
        const login = (await api('/api/auth/login', 'POST', { username: 'reused', password: f.password })).value;
        check(login.success, 'Replacement account login failed after restart');
        const token = login.data.token;
        await denied(token, f.resume.old); await denied(token, f.resume.legacy);
        const current = await connect(token, 'session_id=' + f.resume.current + '&join=1');
        await wait(() => current.session === f.resume.current, 'Restart lost immutable current ownership');
        current.ws.close();
        return { resumed: true };
      }
      check((await api('/api/users', 'POST', { username: 'reused', password: f.password, role: 'admin' })).value.success, 'Owner creation failed');
      let users = (await api('/api/users')).value.data;
      const oldId = users.find(u => u.username === 'reused').id;
      const ownerLogin = (await api('/api/auth/login', 'POST', { username: 'reused', password: f.password })).value;
      const oldToken = ownerLogin.data.token;
      const owner = await connect(oldToken); await wait(() => owner.session, 'Owner session missing');
      owner.ws.send("printf 'OWNER-OLD-ID\\n'\n"); await wait(() => owner.text.includes('OWNER-OLD-ID'), 'Owner screen fixture missing');
      const local = await connect(f.token); await wait(() => local.session, 'Unrelated administrator session missing');
      check((await api('/api/terminal/sessions/' + owner.session + '/share', 'POST', { shared: true }, oldToken)).value.success, 'Account share flag failed');
      const watcher = await connect(f.token, 'session_id=' + owner.session + '&join=1');
      await wait(() => watcher.session === owner.session && watcher.text.includes('OWNER-OLD-ID'), 'Explicit account sharing failed');
      const share = (await api('/api/terminal/sessions/' + owner.session + '/share-link', 'POST', { expires_in_secs: 3600 }, oldToken)).value.data.token;
      check((await api('/api/users/' + oldId, 'DELETE')).value.success, 'Owner deletion failed');
      await wait(() => owner.closed === 1008 && watcher.closed === 1008, 'Deleted owner kept owner or account-shared attachment');
      check(local.closed === null, 'Owner deletion closed an unrelated administrator shell');
      check((await api('/api/users', 'POST', { username: 'reused', password: f.password, role: 'admin' })).value.success, 'Same-name replacement creation failed');
      users = (await api('/api/users')).value.data; const newId = users.find(u => u.username === 'reused').id;
      check(newId !== oldId, 'Replacement identity reused the deleted ID');
      const login = (await api('/api/auth/login', 'POST', { username: 'reused', password: f.password })).value;
      const token = login.data.token; await denied(token, owner.session);
      check(!(await api('/api/share/' + share)).value.success, 'Deleted owner public share remained usable');
      const current = await connect(token); await wait(() => current.session, 'Fresh replacement session failed');
      const legacy = await connect(token); await wait(() => legacy.session, 'Legacy migration candidate missing');
      current.ws.close(); legacy.ws.close(); local.ws.close();
      return { old: owner.session, current: current.session, legacy: legacy.session, old_owner: oldId, new_owner: newId };
    }, f);
    console.log(JSON.stringify(result));
  } finally { await browser.close(); }
})().catch(e => { console.error('FAIL: terminal identity fixture: ' + e.message.split('\n')[0]); process.exit(1); });
