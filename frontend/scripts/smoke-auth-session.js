const fs = require('fs');
const { chromium } = require('playwright');

(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  const modes = ['late-401', 'late-refresh-success', 'late-refresh-failure'];
  try {
    for (const mode of modes) {
      const context = await browser.newContext();
      let release;
      try {
        const login = await context.request.post(fixture.base + '/api/auth/login', {
          data: { username: 'admin', password: fixture.adminPassword },
        });
        const session = (await login.json()).data;
        if (!session?.token || !session.refresh_token) throw new Error('Fixture login failed');
        const page = await context.newPage();
        page.setDefaultTimeout(15000);
        await page.addInitScript(({ token, refresh_token }) => {
          sessionStorage.setItem('nabiman_token', token);
          sessionStorage.setItem('nabiman_refresh', refresh_token);
          localStorage.setItem('nabiman_lang', 'en');
        }, session);
        await page.route('**/api/services', route => route.fulfill({ json: {
          success: true, data: [{ name: 'never-dispatched-fixture', description: 'Race fixture',
            load_state: 'not-found', active_state: 'inactive', sub_state: 'dead', enabled: true }],
        } }));
        let posts = 0, refreshes = 0, signal;
        const seen = new Promise(resolve => { signal = resolve; });
        const gate = new Promise(resolve => { release = resolve; });
        await page.route('**/api/services/action', async route => {
          posts++;
          if (mode === 'late-401') { signal(); await gate; }
          await route.fulfill({ status: 401, json: { success: false, message: 'Fixture expired access token' } });
        });
        await page.route('**/api/auth/refresh', async route => {
          refreshes++;
          const result = mode === 'late-refresh-success' ? await route.fetch() : null;
          if (result && result.status() !== 200) throw new Error('Actual refresh did not succeed');
          signal(); await gate;
          if (result) await route.fulfill({ response: result });
          else await route.fulfill({ status: 401, json: { success: false } });
        });
        await page.goto(fixture.base + '/#management/services');
        await page.locator('.app-header').waitFor();
        if (!(await page.getByRole('heading', { name: 'Service Management', exact: true }).count())) {
          await page.locator('.cat-btn').filter({ hasText: 'Management' }).click();
          await page.getByRole('button', { name: 'Services', exact: true }).click();
        }
        await page.getByRole('button', { name: 'Restart', exact: true }).click();
        let timer;
        try { await Promise.race([seen, new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('Race boundary was not reached')), 15000); })]); }
        finally { clearTimeout(timer); }
        await page.locator('.logout-btn').click();
        await page.locator('.login-form input[type="text"]').fill('viewer');
        await page.locator('.login-form input[type="password"]').fill(fixture.viewerPassword);
        await page.locator('.login-form button[type="submit"]').click();
        await page.locator('.app-header').waitFor();
        const before = await page.evaluate(() => [sessionStorage.getItem('nabiman_token'), sessionStorage.getItem('nabiman_refresh')]);
        if (!before[0] || !before[1] || before[0] === session.token) throw new Error('Replacement login did not complete');
        release();
        await page.waitForTimeout(800);
        const after = await page.evaluate(() => [sessionStorage.getItem('nabiman_token'), sessionStorage.getItem('nabiman_refresh')]);
        if (JSON.stringify(before) !== JSON.stringify(after)) throw new Error('Old response changed the replacement credentials');
        if (posts !== 1 || refreshes !== (mode === 'late-401' ? 0 : 1)) throw new Error('Old request refreshed or replayed across logins');
        if (!(await page.locator('.app-header').count())) throw new Error('Old failure logged out the new account');
        const me = await page.evaluate(async () => (await (await fetch('/api/users/me', {
          headers: { Authorization: 'Bearer ' + sessionStorage.getItem('nabiman_token') },
        })).json()).data);
        if (me?.username !== 'viewer') throw new Error('Authenticated identity changed after the delayed response');
      } finally { release?.(); await context.close(); }
    }
    process.stdout.write(JSON.stringify({ scenarios: modes.length, replayed: 0 }));
  } finally { await browser.close(); }
})().catch(error => { console.error(error.message); process.exit(1); });
