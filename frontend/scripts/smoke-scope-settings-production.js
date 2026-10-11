// Read-only authenticated verification. Receives a short-lived token over stdin.
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
(async () => {
  const { token } = JSON.parse(fs.readFileSync(0, 'utf8'));
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 }, locale: 'ko-KR' });
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  try {
    await page.addInitScript(value => sessionStorage.setItem('nabiman_token', value), token);
    await page.goto('https://man.bebo.kr/#administration/usermgmt');
    await page.getByRole('button', { name: '최고관리자', exact: true }).waitFor();
    await page.getByRole('heading', { name: '최고관리자 · 회원 승인 및 소속 관리', exact: true }).waitFor();
    await page.getByRole('button', { name: '가입 신청 확인', exact: true }).waitFor();
    await page.getByRole('button', { name: 'tenant·서버 운영 설정', exact: true }).click();
    await page.getByRole('heading', { name: 'tenant·서버 운영 설정', exact: true }).waitFor();
    await page.getByLabel('담당자 이메일', { exact: true }).waitFor();
    await page.getByLabel('회원 수 제한', { exact: true }).waitFor();
    await page.screenshot({ path: path.resolve(__dirname, '../../screenshots/production-scope-settings-20261005.png'), fullPage: true });
    if (errors.length) throw new Error('Scope settings page errors: ' + JSON.stringify(errors));
    await page.goto('https://man.bebo.kr/#security/usermgmt');
    await page.getByRole('heading', { name: '최고관리자 · 회원 승인 및 소속 관리', exact: true }).waitFor();
    console.log('PASS: production administration navigation, pending signup entry, persisted policy form and legacy member-management link');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
