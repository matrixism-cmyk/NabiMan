const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const origin = new URL(process.argv[2]).origin;
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  const errors = [];
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    page.on('pageerror', () => errors.push('Browser page error'));
    page.on('request', request => {
      if (request.url().includes(fixture.token) || request.url().includes('token=')) errors.push('Credential in URL');
    });
    await page.addInitScript(({ token, refresh, origin }) => {
      if (window.top !== window || window.location.origin !== origin) return;
      sessionStorage.setItem('nabiman_token', token);
      sessionStorage.setItem('nabiman_refresh', refresh);
      sessionStorage.setItem('nabiman_role', 'admin');
      localStorage.setItem('nabiman_lang', 'ko');
    }, { ...fixture, origin });
    await page.goto(origin + '/#administration/usermgmt');
    const row = page.getByRole('row').filter({ hasText: 'identity-fixture' });
    await row.getByRole('button', { name: 'Rancher 계정 연결', exact: true }).click();
    await page.getByText('mock://rancher', { exact: true }).waitFor();
    await page.getByLabel('Rancher 계정 ID', { exact: true }).fill(fixture.remote_id);
    await page.getByLabel('계정 확인 및 연결 사유', { exact: true }).fill('Fixture owner verified by highest administrator');
    const linked = page.waitForResponse(response => response.url().endsWith('/external-identities/rancher') && response.request().method() === 'POST');
    await page.getByRole('button', { name: '확인하고 연결', exact: true }).click();
    const response = await linked;
    if (response.status() !== 201 || response.headers()['cache-control'] !== 'no-store') throw new Error('Link API failed');
    await page.getByText('external-fixture · ' + fixture.remote_id, { exact: true }).waitFor();
    await page.reload();
    await page.getByRole('row').filter({ hasText: 'identity-fixture' }).getByRole('button', { name: 'Rancher 계정 연결', exact: true }).click();
    await page.getByText('external-fixture · ' + fixture.remote_id, { exact: true }).waitFor();
    if (await page.getByRole('button', { name: '확인하고 연결', exact: true }).count()) throw new Error('Existing identity could be replaced');
    await page.screenshot({ path: path.resolve(__dirname, '../../screenshots/external-identity.png'), fullPage: true });
    if (errors.length) throw new Error(errors.join('; '));
    process.stdout.write('PASS: actual member UI, explicit remote ID, no-store link, reload persistence and no credential URLs.\n');
  } finally { await browser.close(); }
})().catch(() => { process.stderr.write('External identity browser smoke failed\n'); process.exitCode = 1; });
