const fs = require('fs');
const { chromium } = require('playwright');
(async () => {
  const f = JSON.parse(fs.readFileSync(0, 'utf8'));
  const b = await chromium.launch({ headless: true, executablePath: f.chromiumPath, args: ['--no-sandbox'] });
  try {
    const p = await b.newPage();
    await p.addInitScript(token => { sessionStorage.setItem('nabiman_token', token); localStorage.setItem('nabiman_lang', 'ko'); }, f.token);
    await p.goto(f.base + '/#security/enterpriseauth');
    await p.getByRole('button', { name: 'API 키 관리', exact: true }).click();
    await p.getByLabel('키 이름', { exact: true }).fill(f.name || 'Browser token fixture');
    await p.getByLabel('유효 기간 (1~90일)', { exact: true }).fill('2');
    await p.getByRole('button', { name: '키 생성', exact: true }).click();
    const secret = p.getByTestId('issued-token'); await secret.waitFor();
    const raw = await secret.textContent();
    if (!raw.startsWith('nbm_')) throw Error('One-time token not shown');
    const headers = { Authorization: 'Bearer ' + raw };
    if ((await p.request.get(f.base + '/api/server/status', { headers })).status() !== 200) throw Error('Issued token rejected');
    if ((await p.request.get(f.base + '/api/users', { headers })).status() !== 403) throw Error('Token exceeded scope');
    p.on('dialog', dialog => dialog.accept());
    const row = p.getByRole('row').filter({ hasText: f.name || 'Browser token fixture' });
    await row.getByRole('button', { name: '폐기', exact: true }).click();
    await row.waitFor({ state: 'detached' });
    if (await secret.count()) throw Error('Revoked token remains displayed');
    if ((await p.request.get(f.base + '/api/server/status', { headers })).status() !== 401) throw Error('Revoked token accepted');
    if (f.screenshot) await p.screenshot({ path: f.screenshot, fullPage: true });
    process.stdout.write(JSON.stringify({ browser_issue_use_revoke: true, out_of_scope_denied: true, revoked_token_denied: true }));
  } finally { await b.close(); }
})().catch(e => { console.error(e.message); process.exit(1); });
