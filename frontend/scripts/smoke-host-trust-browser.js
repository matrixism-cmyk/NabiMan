const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    await page.addInitScript(() => localStorage.setItem('nabiman_lang', 'ko'));
    await page.goto(process.argv[2]);
    await page.locator('input[autocomplete="username"]').fill('manager');
    await page.locator('input[autocomplete="current-password"]').fill('isolated-member-password-123');
    await page.getByRole('button', { name: /^(로그인|Login|ログイン)$/ }).click();
    await page.getByRole('button', { name: /^(로그아웃|Logout|ログアウト)$/ }).waitFor();
    await page.goto(process.argv[2] + '/#remote/remote');
    await page.getByRole('heading', { name: '내 소속 서버', exact: true }).waitFor();
    await page.getByRole('button', { name: '서버 신원 확인', exact: true }).click();
    await page.getByLabel('서버 공개키', { exact: true }).fill(fixture.public_key);
    await page.getByLabel('독립 확인한 SHA256 지문', { exact: true }).fill(fixture.fingerprint);
    await page.getByLabel('확인 경로와 변경 사유', { exact: true }).fill('Verified isolated host console');
    let response = page.waitForResponse(r => r.url().endsWith('/host-trust') && r.request().method() === 'PUT');
    await page.getByRole('button', { name: '확인한 키 등록 또는 교체', exact: true }).click();
    if ((await response).status() !== 200) throw new Error('Scoped manager host trust registration failed');
    await page.reload();
    await page.getByRole('button', { name: '서버 신원 확인', exact: true }).click();
    if (await page.getByLabel('독립 확인한 SHA256 지문', { exact: true }).inputValue() !== fixture.fingerprint) throw new Error('Host trust did not persist');
    await page.screenshot({ path: path.resolve(__dirname, '../../screenshots/host-trust-scope-admin.png'), fullPage: true });
    await page.getByLabel('확인 경로와 변경 사유', { exact: true }).fill('Retire verified test key');
    response = page.waitForResponse(r => r.url().endsWith('/host-trust') && r.request().method() === 'PUT');
    await page.getByRole('button', { name: '신뢰 해제', exact: true }).click();
    if ((await response).status() !== 200) throw new Error('Host trust revocation failed');
    console.log('PASS: scoped administrator registers, reloads and revokes host trust in browser');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
